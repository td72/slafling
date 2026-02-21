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

### Quick Start

```bash
slafling init
```

This creates `~/.config/slafling/config.toml` and stores your Bot Token securely (macOS Keychain on macOS, token file on other platforms).

### Token Management

Tokens are **not** stored in `config.toml`. They are resolved in this order:

1. **`SLAFLING_TOKEN` environment variable** (shared across profiles, for CI/CD and temporary overrides)
2. **Backend specified by `token_store`** — Keychain (`"keychain"`, default on macOS) or token file (`"file"`, default on other platforms)

Token storage location: `<data_dir>/slafling/tokens/<profile>` (file) or macOS Keychain service `slafling` (keychain). `<data_dir>` is `~/Library/Application Support` on macOS, `~/.local/share` on Linux.

```bash
# Store a token
slafling token set

# Store a token for a specific profile
slafling token set -p work

# Show where the token is resolved from
slafling token show

# Remove a stored token
slafling token delete
```

### Manual Setup

Create `~/.config/slafling/config.toml`:

```toml
[default]
channel = "#general"
max_file_size = "100MB"       # optional (default: 100MB, Slack API max: 1GB)
confirm = true                # optional: prompt before sending (default: false)
output = "table"              # optional: search output format — table, tsv, json (default: auto-detect)
search_types = ["public_channel", "private_channel"]  # optional (default: public_channel) — public_channel, private_channel, im, mpim
# token_store = "keychain"    # optional: keychain or file (default: keychain on macOS, file on other platforms)

[profiles.random]
channel = "#random"

[profiles.dm-alice]
channel = "D0123456789"   # Conversation ID for DM (not User ID)

[profiles.other-workspace]
channel = "#alerts"       # Use `slafling token set -p other-workspace` to store a different token
```

### Bot Token Scopes

| Scope | Required for |
|---|---|
| `chat:write` | Send text messages (`-t`) — bot must be invited to the channel |
| `chat:write.public` | Send to public channels without being invited |
| `files:write` | Upload files (`-f`) — bot must be invited to the channel |
| `channels:read` | Search public channels (`search`) |
| `groups:read` | Search private channels (`search --types private_channel`) |
| `im:read` | Search DMs (`search --types im`) |
| `mpim:read` | Search group DMs (`search --types mpim`) |

`chat:write` and `files:write` work for all conversation types (channels, DMs, group DMs). The `*:read` scopes are only needed for `search`. Only add the scopes you need.

## Usage

### Send (default)

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
```

### Search

```bash
# Search for channels by name
slafling search general

# Override output format via environment variable
export SLAFLING_OUTPUT=json
slafling search general

# Search specific channel types
slafling search general --types public_channel,private_channel

# Search with a profile (uses that profile's token)
slafling -p work search deploy

# Output as JSON
slafling search general -o json

# Pick a channel with fzf and copy its ID
slafling search dev | fzf | cut -f3 | pbcopy
```

### Init

```bash
# Create config file interactively
slafling init
```

### Token

```bash
# Store token interactively
slafling token set

# Store token for a profile
slafling token set -p work

# Show token source
slafling token show
slafling token show -p work

# Delete stored token
slafling token delete
slafling token delete -p work
```

### Validate

```bash
# Validate config file
slafling validate
```

## License

MIT
