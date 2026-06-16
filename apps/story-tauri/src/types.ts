export type ImportStatus = "queued" | "running" | "completed" | "failed";

export type BookSource = {
  path: string;
  title?: string | null;
};

export type ImportJob = {
  id: string;
  status: ImportStatus;
  progress: number;
  message: string;
  bookId?: string | null;
  createdAt: string;
};

export type BookManifest = {
  id: string;
  title: string;
  sourcePath: string;
  chapterCount: number;
  chunkCount: number;
  charCount: number;
  status: ImportStatus;
  createdAt: string;
};

export type ChunkRef = {
  bookId: string;
  chunkIndex: number;
  chapterTitle: string;
  startChar: number;
  endChar: number;
  preview: string;
  score: number;
};

export type EvidenceSpan = {
  chunk: ChunkRef;
  text: string;
};

export type PlayerIdentity = {
  name: string;
  role: string;
  goal: string;
  tone: string;
};

export type PlayerActionKind = "speak" | "act" | "continue";

export type PlayerAction = {
  kind: PlayerActionKind;
  text: string;
};

export type MemoryItem = {
  id: string;
  label: string;
  value: string;
  turnIndex: number;
};

export type TimelineEvent = {
  id: string;
  title: string;
  summary: string;
  turnIndex: number;
};

export type WorldState = {
  memories: MemoryItem[];
  timeline: TimelineEvent[];
};

export type ToolCallRecord = {
  name: string;
  inputSummary: string;
  outputSummary: string;
  durationMs: number;
};

export type HarnessEvent = {
  id: string;
  kind:
    | "searchSource"
    | "readChunk"
    | "retrieveContext"
    | "draftScene"
    | "continuityCheck"
    | "updateMemory"
    | "commitTurn"
    | "rollbackTurn";
  title: string;
  detail: string;
  tool?: ToolCallRecord | null;
  createdAt: string;
};

export type StorySession = {
  id: string;
  bookId: string;
  identity: PlayerIdentity;
  currentScene: string;
  turnCount: number;
  world: WorldState;
  updatedAt: string;
};

export type StoryTurn = {
  id: string;
  sessionId: string;
  turnIndex: number;
  action: PlayerAction;
  scene: string;
  evidence: EvidenceSpan[];
  trace: HarnessEvent[];
  createdAt: string;
};

export type PlaythroughState = {
  book: BookManifest;
  session: StorySession;
  recentTurns: StoryTurn[];
  lastEvidence: EvidenceSpan[];
};
