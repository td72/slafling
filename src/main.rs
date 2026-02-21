mod cli;
mod config;
mod slack;

use std::io::{BufRead, IsTerminal, Read, Write};

use anyhow::{bail, Context, Result};
use clap::Parser;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    // Handle init before loading config (config may not exist yet)
    if matches!(cli.command, Some(cli::Command::Init)) {
        return run_init();
    }

    let cfg = config::load_config()?;

    let profile = cli
        .profile
        .or_else(|| std::env::var("SLAFLING_PROFILE").ok());

    match cli.command {
        Some(cli::Command::Init) => unreachable!(),
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
    let mut token = String::new();
    stdin.lock().read_line(&mut token)?;
    let token = token.trim();
    if token.is_empty() {
        bail!("token is required");
    }

    config::write_init_config(&path, token)?;

    println!("created {}", path.display());
    Ok(())
}

fn run_search(
    profile: Option<&str>,
    query: &str,
    cli_output: Option<cli::OutputFormat>,
    types: &str,
    cfg: &config::ConfigFile,
) -> Result<()> {
    let token = config::resolve_token(cfg, profile)?;
    let format = resolve_output_format(cli_output, cfg, profile);

    let channels = slack::search_channels(&token, query, types)?;

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
