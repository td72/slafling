use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ConfigFile {
    pub default: DefaultConfig,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

#[derive(Deserialize)]
pub struct DefaultConfig {
    pub token: String,
    pub channel: String,
    pub max_file_size: Option<String>,
}

#[derive(Deserialize)]
pub struct Profile {
    pub token: Option<String>,
    pub channel: Option<String>,
    pub max_file_size: Option<String>,
}

pub struct ResolvedConfig {
    pub token: String,
    pub channel: String,
    pub max_file_size: u64,
}

const KB: u64 = 1_024;
const MB: u64 = 1_048_576;
const GB: u64 = 1_073_741_824;

const DEFAULT_MAX_FILE_SIZE: u64 = GB; // Slack limit

pub fn parse_file_size(s: &str) -> Result<u64> {
    let s = s.trim();
    let (num_part, unit) = match s.find(|c: char| c.is_ascii_alphabetic()) {
        Some(i) => (s[..i].trim(), s[i..].trim().to_ascii_uppercase()),
        None => (s, String::new()),
    };

    let num: f64 = num_part
        .parse()
        .with_context(|| format!("invalid number in file size: '{s}'"))?;

    let multiplier: u64 = match unit.as_str() {
        "" | "B" => 1,
        "KB" | "K" => KB,
        "MB" | "M" => MB,
        "GB" | "G" => GB,
        _ => bail!("unknown file size unit: '{unit}' (use KB, MB, or GB)"),
    };

    Ok((num * multiplier as f64) as u64)
}

pub fn config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home.join(".config").join("slafling").join("config.toml"))
}

pub fn load_config() -> Result<ConfigFile> {
    let path = config_path()?;
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))
}

pub fn resolve(
    config: &ConfigFile,
    profile_name: Option<&str>,
    cli_channel: Option<&str>,
) -> Result<ResolvedConfig> {
    let mut token = config.default.token.clone();
    let mut channel = config.default.channel.clone();
    let mut max_file_size_str = config.default.max_file_size.clone();

    if let Some(name) = profile_name {
        let profile = config
            .profiles
            .get(name)
            .with_context(|| format!("profile '{}' not found in config", name))?;
        if let Some(t) = &profile.token {
            token = t.clone();
        }
        if let Some(c) = &profile.channel {
            channel = c.clone();
        }
        if profile.max_file_size.is_some() {
            max_file_size_str = profile.max_file_size.clone();
        }
    }

    if let Some(c) = cli_channel {
        channel = c.to_string();
    }

    if token.is_empty() {
        bail!("token is not configured");
    }
    if channel.is_empty() {
        bail!("channel is not configured");
    }

    let max_file_size = match max_file_size_str {
        Some(s) => parse_file_size(&s)?,
        None => DEFAULT_MAX_FILE_SIZE,
    };

    Ok(ResolvedConfig {
        token,
        channel,
        max_file_size,
    })
}

pub fn format_size(bytes: u64) -> String {
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
