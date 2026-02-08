# Changelog

本プロジェクトの重要な変更履歴を記録します。記法は概ね Keep a Changelog に準拠し、バージョニングは Semantic Versioning を目安にします。

## [Unreleased]

- generator: 実行中 EXE によるロック検出の保護（警告/スキップ、または `off.exe` 自動実行→再試行）の検討
- runner: 終了手順の丁寧化（WM_CLOSE → 待機 → Terminate など）
- runner: 実験的な前面維持（`keep_foreground.txt` がある場合、1x1 の前面・最前面ウィンドウを維持。フォーカスは奪わない）
- runner/generator: `awcc` 設定で AWCC 未起動時にバックグラウンド起動（任意）
- CI: Windows Release ビルド・アーティファクトの公開（任意）

## [1.0.0] - 2026-01-31

### Added

- Windows 常駐トレイアプリ（runner）
  - 隠しウィンドウ、トレイアイコン、右クリックメニュー（先頭に「awcc-ctrl-exe-moc - 色」（クリック不可）／下段に `Exit`）
  - マウスオーバー時のツールチップ（「awcc-ctrl-exe-moc - 色」）
- 生成ツール（generator）
  - `configure.yaml` から `dist/<name>.exe` を生成（毎回 runner をビルドしてから複製。`--no-build` でスキップ可）
  - `family.txt` を出力し、runner は起動直後に同系統 EXE を自動停止（シングルトン）
  - `off_name` 指定で `off.exe` を生成（同系統停止→自身も即終了で System Default に復帰）
  - 以前の構成から外れた EXE を自動削除（`family.txt`/`off.txt` に基づくクリーンアップ）
- ドキュメント
  - README（セットアップ、運用、Mermaid 図、UX、トラブルシューティング）
  - AGENTS.md（運用方針、ビルド・自己検証の原則、lint 方針）
  - SKILLS.md（学びと注意点）
  - CLAUDE.md（参照先の明記）
- CI
  - markdownlint（DavidAnson/markdownlint-cli2-action）
  - Windows でのビルド＋ generator ユニットテスト（`cargo test -p generator`）

### Changed

- Mermaid 図のプレースホルダを安定する表記に統一し、パースエラーを解消
- README の PATH 設定を恒久対策（GUI/.NET API）ベースに変更

### Known limitations

- generator 実行時、該当 EXE が起動中だと削除/上書きに失敗する（Windows のファイルロック仕様）。`off.exe` で停止してから実行する運用を推奨（保護実装は Unreleased）。
