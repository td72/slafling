mod cli;
mod config;
mod slack;

use std::io::{IsTerminal, Read};

use anyhow::{bail, Result};
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
