# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

slafling — a CLI tool that flings messages to Slack. Reads from arguments or stdin, posts via Bot Token to a configured channel.

## Build & Dev Commands

```bash
cargo build              # compile
cargo build --release    # optimized binary
cargo check              # type-check without building
cargo clippy             # lint
cargo fmt                # format
cargo install --path .   # install locally
```

## Architecture

Synchronous CLI app (no async runtime). Four modules orchestrated by `main.rs`:

```
main.rs  →  cli.rs     (clap derive: --text, --file, --filename, --profile)
         →  config.rs  (TOML load from ~/.config/slafling/config.toml, 2-layer merge: default → profile)
         →  slack.rs   (ureq POST to chat.postMessage with Bearer auth)
```

Config resolution priority: profile > default section. No runtime channel override (safety-first design).

stdin is read when no message argument is given; errors if stdin is a TTY.

## Conventions

- Commit messages in English with gitmoji prefix
