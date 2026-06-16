# Vellum

<p align="center">
  <img src=".github/assets/vellum-banner.svg" alt="Vellum - local-first story harness" width="100%">
</p>

<p align="center">
  <strong>A local-first story harness for entering long novels as a playable character.</strong>
</p>

<p align="center">
  <a href="README.md">English</a>
  ·
  <a href="docs/README.zh-CN.md">简体中文</a>
  ·
  <a href="docs/README.ja.md">日本語</a>
  ·
  <a href="docs/README.ko.md">한국어</a>
</p>

<p align="center">
  <a href="https://github.com/wimi321/vellum/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/wimi321/vellum?sort=semver"></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/github/license/wimi321/vellum"></a>
  <img alt="Built with Tauri" src="https://img.shields.io/badge/Tauri-2.x-24c8db">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-core-b7410e">
  <img alt="React" src="https://img.shields.io/badge/React-UI-149eca">
  <img alt="Local first" src="https://img.shields.io/badge/local--first-privacy-2f7d5e">
</p>

Vellum turns the Chinese "穿书" fantasy into a real desktop and Android app: import a long novel, choose who you are, step into the current scene, then speak, act, continue, inspect source evidence, or roll back a turn.

Unlike a single chat prompt, Vellum is built as a Codex-style harness. Every story turn is a recoverable thread of tool calls: retrieve the book, assemble the minimum context, draft the next scene, check continuity, update memory, record evidence, and commit the turn.

## Download

Android alpha builds are published on the [latest GitHub Release](https://github.com/wimi321/vellum/releases/latest).

- `Vellum-0.1.0-android-universal.apk` is the installable Android package for sideload testing.
- `Vellum-0.1.0-android-universal.aab` is the Android App Bundle for store-oriented validation.
- `Vellum-0.1.0-checksums.txt` contains SHA-256 checksums for release assets.

Vellum is still an early alpha. The current release is meant for product validation, import scale testing, and harness-flow testing before production provider credentials and stable signing are finalized.

## Why Vellum

Most long-novel roleplay systems fail in one of two ways: they upload too much text, or they lose the book as soon as the player starts improvising. Vellum takes a different path.

| Principle | What it means |
| --- | --- |
| Local first | Books, chunks, indexes, sessions, evidence, trace, and play history stay on the device. |
| Million-character ready | Import streams text into chapters and chunks instead of sending the whole novel to a model. |
| Evidence-led play | Generated scenes can expose the source spans that shaped the turn. |
| Harness, not chat glue | Turns are modeled as stateful tasks with tools, trace, rollback, memory, and continuity checks. |
| Simple by default | Players see "import novel -> choose identity -> start story -> speak / act / continue". |
| Desktop and Android | One Tauri 2 app targets desktop and Android, with shared React UI and Rust core. |

## Experience

1. Import a `.txt`, `.md`, DRM-free `.epub`, or chapter folder.
2. Vellum streams the text, splits chapters, creates chunks, and builds local search metadata.
3. Choose an identity: name, role, and intention.
4. Start a playthrough from the current scene.
5. Use one of three core actions:
   - **Say** something to a character.
   - **Act** inside the scene.
   - **Continue** the story.
6. Inspect source evidence, memory, timeline, and optional harness trace.
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

### Harness turn loop

```text
PlayerAction
  -> retrieve_context
  -> draft_scene
  -> continuity_check
  -> update_memory
  -> commit_turn
```

Internal tool calls are recorded as trace events. The UI keeps them folded away unless the player wants to inspect how a turn was produced.

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

The design rule is simple: **never upload the whole book**. When remote model providers are enabled, only the current turn's necessary retrieved spans should be sent to the user-selected BYOK provider.

## Current Status

Vellum is an early, runnable prototype. The architecture, storage layer, import pipeline, harness loop, desktop shell, Android project, and mock model flow are in place.

Provider adapters are scaffolded for BYOK use, but production model calls and secure key storage are still V1 roadmap work. The default development flow uses a local mock model so the app can be tested without sending content to any external service.

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

## Release Signing

Tauri's Android release output is unsigned by default. To produce an installable sideload APK, provide a local keystore through environment variables and run:

```bash
VELLUM_ANDROID_KEYSTORE="$HOME/.android/vellum-release.jks" \
VELLUM_ANDROID_KEYSTORE_PASSWORD="..." \
VELLUM_ANDROID_KEY_PASSWORD="..." \
scripts/sign-android-apk.sh
```

The script aligns, signs, and verifies `dist/release/v0.1.0/Vellum-0.1.0-android-universal.apk`.

## Validation

The current prototype has been validated with:

- Rust unit tests for import, search, persistence, rollback, and a synthetic 2M Chinese-character scale import.
- Harness loop test for `import -> start -> action -> retrieve -> scene -> memory -> resume`.
- Vitest coverage for copy and first-time user-flow guards.
- Web production build.
- macOS Tauri bundle build.
- Android universal APK and AAB build.
- APK metadata inspection with `aapt dump badging`.
- APK signing verification with `apksigner verify`.

Android emulator runtime smoke is still tracked separately because the current development machine does not have a usable emulator binary or connected device.

## Roadmap

- Secure local key storage for provider credentials.
- Real OpenAI-compatible, Anthropic-compatible, Gemini-compatible, and local Ollama/OpenAI-compatible model calls.
- Better `.epub` extraction and chapter normalization.
- Import pause/resume UI for very large books.
- Character, location, and event extraction passes.
- Dedicated Android emulator and device smoke tests in CI.
- More complete continuity checking and world-state diff review.
- Stable production signing and update channel for Android releases.

## License

Apache-2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
