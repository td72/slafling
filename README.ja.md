# slafling

[English](README.md)

Slackにメッセージやファイルを送信するCLIツール。Bot Tokenを使って、設定済みのチャンネルにテキスト送信やファイルアップロードができます。標準入力にも対応。

## インストール

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

[profiles.random]
channel = "#random"

[profiles.dm-alice]
channel = "D0123456789"   # DMの会話ID (ユーザーIDではない)

[profiles.other-workspace]
token = "xoxb-..."        # 別ワークスペースのトークン
channel = "#alerts"
```

Slack Bot Tokenには `chat:write` と `files:write` のスコープが必要です。

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

# チャンネルを上書き
slafling -c "#test" -t "override test"

# 組み合わせ
cat error.log | slafling -t -p other-workspace -c "#incidents"
```

## ライセンス

MIT
