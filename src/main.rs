mod cli;
mod config;
mod slack;

use std::io::{BufRead, IsTerminal, Read, Write};

use anyhow::{bail, Context, Result};
use clap::Parser;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    match cli.command {
        Some(cli::Command::Search { query }) => run_search(cli.profile.as_deref(), &query),
        None => run_send(cli.profile.as_deref(), cli.send),
    }
}

fn run_search(profile: Option<&str>, query: &str) -> Result<()> {
    let cfg = config::load_config()?;
    let token = config::resolve_token(&cfg, profile)?;

    let channels = slack::search_channels(&token, query)?;

    if channels.is_empty() {
        eprintln!("no channels matching '{query}'");
        std::process::exit(1);
    }

    for (name, id) in &channels {
        println!("{name}\t{id}");
    }

    Ok(())
}

fn run_send(profile: Option<&str>, send: cli::SendArgs) -> Result<()> {
    let cfg = config::load_config()?;
    let resolved = config::resolve(&cfg, profile)?;

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
