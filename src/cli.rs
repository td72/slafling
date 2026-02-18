use clap::Parser;

/// Fling messages to Slack
#[derive(Parser)]
#[command(version)]
pub struct Cli {
    /// Text message (reads from stdin if value omitted)
    #[arg(short, long, num_args = 0..=1, default_missing_value = "")]
    pub text: Option<String>,

    /// File to upload (reads from stdin if path omitted)
    #[arg(short, long, num_args = 0..=1, default_missing_value = "")]
    pub file: Option<String>,

    /// Filename for stdin file upload
    #[arg(short = 'n', long, default_value = "stdin")]
    pub filename: String,

    /// Target channel (overrides config)
    #[arg(short, long)]
    pub channel: Option<String>,

    /// Profile name from config file
    #[arg(short, long)]
    pub profile: Option<String>,
}
