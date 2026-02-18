# slafling

Fling messages and files to Slack. Sends text or uploads files to a configured Slack channel via Bot Token. Supports stdin for both text and file input.

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

The Slack Bot Token requires the `chat:write` and `files:write` scopes.

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

# Override channel
slafling -c "#test" -t "override test"

# Combine
cat error.log | slafling -t -p other-workspace -c "#incidents"
```

## License

MIT
