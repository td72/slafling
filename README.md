# slafling

[日本語](README.ja.md)

Fling messages and files to Slack. Sends text or uploads files to a configured Slack channel via Bot Token. Supports stdin for both text and file input.

## Concept

slafling is a **safety-first** Slack CLI tool. Messages always go to pre-configured destinations — there is no ad-hoc channel override flag. This design prevents accidental messages to wrong channels caused by typos or copy-paste mistakes.

Use **profiles** to manage multiple channels. Each profile explicitly maps to a destination, making message routing deliberate and reviewable.

## Install

```bash
cargo install --path .
```

## Setup

Create `~/.config/slafling/config.toml`:

```toml
[default]
token = "xoxb-..."
channel = "#general"
max_file_size = "100MB"       # optional (default: 1GB)

[profiles.random]
channel = "#random"

[profiles.dm-alice]
channel = "D0123456789"   # Conversation ID for DM (not User ID)

[profiles.other-workspace]
token = "xoxb-..."        # Different workspace token
channel = "#alerts"
```

The Slack Bot Token requires the `chat:write`, `files:write`, and `channels:read` scopes.

## Usage

```bash
# Send a text message
slafling -t "hello world"

# Pipe text from stdin
echo "piped message" | slafling -t

# Upload a file
slafling -f image.png

# Upload from stdin with a filename
cat report.csv | slafling -f -n report.csv

# File upload with a comment
slafling -f error.log -t "Check this log"

# Use a profile
slafling -p random -t "hello random"

# Search for channels by name
slafling search general

# Search with a profile (uses that profile's token)
slafling -p work search deploy

# Pick a channel with fzf and copy its ID
slafling search dev | fzf | cut -f2 | pbcopy
```

## License

MIT
