# slafling

[日本語](README.ja.md)

Fling messages and files to Slack. Sends text or uploads files to a configured Slack channel via Bot Token. Supports stdin for both text and file input.

## Concept

slafling is a **safety-first** Slack CLI tool. Messages always go to pre-configured destinations — there is no ad-hoc channel override flag. This design prevents accidental messages to wrong channels caused by typos or copy-paste mistakes.

Use **profiles** to manage multiple channels. Each profile explicitly maps to a destination, making message routing deliberate and reviewable.

## Install

### Homebrew

```bash
brew install td72/tap/slafling
```

### From crates.io

```bash
cargo install slafling
```

### From GitHub Releases

Download a prebuilt binary from [Releases](https://github.com/td72/slafling/releases).

Available targets:
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `aarch64-apple-darwin`
- `x86_64-apple-darwin`

### From source

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
confirm = true                # optional: prompt before sending (default: false)
output = "table"              # optional: search output format — table, tsv, json (default: auto-detect)
search_types = ["public_channel", "private_channel"]  # optional (default: public_channel) — public_channel, private_channel, im, mpim

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

# Use a profile via environment variable
export SLAFLING_PROFILE=random
slafling -t "hello random"

# Confirm before sending (when confirm = true in config)
slafling -t "important message"   # prompts: Send? [y/N]
slafling -t "skip prompt" -y      # skip confirmation with --yes

# Validate config file
slafling validate

# Search for channels by name
slafling search general

# Search specific channel types
slafling search general --types public-channel,private-channel

# Search with a profile (uses that profile's token)
slafling -p work search deploy

# Output as JSON
slafling search general -o json

# Pick a channel with fzf and copy its ID
slafling search dev | fzf | cut -f3 | pbcopy
```

## License

MIT
