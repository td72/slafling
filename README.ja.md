# slafling

[English](README.md)

Slackにメッセージやファイルを送信するCLIツール。Bot Tokenを使って、設定済みのチャンネルにテキスト送信やファイルアップロードができます。標準入力にも対応。

## コンセプト

slaflingは**安全第一**のSlack CLIツールです。メッセージは常に事前設定された送信先に送られ、アドホックなチャンネル指定フラグはありません。タイポやコピペミスによる誤送信を防ぐ設計です。

複数のチャンネルを使い分けるには**プロファイル**を利用します。各プロファイルが送信先を明示的にマッピングするため、メッセージの送信先が意図的かつレビュー可能になります。

## インストール

### Homebrew

```bash
brew install td72/tap/slafling
```

### crates.io から

```bash
cargo install slafling
```

### GitHub Releases から

[Releases](https://github.com/td72/slafling/releases) からビルド済みバイナリをダウンロード。

対応ターゲット:
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `aarch64-apple-darwin`
- `x86_64-apple-darwin`

### ソースから

```bash
cargo install --path .
```

## セットアップ

### クイックスタート

```bash
slafling init
```

Bot Tokenを入力すると `~/.config/slafling/config.toml` を生成し、トークンを安全に保存します（macOS では Keychain、他プラットフォームではトークンファイル）。

### トークン管理

トークンは `config.toml` には**保存されません**。以下の優先順位で解決されます:

1. **`SLAFLING_TOKEN` 環境変数** (全プロファイル共通、CI/CD や一時的なオーバーライド用)
2. **`token_store` で指定されたバックエンド** — Keychain (`"keychain"`, macOS デフォルト) またはトークンファイル (`"file"`, 他プラットフォームのデフォルト)

トークン保存先: `<data_dir>/slafling/tokens/<プロファイル名>` (file) または macOS Keychain サービス `slafling` (keychain)。`<data_dir>` は macOS では `~/Library/Application Support`、Linux では `~/.local/share`。

```bash
# トークンを保存
slafling token set

# 特定プロファイルのトークンを保存
slafling token set -p work

# トークンの解決元を表示
slafling token show

# トークンを削除
slafling token delete
```

### 手動セットアップ

`~/.config/slafling/config.toml` を作成:

```toml
[default]
channel = "#general"
max_file_size = "100MB"       # 任意 (デフォルト: 100MB, Slack API上限: 1GB)
confirm = true                # 任意: 送信前に確認プロンプトを表示 (デフォルト: false)
output = "table"              # 任意: 検索の出力形式 — table, tsv, json (デフォルト: 自動判定)
search_types = ["public_channel", "private_channel"]  # 任意 (デフォルト: public_channel) — public_channel, private_channel, im, mpim
# token_store = "keychain"    # 任意: keychain or file (デフォルト: macOS は keychain、他は file)

[profiles.random]
channel = "#random"

[profiles.dm-alice]
channel = "D0123456789"   # DMの会話ID (ユーザーIDではない)

[profiles.other-workspace]
channel = "#alerts"       # `slafling token set -p other-workspace` で別トークンを保存
```

### Bot Token スコープ

| スコープ | 用途 |
|---|---|
| `chat:write` | テキスト送信 (`-t`) — bot をチャンネルに招待する必要あり |
| `chat:write.public` | パブリックチャンネルへの送信（招待不要） |
| `files:write` | ファイルアップロード (`-f`) — bot をチャンネルに招待する必要あり |
| `channels:read` | パブリックチャンネル検索 (`search`) |
| `groups:read` | プライベートチャンネル検索 (`search --types private_channel`) |
| `im:read` | DM検索 (`search --types im`) |
| `mpim:read` | グループDM検索 (`search --types mpim`) |

`chat:write` と `files:write` は全会話タイプ（チャンネル、DM、グループDM）で動作します。`*:read` 系スコープは `search` でのみ必要です。必要なスコープだけ追加すれば十分です。

## 使い方

### Send (デフォルト)

```bash
# テキストメッセージを送信
slafling -t "hello world"

# 標準入力からテキストを送信
echo "piped message" | slafling -t

# ファイルをアップロード
slafling -f image.png

# 標準入力からファイルをアップロード (-n でファイル名を指定)
cat report.csv | slafling -f -n report.csv

# ファイルアップロード + コメント
slafling -f error.log -t "このログを確認してください"

# プロファイルを指定
slafling -p random -t "hello random"

# 環境変数でプロファイルを指定
export SLAFLING_PROFILE=random
slafling -t "hello random"

# 送信前に確認 (config で confirm = true の場合)
slafling -t "重要なメッセージ"    # プロンプト表示: Send? [y/N]
slafling -t "確認スキップ" -y     # --yes で確認をスキップ
```

### Search

```bash
# チャンネル名で検索
slafling search general

# 環境変数で出力形式を指定
export SLAFLING_OUTPUT=json
slafling search general

# チャンネルタイプを指定して検索
slafling search general --types public_channel,private_channel

# プロファイル指定で検索 (そのプロファイルのトークンを使用)
slafling -p work search deploy

# JSON形式で出力
slafling search general -o json

# fzfでチャンネルを選んでIDをコピー
slafling search dev | fzf | cut -f3 | pbcopy
```

### Init

```bash
# 設定ファイルを対話的に作成
slafling init
```

### Token

`-p/--profile` と `SLAFLING_PROFILE` は `token` を含む全サブコマンドで使用可能です。

```bash
# トークンを対話的に保存
slafling token set

# プロファイル指定でトークンを保存
slafling token set -p work

# トークンの解決元を表示
slafling token show
slafling token show -p work

# トークンを削除
slafling token delete
slafling token delete -p work
```

### Validate

```bash
# 設定ファイルのバリデーション
slafling validate
```

### 環境変数

| 変数 | 説明 | 利用可能なモード |
|---|---|---|
| `SLAFLING_PROFILE` | プロファイル選択 | 通常 |
| `SLAFLING_TOKEN` | Bot トークン | Headless |
| `SLAFLING_OUTPUT` | 検索の出力形式 (`table`, `tsv`, `json`) | 通常, Headless |
| `SLAFLING_HEADLESS` | Headless モード有効化 (`1`, `true`, `yes`) | — |
| `SLAFLING_CHANNEL` | 送信先チャンネル (`#channel` or `C01ABCDEF`) | Headless |
| `SLAFLING_MAX_FILE_SIZE` | ファイルサイズ上限 (`100MB`, `1GB` 等) | 通常, Headless |
| `SLAFLING_CONFIRM` | 送信前に確認 (`true`, `1`, `yes`) | 通常, Headless |
| `SLAFLING_SEARCH_TYPES` | 検索するチャンネルタイプ (カンマ区切り) | 通常, Headless |

### Headless モード

設定ファイルなしで動作 — すべての設定を環境変数から取得します（上記参照）。CI/CD、Docker、cron、その他の非対話環境で便利です。

`--headless` フラグまたは `SLAFLING_HEADLESS=1` で有効化。`SLAFLING_TOKEN` と `SLAFLING_CHANNEL` (送信時) が必須です。

```bash
# メッセージを送信
SLAFLING_TOKEN=xoxb-... SLAFLING_CHANNEL="#deploy" slafling --headless -t "deploy complete"

# 標準入力から送信
echo "build log" | SLAFLING_TOKEN=xoxb-... SLAFLING_CHANNEL="#ci" slafling --headless -t

# チャンネル検索
SLAFLING_TOKEN=xoxb-... slafling --headless search general

# SLAFLING_HEADLESS 環境変数を使用 (--headless フラグ不要)
export SLAFLING_HEADLESS=1
export SLAFLING_TOKEN=xoxb-...
export SLAFLING_CHANNEL="#alerts"
slafling -t "alert message"
```

`--profile` は headless モードでは無視されます（警告を表示）。`init`、`token`、`validate` サブコマンドは headless モードでは使用できません。

## ライセンス

MIT
