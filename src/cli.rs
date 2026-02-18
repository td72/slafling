use clap::{Parser, Subcommand};

/// Fling messages to Slack
#[derive(Parser)]
#[command(version, args_conflicts_with_subcommands = true)]
pub struct Cli {
    /// Profile name from config file
    #[arg(short, long, global = true)]
    pub profile: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub send: SendArgs,
}

#[derive(clap::Args)]
pub struct SendArgs {
    /// Text message (reads from stdin if value omitted)
    #[arg(short, long, num_args = 0..=1, default_missing_value = "")]
    pub text: Option<String>,

    /// File to upload (reads from stdin if path omitted)
    #[arg(short, long, num_args = 0..=1, default_missing_value = "")]
    pub file: Option<String>,

    /// Filename for stdin file upload
    #[arg(short = 'n', long, default_value = "stdin")]
    pub filename: String,

    /// Skip confirmation prompt
    #[arg(short = 'y', long)]
    pub yes: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Search for Slack channels by name
    Search {
        /// Channel name to search for (partial match)
        query: String,
    },
}
