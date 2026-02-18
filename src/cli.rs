use std::path::PathBuf;

use clap::Parser;

/// Fling messages to Slack
#[derive(Parser)]
#[command(version)]
pub struct Cli {
    /// Message to send (reads from stdin if omitted)
    pub message: Option<String>,

    /// Target channel (overrides config)
    #[arg(short, long)]
    pub channel: Option<String>,

    /// File to upload
    #[arg(short, long, value_name = "PATH")]
    pub file: Option<PathBuf>,

    /// Profile name from config file
    #[arg(short, long)]
    pub profile: Option<String>,
}
