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

Synchronous CLI app (no async runtime). Six modules orchestrated by `main.rs`:

```
main.rs  →  cli.rs      (clap derive: subcommands + --text, --file, --filename, --profile, --yes, --output, --types)
         →  config.rs   (TOML load from ~/.config/slafling/config.toml, 2-layer merge: default → profile, config validation)
         →  slack.rs    (ureq POST to chat.postMessage / files.getUploadURLExternal / conversations.list with Bearer auth)
         →  keychain.rs (macOS Keychain ops via keyring crate, #[cfg(target_os = "macos")], non-macOS stubs)
         →  token.rs    (token file read/write at <data_dir>/slafling/tokens/<profile>, where data_dir = dirs::data_dir())
```

Subcommands: `init` (interactive config generation), `validate` (config validation), `search <query>` (channel search), `token set/delete/show` (token management). No subcommand = send mode (original behavior).

`-p/--profile` is a global flag (works for all subcommands including `token`). Profile name validation rejects empty, `/`, `\`, `..`, and null characters to prevent path traversal.

Config resolution priority: profile > default section. No runtime channel override (safety-first design).

Config fields: `channel`, `max_file_size`, `confirm`, `output`, `search_types`, `token_store`. Token is **not** stored in config.toml.

Token storage backend (`token_store`): `"keychain"` (default on macOS) or `"file"` (default on other platforms). Set in `[default]` section only.

Token resolution priority (per profile, normal mode):
1. Backend specified by `token_store` — Keychain or token file
Note: `SLAFLING_TOKEN` is **only** available in headless mode (safety-first design — prevents accidental token override in normal mode).

Environment variables:
- Normal + Headless: `SLAFLING_OUTPUT` (search output format), `SLAFLING_MAX_FILE_SIZE` (file size limit), `SLAFLING_CONFIRM` (confirmation), `SLAFLING_SEARCH_TYPES` (search types)
- Normal only: `SLAFLING_PROFILE` (profile selection)
- Headless only: `SLAFLING_HEADLESS` (enable headless mode), `SLAFLING_TOKEN` (bot token), `SLAFLING_CHANNEL` (channel)

`--headless` mode: runs without config file, all settings from environment variables. Enabled by `--headless` flag or `SLAFLING_HEADLESS=1`. Requires `SLAFLING_TOKEN` and `SLAFLING_CHANNEL` (for send). `--profile` is ignored with a warning. `init`, `token`, and `validate` subcommands error in headless mode.

stdin is read when no message argument is given; errors if stdin is a TTY.

## Conventions

- Commit messages in English with gitmoji prefix
