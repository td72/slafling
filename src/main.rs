mod cli;
mod config;
mod keychain;
mod slack;
mod token;

use std::io::{BufRead, IsTerminal, Read, Write};

use anyhow::{bail, Context, Result};
use clap::Parser;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    // Headless mode: all settings from environment variables
    let headless = cli.headless || config::is_headless_env();

    // Handle init and token before loading config (config may not exist yet)
    match &cli.command {
        Some(cli::Command::Init) => {
            if headless {
                bail!("init is not available in headless mode");
            }
            return run_init();
        }
        Some(cli::Command::Token { action }) => {
            if headless {
                bail!("token is not available in headless mode");
            }
            let profile = cli
                .profile
                .or_else(|| std::env::var("SLAFLING_PROFILE").ok());
            return run_token(action, profile.as_deref());
        }
        _ => {}
    }

    if headless {
        if cli.profile.is_some() || std::env::var("SLAFLING_PROFILE").ok().is_some() {
            eprintln!("warning: --profile is ignored in headless mode");
        }
        return run_headless(cli.command, cli.send);
    }

    let cfg = config::load_config()?;

    let profile = cli
        .profile
        .or_else(|| std::env::var("SLAFLING_PROFILE").ok());

    match cli.command {
        Some(cli::Command::Init) => unreachable!(),
        Some(cli::Command::Token { .. }) => unreachable!(),
        Some(cli::Command::Validate) => {
            let path = config::config_path()?;
            println!("{}: ok", path.display());
            Ok(())
        }
        Some(cli::Command::Search {
            query,
            output,
            types,
        }) => {
            let types_str = match types {
                Some(t) => cli::search_types_to_api_string(&t),
                None => config::resolve_search_types(&cfg, profile.as_deref())
                    .unwrap_or_else(|| "public_channel".to_string()),
            };
            run_search(profile.as_deref(), &query, output, &types_str, &cfg)
        }
        None => run_send(profile.as_deref(), cli.send, &cfg),
    }
}

fn run_init() -> Result<()> {
    let path = config::config_path()?;

    if path.exists() {
        let stdin = std::io::stdin();
        if !stdin.is_terminal() {
            bail!(
                "{} already exists (run interactively to confirm overwrite)",
                path.display()
            );
        }
        eprint!("{} already exists. Overwrite? [y/N] ", path.display());
        std::io::stderr().flush()?;
        let mut input = String::new();
        stdin.lock().read_line(&mut input)?;
        if !matches!(input.trim(), "y" | "Y") {
            bail!("aborted");
        }
    }

    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        bail!("init requires interactive input (stdin must be a TTY)");
    }

    eprint!("Bot Token (xoxb-...): ");
    std::io::stderr().flush()?;
    let mut token_input = String::new();
    stdin.lock().read_line(&mut token_input)?;
    let token_value = token_input.trim();
    if token_value.is_empty() {
        bail!("token is required");
    }

    // Store token using platform default (config doesn't exist yet)
    store_token(config::default_token_store(), None, token_value)?;

    // Write config without token
    config::write_init_config(&path)?;

    println!("created {}", path.display());
    Ok(())
}

fn run_headless(command: Option<cli::Command>, send: cli::SendArgs) -> Result<()> {
    match command {
        Some(cli::Command::Init) | Some(cli::Command::Token { .. }) => unreachable!(),
        Some(cli::Command::Validate) => {
            bail!("validate has no effect in headless mode");
        }
        Some(cli::Command::Search {
            query,
            output,
            types,
        }) => {
            let token = config::resolve_token_from_env()?;
            let types_str = match types {
                Some(t) => cli::search_types_to_api_string(&t),
                None => {
                    let s = config::resolve_search_types_from_env()
                        .unwrap_or_else(|| "public_channel".to_string());
                    config::validate_search_types_str(&s)?;
                    s
                }
            };
            let format = resolve_output_format_headless(output);
            run_search_with_token(&token, &query, format, &types_str)
        }
        None => {
            let resolved = config::resolve_from_env()?;
            run_send_with_resolved(send, &resolved)
        }
    }
}

fn resolve_output_format_headless(cli_output: Option<cli::OutputFormat>) -> cli::OutputFormat {
    if let Some(f) = cli_output {
        return f;
    }

    if let Ok(s) = std::env::var("SLAFLING_OUTPUT") {
        match s.to_lowercase().as_str() {
            "table" => return cli::OutputFormat::Table,
            "tsv" => return cli::OutputFormat::Tsv,
            "json" => return cli::OutputFormat::Json,
            _ => {}
        }
    }

    if std::io::stdout().is_terminal() {
        cli::OutputFormat::Table
    } else {
        cli::OutputFormat::Tsv
    }
}

fn store_token(token_store: &str, profile: Option<&str>, token_value: &str) -> Result<()> {
    match token_store {
        "keychain" => {
            keychain::set_token(profile, token_value)?;
            let account = profile.unwrap_or("default");
            eprintln!("token stored in Keychain (account: {account})");
        }
        "file" => {
            token::set_token(profile, token_value)?;
            let path = token::token_path(profile)?;
            eprintln!("token stored in {}", path.display());
        }
        _ => bail!("invalid token_store '{token_store}'"),
    }
    Ok(())
}

/// Load token_store from config file, falling back to platform default if config doesn't exist.
fn load_token_store() -> Result<String> {
    let path = config::config_path()?;
    if !path.exists() {
        return Ok(config::default_token_store().to_string());
    }
    let cfg = config::load_config()?;
    Ok(config::resolve_token_store(&cfg))
}

fn run_token(action: &cli::TokenAction, profile: Option<&str>) -> Result<()> {
    match action {
        cli::TokenAction::Set => run_token_set(profile),
        cli::TokenAction::Delete => run_token_delete(profile),
        cli::TokenAction::Show => run_token_show(profile),
    }
}

fn run_token_set(profile: Option<&str>) -> Result<()> {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        bail!("token set requires interactive input (stdin must be a TTY)");
    }

    eprint!("Bot Token (xoxb-...): ");
    std::io::stderr().flush()?;
    let mut token_input = String::new();
    stdin.lock().read_line(&mut token_input)?;
    let token_value = token_input.trim();
    if token_value.is_empty() {
        bail!("token is required");
    }

    let token_store = load_token_store()?;
    store_token(&token_store, profile, token_value)?;
    Ok(())
}

fn run_token_delete(profile: Option<&str>) -> Result<()> {
    let token_store = load_token_store()?;

    match token_store.as_str() {
        "keychain" => {
            let account = profile.unwrap_or("default");
            if keychain::get_token(profile)?.is_none() {
                bail!("no stored token found for profile '{account}'");
            }
            keychain::delete_token(profile)?;
            eprintln!("deleted token from Keychain (account: {account})");
        }
        "file" => {
            let path = token::token_path(profile)?;
            if !path.exists() {
                let name = profile.unwrap_or("default");
                bail!("no stored token found for profile '{name}'");
            }
            token::delete_token(profile)?;
            eprintln!("deleted {}", path.display());
        }
        _ => bail!("invalid token_store '{token_store}'"),
    }

    Ok(())
}

fn run_token_show(profile: Option<&str>) -> Result<()> {
    let token_store = load_token_store()?;
    match config::describe_token_source(&token_store, profile) {
        Ok((source, location)) => {
            println!("source: {source}");
            println!("location: {location}");
        }
        Err(e) => {
            println!("not configured: {e}");
        }
    }
    Ok(())
}

fn run_search(
    profile: Option<&str>,
    query: &str,
    cli_output: Option<cli::OutputFormat>,
    types: &str,
    cfg: &config::ConfigFile,
) -> Result<()> {
    let token_store = config::resolve_token_store(cfg);
    let token = config::resolve_token(&token_store, profile)?;
    let format = resolve_output_format(cli_output, cfg, profile);

    run_search_with_token(&token, query, format, types)
}

fn run_search_with_token(
    token: &str,
    query: &str,
    format: cli::OutputFormat,
    types: &str,
) -> Result<()> {
    let channels = slack::search_channels(token, query, types)?;

    if channels.is_empty() {
        eprintln!("no channels matching '{query}'");
        std::process::exit(1);
    }

    match format {
        cli::OutputFormat::Table => print_table(&channels),
        cli::OutputFormat::Tsv => print_tsv(&channels),
        cli::OutputFormat::Json => print_json(&channels)?,
    }

    Ok(())
}

fn resolve_output_format(
    cli_output: Option<cli::OutputFormat>,
    cfg: &config::ConfigFile,
    profile: Option<&str>,
) -> cli::OutputFormat {
    // 1. CLI flag
    if let Some(f) = cli_output {
        return f;
    }

    // 2. env var / 3. config
    if let Some(s) = config::resolve_output(cfg, profile) {
        match s.to_lowercase().as_str() {
            "table" => return cli::OutputFormat::Table,
            "tsv" => return cli::OutputFormat::Tsv,
            "json" => return cli::OutputFormat::Json,
            _ => {}
        }
    }

    // 4. auto-detect
    if std::io::stdout().is_terminal() {
        cli::OutputFormat::Table
    } else {
        cli::OutputFormat::Tsv
    }
}

fn print_table(channels: &[slack::ChannelInfo]) {
    let name_width = channels
        .iter()
        .map(|c| c.name.len())
        .max()
        .unwrap_or(4)
        .max(4);
    let type_width = channels
        .iter()
        .map(|c| c.channel_type.len())
        .max()
        .unwrap_or(4)
        .max(4);

    let has_user_id = channels.iter().any(|c| c.user_id.is_some());

    let header_name: &str = "NAME";
    let header_type: &str = "TYPE";
    let header_ch_id: &str = "CHANNEL_ID";
    let header_user_id: &str = "USER_ID";

    if has_user_id {
        println!(
            "{:<name_width$}  {:<type_width$}  {:<13}  {}",
            header_name, header_type, header_ch_id, header_user_id
        );
        for ch in channels {
            println!(
                "{:<name_width$}  {:<type_width$}  {:<13}  {}",
                ch.name,
                ch.channel_type,
                ch.channel_id,
                ch.user_id.as_deref().unwrap_or("")
            );
        }
    } else {
        println!(
            "{:<name_width$}  {:<type_width$}  {}",
            header_name, header_type, header_ch_id
        );
        for ch in channels {
            println!(
                "{:<name_width$}  {:<type_width$}  {}",
                ch.name, ch.channel_type, ch.channel_id
            );
        }
    }
}

fn print_tsv(channels: &[slack::ChannelInfo]) {
    for ch in channels {
        println!(
            "{}\t{}\t{}\t{}",
            ch.name,
            ch.channel_type,
            ch.channel_id,
            ch.user_id.as_deref().unwrap_or("")
        );
    }
}

fn print_json(channels: &[slack::ChannelInfo]) -> Result<()> {
    let json = serde_json::to_string_pretty(channels)
        .context("failed to serialize search results to JSON")?;
    println!("{json}");
    Ok(())
}

fn run_send(profile: Option<&str>, send: cli::SendArgs, cfg: &config::ConfigFile) -> Result<()> {
    let resolved = config::resolve(cfg, profile)?;
    run_send_with_resolved(send, &resolved)
}

fn run_send_with_resolved(send: cli::SendArgs, resolved: &config::ResolvedConfig) -> Result<()> {
    let text_needs_stdin = send.text.as_deref() == Some("");
    let file_needs_stdin = send.file.as_deref() == Some("");

    // No flags at all → treat as implicit -t (stdin text)
    let (text, file) = if send.text.is_none() && send.file.is_none() {
        let stdin = std::io::stdin();
        if stdin.is_terminal() {
            bail!("no input provided (use -t, -f, or pipe via stdin)");
        }
        let mut buf = String::new();
        stdin.lock().read_to_string(&mut buf)?;
        buf.truncate(buf.trim_end().len());
        (Some(buf), None)
    } else {
        // Both requesting stdin is ambiguous
        if text_needs_stdin && file_needs_stdin {
            bail!("both --text and --file require stdin; provide a value for at least one");
        }

        // Resolve file
        let file_data = match &send.file {
            Some(path) if path.is_empty() => {
                // stdin → binary
                let stdin = std::io::stdin();
                if stdin.is_terminal() {
                    bail!("--file requires stdin input but stdin is a terminal");
                }
                let mut buf = Vec::new();
                stdin.lock().read_to_end(&mut buf)?;
                Some((send.filename.clone(), buf))
            }
            Some(path) => {
                // file from path
                let p = std::path::Path::new(path);
                let data =
                    std::fs::read(p).with_context(|| format!("failed to read file: {path}"))?;
                let name = p
                    .file_name()
                    .context("invalid file path")?
                    .to_string_lossy()
                    .into_owned();
                Some((name, data))
            }
            None => None,
        };

        // Resolve text
        let text = match &send.text {
            Some(t) if t.is_empty() => {
                // stdin → text
                let stdin = std::io::stdin();
                if stdin.is_terminal() {
                    bail!("--text requires stdin input but stdin is a terminal");
                }
                let mut buf = String::new();
                stdin.lock().read_to_string(&mut buf)?;
                buf.truncate(buf.trim_end().len());
                Some(buf)
            }
            Some(t) => Some(t.clone()),
            None => None,
        };

        (text, file_data)
    };

    if resolved.confirm && !send.yes {
        let summary = if let Some((filename, _)) = &file {
            match text.as_deref() {
                Some(t) if !t.is_empty() => format!("file: {filename}\n> {t}"),
                _ => format!("file: {filename}"),
            }
        } else {
            let message = text.as_deref().unwrap_or("");
            format!("> {message}")
        };

        let stdin = std::io::stdin();
        if !stdin.is_terminal() {
            bail!("confirm is enabled but stdin is not a TTY (pass -y to skip confirmation)");
        }

        eprint!("Send to {}:\n{summary}\nSend? [y/N] ", resolved.channel);
        std::io::stderr().flush()?;

        let mut input = String::new();
        std::io::stdin().lock().read_line(&mut input)?;
        if !matches!(input.trim(), "y" | "Y") {
            bail!("aborted");
        }
    }

    if let Some((filename, data)) = &file {
        // max_file_size check
        if data.len() as u64 > resolved.max_file_size {
            bail!(
                "file size ({}) exceeds limit ({})",
                config::format_size(data.len() as u64),
                config::format_size(resolved.max_file_size),
            );
        }

        // For file upload, empty text means no comment
        let comment = match text.as_deref() {
            Some("") | None => None,
            Some(t) => Some(t),
        };

        slack::upload_file_bytes(&resolved.token, &resolved.channel, filename, data, comment)?;
    } else {
        // Text-only mode
        let message = text.unwrap_or_default();
        if message.is_empty() {
            bail!("message is empty");
        }
        slack::post_message(&resolved.token, &resolved.channel, &message)?;
    }

    Ok(())
}
