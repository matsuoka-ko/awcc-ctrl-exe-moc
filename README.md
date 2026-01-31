# awcc-ctrl-exe-moc

[![Rust](https://img.shields.io/badge/rust-1.93.0-stable?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Target](https://img.shields.io/badge/target-x86__64--pc--windows--gnu-blue)](#)
[![MSYS2](https://img.shields.io/badge/MSYS2-UCRT64-green)](https://www.msys2.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

Alienware Command Center（AWCC）の「Per Game（アプリごとに照明プロファイル適用）」を“逆利用”して、外部からRGBを切り替えるための最小構成のWindows常駐トレイアプリと、名前別EXEをまとめて生成する仕組みを提供します。

> このアプリはAWCCのプロファイルやLEDを直接制御しません。特定のEXEが起動していることをAWCCに認識させ、そのEXEに紐づけた照明プロファイルをAWCC側で適用させます。

---

## 背景

- 近年のAWCCは「グローバルプロファイル一覧→手動切替UI」が実質消えつつあり、外部からの切替が難しい。
- 公開APIがない。
- OpenRGB は「Dell G Series D Controller」が見えてもLED定義が0で制御できないケースがある。
- 一方で AWCC の Per Game 自動切替は残っている。

本プロジェクトでは Per Game を“逆利用”して、外部からの切替（状態指定）を実現します。

---

## 完成形アーキテクチャ

```text
AWCC
 ├ System Default → ライトOFF（デフォルト状態）
 ├ NAME1.exe      → NAME1 に紐づく照明プロファイル
 ├ NAME2.exe      → NAME2 に紐づく照明プロファイル
 └ ...            → 追加分

Stream Deck
 ├ ボタン: NAME1 → NAME1.exe を起動
 ├ ボタン: NAME2 → NAME2.exe を起動
 └ ボタン: Off   → これら常駐EXEを終了（System Defaultに戻す）
```

ポイント:

- EXE名のセットは `configure.yaml` で管理し、そこに列挙した名前のEXEを生成。
- EXEの中身は同一でOK（ファイル名のみ異なる）。
- 色（照明プロファイル）の違いは AWCC 側で EXE ごとに設定。

---

## 要件（Functional）

### 1) 生成物（EXE）

各 `NAME.exe` は次を満たす:

- Windows専用（GUIサブシステム、コンソールウィンドウ非表示）
- 起動したら常駐（メッセージループ）
- タスクトレイにアイコンを表示
- 右クリックメニューに「終了」だけを表示し、それで終了可能
- 色やプロファイル内容は AWCC 側で管理（本アプリは AWCC を直接操作しない）
- 起動直後に「同系統の他色 EXE（同一ファミリー）」が動作中なら安全に終了させ、自分だけが残る（シングルトン運用）

### 2) 生成機構

- `configure.yaml` を読み、指定された名前のEXEをまとめて生成できる
- 設定ファイルを変えれば後からEXEを追加可能
- 生成方式は問わない（ベースEXEをビルドしてコピー＆リネームでも可）

---

## 設定ファイル仕様（configure.yaml）

- 形式: YAML / 文字コード: UTF-8
- `profiles` に並べた順でEXEを生成
- `name` はEXE名（拡張子なし）。Windowsファイル名として安全な文字のみ許可（推奨: `^[A-Za-z0-9_-]+$`）

最小構成例:

```yaml
version: 1
profiles:
  - name: streaming
  - name: dark
  - name: meeting
```

生成されるEXE:

- `streaming.exe`
- `dark.exe`
- `meeting.exe`

任意項目（あれば便利）:

```yaml
version: 1
output_dir: dist
profiles:
  - name: streaming
  - name: dark
```

- `output_dir` 未指定時は `dist` をデフォルトにします

---

## 非目標（Non-goals）

- AWCC のリバース/非公開プロトコル解析はしない
- USB HID を直接叩かない
- OpenRGB 連携はしない
- AWCC の現在状態取得やトグルはしない（状態指定型で運用）

---

## 開発環境（Visual Studio不要）

PowerShell中心。winget活用。GNUツールチェーン（MSYS2 UCRT）でのビルドを前提。

- Rust: rustup
- Toolchain: `stable-x86_64-pc-windows-gnu`
- MinGW: MSYS2（UCRT64）

winget 例:

```powershell
winget install --id Rustlang.Rustup -e
winget install --id MSYS2.MSYS2 -e
```

MSYS2 UCRT64 で:

```bash
pacman -Syu
pacman -S --needed base-devel mingw-w64-ucrt-x86_64-toolchain
```

PowerShell の PATH に追加:

```text
C:\msys64\ucrt64\bin
```

---

## 実装方針（Rust）

最小の Win32 メッセージループ＋トレイアイコン。

- `windows` crate で Win32 API（`CreateWindowExW` / `RegisterClassW` / `GetMessageW` 等）
- トレイ: `Shell_NotifyIconW` を直接使用（または最小ラッパを利用）
- 必須挙動:
  - 隠しウィンドウを作る（メッセージ受信のため）
  - トレイアイコンを追加
  - 右クリックでコンテキストメニューを出し、「終了」クリックで終了
  - 終了時にトレイアイコン削除

---

## ビルドと生成

実装完了後の想定手順（暫定）:

```powershell
# 依存の取得・ビルド
cargo build --release

# 生成コマンドの例（最終形は実装で決定）
# configure.yaml を元に dist/ 以下へ NAME.exe を生成
cargo run --bin generator -- -c configure.yaml
```

- 生成物の出力先は既定で `dist/` を想定

---

## 運用（AWCC / Stream Deck）

AWCC 側:

- System Default: ライトOFF に設定
- `dist/NAME.exe` を各名前ごとに Per Game の対象として追加し、照明プロファイルを割り当て

Stream Deck 側:

- 「NAME」ボタン: `dist/NAME.exe` を起動（他色EXEは起動直後に本アプリ側で自動停止）
- 「Off」ボタン: すべての色EXEを終了（任意運用。何も起動していない状態は System Default=OFF）

---

## 受け入れ条件（Acceptance Criteria）

- `configure.yaml` に指定した名前のEXEが `dist/` に生成される
- `NAME.exe` 起動 → AWCCが `NAME.exe` に紐づく照明に切り替わる
- トレイの右クリックメニューから「終了」で終了できる
- `NAME.exe` 終了 → System Default（OFF）に戻る
- `configure.yaml` に名前を追加すれば、同様にEXEが追加生成できる（拡張容易）

---

## 使い方の流れ（Mermaid 図）

概要フロー（flowchart）:

```mermaid
graph LR
    SD[Stream Deck ボタン]
    EXE[NAME.exe]
    PROF[NAME プロファイル適用]
    DEF[System Default]

    SD -->|起動| EXE
    EXE -->|Per Game マッチ| PROF
    EXE -->|非マッチ| DEF
    KILL[Offボタン/終了] -->|終了| EXE
    EXE -. 停止 .-> DEF
```

シーケンス図（sequenceDiagram）:

```mermaid
sequenceDiagram
    participant User as ユーザー
    participant SD as Stream Deck
    participant EXE as NAME.exe
    participant OTH as 他色EXE群
    participant AW as AWCC

    User->>SD: ボタン押下（NAME）
    SD->>EXE: プロセス起動
    EXE-->>OTH: 起動直後に他色EXEを終了
    EXE-->>AW: 実行中（OS がプロセス名を通知）
    AW-->>AW: Per Game 設定で NAME.exe を検出
    AW->>AW: NAME に紐づく照明プロファイル適用
    Note over AW: EXE 生存中は継続適用

    User->>SD: Off/終了 操作（任意）
    SD->>EXE: 終了（またはトレイから終了）
    EXE-->>AW: プロセス終了
    AW->>AW: System Default（ライトOFF）に復帰
```

状態遷移図（stateDiagram）:

```mermaid
stateDiagram-v2
    [*] --> SystemDefault
    SystemDefault: Light OFF

    SystemDefault --> ProfileApplied: NAME.exe 起動
    ProfileApplied --> SystemDefault: NAME.exe 終了

    ProfileApplied: NAME プロファイル適用中
```

---

## Stream Deck 応用例（本体色のワンボタン切替）

目的: Stream Deck から任意の色（= AWCC プロファイル）に即時切替し、Off で System Default に戻す。

セットアップ:

- `configure.yaml` に色名を追加し、EXE を生成（例: `streaming.exe`, `dark.exe`, `meeting.exe`）
- AWCC の Per Game で各 EXE に照明プロファイルを割り当て

ボタン例（推奨パターン）:

- シングルトン対応により、各色ボタンは単純に `dist/NAME.exe` を起動するだけで他色が自動停止し、対象色のみが残ります

応用メモ:

- 既定を OFF 運用にしたい場合、Off ボタンは `taskkill /IM streaming.exe /IM dark.exe /IM meeting.exe /F` を実行（任意）
- EXE 名を増やしたら、本アプリの生成時にファミリーとして内包されるため、起動直後の自動停止対象に含まれます

---

## 使用例: 色に合わせた EXE でライト切替

色ごとに EXE を用意し、AWCC の Per Game で色プロファイルを割り当て、Stream Deck から起動して本体ライトを切り替える例です。

1. `configure.yaml` に色名を列挙（例）:

    ```yaml
    version: 1
    profiles:
      - name: red
      - name: blue
      - name: green
      - name: warm_white
    ```

1. 生成後、`dist/` に以下が並ぶ想定:
    - `red.exe`, `blue.exe`, `green.exe`, `warm_white.exe`

1. AWCC で設定:
    - Per Game に上記 EXE をそれぞれ追加
    - `red.exe` → 真っ赤の照明プリセット、`blue.exe` → 青、`green.exe` → 緑、`warm_white.exe` → 暖色白 …のように割り当て

1. Stream Deck のボタン割り当て:
    - 「Red」: `dist/red.exe` を起動
    - 「Blue」: `dist/blue.exe` を起動
    - 「Green」: `dist/green.exe` を起動
    - 「Warm」: `dist/warm_white.exe` を起動
    - 「Off」: すべての色EXEを停止
      - 例（PowerShell）:

        ```powershell
        powershell -NoProfile -WindowStyle Hidden -Command "Get-Process red,blue,green,warm_white -ErrorAction SilentlyContinue | Stop-Process -Force"
        ```

ポイント:

- 各 EXE の中身は同一で、ファイル名のみが AWCC のマッチキーになります
- 複数色 EXE を同時起動しても AWCC の動作は環境依存になるため、常に一つだけ動かす運用を推奨します
- 既定（何も起動していない状態）は System Default（ライトOFF）にしておくと分かりやすいです

---

## ライセンス

- MIT License（`LICENSE` を参照）

---

## ステータス / ロードマップ（更新）

完了（現在）:

- 環境セットアップ手順（MSYS2 UCRT64 + GNU toolchain）を整備
- ワークスペース作成（`crates/runner`, `crates/generator`）
- runner: 最小のWin32トレイ常駐（右クリック「終了」）
- generator: `configure.yaml` → `dist/<name>.exe` 生成（複製方式）
- CI: markdownlint（GitHub Actions）を導入
- ドキュメント: README/AGENTS/SKILLS/CLAUDE を追加・整備

次の予定:

- runner: 起動直後に同系統EXEを停止（シングルトン運用の実装）
- runner: トレイ表示の磨き込み（アイコン/ツールチップ/バージョン）
- generator: 将来的にファミリー情報の埋め込み（設定からの読込orビルド時埋込）
- CI: Windows ビルド（GNU）とリリースアセットの作成
- 配布: アイコン付与・コードサイン（任意、可能なら）
- ドキュメント: AWCC Per Game 設定例の拡充（スクリーンショット等・任意）
# awcc-ctrl-exe-moc

Alienware Command Center（AWCC）の「Per Game（アプリごとに照明プロファイル適用）」を“逆利用”して、外部からRGBを切り替えるための最小構成のWindows常駐トレイアプリと、名前別EXEをまとめて生成する仕組みを提供します。

> このアプリはAWCCのプロファイルやLEDを直接制御しません。特定のEXEが起動していることをAWCCに認識させ、そのEXEに紐づけた照明プロファイルをAWCC側で適用させます。

---

## 背景

- 近年のAWCCは「グローバルプロファイル一覧→手動切替UI」が実質消えつつあり、外部からの切替が難しい。
- 公開APIがない。
- OpenRGB は「Dell G Series D Controller」が見えてもLED定義が0で制御できないケースがある。
- 一方で AWCC の Per Game 自動切替は残っている。

本プロジェクトでは Per Game を“逆利用”して、外部からの切替（状態指定）を実現します。

---

## 完成形アーキテクチャ

```text
AWCC
 ├ System Default → ライトOFF（デフォルト状態）
 ├ NAME1.exe      → NAME1 に紐づく照明プロファイル
 ├ NAME2.exe      → NAME2 に紐づく照明プロファイル
 └ ...            → 追加分

Stream Deck
 ├ ボタン: NAME1 → NAME1.exe を起動
 ├ ボタン: NAME2 → NAME2.exe を起動
 └ ボタン: Off   → これら常駐EXEを終了（System Defaultに戻す）
```

ポイント:

- EXE名のセットは `configure.yaml` で管理し、そこに列挙した名前のEXEを生成。
- EXEの中身は同一でOK（ファイル名のみ異なる）。
- 色（照明プロファイル）の違いは AWCC 側で EXE ごとに設定。

---

## 要件（Functional）

### 1) 生成物（EXE）

各 `NAME.exe` は次を満たす:

- Windows専用（GUIサブシステム、コンソールウィンドウ非表示）
- 起動したら常駐（メッセージループ）
- タスクトレイにアイコンを表示
- 右クリックメニューに「終了」だけを表示し、それで終了可能
- 色やプロファイル内容は AWCC 側で管理（本アプリは AWCC を直接操作しない）
- 起動直後に「同系統の他色 EXE（同一ファミリー）」が動作中なら安全に終了させ、自分だけが残る（シングルトン運用）

### 2) 生成機構

- `configure.yaml` を読み、指定された名前のEXEをまとめて生成できる
- 設定ファイルを変えれば後からEXEを追加可能
- 生成方式は問わない（ベースEXEをビルドしてコピー＆リネームでも可）

---

## 設定ファイル仕様（configure.yaml）

- 形式: YAML / 文字コード: UTF-8
- `profiles` に並べた順でEXEを生成
- `name` はEXE名（拡張子なし）。Windowsファイル名として安全な文字のみ許可（推奨: `^[A-Za-z0-9_-]+$`）

最小構成例:

```yaml
version: 1
profiles:
  - name: streaming
  - name: dark
  - name: meeting
```

生成されるEXE:

- `streaming.exe`
- `dark.exe`
- `meeting.exe`

任意項目（あれば便利）:

```yaml
version: 1
output_dir: dist
profiles:
  - name: streaming
  - name: dark
```

- `output_dir` 未指定時は `dist` をデフォルトにします

---

## 非目標（Non-goals）

- AWCC のリバース/非公開プロトコル解析はしない
- USB HID を直接叩かない
- OpenRGB 連携はしない
- AWCC の現在状態取得やトグルはしない（状態指定型で運用）

---

## 開発環境（Visual Studio不要）

PowerShell中心。winget活用。GNUツールチェーン（MSYS2 UCRT）でのビルドを前提。

- Rust: rustup
- Toolchain: `stable-x86_64-pc-windows-gnu`
- MinGW: MSYS2（UCRT64）

winget 例:

```powershell
winget install --id Rustlang.Rustup -e
winget install --id MSYS2.MSYS2 -e
```

MSYS2 UCRT64 で:

```bash
pacman -Syu
pacman -S --needed base-devel mingw-w64-ucrt-x86_64-toolchain
```

PowerShell の PATH に追加:

```text
C:\msys64\ucrt64\bin
```

---

## セットアップ（ゼロから）

Windows が素の状態（何も入っていない前提）からの構築手順です。

1. winget を有効化（未導入の場合）

    - Microsoft Store で「App Installer」をインストール/更新
    - 新しい PowerShell を開き直して `winget --version` を確認

1. Git / GitHub CLI（任意）をインストール

    ```powershell
    winget install --id Git.Git -e
    winget install --id GitHub.cli -e
    ```

1. Rustup をインストール

    ```powershell
    winget install --id Rustlang.Rustup -e
    ```

1. MSYS2 をインストール

    ```powershell
    winget install --id MSYS2.MSYS2 -e
    ```

1. MSYS2 UCRT64 で MinGW ツールチェーンを導入

    - スタートメニューから「MSYS2 UCRT64」を起動

    ```bash
    pacman -Syu
    pacman -S --needed base-devel mingw-w64-ucrt-x86_64-toolchain
    ```

1. PATH に UCRT64 の bin を追加（恒久設定を推奨）

    推奨はいずれも「ユーザー環境変数 Path」を安全に編集する方法です。`setx` は 1024 文字制限で既存の Path が切り捨てられる恐れがあるため推奨しません。

    - GUI（推奨・安全）
      - `Win + R` → `sysdm.cpl` → 詳細設定 → 環境変数
      - ユーザーの「Path」を選択 → 編集 → 新規 で `C:\msys64\ucrt64\bin` を追加
      - 可能なら上位に移動 → OK で保存 → 新しい PowerShell を開き直す

    - PowerShell（.NET API を使用）

      ```powershell
      $ucrt = 'C:\msys64\ucrt64\bin'
      $user = [Environment]::GetEnvironmentVariable('Path','User')
      if ($user -notlike "*$ucrt*") {
        [Environment]::SetEnvironmentVariable('Path', ($user + ';' + $ucrt), 'User')
      }
      ```

      反映のため PowerShell を開き直します。

    - 反映までの一時回避（セッション限定）

      ```powershell
      $env:Path = "C:\msys64\ucrt64\bin;" + $env:Path
      ```

1. Rust の GNU ツールチェーンを既定に設定

    ```powershell
    rustup toolchain install stable-x86_64-pc-windows-gnu
    rustup default stable-x86_64-pc-windows-gnu
    ```

1. リポジトリの取得（未取得の場合）

    ```powershell
    git clone https://github.com/<owner>/<repo>.git
    cd <repo>
    ```

1. 動作確認

    ```powershell
    rustc -Vv   # host: x86_64-pc-windows-gnu が期待値
    where gcc   # C:\msys64\ucrt64\bin\gcc.exe が見える
    ```

1. ビルド

    ```powershell
    cargo build --release
    ```

メモ:

- 「MinGW は入れる必要があるのか？」→ はい。MSYS2 の UCRT64 環境で `mingw-w64-ucrt-x86_64-toolchain` を導入します。
- Visual Studio/Build Tools なしでビルドするため、GNU ツールチェーン（UCRT）を採用しています。

---

## 実装方針（Rust）

最小の Win32 メッセージループ＋トレイアイコン。

- `windows` crate で Win32 API（`CreateWindowExW` / `RegisterClassW` / `GetMessageW` 等）
- トレイ: `Shell_NotifyIconW` を直接使用（または最小ラッパを利用）
- 必須挙動:
  - 隠しウィンドウを作る（メッセージ受信のため）
  - トレイアイコンを追加
  - 右クリックでコンテキストメニューを出し、「終了」クリックで終了
  - 終了時にトレイアイコン削除

---

## ビルドと生成

実装完了後の想定手順（暫定）:

```powershell
# 依存の取得・ビルド
cargo build --release

# 生成コマンドの例（最終形は実装で決定）
# configure.yaml を元に dist/ 以下へ NAME.exe を生成
cargo run --bin generator -- -c configure.yaml
```

- 生成物の出力先は既定で `dist/` を想定

---

## 運用（AWCC / Stream Deck）

AWCC 側:

- System Default: ライトOFF に設定
- `dist/NAME.exe` を各名前ごとに Per Game の対象として追加し、照明プロファイルを割り当て

Stream Deck 側:

- 「NAME」ボタン: `dist/NAME.exe` を起動（他色EXEは起動直後に本アプリ側で自動停止）
- 「Off」ボタン: すべての色EXEを終了（任意運用。何も起動していない状態は System Default=OFF）

---

## 受け入れ条件（Acceptance Criteria）

- `configure.yaml` に指定した名前のEXEが `dist/` に生成される
- `NAME.exe` 起動 → AWCCが `NAME.exe` に紐づく照明に切り替わる
- トレイの右クリックメニューから「終了」で終了できる
- `NAME.exe` 終了 → System Default（OFF）に戻る
- `configure.yaml` に名前を追加すれば、同様にEXEが追加生成できる（拡張容易）

---

## 使い方の流れ（Mermaid 図）

概要フロー（flowchart）:

```mermaid
graph LR
    SD[Stream Deck ボタン]
    EXE[NAME.exe]
    PROF[NAME プロファイル適用]
    DEF[System Default]

    SD -->|起動| EXE
    EXE -->|Per Game マッチ| PROF
    EXE -->|非マッチ| DEF
    KILL[Offボタン/終了] -->|終了| EXE
    EXE -. 停止 .-> DEF
```

シーケンス図（sequenceDiagram）:

```mermaid
sequenceDiagram
    participant User as ユーザー
    participant SD as Stream Deck
    participant EXE as NAME.exe
    participant OTH as 他色EXE群
    participant AW as AWCC

    User->>SD: ボタン押下（NAME）
    SD->>EXE: プロセス起動
    EXE-->>OTH: 起動直後に他色EXEを終了
    EXE-->>AW: 実行中（OS がプロセス名を通知）
    AW-->>AW: Per Game 設定で NAME.exe を検出
    AW->>AW: NAME に紐づく照明プロファイル適用
    Note over AW: EXE 生存中は継続適用

    User->>SD: Off/終了 操作（任意）
    SD->>EXE: 終了（またはトレイから終了）
    EXE-->>AW: プロセス終了
    AW->>AW: System Default（ライトOFF）に復帰
```

状態遷移図（stateDiagram）:

```mermaid
stateDiagram-v2
    [*] --> SystemDefault
    SystemDefault: Light OFF

    SystemDefault --> ProfileApplied: NAME.exe 起動
    ProfileApplied --> SystemDefault: NAME.exe 終了

    ProfileApplied: NAME プロファイル適用中
```

---

## Stream Deck 応用例（本体色のワンボタン切替）

目的: Stream Deck から任意の色（= AWCC プロファイル）に即時切替し、Off で System Default に戻す。

セットアップ:

- `configure.yaml` に色名を追加し、EXE を生成（例: `streaming.exe`, `dark.exe`, `meeting.exe`）
- AWCC の Per Game で各 EXE に照明プロファイルを割り当て

ボタン例（推奨パターン）:

- シングルトン対応により、各色ボタンは単純に `dist/NAME.exe` を起動するだけで他色が自動停止し、対象色のみが残ります

応用メモ:

- 既定を OFF 運用にしたい場合、Off ボタンは `taskkill /IM streaming.exe /IM dark.exe /IM meeting.exe /F` を実行（任意）
- EXE 名を増やしたら、本アプリの生成時にファミリーとして内包されるため、起動直後の自動停止対象に含まれます

---

## 使用例: 色に合わせた EXE でライト切替

色ごとに EXE を用意し、AWCC の Per Game で色プロファイルを割り当て、Stream Deck から起動して本体ライトを切り替える例です。

1. `configure.yaml` に色名を列挙（例）:

    ```yaml
    version: 1
    profiles:
      - name: red
      - name: blue
      - name: green
      - name: warm_white
    ```

1. 生成後、`dist/` に以下が並ぶ想定:

    - `red.exe`, `blue.exe`, `green.exe`, `warm_white.exe`

1. AWCC で設定:

    - Per Game に上記 EXE をそれぞれ追加
    - `red.exe` → 真っ赤の照明プリセット、`blue.exe` → 青、`green.exe` → 緑、`warm_white.exe` → 暖色白 …のように割り当て

1. Stream Deck のボタン割り当て:

    - 「Red」: `dist/red.exe` を起動
    - 「Blue」: `dist/blue.exe` を起動
    - 「Green」: `dist/green.exe` を起動
    - 「Warm」: `dist/warm_white.exe` を起動
    - 「Off」: すべての色EXEを停止

      - 例（PowerShell）:

        ```powershell
        powershell -NoProfile -WindowStyle Hidden -Command "Get-Process red,blue,green,warm_white -ErrorAction SilentlyContinue | Stop-Process -Force"
        ```

ポイント:

- 各 EXE の中身は同一で、ファイル名のみが AWCC のマッチキーになります
- 複数色 EXE を同時起動しても AWCC の動作は環境依存になるため、常に一つだけ動かす運用を推奨します
- 既定（何も起動していない状態）は System Default（ライトOFF）にしておくと分かりやすいです

---

## ライセンス

- MIT License（`LICENSE` を参照）

---

## ステータス / ロードマップ

- 現在: リポジトリ初期化・要件定義（本README）
- 次: Rust ワークスペース作成、トレイ常駐ランナーの実装、`configure.yaml` からの EXE 生成 CLI 実装、README 運用手順の拡充、Windows（GNU）向けCI
