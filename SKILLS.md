# Skills & Lessons

本プロジェクトで得たコーディング/運用の学びを簡潔に記録します。非規範（方針は AGENTS.md）。増えたら将来分割を検討します。

## Coding（Rust/Win32）

- windows クレート（0.52系）の多くの API は `Result` を返す。
  - `LoadIconW`/`LoadCursorW`/`CreatePopupMenu` は `?` または `match` で扱う。
  - `AppendMenuW`/`TrackPopupMenu`/`DestroyMenu` は引数型（`IntoParam`）や `Option<*const RECT>` に注意。
- 非表示ウィンドウ＋トレイの最小構成:
  - 隠しウィンドウ作成 → `Shell_NotifyIconW` 追加 → 右クリックでポップアップ → 終了時に削除。
  - `WM_USER + n` の独自メッセージで衝突回避。
- ワークスペースは edition 2021 に合わせ `resolver = "2"` を設定。

- コンテキストメニュー構築の注意:
  - 先頭のタイトル（EXE 名）は `AppendMenuW(MF_STRING | MF_DISABLED | MF_GRAYED, …)` でクリック不可にする。
  - メニューに渡すワイド文字列のバッファは `TrackPopupMenu` 完了までスコープ内に保持（寿命に注意）。
  - `Exit` は英語表記にして文字化けを回避。

## Toolchain / Build（Windows GNU + MSYS2）

- MinGW は MSYS2 の UCRT64 環境で導入（`mingw-w64-ucrt-x86_64-toolchain`）。
- PATH 恒久設定は GUI（`sysdm.cpl`）または PowerShell の .NET API を使用（`setx` は 1024 文字制限に留意）。
- PowerShell の実行ファイル検索は `Get-Command` または `where.exe` を使用（`where` は `Where-Object` の別名）。

## Docs / CI / 運用

- markdownlint に準拠（フェンスに言語、リスト前後の空行、番号整形）。
- Mermaid は `<name>` などの角括弧を避け、`NAME` 等の表記が安定。
- 単独開発時のエージェントコミットは PR 必須（自己レビュー＋CI通過）。
- Issue 作業では、ユーザーから Issue ID を受け取ったら `gh` コマンドで内容を取得して把握することをルール化する。
- 実装前に方針を提示して確認を取り、OKを得てから実装する。

## 反省と実務メモ（自己検証の徹底）

- ソースを変更したら、自分で必ずビルドを通す。
  - `cargo build --release -p runner` で UI 側の変更を検証。
  - `cargo run --release -p generator -- -c configure.yaml` まで実行し、`dist/*` が更新されることを確認。
- Windows API 追加時は必要な定数/フラグのインポート漏れに注意（例: `NIF_TIP`）。
- 生成の反映漏れを防ぐため、generator は毎回 runner をビルドする設計にしておき、ドキュメントにも明記。
- 依頼がなくても、ビルドエラーがあれば自分で直して再ビルドしてからハンドオフする。

- 実行中プロセスによる生成失敗（ファイル使用中）を避ける。
  - 生成前に `off.exe` を起動するか、対象 EXE を明示的に停止してから generator を実行。
  - 将来的に generator 側に「実行中を検出して警告/スキップ」する保護を検討。

## Generator の運用上の学び

- generator は runner を毎回ビルドしてから複製する設計に変更（変更反映漏れを防止）。
- 例外として高速化したい場合は `--no-build` を指定（その際は事前に `cargo build --release -p runner` を実施）。
