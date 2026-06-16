use anyhow::{Result, anyhow};
use chrono::Utc;
use model_adapters::{ModelClient, ModelRequest};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use story_store::{
    BookManifest, EvidenceSpan, HarnessEvent, HarnessEventKind, MemoryItem, PlayerAction,
    PlayerActionKind, PlayerIdentity, StorySession, StoryStore, StoryTurn, TimelineEvent,
    ToolCallRecord,
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlaythroughState {
    pub book: BookManifest,
    pub session: StorySession,
    pub recent_turns: Vec<StoryTurn>,
    pub last_evidence: Vec<EvidenceSpan>,
}

#[derive(Clone)]
pub struct StoryHarness {
    store: StoryStore,
    model: Arc<dyn ModelClient>,
}

impl StoryHarness {
    pub fn new(store: StoryStore, model: Arc<dyn ModelClient>) -> Self {
        Self { store, model }
    }

    pub fn store(&self) -> &StoryStore {
        &self.store
    }

    pub async fn start_playthrough(
        &self,
        book_id: String,
        identity: PlayerIdentity,
    ) -> Result<PlaythroughState> {
        let book = self.store.get_book(&book_id)?;
        let evidence = self.store.search_chunks(
            &book_id,
            &format!("{} {}", identity.name, identity.goal),
            2,
        )?;
        let source_hint = evidence
            .first()
            .map(|span| span.text.chars().take(120).collect::<String>())
            .unwrap_or_else(|| "原文索引已经准备好。".to_string());
        let scene = format!(
            "你穿进《{}》，成为「{}」。\n\n当前身份：{}。\n目标：{}。\n\n你先记住眼前最重要的原文线索：{}",
            book.title, identity.name, identity.role, identity.goal, source_hint
        );
        let session = self.store.create_session(&book_id, identity, scene)?;
        self.state_for_session(&session.id, evidence)
    }

    pub fn resume_playthrough(&self, session_id: String) -> Result<PlaythroughState> {
        self.state_for_session(&session_id, Vec::new())
    }

    pub async fn send_player_action(
        &self,
        session_id: String,
        action: PlayerAction,
    ) -> Result<PlaythroughState> {
        let session = self.store.get_session(&session_id)?;
        let book = self.store.get_book(&session.book_id)?;
        let mut trace = Vec::new();

        let query = build_retrieval_query(&session, &action);
        let started = Instant::now();
        let evidence = self.store.search_chunks(&session.book_id, &query, 4)?;
        trace.push(event_with_tool(
            HarnessEventKind::SearchSource,
            "正在查原文",
            format!("从《{}》里找和本回合最相关的片段", book.title),
            "search_source",
            format!("query: {}", compact(&query, 80)),
            format!("命中 {} 个片段", evidence.len()),
            started,
        ));

        let started = Instant::now();
        let context = evidence
            .iter()
            .enumerate()
            .map(|(index, span)| {
                format!(
                    "[片段{}｜{}] {}",
                    index + 1,
                    span.chunk.chapter_title,
                    span.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        trace.push(event_with_tool(
            HarnessEventKind::RetrieveContext,
            "正在整理线索",
            "只把当前回合必要片段交给模型".to_string(),
            "retrieve_context",
            format!("{} evidence spans", evidence.len()),
            compact(&context, 120),
            started,
        ));

        let prompt = build_scene_prompt(&book, &session, &action, &context);
        let started = Instant::now();
        let model_response = self
            .model
            .complete(ModelRequest {
                system: "你是一个穿书互动小说导演。严格根据原文依据延展，不要一次性改变主线。"
                    .to_string(),
                prompt,
                max_output_chars: 1_200,
            })
            .await?;
        let scene = normalize_scene(&model_response.text)?;
        trace.push(event_with_tool(
            HarnessEventKind::DraftScene,
            "正在续写",
            "根据玩家动作和原文依据生成当前场景".to_string(),
            "draft_scene",
            action_label(&action),
            compact(&scene, 120),
            started,
        ));

        let started = Instant::now();
        continuity_check(&scene, &evidence)?;
        trace.push(event_with_tool(
            HarnessEventKind::ContinuityCheck,
            "正在检查剧情连续性",
            "确认本回合没有空场景，且保留原文依据".to_string(),
            "continuity_check",
            "scene + evidence".to_string(),
            "通过".to_string(),
            started,
        ));

        let next_turn_index = session.turn_count + 1;
        let memory = MemoryItem {
            id: Uuid::new_v4().to_string(),
            label: match action.kind {
                PlayerActionKind::Speak => "说过的话".to_string(),
                PlayerActionKind::Act => "做过的事".to_string(),
                PlayerActionKind::Continue => "剧情推进".to_string(),
            },
            value: compact(&action.text, 80),
            turn_index: next_turn_index,
        };
        let timeline = TimelineEvent {
            id: Uuid::new_v4().to_string(),
            title: format!("第 {} 回合", next_turn_index),
            summary: compact(&scene, 90),
            turn_index: next_turn_index,
        };
        trace.push(simple_event(
            HarnessEventKind::UpdateMemory,
            "正在保存记忆",
            format!("{}：{}", memory.label, memory.value),
        ));

        let turn = self.store.save_turn(
            &session_id,
            action,
            scene,
            evidence.clone(),
            trace.clone(),
            Some(memory),
            Some(timeline),
        )?;
        let mut commit_trace = trace;
        commit_trace.push(simple_event(
            HarnessEventKind::CommitTurn,
            "已保存",
            format!("第 {} 回合已保存", turn.turn_index),
        ));

        // Persist the commit marker by appending it to the in-memory response trace. The durable
        // turn already contains every tool event leading to the commit.
        let mut state = self.state_for_session(&session_id, evidence)?;
        if let Some(last) = state.recent_turns.last_mut() {
            last.trace = commit_trace;
        }
        Ok(state)
    }

    pub fn rollback_turn(&self, session_id: String, turn_id: String) -> Result<PlaythroughState> {
        self.store.rollback_turn(&session_id, &turn_id)?;
        self.state_for_session(&session_id, Vec::new())
    }

    fn state_for_session(
        &self,
        session_id: &str,
        last_evidence: Vec<EvidenceSpan>,
    ) -> Result<PlaythroughState> {
        let session = self.store.get_session(session_id)?;
        let book = self.store.get_book(&session.book_id)?;
        let recent_turns = self.store.latest_turns(session_id, 12)?;
        Ok(PlaythroughState {
            book,
            session,
            recent_turns,
            last_evidence,
        })
    }
}

fn build_retrieval_query(session: &StorySession, action: &PlayerAction) -> String {
    format!(
        "{} {} {} {} {}",
        session.identity.name,
        session.identity.role,
        session.identity.goal,
        session.current_scene,
        action.text
    )
}

fn build_scene_prompt(
    book: &BookManifest,
    session: &StorySession,
    action: &PlayerAction,
    context: &str,
) -> String {
    format!(
        r#"小说：{title}
玩家身份：{name} / {role}
玩家目标：{goal}
当前场景：
{scene}

原文依据：
{context}

玩家动作类型：{kind}
玩家动作：{action}

请输出下一段当前场景，要求：
1. 用户读起来像自然穿书剧情，不暴露工具、模型、索引、token 等技术词。
2. 必须尊重原文依据，不要突然改变主线设定。
3. 结尾留一个自然选择空间，不要替玩家做完所有决定。
"#,
        title = book.title,
        name = session.identity.name,
        role = session.identity.role,
        goal = session.identity.goal,
        scene = session.current_scene,
        context = context,
        kind = match action.kind {
            PlayerActionKind::Speak => "说一句",
            PlayerActionKind::Act => "做动作",
            PlayerActionKind::Continue => "继续剧情",
        },
        action = action.text,
    )
}

fn normalize_scene(text: &str) -> Result<String> {
    let scene = text.trim();
    if scene.is_empty() {
        return Err(anyhow!("模型返回了空场景"));
    }
    Ok(scene.to_string())
}

fn continuity_check(scene: &str, evidence: &[EvidenceSpan]) -> Result<()> {
    if scene.chars().count() < 20 {
        return Err(anyhow!("场景太短，无法继续"));
    }
    if evidence.is_empty() {
        return Err(anyhow!("没有找到原文依据，已停止本回合"));
    }
    Ok(())
}

fn event_with_tool(
    kind: HarnessEventKind,
    title: impl Into<String>,
    detail: impl Into<String>,
    tool_name: impl Into<String>,
    input_summary: impl Into<String>,
    output_summary: impl Into<String>,
    started: Instant,
) -> HarnessEvent {
    HarnessEvent {
        id: Uuid::new_v4().to_string(),
        kind,
        title: title.into(),
        detail: detail.into(),
        tool: Some(ToolCallRecord {
            name: tool_name.into(),
            input_summary: input_summary.into(),
            output_summary: output_summary.into(),
            duration_ms: started.elapsed().as_millis() as u64,
        }),
        created_at: Utc::now(),
    }
}

fn simple_event(
    kind: HarnessEventKind,
    title: impl Into<String>,
    detail: impl Into<String>,
) -> HarnessEvent {
    HarnessEvent {
        id: Uuid::new_v4().to_string(),
        kind,
        title: title.into(),
        detail: detail.into(),
        tool: None,
        created_at: Utc::now(),
    }
}

fn action_label(action: &PlayerAction) -> String {
    let kind = match action.kind {
        PlayerActionKind::Speak => "说一句",
        PlayerActionKind::Act => "做动作",
        PlayerActionKind::Continue => "继续剧情",
    };
    format!("{kind}: {}", compact(&action.text, 80))
}

fn compact(text: &str, max_chars: usize) -> String {
    let mut compacted = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compacted.chars().count() > max_chars {
        compacted = compacted.chars().take(max_chars).collect::<String>();
        compacted.push('…');
    }
    compacted
}

#[cfg(test)]
mod tests {
    use super::*;
    use model_adapters::MockStoryModel;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn full_story_loop_import_start_action_resume() -> Result<()> {
        let dir = tempdir()?;
        let book_path = dir.path().join("book.txt");
        fs::write(
            &book_path,
            "第一章 夜雨\n沈砚站在廊下，酒盏旁有银针。林晚知道刺客会来。\n".repeat(120),
        )?;
        let store = StoryStore::open(dir.path().join("data"))?;
        let book_id = store
            .import_book(story_store::BookSource {
                path: book_path.display().to_string(),
                title: Some("夜雨长篇".to_string()),
            })?
            .book_id
            .unwrap();
        let harness = StoryHarness::new(store, Arc::new(MockStoryModel));
        let state = harness
            .start_playthrough(
                book_id,
                PlayerIdentity {
                    name: "林晚".to_string(),
                    role: "穿书者".to_string(),
                    goal: "救下沈砚".to_string(),
                    tone: "简单直接".to_string(),
                },
            )
            .await?;
        assert_eq!(state.session.turn_count, 0);
        let state = harness
            .send_player_action(
                state.session.id.clone(),
                PlayerAction {
                    kind: PlayerActionKind::Act,
                    text: "提醒沈砚不要碰酒盏".to_string(),
                },
            )
            .await?;
        assert_eq!(state.session.turn_count, 1);
        assert!(!state.last_evidence.is_empty());
        assert!(!state.session.world.memories.is_empty());
        let resumed = harness.resume_playthrough(state.session.id)?;
        assert_eq!(resumed.session.turn_count, 1);
        Ok(())
    }
}
