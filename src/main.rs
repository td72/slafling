mod cli;
mod config;
mod slack;

use std::io::{IsTerminal, Read};

use anyhow::{bail, Context, Result};
use clap::Parser;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    let cfg = config::load_config()?;
    let resolved = config::resolve(&cfg, cli.profile.as_deref(), cli.channel.as_deref())?;

    let text_needs_stdin = cli.text.as_deref() == Some("");
    let file_needs_stdin = cli.file.as_deref() == Some("");

    // No flags at all → treat as implicit -t (stdin text)
    let (text, file) = if cli.text.is_none() && cli.file.is_none() {
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
        let file_data = match &cli.file {
            Some(path) if path.is_empty() => {
                // stdin → binary
                let stdin = std::io::stdin();
                if stdin.is_terminal() {
                    bail!("--file requires stdin input but stdin is a terminal");
                }
                let mut buf = Vec::new();
                stdin.lock().read_to_end(&mut buf)?;
                Some((cli.filename.clone(), buf))
            }
            Some(path) => {
                // file from path
                let p = std::path::Path::new(path);
                let data = std::fs::read(p)
                    .with_context(|| format!("failed to read file: {path}"))?;
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
        let text = match &cli.text {
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

        slack::upload_file_bytes(
            &resolved.token,
            &resolved.channel,
            filename,
            data,
            comment,
        )?;
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

