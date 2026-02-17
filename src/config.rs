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
}

#[derive(Deserialize)]
pub struct Profile {
    pub token: Option<String>,
    pub channel: Option<String>,
}

pub struct ResolvedConfig {
    pub token: String,
    pub channel: String,
}

pub fn config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home.join(".config").join("slafling").join("config.toml"))
}

pub fn load_config() -> Result<ConfigFile> {
    let path = config_path()?;
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))
}

pub fn resolve(
    config: &ConfigFile,
    profile_name: Option<&str>,
    cli_channel: Option<&str>,
) -> Result<ResolvedConfig> {
    let mut token = config.default.token.clone();
    let mut channel = config.default.channel.clone();

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

    Ok(ResolvedConfig { token, channel })
}
