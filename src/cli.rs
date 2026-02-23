use anyhow::bail;
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;

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
        types: Option<Vec<ChannelType>>,
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

#[derive(Clone, Copy, Debug, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
#[value(rename_all = "snake_case")]
pub enum ChannelType {
    PublicChannel,
    PrivateChannel,
    Im,
    Mpim,
}

impl ChannelType {
    pub fn as_api_str(self) -> &'static str {
        match self {
            Self::PublicChannel => "public_channel",
            Self::PrivateChannel => "private_channel",
            Self::Im => "im",
            Self::Mpim => "mpim",
        }
    }
}

pub fn channel_types_to_api_string(types: &[ChannelType]) -> String {
    types
        .iter()
        .map(|t| t.as_api_str())
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Clone, Copy, Debug, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Table,
    Tsv,
    Json,
}

pub fn parse_channel_types_str(s: &str) -> anyhow::Result<Vec<ChannelType>> {
    s.split(',').map(|t| t.trim().parse()).collect()
}

impl std::str::FromStr for OutputFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.to_lowercase().as_str() {
            "table" => Ok(Self::Table),
            "tsv" => Ok(Self::Tsv),
            "json" => Ok(Self::Json),
            _ => bail!("invalid output '{}' (valid: table, tsv, json)", s),
        }
    }
}

impl std::str::FromStr for ChannelType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.to_lowercase().as_str() {
            "public_channel" => Ok(Self::PublicChannel),
            "private_channel" => Ok(Self::PrivateChannel),
            "im" => Ok(Self::Im),
            "mpim" => Ok(Self::Mpim),
            _ => bail!(
                "invalid search type '{}' (valid: public_channel, private_channel, im, mpim)",
                s
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_types_to_api_string_single() {
        assert_eq!(
            channel_types_to_api_string(&[ChannelType::PublicChannel]),
            "public_channel"
        );
    }

    #[test]
    fn search_types_to_api_string_multiple() {
        assert_eq!(
            channel_types_to_api_string(&[
                ChannelType::PublicChannel,
                ChannelType::PrivateChannel,
                ChannelType::Im,
                ChannelType::Mpim,
            ]),
            "public_channel,private_channel,im,mpim"
        );
    }

    #[test]
    fn search_types_to_api_string_empty() {
        assert_eq!(channel_types_to_api_string(&[]), "");
    }

    #[test]
    fn search_types_to_api_string_order_preserved() {
        assert_eq!(
            channel_types_to_api_string(&[ChannelType::Mpim, ChannelType::Im]),
            "mpim,im"
        );
    }
}
