# Vellum

<p align="center">
  <img src="../.github/assets/vellum-banner.svg" alt="Vellum - local-first story harness" width="100%">
</p>

<p align="center">
  <strong>長編小説の中へ、プレイヤーとして入るためのローカルファーストな story harness。</strong>
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

Vellum は、中国語圏の「穿書」体験をデスクトップと Android のアプリとして扱います。小説を取り込み、自分の立場を選び、現在のシーンへ入り、発話・行動・続きを進める・根拠を見る・ターンを戻すことができます。

単一のチャットプロンプトではなく、Vellum は Codex 風の harness として設計されています。各ターンは復元可能なタスクスレッドであり、原文検索、必要最小限の文脈構築、シーン生成、連続性チェック、記憶更新、根拠記録、コミットを行います。

## ダウンロード

Android alpha は [GitHub Releases](https://github.com/wimi321/vellum/releases/latest) から入手できます。

- `Vellum-0.1.0-android-universal.apk`: sideload 用のインストール可能な Android APK。
- `Vellum-0.1.0-android-universal.aab`: ストア検証向けの Android App Bundle。
- `Vellum-0.1.0-checksums.txt`: release asset の SHA-256。

現在のバージョンは早期 alpha です。製品方向、長文インポート、harness flow の検証を目的としています。

## 設計原則

| 原則 | 内容 |
| --- | --- |
| Local first | 本、チャンク、インデックス、セッション、根拠、trace は端末に保存。 |
| 長編対応 | 本文を一括送信せず、ストリーミングで章とチャンクに分割。 |
| Evidence-led | 生成されたシーンは、参照された原文 span を確認できる。 |
| Harness first | ターンは tool、trace、rollback、memory、continuity check を持つ状態タスク。 |
| Simple by default | 通常のユーザーは「import -> identity -> start -> say / act / continue」だけで遊べる。 |
| Desktop and Android | Tauri 2、React、Rust で desktop と Android を同時に扱う。 |

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

Vellum の基本ルールは **本全体をアップロードしない** ことです。リモートモデルを使う場合でも、そのターンに必要な検索済み span だけを、ユーザーが選んだ BYOK provider へ送る設計です。

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
