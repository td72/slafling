mod cli;
mod config;
mod slack;

use std::io::{IsTerminal, Read};

use anyhow::{bail, Context, Result};
use clap::Parser;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    let cfg = config::load_config()?;
    let resolved = config::resolve(
        &cfg,
        cli.profile.as_deref(),
        cli.channel.as_deref(),
    )?;

    if let Some(ref path) = cli.file {
        // File upload mode
        let file_size = std::fs::metadata(path)
            .with_context(|| format!("failed to read file: {}", path.display()))?
            .len();
        if file_size > resolved.max_file_size {
            bail!(
                "file size ({}) exceeds limit ({})",
                format_size(file_size),
                format_size(resolved.max_file_size),
            );
        }

        let comment = match cli.message {
            Some(m) => Some(m),
            None => {
                let stdin = std::io::stdin();
                if stdin.is_terminal() {
                    None
                } else {
                    let mut buf = String::new();
                    stdin.lock().read_to_string(&mut buf)?;
                    buf.truncate(buf.trim_end().len());
                    if buf.is_empty() { None } else { Some(buf) }
                }
            }
        };
        slack::upload_file(
            &resolved.token,
            &resolved.channel,
            path,
            comment.as_deref(),
        )?;
    } else {
        // Message mode
        let message = match cli.message {
            Some(m) => m,
            None => {
                let stdin = std::io::stdin();
                if stdin.is_terminal() {
                    bail!("no message provided (pass as argument or pipe via stdin)");
                }
                let mut buf = String::new();
                stdin.lock().read_to_string(&mut buf)?;
                buf.truncate(buf.trim_end().len());
                buf
            }
        };

        if message.is_empty() {
            bail!("message is empty");
        }

        slack::post_message(&resolved.token, &resolved.channel, &message)?;
    }

    Ok(())
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1_024;
    const MB: u64 = 1_048_576;
    const GB: u64 = 1_073_741_824;

    if bytes >= GB {
        format!("{:.1}GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes}B")
    }
}
