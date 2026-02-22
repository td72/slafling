use clap::{Parser, Subcommand, ValueEnum};

/// Fling messages to Slack
#[derive(Parser)]
#[command(version, args_conflicts_with_subcommands = true)]
pub struct Cli {
    /// Profile name from config file
    #[arg(short, long, global = true)]
    pub profile: Option<String>,

    /// Run without config file (all settings from env vars)
    #[arg(long, global = true)]
    pub headless: bool,

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
    /// Initialize config file
    Init,

    /// Validate config file
    Validate,

    /// Search for Slack channels by name
    Search {
        /// Channel name to search for (partial match)
        query: String,

        /// Output format (auto-detected if omitted: table for TTY, tsv for pipe)
        #[arg(short, long)]
        output: Option<OutputFormat>,

        /// Channel types to search
        #[arg(long, value_delimiter = ',')]
        types: Option<Vec<SearchType>>,
    },

    /// Manage token storage
    Token {
        #[command(subcommand)]
        action: TokenAction,
    },
}

#[derive(Subcommand)]
pub enum TokenAction {
    /// Store token in Keychain (macOS) or token file
    Set,

    /// Remove stored token
    Delete,

    /// Show where token is resolved from
    Show,
}

#[derive(Clone, Copy, ValueEnum)]
#[value(rename_all = "snake_case")]
pub enum SearchType {
    PublicChannel,
    PrivateChannel,
    Im,
    Mpim,
}

impl SearchType {
    pub fn as_api_str(self) -> &'static str {
        match self {
            Self::PublicChannel => "public_channel",
            Self::PrivateChannel => "private_channel",
            Self::Im => "im",
            Self::Mpim => "mpim",
        }
    }
}

pub fn search_types_to_api_string(types: &[SearchType]) -> String {
    types
        .iter()
        .map(|t| t.as_api_str())
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Table,
    Tsv,
    Json,
}
