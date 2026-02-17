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

    /// Profile name from config file
    #[arg(short, long)]
    pub profile: Option<String>,
}
