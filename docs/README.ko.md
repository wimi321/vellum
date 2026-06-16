# Vellum

<p align="center">
  <img src="../.github/assets/vellum-banner.svg" alt="Vellum - local-first story harness" width="100%">
</p>

<p align="center">
  <strong>긴 소설 속으로 플레이어가 들어가기 위한 로컬 우선 story harness.</strong>
</p>

<p align="center">
  <a href="../README.md">English</a>
  ·
  <a href="README.zh-CN.md">简体中文</a>
  ·
  <a href="README.ja.md">日本語</a>
  ·
  <a href="README.ko.md">한국어</a>
</p>

Vellum은 중국어권의 "穿書" 경험을 데스크톱과 Android 앱으로 구현합니다. 소설을 가져오고, 자신의 정체성을 고른 뒤, 현재 장면에 들어가 말하고 행동하고 이야기를 이어가며 원문 근거를 확인하거나 턴을 되돌릴 수 있습니다.

Vellum은 하나의 채팅 프롬프트가 아니라 Codex 스타일의 harness로 설계되었습니다. 각 턴은 복구 가능한 작업 스레드이며, 원문 검색, 필요한 컨텍스트 구성, 장면 작성, 연속성 검사, 메모리 업데이트, 근거 기록, 턴 커밋을 수행합니다.

## Download

Android alpha builds are available from [GitHub Releases](https://github.com/wimi321/vellum/releases/latest).

- `Vellum-0.1.0-android-universal.apk`: sideload 가능한 Android 설치 패키지.
- `Vellum-0.1.0-android-universal.aab`: 스토어 검증용 Android App Bundle.
- `Vellum-0.1.0-checksums.txt`: release asset SHA-256 checksums.

The current build is an early alpha for validating product direction, import scale, and harness flow.

## Principles

| Principle | Meaning |
| --- | --- |
| Local first | Books, chunks, indexes, sessions, evidence, and trace stay on the device. |
| Long-novel ready | Text is streamed into chapters and chunks instead of being uploaded whole. |
| Evidence-led | Generated scenes can show the source spans that shaped the turn. |
| Harness first | Turns are stateful tasks with tools, trace, rollback, memory, and continuity checks. |
| Simple by default | Users play through import, identity, start, say, act, and continue. |
| Desktop and Android | One Tauri 2 app targets desktop and Android with React and Rust. |

## Architecture

```text
crates/
  story-store/          SQLite, import jobs, chunks, search, sessions, evidence, trace
  story-harness-core/   StoryTurn loop, tool orchestration, memory, rollback
  model-adapters/       BYOK provider profiles and model-client boundary

apps/
  story-tauri/          Tauri 2 + React app for desktop and Android
```

## Privacy

Vellum's rule is simple: **never upload the whole book**. When remote model providers are enabled, only the retrieved spans needed for the current turn should be sent to the user's selected BYOK provider.

## Quick Start

```bash
npm install
npm test
cargo test --workspace
npm run tauri:build
npm run android:build
```

## License

Apache-2.0. See [LICENSE](../LICENSE) and [NOTICE](../NOTICE).
