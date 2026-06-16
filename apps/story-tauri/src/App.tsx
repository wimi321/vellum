import {
  BookOpen,
  Brain,
  ChevronDown,
  Database,
  Footprints,
  MessageCircle,
  Play,
  RotateCcw,
  Send,
  Sparkles,
  UploadCloud,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  getImportStatus,
  importBook,
  listBooks,
  pickBookFolder,
  pickBookPath,
  rollbackTurn,
  sendPlayerAction,
  startPlaythrough,
} from "./api";
import { actionLabel, actionPlaceholder, defaultActionText, formatCount } from "./storyCopy";
import type {
  BookManifest,
  ImportJob,
  PlayerActionKind,
  PlayerIdentity,
  PlaythroughState,
} from "./types";
import { canImportBook, canStartStory, importStatusText } from "./userFlow";

const starterIdentity: PlayerIdentity = {
  name: "林晚",
  role: "刚穿进书里的普通人",
  goal: "活下来，并尽量不让喜欢的角色走向坏结局",
  tone: "简单直接",
};

export default function App() {
  const [books, setBooks] = useState<BookManifest[]>([]);
  const [selectedBookId, setSelectedBookId] = useState<string>("");
  const [identity, setIdentity] = useState<PlayerIdentity>(starterIdentity);
  const [state, setState] = useState<PlaythroughState | null>(null);
  const [importPath, setImportPath] = useState("");
  const [importTitle, setImportTitle] = useState("");
  const [importJob, setImportJob] = useState<ImportJob | null>(null);
  const [actionKind, setActionKind] = useState<PlayerActionKind>("speak");
  const [actionText, setActionText] = useState("");
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(false);
  const setupRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    void refreshBooks();
  }, []);

  const selectedBook = useMemo(
    () => books.find((book) => book.id === selectedBookId) || books[0],
    [books, selectedBookId],
  );
  const latestTurn = state?.recentTurns.at(-1);
  const evidence = state?.lastEvidence.length ? state.lastEvidence : latestTurn?.evidence || [];
  const trace = latestTurn?.trace || [];

  async function refreshBooks() {
    const loaded = await listBooks();
    setBooks(loaded);
    setSelectedBookId((current) => current || loaded[0]?.id || "");
  }

  async function chooseFile() {
    const path = await pickBookPath();
    if (path) {
      setImportPath(path);
      setImportTitle(path.split(/[\\/]/).pop()?.replace(/\.(txt|md|epub)$/i, "") || "");
    }
  }

  async function chooseFolder() {
    const path = await pickBookFolder();
    if (path) {
      setImportPath(path);
      setImportTitle(path.split(/[\\/]/).pop() || "");
    }
  }

  async function runImport() {
    if (!importPath.trim()) {
      setError("请先选择小说文件或章节文件夹");
      focusImport();
      return;
    }
    setBusy("正在导入小说");
    setError("");
    try {
      const job = await importBook({
        path: importPath.trim() || "浏览器演示小说",
        title: importTitle.trim() || null,
      });
      const completed = await waitForImport(job);
      setImportJob(completed);
      await refreshBooks();
      if (completed.bookId) {
        setSelectedBookId(completed.bookId);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy("");
    }
  }

  async function beginStory() {
    if (!selectedBook) {
      setError("请先导入一本小说，索引完成后再开始穿书");
      focusImport();
      return;
    }
    setBusy("正在进入故事");
    setError("");
    try {
      setState(await startPlaythrough(selectedBook.id, identity));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy("");
    }
  }

  async function waitForImport(job: ImportJob): Promise<ImportJob> {
    let latest = job;
    setImportJob(latest);
    for (let attempt = 0; attempt < 240; attempt += 1) {
      if (latest.status === "completed") {
        return latest;
      }
      if (latest.status === "failed") {
        throw new Error(latest.message || "导入失败");
      }
      await new Promise((resolve) => window.setTimeout(resolve, 500));
      latest = await getImportStatus(job.id);
      setImportJob(latest);
    }
    throw new Error("导入时间太久，请稍后在书库里查看结果");
  }

  function focusImport() {
    setupRef.current?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  async function submitAction(kind = actionKind) {
    if (!state) {
      await beginStory();
      return;
    }
    const text = (kind === "continue" ? actionText || defaultActionText(kind) : actionText).trim();
    if (!text) {
      setActionText(defaultActionText(kind));
      return;
    }
    setBusy(kind === "continue" ? "正在推进剧情" : "正在续写");
    setError("");
    try {
      const next = await sendPlayerAction(state.session.id, { kind, text });
      setState(next);
      setActionText("");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy("");
    }
  }

  async function undoLatest() {
    if (!state || !latestTurn) {
      return;
    }
    setBusy("正在回到上一步");
    try {
      setState(await rollbackTurn(state.session.id, latestTurn.id));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy("");
    }
  }

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark">穿</span>
          <div>
            <strong>Vellum</strong>
            <small>本地保存 · 桌面和手机都能玩</small>
          </div>
        </div>
        <div className="save-pill">
          <Database size={16} />
          本地保存
        </div>
      </header>

      <main className="shell">
        <section className="setup-panel" aria-label="导入和身份" ref={setupRef}>
          <div className="flow">
            <span className="flow-step is-done">导入小说</span>
            <span className="flow-step">选择身份</span>
            <span className="flow-step">开始穿书</span>
          </div>

          <div className="section-head">
            <BookOpen size={20} />
            <h2>书库</h2>
          </div>
          <div className="import-box">
            <button className="icon-button" onClick={chooseFile} title="选择小说文件">
              <UploadCloud size={18} />
            </button>
            <input
              value={importPath}
              onChange={(event) => setImportPath(event.target.value)}
              placeholder="选择文件/文件夹，或粘贴路径"
            />
          </div>
          <button className="folder-button" onClick={chooseFolder} type="button">
            选择章节文件夹
          </button>
          <input
            className="title-input"
            value={importTitle}
            onChange={(event) => setImportTitle(event.target.value)}
            placeholder="书名，可不填"
          />
          <button
            className="primary-button"
            onClick={runImport}
            disabled={!canImportBook(importPath, busy)}
          >
            <UploadCloud size={18} />
            导入并建立索引
          </button>
          {importJob && (
            <div className="import-status" aria-label="导入进度">
              <div>
                <span style={{ width: `${Math.max(4, importJob.progress * 100)}%` }} />
              </div>
              <p>{importStatusText(importJob)}</p>
            </div>
          )}

          <div className="book-list">
            {books.length ? (
              books.map((book) => (
                <button
                  key={book.id}
                  className={`book-row ${selectedBook?.id === book.id ? "is-selected" : ""}`}
                  onClick={() => setSelectedBookId(book.id)}
                >
                  <span>
                    <strong>{book.title}</strong>
                    <small>
                      {formatCount(book.charCount, "字")} · {formatCount(book.chapterCount, "章")}
                    </small>
                  </span>
                  <span className="ready-dot">可玩</span>
                </button>
              ))
            ) : (
              <div className="empty-book">
                <strong>还没有小说</strong>
                <span>先导入 txt、md、epub，或一个章节文件夹。</span>
              </div>
            )}
          </div>

          <div className="section-head identity-head">
            <Sparkles size={20} />
            <h2>我的身份</h2>
          </div>
          <label>
            名字
            <input
              value={identity.name}
              onChange={(event) => setIdentity({ ...identity, name: event.target.value })}
            />
          </label>
          <label>
            我是谁
            <input
              value={identity.role}
              onChange={(event) => setIdentity({ ...identity, role: event.target.value })}
            />
          </label>
          <label>
            我想做到
            <textarea
              value={identity.goal}
              onChange={(event) => setIdentity({ ...identity, goal: event.target.value })}
            />
          </label>
          <button
            className="start-button"
            onClick={beginStory}
            disabled={!canStartStory(Boolean(selectedBook), busy)}
          >
            <Play size={18} />
            开始穿书
          </button>
        </section>

        <section className="story-panel" aria-label="当前场景">
          <div className="story-progress" aria-label="穿书步骤">
            <span>导入小说</span>
            <span>选择身份</span>
            <span>开始穿书</span>
          </div>
          <div className="scene-toolbar">
            <div>
              <span className="eyeless-label">当前场景</span>
              <h1>{state?.book.title || selectedBook?.title || "先导入一本小说"}</h1>
            </div>
            <button className="ghost-button" onClick={undoLatest} disabled={!latestTurn || Boolean(busy)}>
              <RotateCcw size={17} />
              回到上一步
            </button>
          </div>

          <article className="scene-text">
            {state?.session.currentScene ||
              (selectedBook
                ? "选择一个你想扮演的身份，然后点“开始穿书”。你可以说话、做动作，也可以直接推进剧情。"
                : "先导入一本小说。导入完成后，选择身份，就能从读者视角进入故事。桌面端和手机端都是同一套简单玩法。")}
          </article>

          {!selectedBook && (
            <button className="inline-start" onClick={focusImport}>
              去导入小说
            </button>
          )}

          <div className="scene-pills" aria-label="当前状态">
            <span>我的身份：{state?.session.identity.name || identity.name}</span>
            <span>记忆 {state?.session.world.memories.length || 0}</span>
            <span>世界线 {state?.session.world.timeline.length || 0}</span>
            <span>背包</span>
          </div>

          <span className="choice-label">剧情选择</span>
          <div className="action-tabs" role="tablist" aria-label="剧情动作">
            <ActionTab
              active={actionKind === "speak"}
              icon={<MessageCircle size={18} />}
              label="说一句"
              onClick={() => setActionKind("speak")}
            />
            <ActionTab
              active={actionKind === "act"}
              icon={<Footprints size={18} />}
              label="做动作"
              onClick={() => setActionKind("act")}
            />
            <ActionTab
              active={actionKind === "continue"}
              icon={<Play size={18} />}
              label="继续剧情"
              onClick={() => {
                setActionKind("continue");
                setActionText(defaultActionText("continue"));
              }}
            />
          </div>

          <div className="composer">
            <textarea
              value={actionText}
              onChange={(event) => setActionText(event.target.value)}
              placeholder={actionPlaceholder(actionKind)}
            />
            <button onClick={() => submitAction()} disabled={Boolean(busy)}>
              <Send size={18} />
              {busy || actionLabel(actionKind)}
            </button>
          </div>
          {error && <p className="error-line">{error}</p>}
        </section>

        <aside className="world-panel" aria-label="原文依据和记忆">
          <div className="mini-section">
            <div className="section-head">
              <BookOpen size={19} />
              <h2>原文依据</h2>
            </div>
            {evidence.length ? (
              evidence.slice(0, 3).map((span) => (
                <div className="evidence" key={`${span.chunk.bookId}-${span.chunk.chunkIndex}`}>
                  <strong>{span.chunk.chapterTitle}</strong>
                  <p>{span.text}</p>
                </div>
              ))
            ) : (
              <p className="empty">开始后会显示本回合用到的原文片段。</p>
            )}
          </div>

          <div className="mini-section">
            <div className="section-head">
              <Brain size={19} />
              <h2>记忆</h2>
            </div>
            {state?.session.world.memories.length ? (
              state.session.world.memories.slice(-4).map((memory) => (
                <p className="memory" key={memory.id}>
                  <span>{memory.label}</span>
                  {memory.value}
                </p>
              ))
            ) : (
              <p className="empty">你的选择会自动保存成记忆。</p>
            )}
          </div>

          <div className="mini-section">
            <button className="diagnostic-toggle" onClick={() => setDiagnosticsOpen((value) => !value)}>
              后台运行
              <ChevronDown className={diagnosticsOpen ? "is-open" : ""} size={18} />
            </button>
            {diagnosticsOpen && (
              <div className="trace-list">
                {trace.length ? (
                  trace.map((item) => (
                    <div className="trace-row" key={item.id}>
                      <span>{item.title}</span>
                      <small>{item.detail}</small>
                    </div>
                  ))
                ) : (
                  <p className="empty">开始行动后，这里会显示原文检索、连续性检查和保存状态。</p>
                )}
              </div>
            )}
          </div>
        </aside>
      </main>
    </div>
  );
}

function ActionTab({
  active,
  icon,
  label,
  onClick,
}: {
  active: boolean;
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <button className={`action-tab ${active ? "is-active" : ""}`} onClick={onClick}>
      {icon}
      {label}
    </button>
  );
}
