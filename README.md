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

Tokens are **not** stored in `config.toml`. They are resolved from the backend specified by `token_store` — Keychain (`"keychain"`, default on macOS) or token file (`"file"`, default on other platforms).

In headless mode, `SLAFLING_TOKEN` environment variable is used instead.

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

`-p/--profile` and `SLAFLING_PROFILE` work for all subcommands including `token`.

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

### Environment Variables

| Variable | Description | Available in |
|---|---|---|
| `SLAFLING_PROFILE` | Profile selection | Normal |
| `SLAFLING_TOKEN` | Bot token | Headless |
| `SLAFLING_OUTPUT` | Search output format (`table`, `tsv`, `json`) | Normal, Headless |
| `SLAFLING_HEADLESS` | Enable headless mode (`1`, `true`, `yes`) | — |
| `SLAFLING_CHANNEL` | Channel to send to (`#channel` or `C01ABCDEF`) | Headless |
| `SLAFLING_MAX_FILE_SIZE` | File size limit (`100MB`, `1GB`, etc.) | Normal, Headless |
| `SLAFLING_CONFIRM` | Prompt before sending (`true`, `1`, `yes`) | Normal, Headless |
| `SLAFLING_SEARCH_TYPES` | Channel types for search (comma-separated) | Normal, Headless |

### Headless Mode

Run without a config file — all settings come from environment variables (see above). Useful for CI/CD, Docker, cron, and other non-interactive environments.

Enable with `--headless` flag or `SLAFLING_HEADLESS=1`. Requires `SLAFLING_TOKEN` and `SLAFLING_CHANNEL` (for send).

```bash
# Send a message
SLAFLING_TOKEN=xoxb-... SLAFLING_CHANNEL="#deploy" slafling --headless -t "deploy complete"

# Pipe from stdin
echo "build log" | SLAFLING_TOKEN=xoxb-... SLAFLING_CHANNEL="#ci" slafling --headless -t

# Search channels
SLAFLING_TOKEN=xoxb-... slafling --headless search general

# Using SLAFLING_HEADLESS env var (no --headless flag needed)
export SLAFLING_HEADLESS=1
export SLAFLING_TOKEN=xoxb-...
export SLAFLING_CHANNEL="#alerts"
slafling -t "alert message"
```

`--profile` is ignored in headless mode (with a warning). `init`, `token`, and `validate` subcommands are not available in headless mode.

## License

MIT
