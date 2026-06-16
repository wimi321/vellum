use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use rusqlite::{Connection, OptionalExtension, params};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

static CHAPTER_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*(第.{1,12}[章节回卷部篇集].*|#{1,3}\s+.+)\s*$").unwrap());
static HTML_TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[^>]+>").unwrap());

const CHUNK_TARGET_CHARS: usize = 4_000;
const MAX_QUERY_NGRAMS: usize = 80;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BookSource {
    pub path: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ImportStatus {
    Queued,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportJob {
    pub id: String,
    pub status: ImportStatus,
    pub progress: f32,
    pub message: String,
    pub book_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BookManifest {
    pub id: String,
    pub title: String,
    pub source_path: String,
    pub chapter_count: usize,
    pub chunk_count: usize,
    pub char_count: usize,
    pub status: ImportStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChunkRef {
    pub book_id: String,
    pub chunk_index: usize,
    pub chapter_title: String,
    pub start_char: usize,
    pub end_char: usize,
    pub preview: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceSpan {
    pub chunk: ChunkRef,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerIdentity {
    pub name: String,
    pub role: String,
    pub goal: String,
    pub tone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PlayerActionKind {
    Speak,
    Act,
    Continue,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerAction {
    pub kind: PlayerActionKind,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryItem {
    pub id: String,
    pub label: String,
    pub value: String,
    pub turn_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEvent {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub turn_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorldState {
    pub memories: Vec<MemoryItem>,
    pub timeline: Vec<TimelineEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HarnessEventKind {
    SearchSource,
    ReadChunk,
    RetrieveContext,
    DraftScene,
    ContinuityCheck,
    UpdateMemory,
    CommitTurn,
    RollbackTurn,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallRecord {
    pub name: String,
    pub input_summary: String,
    pub output_summary: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HarnessEvent {
    pub id: String,
    pub kind: HarnessEventKind,
    pub title: String,
    pub detail: String,
    pub tool: Option<ToolCallRecord>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StorySession {
    pub id: String,
    pub book_id: String,
    pub identity: PlayerIdentity,
    pub current_scene: String,
    pub turn_count: usize,
    pub world: WorldState,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StoryTurn {
    pub id: String,
    pub session_id: String,
    pub turn_index: usize,
    pub action: PlayerAction,
    pub scene: String,
    pub evidence: Vec<EvidenceSpan>,
    pub trace: Vec<HarnessEvent>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct TextUnit {
    title: String,
    text: String,
}

#[derive(Debug, Clone)]
struct PendingChunk {
    chapter_title: String,
    start_char: usize,
    end_char: usize,
    text: String,
}

#[derive(Clone)]
pub struct StoryStore {
    conn: Arc<Mutex<Connection>>,
    data_dir: Arc<PathBuf>,
}

impl StoryStore {
    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        fs::create_dir_all(&data_dir)
            .with_context(|| format!("create data dir {}", data_dir.display()))?;
        let db_path = data_dir.join("story-harness.sqlite3");
        let conn = Connection::open(&db_path)
            .with_context(|| format!("open sqlite database {}", db_path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            data_dir: Arc::new(data_dir),
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.lock()?;
        conn.execute_batch(
            r#"
CREATE TABLE IF NOT EXISTS books (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  source_path TEXT NOT NULL,
  chapter_count INTEGER NOT NULL,
  chunk_count INTEGER NOT NULL,
  char_count INTEGER NOT NULL,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS import_jobs (
  id TEXT PRIMARY KEY,
  status TEXT NOT NULL,
  progress REAL NOT NULL,
  message TEXT NOT NULL,
  book_id TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS chunks (
  book_id TEXT NOT NULL,
  chunk_index INTEGER NOT NULL,
  chapter_title TEXT NOT NULL,
  start_char INTEGER NOT NULL,
  end_char INTEGER NOT NULL,
  text TEXT NOT NULL,
  PRIMARY KEY(book_id, chunk_index),
  FOREIGN KEY(book_id) REFERENCES books(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS ngrams (
  book_id TEXT NOT NULL,
  ngram TEXT NOT NULL,
  chunk_index INTEGER NOT NULL,
  count INTEGER NOT NULL,
  PRIMARY KEY(book_id, ngram, chunk_index),
  FOREIGN KEY(book_id, chunk_index) REFERENCES chunks(book_id, chunk_index) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_ngrams_lookup ON ngrams(book_id, ngram);

CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY,
  book_id TEXT NOT NULL,
  identity_json TEXT NOT NULL,
  current_scene TEXT NOT NULL,
  turn_count INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(book_id) REFERENCES books(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS turns (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  turn_index INTEGER NOT NULL,
  action_json TEXT NOT NULL,
  scene TEXT NOT NULL,
  evidence_json TEXT NOT NULL,
  trace_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_turns_session_index ON turns(session_id, turn_index);

CREATE TABLE IF NOT EXISTS memories (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  label TEXT NOT NULL,
  value TEXT NOT NULL,
  turn_index INTEGER NOT NULL,
  FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS timeline_events (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  title TEXT NOT NULL,
  summary TEXT NOT NULL,
  turn_index INTEGER NOT NULL,
  FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS provider_profiles (
  id TEXT PRIMARY KEY,
  json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
"#,
        )?;
        Ok(())
    }

    pub fn import_book(&self, source: BookSource) -> Result<ImportJob> {
        let job = self.start_import_book()?;
        self.run_import_job(&job.id, source)?;
        self.import_status(&job.id)
    }

    pub fn start_import_book(&self) -> Result<ImportJob> {
        let created_at = Utc::now();
        let job = ImportJob {
            id: Uuid::new_v4().to_string(),
            status: ImportStatus::Queued,
            progress: 0.0,
            message: "等待导入".to_string(),
            book_id: None,
            created_at,
        };
        self.insert_import_job(&job)?;
        Ok(job)
    }

    pub fn run_import_job(&self, job_id: &str, source: BookSource) -> Result<()> {
        let existing = self.import_status(job_id)?;
        let running = ImportJob {
            status: ImportStatus::Running,
            progress: 0.05,
            message: "正在读取小说".to_string(),
            ..existing.clone()
        };
        self.update_import_job(&running)?;

        match self.import_book_inner(job_id, &source, existing.created_at) {
            Ok(manifest) => {
                let job = ImportJob {
                    id: job_id.to_string(),
                    status: ImportStatus::Completed,
                    progress: 1.0,
                    message: "索引完成，可以开始穿书".to_string(),
                    book_id: Some(manifest.id),
                    created_at: existing.created_at,
                };
                self.update_import_job(&job)?;
                Ok(())
            }
            Err(error) => {
                let job = ImportJob {
                    id: job_id.to_string(),
                    status: ImportStatus::Failed,
                    progress: 1.0,
                    message: error.to_string(),
                    book_id: None,
                    created_at: existing.created_at,
                };
                let _ = self.update_import_job(&job);
                Err(error)
            }
        }
    }

    pub fn import_status(&self, job_id: &str) -> Result<ImportJob> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, status, progress, message, book_id, created_at FROM import_jobs WHERE id = ?1",
            params![job_id],
            row_to_import_job,
        )
        .optional()?
        .ok_or_else(|| anyhow!("import job not found: {job_id}"))
    }

    pub fn list_books(&self) -> Result<Vec<BookManifest>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, source_path, chapter_count, chunk_count, char_count, status, created_at
             FROM books ORDER BY created_at DESC",
        )?;
        let rows = stmt
            .query_map([], row_to_book_manifest)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_book(&self, book_id: &str) -> Result<BookManifest> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, title, source_path, chapter_count, chunk_count, char_count, status, created_at
             FROM books WHERE id = ?1",
            params![book_id],
            row_to_book_manifest,
        )
        .optional()?
        .ok_or_else(|| anyhow!("book not found: {book_id}"))
    }

    pub fn search_chunks(
        &self,
        book_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<EvidenceSpan>> {
        let query_grams = ordered_ngrams(query)
            .into_iter()
            .take(MAX_QUERY_NGRAMS)
            .collect::<Vec<_>>();
        if query_grams.is_empty() {
            return self.first_chunks(book_id, limit);
        }

        let conn = self.lock()?;
        let mut scores: HashMap<usize, i64> = HashMap::new();
        for gram in query_grams {
            let mut stmt = conn.prepare(
                "SELECT chunk_index, count FROM ngrams WHERE book_id = ?1 AND ngram = ?2",
            )?;
            let matches = stmt.query_map(params![book_id, gram], |row| {
                Ok((row.get::<_, i64>(0)? as usize, row.get::<_, i64>(1)?))
            })?;
            for item in matches {
                let (chunk_index, count) = item?;
                *scores.entry(chunk_index).or_insert(0) += count;
            }
        }

        let mut ranked = scores.into_iter().collect::<Vec<_>>();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        if ranked.is_empty() {
            drop(conn);
            return self.first_chunks(book_id, limit);
        }

        let mut spans = Vec::new();
        for (chunk_index, score) in ranked.into_iter().take(limit) {
            let span = fetch_evidence(&conn, book_id, chunk_index, score as f32)?;
            spans.push(span);
        }
        Ok(spans)
    }

    pub fn create_session(
        &self,
        book_id: &str,
        identity: PlayerIdentity,
        initial_scene: String,
    ) -> Result<StorySession> {
        self.get_book(book_id)?;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let identity_json = serde_json::to_string(&identity)?;
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO sessions (id, book_id, identity_json, current_scene, turn_count, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)",
            params![id, book_id, identity_json, initial_scene, now.to_rfc3339()],
        )?;
        drop(conn);
        self.get_session(&id)
    }

    pub fn get_session(&self, session_id: &str) -> Result<StorySession> {
        let conn = self.lock()?;
        let row = conn
            .query_row(
                "SELECT id, book_id, identity_json, current_scene, turn_count, updated_at
                 FROM sessions WHERE id = ?1",
                params![session_id],
                |row| {
                    let identity_json: String = row.get(2)?;
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        identity_json,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)? as usize,
                        parse_dt(row.get::<_, String>(5)?)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| anyhow!("story session not found: {session_id}"))?;
        let identity = serde_json::from_str(&row.2)?;
        let world = self.world_state_with_conn(&conn, &row.0)?;
        Ok(StorySession {
            id: row.0,
            book_id: row.1,
            identity,
            current_scene: row.3,
            turn_count: row.4,
            world,
            updated_at: row.5,
        })
    }

    pub fn save_turn(
        &self,
        session_id: &str,
        action: PlayerAction,
        scene: String,
        evidence: Vec<EvidenceSpan>,
        trace: Vec<HarnessEvent>,
        memory: Option<MemoryItem>,
        timeline: Option<TimelineEvent>,
    ) -> Result<StoryTurn> {
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let current_count: usize = tx.query_row(
            "SELECT turn_count FROM sessions WHERE id = ?1",
            params![session_id],
            |row| Ok(row.get::<_, i64>(0)? as usize),
        )?;
        let turn_index = current_count + 1;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let action_json = serde_json::to_string(&action)?;
        let evidence_json = serde_json::to_string(&evidence)?;
        let trace_json = serde_json::to_string(&trace)?;
        tx.execute(
            "INSERT INTO turns (id, session_id, turn_index, action_json, scene, evidence_json, trace_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                session_id,
                turn_index as i64,
                action_json,
                scene,
                evidence_json,
                trace_json,
                now.to_rfc3339()
            ],
        )?;
        if let Some(memory) = &memory {
            tx.execute(
                "INSERT INTO memories (id, session_id, label, value, turn_index)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    memory.id,
                    session_id,
                    memory.label,
                    memory.value,
                    memory.turn_index as i64
                ],
            )?;
        }
        if let Some(event) = &timeline {
            tx.execute(
                "INSERT INTO timeline_events (id, session_id, title, summary, turn_index)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    event.id,
                    session_id,
                    event.title,
                    event.summary,
                    event.turn_index as i64
                ],
            )?;
        }
        tx.execute(
            "UPDATE sessions SET current_scene = ?1, turn_count = ?2, updated_at = ?3 WHERE id = ?4",
            params![scene, turn_index as i64, now.to_rfc3339(), session_id],
        )?;
        tx.commit()?;
        drop(conn);
        self.get_turn(&id)
    }

    pub fn get_turn(&self, turn_id: &str) -> Result<StoryTurn> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT id, session_id, turn_index, action_json, scene, evidence_json, trace_json, created_at
             FROM turns WHERE id = ?1",
            params![turn_id],
            row_to_story_turn,
        )
        .optional()?
        .ok_or_else(|| anyhow!("turn not found: {turn_id}"))
    }

    pub fn latest_turns(&self, session_id: &str, limit: usize) -> Result<Vec<StoryTurn>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, turn_index, action_json, scene, evidence_json, trace_json, created_at
             FROM turns WHERE session_id = ?1 ORDER BY turn_index DESC LIMIT ?2",
        )?;
        let mut turns = stmt
            .query_map(params![session_id, limit as i64], row_to_story_turn)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        turns.reverse();
        Ok(turns)
    }

    pub fn get_evidence(&self, turn_id: &str) -> Result<Vec<EvidenceSpan>> {
        Ok(self.get_turn(turn_id)?.evidence)
    }

    pub fn get_trace(&self, session_id: &str) -> Result<Vec<HarnessEvent>> {
        let turns = self.latest_turns(session_id, 200)?;
        let mut trace = Vec::new();
        for turn in turns {
            trace.extend(turn.trace);
        }
        Ok(trace)
    }

    pub fn rollback_turn(&self, session_id: &str, turn_id: &str) -> Result<StorySession> {
        let turn = self.get_turn(turn_id)?;
        if turn.session_id != session_id {
            return Err(anyhow!("turn does not belong to session"));
        }
        let keep_before = turn.turn_index.saturating_sub(1);
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM turns WHERE session_id = ?1 AND turn_index >= ?2",
            params![session_id, turn.turn_index as i64],
        )?;
        tx.execute(
            "DELETE FROM memories WHERE session_id = ?1 AND turn_index >= ?2",
            params![session_id, turn.turn_index as i64],
        )?;
        tx.execute(
            "DELETE FROM timeline_events WHERE session_id = ?1 AND turn_index >= ?2",
            params![session_id, turn.turn_index as i64],
        )?;
        let previous_scene = if keep_before == 0 {
            "你回到了刚穿入故事的那一刻。".to_string()
        } else {
            tx.query_row(
                "SELECT scene FROM turns WHERE session_id = ?1 AND turn_index = ?2",
                params![session_id, keep_before as i64],
                |row| row.get::<_, String>(0),
            )?
        };
        let now = Utc::now();
        tx.execute(
            "UPDATE sessions SET current_scene = ?1, turn_count = ?2, updated_at = ?3 WHERE id = ?4",
            params![previous_scene, keep_before as i64, now.to_rfc3339(), session_id],
        )?;
        tx.commit()?;
        drop(conn);
        self.get_session(session_id)
    }

    pub fn save_provider_profile_json(&self, id: &str, json: &str) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO provider_profiles (id, json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET json = excluded.json, updated_at = excluded.updated_at",
            params![id, json, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn provider_profile_json(&self, id: &str) -> Result<String> {
        let conn = self.lock()?;
        conn.query_row(
            "SELECT json FROM provider_profiles WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| anyhow!("provider profile not found: {id}"))
    }

    fn import_book_inner(
        &self,
        job_id: &str,
        source: &BookSource,
        created_at: DateTime<Utc>,
    ) -> Result<BookManifest> {
        self.update_job_message(job_id, 0.15, "正在分章和分片")?;
        let path = PathBuf::from(&source.path);
        let units = read_source_units(&path)?;
        if units.is_empty() {
            return Err(anyhow!("没有读到可导入的正文"));
        }
        let title = source
            .title
            .clone()
            .filter(|title| !title.trim().is_empty())
            .or_else(|| {
                path.file_stem()
                    .map(|name| name.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| "未命名小说".to_string());
        let chunks = chunk_units(&units, CHUNK_TARGET_CHARS);
        if chunks.is_empty() {
            return Err(anyhow!("正文太短或无法分片"));
        }
        self.update_job_message(job_id, 0.45, "正在建立本地检索索引")?;
        let book_id = Uuid::new_v4().to_string();
        let char_count = chunks
            .last()
            .map(|chunk| chunk.end_char)
            .unwrap_or_default();
        let manifest = BookManifest {
            id: book_id.clone(),
            title,
            source_path: source.path.clone(),
            chapter_count: units.len(),
            chunk_count: chunks.len(),
            char_count,
            status: ImportStatus::Completed,
            created_at,
        };
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO books (id, title, source_path, chapter_count, chunk_count, char_count, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                manifest.id,
                manifest.title,
                manifest.source_path,
                manifest.chapter_count as i64,
                manifest.chunk_count as i64,
                manifest.char_count as i64,
                status_to_str(&manifest.status),
                manifest.created_at.to_rfc3339()
            ],
        )?;
        for (chunk_index, chunk) in chunks.iter().enumerate() {
            tx.execute(
                "INSERT INTO chunks (book_id, chunk_index, chapter_title, start_char, end_char, text)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    manifest.id,
                    chunk_index as i64,
                    chunk.chapter_title,
                    chunk.start_char as i64,
                    chunk.end_char as i64,
                    chunk.text
                ],
            )?;
            let grams = ngram_counts(&chunk.text);
            for (gram, count) in grams {
                tx.execute(
                    "INSERT INTO ngrams (book_id, ngram, chunk_index, count)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![manifest.id, gram, chunk_index as i64, count as i64],
                )?;
            }
        }
        tx.commit()?;
        Ok(manifest)
    }

    fn first_chunks(&self, book_id: &str, limit: usize) -> Result<Vec<EvidenceSpan>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT chunk_index FROM chunks WHERE book_id = ?1 ORDER BY chunk_index ASC LIMIT ?2",
        )?;
        let indices = stmt
            .query_map(params![book_id, limit as i64], |row| {
                Ok(row.get::<_, i64>(0)? as usize)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        indices
            .into_iter()
            .map(|idx| fetch_evidence(&conn, book_id, idx, 1.0))
            .collect()
    }

    fn world_state_with_conn(&self, conn: &Connection, session_id: &str) -> Result<WorldState> {
        let mut memory_stmt = conn.prepare(
            "SELECT id, label, value, turn_index FROM memories WHERE session_id = ?1 ORDER BY turn_index ASC",
        )?;
        let memories = memory_stmt
            .query_map(params![session_id], |row| {
                Ok(MemoryItem {
                    id: row.get(0)?,
                    label: row.get(1)?,
                    value: row.get(2)?,
                    turn_index: row.get::<_, i64>(3)? as usize,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut timeline_stmt = conn.prepare(
            "SELECT id, title, summary, turn_index FROM timeline_events WHERE session_id = ?1 ORDER BY turn_index ASC",
        )?;
        let timeline = timeline_stmt
            .query_map(params![session_id], |row| {
                Ok(TimelineEvent {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    summary: row.get(2)?,
                    turn_index: row.get::<_, i64>(3)? as usize,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(WorldState { memories, timeline })
    }

    fn insert_import_job(&self, job: &ImportJob) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO import_jobs (id, status, progress, message, book_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                job.id,
                status_to_str(&job.status),
                job.progress,
                job.message,
                job.book_id,
                job.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    fn update_import_job(&self, job: &ImportJob) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "UPDATE import_jobs SET status = ?1, progress = ?2, message = ?3, book_id = ?4 WHERE id = ?5",
            params![
                status_to_str(&job.status),
                job.progress,
                job.message,
                job.book_id,
                job.id
            ],
        )?;
        Ok(())
    }

    fn update_job_message(&self, job_id: &str, progress: f32, message: &str) -> Result<()> {
        let conn = self.lock()?;
        conn.execute(
            "UPDATE import_jobs SET progress = ?1, message = ?2 WHERE id = ?3",
            params![progress, message, job_id],
        )?;
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| anyhow!("story store lock poisoned"))
    }
}

fn read_source_units(path: &Path) -> Result<Vec<TextUnit>> {
    if path.is_dir() {
        return read_folder_units(path);
    }
    let ext = path
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "txt" | "md" => read_text_file_units(path),
        "epub" => read_epub_units(path),
        _ => Err(anyhow!(
            "暂不支持该文件类型：{}。请使用 txt、md、无 DRM epub 或章节文件夹",
            ext
        )),
    }
}

fn read_folder_units(path: &Path) -> Result<Vec<TextUnit>> {
    let mut files = fs::read_dir(path)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .map(|ext| {
                    let ext = ext.to_string_lossy().to_ascii_lowercase();
                    ext == "txt" || ext == "md"
                })
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    files.sort();
    let mut units = Vec::new();
    for file in files {
        units.extend(read_text_file_units(&file)?);
    }
    Ok(units)
}

fn read_text_file_units(path: &Path) -> Result<Vec<TextUnit>> {
    let text = fs::read_to_string(path)
        .or_else(|_| fs::read(path).map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
        .with_context(|| format!("read {}", path.display()))?;
    let fallback_title = path
        .file_stem()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "正文".to_string());
    Ok(split_text_into_units(&text, &fallback_title))
}

fn read_epub_units(path: &Path) -> Result<Vec<TextUnit>> {
    let file = fs::File::open(path).with_context(|| format!("open epub {}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("read epub zip archive")?;
    let mut units = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let name = file.name().to_ascii_lowercase();
        if !(name.ends_with(".xhtml") || name.ends_with(".html") || name.ends_with(".txt")) {
            continue;
        }
        let mut raw = String::new();
        file.read_to_string(&mut raw).ok();
        let text = if name.ends_with(".txt") {
            raw
        } else {
            HTML_TAG_RE.replace_all(&raw, "\n").to_string()
        };
        let title = Path::new(&name)
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("epub-{index}"));
        units.extend(split_text_into_units(&text, &title));
    }
    Ok(units)
}

fn split_text_into_units(text: &str, fallback_title: &str) -> Vec<TextUnit> {
    let mut units = Vec::new();
    let mut current_title = fallback_title.to_string();
    let mut current = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if CHAPTER_RE.is_match(trimmed) && current.chars().count() > 120 {
            units.push(TextUnit {
                title: current_title,
                text: current.trim().to_string(),
            });
            current_title = trimmed.trim_start_matches('#').trim().to_string();
            current.clear();
            continue;
        }
        if CHAPTER_RE.is_match(trimmed) {
            current_title = trimmed.trim_start_matches('#').trim().to_string();
        }
        current.push_str(line);
        current.push('\n');
    }
    if !current.trim().is_empty() {
        units.push(TextUnit {
            title: current_title,
            text: current.trim().to_string(),
        });
    }
    units
}

fn chunk_units(units: &[TextUnit], target_chars: usize) -> Vec<PendingChunk> {
    let mut chunks = Vec::new();
    let mut global_cursor = 0usize;
    for unit in units {
        let mut buffer = String::new();
        let mut start = global_cursor;
        for ch in unit.text.chars() {
            buffer.push(ch);
            global_cursor += 1;
            if buffer.chars().count() >= target_chars {
                chunks.push(PendingChunk {
                    chapter_title: unit.title.clone(),
                    start_char: start,
                    end_char: global_cursor,
                    text: buffer.trim().to_string(),
                });
                buffer.clear();
                start = global_cursor;
            }
        }
        if !buffer.trim().is_empty() {
            chunks.push(PendingChunk {
                chapter_title: unit.title.clone(),
                start_char: start,
                end_char: global_cursor,
                text: buffer.trim().to_string(),
            });
        }
        global_cursor += 1;
    }
    chunks
}

fn ngram_counts(text: &str) -> HashMap<String, usize> {
    let normalized = normalized_chars(text);
    let mut counts = HashMap::new();
    for n in [2usize, 3] {
        if normalized.len() < n {
            continue;
        }
        for window in normalized.windows(n) {
            let gram = window.iter().collect::<String>();
            *counts.entry(gram).or_insert(0) += 1;
        }
    }
    counts
}

fn ordered_ngrams(text: &str) -> Vec<String> {
    let normalized = normalized_chars(text);
    let mut seen = HashSet::new();
    let mut grams = Vec::new();
    for n in [3usize, 2] {
        if normalized.len() < n {
            continue;
        }
        for window in normalized.windows(n) {
            let gram = window.iter().collect::<String>();
            if seen.insert(gram.clone()) {
                grams.push(gram);
            }
        }
    }
    grams
}

fn normalized_chars(text: &str) -> Vec<char> {
    text.chars()
        .filter(|ch| !ch.is_whitespace() && !ch.is_ascii_punctuation())
        .collect()
}

fn fetch_evidence(
    conn: &Connection,
    book_id: &str,
    chunk_index: usize,
    score: f32,
) -> Result<EvidenceSpan> {
    conn.query_row(
        "SELECT chapter_title, start_char, end_char, text FROM chunks WHERE book_id = ?1 AND chunk_index = ?2",
        params![book_id, chunk_index as i64],
        |row| {
            let text: String = row.get(3)?;
            let preview = text.chars().take(80).collect::<String>();
            Ok(EvidenceSpan {
                chunk: ChunkRef {
                    book_id: book_id.to_string(),
                    chunk_index,
                    chapter_title: row.get(0)?,
                    start_char: row.get::<_, i64>(1)? as usize,
                    end_char: row.get::<_, i64>(2)? as usize,
                    preview,
                    score,
                },
                text: text.chars().take(900).collect(),
            })
        },
    )
    .map_err(Into::into)
}

fn row_to_book_manifest(row: &rusqlite::Row<'_>) -> rusqlite::Result<BookManifest> {
    Ok(BookManifest {
        id: row.get(0)?,
        title: row.get(1)?,
        source_path: row.get(2)?,
        chapter_count: row.get::<_, i64>(3)? as usize,
        chunk_count: row.get::<_, i64>(4)? as usize,
        char_count: row.get::<_, i64>(5)? as usize,
        status: status_from_str(row.get::<_, String>(6)?.as_str()),
        created_at: parse_dt(row.get::<_, String>(7)?)?,
    })
}

fn row_to_import_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<ImportJob> {
    Ok(ImportJob {
        id: row.get(0)?,
        status: status_from_str(row.get::<_, String>(1)?.as_str()),
        progress: row.get(2)?,
        message: row.get(3)?,
        book_id: row.get(4)?,
        created_at: parse_dt(row.get::<_, String>(5)?)?,
    })
}

fn row_to_story_turn(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoryTurn> {
    let action_json: String = row.get(3)?;
    let evidence_json: String = row.get(5)?;
    let trace_json: String = row.get(6)?;
    Ok(StoryTurn {
        id: row.get(0)?,
        session_id: row.get(1)?,
        turn_index: row.get::<_, i64>(2)? as usize,
        action: serde_json::from_str(&action_json).map_err(json_to_sql_error)?,
        scene: row.get(4)?,
        evidence: serde_json::from_str(&evidence_json).map_err(json_to_sql_error)?,
        trace: serde_json::from_str(&trace_json).map_err(json_to_sql_error)?,
        created_at: parse_dt(row.get::<_, String>(7)?)?,
    })
}

fn json_to_sql_error(error: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}

fn parse_dt(value: String) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}

fn status_to_str(status: &ImportStatus) -> &'static str {
    match status {
        ImportStatus::Queued => "queued",
        ImportStatus::Running => "running",
        ImportStatus::Completed => "completed",
        ImportStatus::Failed => "failed",
    }
}

fn status_from_str(status: &str) -> ImportStatus {
    match status {
        "queued" => ImportStatus::Queued,
        "running" => ImportStatus::Running,
        "failed" => ImportStatus::Failed,
        _ => ImportStatus::Completed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_text() -> String {
        [
            "第一章 初入京城\n林晚推开雨中的朱门，看见沈砚站在廊下。她知道原文里今晚会有刺客潜入。\n",
            "第二章 夜宴风波\n夜宴上，贵妃提起旧案，沈砚神色微冷。林晚决定先提醒他小心酒盏。\n",
            "第三章 暗线浮出\n刺客留下半枚玉佩，牵出城南旧宅。林晚必须在天亮前做出选择。\n",
        ]
        .join("")
        .repeat(60)
    }

    #[test]
    fn imports_chapters_and_retrieves_relevant_chunks() -> Result<()> {
        let dir = tempdir()?;
        let book_path = dir.path().join("sample.txt");
        fs::write(&book_path, sample_text())?;
        let store = StoryStore::open(dir.path().join("data"))?;

        let job = store.import_book(BookSource {
            path: book_path.display().to_string(),
            title: Some("测试长篇".to_string()),
        })?;
        assert_eq!(job.status, ImportStatus::Completed);
        let book_id = job.book_id.expect("book id");
        let books = store.list_books()?;
        assert_eq!(books.len(), 1);
        assert!(books[0].chunk_count > 1);

        let hits = store.search_chunks(&book_id, "沈砚 酒盏 夜宴", 3)?;
        assert!(!hits.is_empty());
        assert!(hits[0].text.contains("沈砚") || hits[0].text.contains("夜宴"));
        Ok(())
    }

    #[test]
    fn saves_resume_and_rolls_back_story_turns() -> Result<()> {
        let dir = tempdir()?;
        let book_path = dir.path().join("sample.txt");
        fs::write(&book_path, sample_text())?;
        let store = StoryStore::open(dir.path().join("data"))?;
        let book_id = store
            .import_book(BookSource {
                path: book_path.display().to_string(),
                title: None,
            })?
            .book_id
            .unwrap();
        let session = store.create_session(
            &book_id,
            PlayerIdentity {
                name: "林晚".to_string(),
                role: "穿书者".to_string(),
                goal: "救下沈砚".to_string(),
                tone: "冷静".to_string(),
            },
            "你站在雨中的朱门前。".to_string(),
        )?;
        let turn = store.save_turn(
            &session.id,
            PlayerAction {
                kind: PlayerActionKind::Act,
                text: "提醒沈砚小心酒盏".to_string(),
            },
            "你压低声音提醒沈砚，酒盏旁的银针泛起暗光。".to_string(),
            Vec::new(),
            Vec::new(),
            Some(MemoryItem {
                id: Uuid::new_v4().to_string(),
                label: "行动".to_string(),
                value: "提醒沈砚小心酒盏".to_string(),
                turn_index: 1,
            }),
            Some(TimelineEvent {
                id: Uuid::new_v4().to_string(),
                title: "夜宴提醒".to_string(),
                summary: "林晚提前干预酒盏危机".to_string(),
                turn_index: 1,
            }),
        )?;
        assert_eq!(store.get_session(&session.id)?.turn_count, 1);
        let rolled = store.rollback_turn(&session.id, &turn.id)?;
        assert_eq!(rolled.turn_count, 0);
        assert!(rolled.world.memories.is_empty());
        Ok(())
    }

    #[test]
    fn import_job_lifecycle_reports_progress() -> Result<()> {
        let dir = tempdir()?;
        let book_path = dir.path().join("sample.txt");
        fs::write(&book_path, sample_text())?;
        let store = StoryStore::open(dir.path().join("data"))?;

        let job = store.start_import_book()?;
        assert_eq!(job.status, ImportStatus::Queued);
        assert_eq!(job.progress, 0.0);

        store.run_import_job(
            &job.id,
            BookSource {
                path: book_path.display().to_string(),
                title: Some("后台导入测试".to_string()),
            },
        )?;

        let completed = store.import_status(&job.id)?;
        assert_eq!(completed.status, ImportStatus::Completed);
        assert_eq!(completed.progress, 1.0);
        assert!(completed.book_id.is_some());
        Ok(())
    }

    #[test]
    fn scale_import_handles_two_million_chars() -> Result<()> {
        let dir = tempdir()?;
        let book_path = dir.path().join("big.txt");
        let body = "主角进入城中，发现线索与玉佩、酒盏、暗巷、旧宅有关。".repeat(80);
        let paragraph = format!("第一章 测试\n{body}\n");
        let mut text = String::new();
        while text.chars().count() < 2_020_000 {
            text.push_str(&paragraph);
        }
        fs::write(&book_path, text)?;
        let store = StoryStore::open(dir.path().join("data"))?;
        let job = store.import_book(BookSource {
            path: book_path.display().to_string(),
            title: Some("百万字测试".to_string()),
        })?;
        let manifest = store.get_book(&job.book_id.unwrap())?;
        assert!(
            manifest.char_count >= 2_000_000,
            "char_count={}",
            manifest.char_count
        );
        assert!(manifest.chunk_count >= 400);
        Ok(())
    }
}
