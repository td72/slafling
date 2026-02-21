# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

slafling — a CLI tool that flings messages to Slack. Reads from arguments or stdin, posts via Bot Token to a configured channel.

## Build & Dev Commands

```bash
cargo build              # compile
cargo build --release    # optimized binary
cargo check              # type-check without building
cargo test               # run tests
cargo clippy             # lint
cargo fmt                # format
cargo install --path .   # install locally
```

## Architecture

Synchronous CLI app (no async runtime). Four modules orchestrated by `main.rs`:

```
main.rs  →  cli.rs     (clap derive: subcommands + --text, --file, --filename, --profile, --yes, --output, --types)
         →  config.rs  (TOML load from ~/.config/slafling/config.toml, 2-layer merge: default → profile, config validation)
         →  slack.rs   (ureq POST to chat.postMessage / files.getUploadURLExternal / conversations.list with Bearer auth)
```

Subcommands: `validate` (config validation), `search <query>` (channel search). No subcommand = send mode (original behavior).

Config resolution priority: profile > default section. No runtime channel override (safety-first design).

Config fields: `token`, `channel`, `max_file_size`, `confirm`, `output`, `search_types`. Environment variables: `SLAFLING_PROFILE` (profile selection), `SLAFLING_OUTPUT` (search output format).

stdin is read when no message argument is given; errors if stdin is a TTY.

## Conventions

- Commit messages in English with gitmoji prefix
