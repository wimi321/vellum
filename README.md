# Vellum

Vellum is a local-first story harness for entering long novels as a playable character.

It turns the “穿书” fantasy into a desktop and Android app: import a novel, choose who you are, step into the current scene, then speak, act, continue, or roll back while the system keeps track of source evidence, memories, and timeline changes.

Vellum is built around a Codex-style harness rather than a single chat prompt. Each story turn is a recoverable thread of tool calls: retrieve the source text, assemble only the necessary context, draft the next scene, check continuity, update memory, and commit the turn.

## Why Vellum

Most long-novel roleplay systems fail in one of two ways: they upload too much text, or they lose the book as soon as the player starts improvising. Vellum takes a different path.

- **Local first**: books, chunks, search indexes, sessions, evidence, traces, and play history stay on the device.
- **Built for million-character books**: import is streamed into chapters and chunks instead of sending the whole novel to a model.
- **Evidence-led play**: every generated scene can expose the source spans that shaped the turn.
- **Harness, not chat glue**: turns are modeled as stateful tasks with tools, trace, rollback, memory, and continuity checks.
- **Simple by default**: normal players see “import novel -> choose identity -> start story -> speak / act / continue”.
- **Desktop and Android**: the same Tauri 2 app targets macOS/desktop and Android, with shared React UI and Rust core.

## Current Status

Vellum is an early, runnable prototype. The architecture, storage layer, import pipeline, harness loop, desktop shell, Android project, and mock model flow are in place. Provider adapters are scaffolded for BYOK use, but production model calls and secure key storage are still V1 roadmap work.

The default development flow uses a local mock model so the app can be tested without sending content to any external service.

## User Flow

1. Import a `.txt`, `.md`, DRM-free `.epub`, or chapter folder.
2. Vellum streams the text, splits chapters, creates chunks, and builds local search metadata.
3. Choose an identity: name, role, and intention.
4. Start a playthrough.
5. Use one of three core actions:
   - **Say** something to a character.
   - **Act** inside the scene.
   - **Continue** the story.
6. Inspect source evidence, memory, timeline, and the optional harness trace.
7. Roll back the latest turn when the story moves in the wrong direction.

## Architecture

```text
crates/
  story-store/          SQLite, import jobs, chunks, search, sessions, evidence, trace
  story-harness-core/   StoryTurn loop, tool orchestration, memory, rollback
  model-adapters/       BYOK provider profiles and model-client boundary

apps/
  story-tauri/          Tauri 2 + React app for desktop and Android
```

### Harness Turn Loop

```text
PlayerAction
  -> retrieve_context
  -> draft_scene
  -> continuity_check
  -> update_memory
  -> commit_turn
```

Internal tool calls are recorded as trace events, but the UI keeps them folded away unless the player wants to inspect how a turn was produced.

## Storage and Privacy

Vellum stores the following locally:

- imported books
- chapter and chunk metadata
- n-gram search index data
- playthrough sessions
- source evidence
- memories and timeline events
- harness trace
- provider profiles

The design rule is simple: **never upload the whole book**. When remote model providers are enabled, only the current turn’s necessary retrieved spans should be sent to the user-selected BYOK provider.

## Quick Start

Requirements:

- Node.js 20+
- Rust
- Tauri 2 prerequisites for your platform
- Android SDK, JDK 17, NDK, and `rustup` for Android builds

Install dependencies:

```bash
npm install
```

Run the web shell:

```bash
npm run dev
```

Run the desktop app:

```bash
npm run tauri:dev
```

Run tests:

```bash
npm test
cargo test --workspace
```

Build desktop bundles:

```bash
npm run tauri:build
```

Initialize and build Android:

```bash
npm run android:init
npm run android:build
```

If Android setup fails because an NDK directory is missing `source.properties`, remove the incomplete NDK directory or reinstall a complete NDK with Android Studio / `sdkmanager`.

## Validation

The current prototype has been validated with:

- Rust unit tests for import, search, persistence, rollback, and a synthetic 2M Chinese-character scale import.
- Harness loop test for `import -> start -> action -> retrieve -> scene -> memory -> resume`.
- Vitest coverage for copy and first-time user-flow guards.
- Web production build.
- macOS Tauri bundle build.
- Android universal APK and AAB build.
- APK metadata inspection with `aapt dump badging`.
- Desktop and mobile viewport Playwright flow tests for import, start, action, evidence, memory, trace, and rollback.

## Repository Layout

```text
.
├── apps/story-tauri
│   ├── src              React UI
│   └── src-tauri        Tauri shell, commands, Android project
├── crates/model-adapters
├── crates/story-harness-core
├── crates/story-store
├── Cargo.toml
└── package.json
```

## Roadmap

- Secure local key storage for provider credentials.
- Real OpenAI-compatible, Anthropic-compatible, Gemini-compatible, and local Ollama/OpenAI-compatible model calls.
- Better `.epub` extraction and chapter normalization.
- Import pause/resume UI for very large books.
- Character, location, and event extraction passes.
- Dedicated Android emulator and device smoke tests in CI.
- More complete continuity checking and world-state diff review.

## License

Apache-2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
