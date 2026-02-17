# slafling

Fling messages to Slack. Reads from arguments or stdin and sends to a configured Slack channel via Bot Token.

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

[profiles.random]
channel = "#random"

[profiles.dm-alice]
channel = "U0123456789"   # User ID for DM

[profiles.other-workspace]
token = "xoxb-..."        # Different workspace token
channel = "#alerts"
```

The Slack Bot Token requires the `chat:write` scope.

## Usage

```bash
# Send a message
slafling "hello world"

# Pipe from stdin
echo "piped message" | slafling

# Use a profile
slafling -p random "hello random"

# Override channel
slafling -c "#test" "override test"

# Combine
cat error.log | slafling -p other-workspace -c "#incidents"
```

## License

MIT
