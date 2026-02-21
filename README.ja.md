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

`~/.config/slafling/config.toml` を作成:

```toml
[default]
token = "xoxb-..."
channel = "#general"
max_file_size = "100MB"       # 任意 (デフォルト: 1GB)
confirm = true                # 任意: 送信前に確認プロンプトを表示 (デフォルト: false)
output = "table"              # 任意: 検索の出力形式 — table, tsv, json (デフォルト: 自動判定)
search_types = ["public_channel", "private_channel"]  # 任意 (デフォルト: public_channel) — public_channel, private_channel, im, mpim

[profiles.random]
channel = "#random"

[profiles.dm-alice]
channel = "D0123456789"   # DMの会話ID (ユーザーIDではない)

[profiles.other-workspace]
token = "xoxb-..."        # 別ワークスペースのトークン
channel = "#alerts"
```

Slack Bot Tokenには `chat:write`、`files:write`、`channels:read` のスコープが必要です。

## 使い方

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

# 設定ファイルのバリデーション
slafling validate

# チャンネル名で検索
slafling search general

# チャンネルタイプを指定して検索
slafling search general --types public-channel,private-channel

# プロファイル指定で検索 (そのプロファイルのトークンを使用)
slafling -p work search deploy

# JSON形式で出力
slafling search general -o json

# fzfでチャンネルを選んでIDをコピー
slafling search dev | fzf | cut -f3 | pbcopy
```

## ライセンス

MIT
