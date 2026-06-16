import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  BookManifest,
  BookSource,
  EvidenceSpan,
  HarnessEvent,
  ImportJob,
  PlayerAction,
  PlayerIdentity,
  PlaythroughState,
  StorySession,
  StoryTurn,
} from "./types";

type TauriWindow = Window & {
  __TAURI_INTERNALS__?: unknown;
};

const isTauri = () => Boolean((window as TauriWindow).__TAURI_INTERNALS__);

const now = () => new Date().toISOString();
const id = (prefix: string) => `${prefix}-${crypto.randomUUID()}`;

const demoEvidence: EvidenceSpan[] = [
  {
    chunk: {
      bookId: "demo-book",
      chunkIndex: 0,
      chapterTitle: "第一章 夜雨",
      startChar: 0,
      endChar: 4200,
      preview: "沈砚站在廊下，酒盏旁有银针。林晚知道刺客会来。",
      score: 9,
    },
    text: "沈砚站在廊下，酒盏旁有银针。林晚知道刺客会来，却不能直接说出自己来自书外。",
  },
];

let mockBooks: BookManifest[] = [];

const mockStates = new Map<string, PlaythroughState>();
const mockJobs = new Map<string, ImportJob>();

export async function pickBookPath(): Promise<string | null> {
  if (!isTauri()) {
    return null;
  }
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [
      { name: "小说文件", extensions: ["txt", "md", "epub"] },
      { name: "所有文件", extensions: ["*"] },
    ],
  });
  return typeof selected === "string" ? selected : null;
}

export async function pickBookFolder(): Promise<string | null> {
  if (!isTauri()) {
    return null;
  }
  const selected = await open({
    multiple: false,
    directory: true,
  });
  return typeof selected === "string" ? selected : null;
}

export async function importBook(source: BookSource): Promise<ImportJob> {
  if (isTauri()) {
    return invoke("import_book", { source });
  }
  const job: ImportJob = {
    id: id("job"),
    status: "running",
    progress: 0.2,
    message: "正在分章和分片",
    bookId: null,
    createdAt: now(),
  };
  mockJobs.set(job.id, job);
  const book: BookManifest = {
    id: id("book"),
    title: source.title || source.path.split(/[\\/]/).pop() || "新导入小说",
    sourcePath: source.path,
    chapterCount: 86,
    chunkCount: 430,
    charCount: 1_060_000,
    status: "completed",
    createdAt: now(),
  };
  mockBooks = [book, ...mockBooks];
  const completed: ImportJob = {
    ...job,
    status: "completed",
    progress: 1,
    message: "索引完成，可以开始穿书",
    bookId: book.id,
  };
  mockJobs.set(job.id, completed);
  return completed;
}

export async function getImportStatus(jobId: string): Promise<ImportJob> {
  if (isTauri()) {
    return invoke("get_import_status", { jobId });
  }
  const job = mockJobs.get(jobId);
  if (!job) {
    throw new Error("导入任务不存在");
  }
  return job;
}

export async function listBooks(): Promise<BookManifest[]> {
  if (isTauri()) {
    return invoke("list_books");
  }
  return mockBooks;
}

export async function startPlaythrough(
  bookId: string,
  identity: PlayerIdentity,
): Promise<PlaythroughState> {
  if (isTauri()) {
    return invoke("start_playthrough", { bookId, identity });
  }
  const book = mockBooks.find((item) => item.id === bookId) || mockBooks[0];
  if (!book) {
    throw new Error("请先导入一本小说");
  }
  const session: StorySession = {
    id: id("session"),
    bookId,
    identity,
    currentScene: `你穿进《${book.title}》，成为「${identity.name}」。眼前是原文里最危险的夜宴前夕，酒盏、雨声和廊下的人影都在提醒你：不要急着改剧情，先找证据。`,
    turnCount: 0,
    world: { memories: [], timeline: [] },
    updatedAt: now(),
  };
  const state = { book, session, recentTurns: [], lastEvidence: demoEvidence };
  mockStates.set(session.id, state);
  return state;
}

export async function sendPlayerAction(
  sessionId: string,
  action: PlayerAction,
): Promise<PlaythroughState> {
  if (isTauri()) {
    return invoke("send_player_action", { sessionId, action });
  }
  const state = mockStates.get(sessionId);
  if (!state) {
    throw new Error("会话不存在");
  }
  const nextIndex = state.session.turnCount + 1;
  const trace: HarnessEvent[] = [
    event("searchSource", "正在查原文", "找到了夜宴、酒盏和沈砚相关片段"),
    event("retrieveContext", "正在整理线索", "只使用当前回合必要原文"),
    event("draftScene", "正在续写", "生成下一段当前场景"),
    event("continuityCheck", "正在检查剧情连续性", "没有偏离主线"),
    event("updateMemory", "正在保存记忆", action.text),
  ];
  const scene = `你没有急着打断所有人，而是顺着原文的节奏靠近沈砚。\n\n「${action.text}」\n\n他眼神微动，像是终于意识到酒盏旁那一点不自然的冷光。世界线没有崩坏，只是轻轻偏了一下：原本会被忽略的银针，现在被你提前看见了。`;
  const turn: StoryTurn = {
    id: id("turn"),
    sessionId,
    turnIndex: nextIndex,
    action,
    scene,
    evidence: demoEvidence,
    trace,
    createdAt: now(),
  };
  const updated: PlaythroughState = {
    ...state,
    session: {
      ...state.session,
      currentScene: scene,
      turnCount: nextIndex,
      updatedAt: now(),
      world: {
        memories: [
          ...state.session.world.memories,
          {
            id: id("memory"),
            label: action.kind === "speak" ? "说过的话" : "做过的事",
            value: action.text,
            turnIndex: nextIndex,
          },
        ],
        timeline: [
          ...state.session.world.timeline,
          {
            id: id("timeline"),
            title: `第 ${nextIndex} 回合`,
            summary: "你提前发现酒盏旁的银针，让世界线轻微偏移。",
            turnIndex: nextIndex,
          },
        ],
      },
    },
    recentTurns: [...state.recentTurns, turn],
    lastEvidence: demoEvidence,
  };
  mockStates.set(sessionId, updated);
  return updated;
}

export async function rollbackTurn(sessionId: string, turnId: string): Promise<PlaythroughState> {
  if (isTauri()) {
    return invoke("rollback_turn", { sessionId, turnId });
  }
  const state = mockStates.get(sessionId);
  if (!state) {
    throw new Error("会话不存在");
  }
  const turn = state.recentTurns.find((item) => item.id === turnId);
  if (!turn) {
    return state;
  }
  const recentTurns = state.recentTurns.filter((item) => item.turnIndex < turn.turnIndex);
  const lastScene =
    recentTurns.at(-1)?.scene || `你回到《${state.book.title}》的开场位置，可以重新选择。`;
  const updated: PlaythroughState = {
    ...state,
    session: {
      ...state.session,
      currentScene: lastScene,
      turnCount: recentTurns.length,
      world: {
        memories: state.session.world.memories.filter((item) => item.turnIndex < turn.turnIndex),
        timeline: state.session.world.timeline.filter((item) => item.turnIndex < turn.turnIndex),
      },
    },
    recentTurns,
  };
  mockStates.set(sessionId, updated);
  return updated;
}

function event(kind: HarnessEvent["kind"], title: string, detail: string): HarnessEvent {
  return {
    id: id("event"),
    kind,
    title,
    detail,
    createdAt: now(),
    tool: null,
  };
}
