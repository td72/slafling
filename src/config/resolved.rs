use anyhow::{anyhow, bail, Context, Result};

use crate::{cli, keychain, token};

use super::env::Env;
use super::file::{resolve_token_store, ConfigFile, TokenStore};
use super::util::{is_truthy, parse_file_size, DEFAULT_MAX_FILE_SIZE};

#[derive(Debug)]
pub struct ResolvedConfig {
    pub token: String,
    pub channel: String,
    pub max_file_size: u64,
    pub confirm: bool,
}

#[derive(Debug)]
pub struct Config {
    pub headless: bool,
    pub profile: Option<String>,
    pub token_store: TokenStore, // placeholder (File) in headless
    token_env: Option<String>,   // headless only (private)
    pub channel: Option<String>,
    pub max_file_size: Option<String>,
    pub confirm: bool,
    pub output: Option<cli::OutputFormat>,
    pub search_types: Option<Vec<cli::ChannelType>>,
}

impl Config {
    pub fn new(file: Option<&ConfigFile>, profile: Option<&str>, env: &Env) -> Result<Self> {
        match file {
            Some(f) => Self::from_file(f, profile, env),
            None => Self::from_env(env),
        }
    }

    fn from_file(file: &ConfigFile, profile: Option<&str>, env: &Env) -> Result<Self> {
        if let Some(name) = profile {
            if !file.profiles.contains_key(name) {
                bail!("profile '{}' not found in config", name);
            }
        }

        let token_store = resolve_token_store(file);
        let mut channel = file.default.channel.clone();
        let mut max_file_size = file.default.max_file_size.clone();
        let mut confirm = file.default.confirm.unwrap_or(false);
        let mut output: Option<cli::OutputFormat> = file
            .default
            .output
            .as_deref()
            .map(|s| {
                s.parse()
                    .with_context(|| format!("invalid output in [default]: '{s}'"))
            })
            .transpose()?;
        let mut search_types: Option<Vec<cli::ChannelType>> = file
            .default
            .search_types
            .as_deref()
            .map(|v| {
                v.iter()
                    .map(|s| {
                        s.parse()
                            .with_context(|| format!("invalid search_types in [default]: '{s}'"))
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;

        if let Some(name) = profile {
            let p = &file.profiles[name];
            if let Some(c) = &p.channel {
                channel = Some(c.clone());
            }
            if p.max_file_size.is_some() {
                max_file_size = p.max_file_size.clone();
            }
            if let Some(c) = p.confirm {
                confirm = c;
            }
            if let Some(ref v) = p.output {
                output = Some(
                    v.parse()
                        .with_context(|| format!("invalid output in [{name}]: '{v}'"))?,
                );
            }
            if let Some(ref v) = p.search_types {
                search_types = Some(
                    v.iter()
                        .map(|s| {
                            s.parse()
                                .with_context(|| format!("invalid search_types in [{name}]: '{s}'"))
                        })
                        .collect::<Result<Vec<_>>>()?,
                );
            }
        }

        // Env overrides (highest priority)
        if let Some(ref val) = env.max_file_size {
            max_file_size = Some(val.clone());
        }
        if let Some(ref val) = env.confirm {
            confirm = is_truthy(val);
        }
        if let Some(ref val) = env.output {
            output = Some(val.parse().map_err(|e| anyhow!("SLAFLING_OUTPUT: {}", e))?);
        }
        if let Some(ref val) = env.search_types {
            search_types = Some(
                cli::parse_channel_types_str(val)
                    .map_err(|e| anyhow!("SLAFLING_SEARCH_TYPES: {}", e))?,
            );
        }

        Ok(Self {
            headless: false,
            profile: profile.map(|s| s.to_string()),
            token_store,
            token_env: None,
            channel,
            max_file_size,
            confirm,
            output,
            search_types,
        })
    }

    fn from_env(env: &Env) -> Result<Self> {
        let output = match env.output.as_deref() {
            Some(s) => Some(
                s.parse::<cli::OutputFormat>()
                    .map_err(|e| anyhow!("SLAFLING_OUTPUT: {}", e))?,
            ),
            None => None,
        };
        let search_types = match env.search_types.as_deref() {
            Some(s) => Some(
                cli::parse_channel_types_str(s)
                    .map_err(|e| anyhow!("SLAFLING_SEARCH_TYPES: {}", e))?,
            ),
            None => None,
        };

        Ok(Self {
            headless: true,
            profile: None,
            token_store: TokenStore::File, // placeholder, unused in headless
            token_env: env.token.clone(),
            channel: env.channel.clone(),
            max_file_size: env.max_file_size.clone(),
            confirm: env.confirm.as_deref().map(is_truthy).unwrap_or(false),
            output,
            search_types,
        })
    }

    pub fn resolve_token(&self) -> Result<String> {
        if self.headless {
            self.token_env
                .clone()
                .context("in headless mode, SLAFLING_TOKEN must be set")
        } else {
            resolve_token(self.token_store, self.profile.as_deref())
        }
    }

    pub fn resolve_send(&self) -> Result<ResolvedConfig> {
        let token = self.resolve_token()?;

        let channel = match &self.channel {
            Some(c) if !c.is_empty() => c.clone(),
            _ => {
                if self.headless {
                    bail!("in headless mode, SLAFLING_CHANNEL must be set");
                } else {
                    bail!("channel is not configured");
                }
            }
        };

        let max_file_size = match &self.max_file_size {
            Some(s) => {
                if self.headless {
                    parse_file_size(s).with_context(|| {
                        format!("in headless mode, invalid SLAFLING_MAX_FILE_SIZE: '{s}'")
                    })?
                } else {
                    parse_file_size(s)?
                }
            }
            None => DEFAULT_MAX_FILE_SIZE,
        };

        Ok(ResolvedConfig {
            token,
            channel,
            max_file_size,
            confirm: self.confirm,
        })
    }
}

/// Resolve token from token_store backend (keychain or file).
pub fn resolve_token(token_store: TokenStore, profile_name: Option<&str>) -> Result<String> {
    match token_store {
        TokenStore::Keychain => {
            if let Some(t) = keychain::get_token(profile_name)? {
                return Ok(t);
            }
        }
        TokenStore::File => {
            if let Some(t) = token::get_token(profile_name)? {
                return Ok(t);
            }
        }
    }

    bail!("token is not configured (use `slafling token set`)")
}

/// Describe where the token is currently resolved from.
pub fn describe_token_source(
    token_store: TokenStore,
    profile_name: Option<&str>,
) -> Result<(&'static str, String)> {
    match token_store {
        TokenStore::Keychain => {
            if keychain::get_token(profile_name)?.is_some() {
                return Ok(("keychain", "macOS Keychain".to_string()));
            }
        }
        TokenStore::File => {
            let path = token::token_path(profile_name)?;
            if token::get_token(profile_name)?.is_some() {
                return Ok(("file", path.display().to_string()));
            }
        }
    }

    bail!("token is not configured (use `slafling token set`)")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::super::env::Env;
    use super::super::file::{ConfigFile, DefaultConfig, Profile};
    use super::super::util::{DEFAULT_MAX_FILE_SIZE, MB};
    use super::*;
    use crate::cli::{ChannelType, OutputFormat};

    fn minimal_config() -> ConfigFile {
        ConfigFile {
            default: DefaultConfig {
                channel: Some("#general".to_string()),
                max_file_size: None,
                confirm: None,
                output: None,
                search_types: None,
                token_store: None,
            },
            profiles: HashMap::new(),
        }
    }

    fn no_env() -> Env {
        Env::default()
    }

    // --- Config::new headless tests ---

    #[test]
    fn config_new_headless_success() {
        let env = Env {
            token: Some("xoxb-headless".to_string()),
            channel: Some("#test".to_string()),
            max_file_size: Some("50MB".to_string()),
            confirm: Some("true".to_string()),
            ..Env::default()
        };
        let config = Config::new(None, None, &env).unwrap();
        let resolved = config.resolve_send().unwrap();
        assert_eq!(resolved.token, "xoxb-headless");
        assert_eq!(resolved.channel, "#test");
        assert_eq!(resolved.max_file_size, 50 * MB);
        assert!(resolved.confirm);
    }

    #[test]
    fn config_new_headless_missing_token() {
        let env = Env::default();
        let config = Config::new(None, None, &env).unwrap();
        let err = config.resolve_send().unwrap_err();
        assert!(err.to_string().contains("SLAFLING_TOKEN must be set"));
    }

    #[test]
    fn config_new_headless_missing_channel() {
        let env = Env {
            token: Some("xoxb-test".to_string()),
            ..Env::default()
        };
        let config = Config::new(None, None, &env).unwrap();
        let err = config.resolve_send().unwrap_err();
        assert!(err.to_string().contains("SLAFLING_CHANNEL must be set"));
    }

    #[test]
    fn config_new_headless_defaults() {
        let env = Env {
            token: Some("xoxb-test".to_string()),
            channel: Some("#general".to_string()),
            ..Env::default()
        };
        let config = Config::new(None, None, &env).unwrap();
        let resolved = config.resolve_send().unwrap();
        assert_eq!(resolved.max_file_size, DEFAULT_MAX_FILE_SIZE);
        assert!(!resolved.confirm);
    }

    // --- Config::new search_types tests ---

    #[test]
    fn config_new_search_types_from_default() {
        let mut cfg = minimal_config();
        cfg.default.search_types = Some(vec!["public_channel".to_string(), "im".to_string()]);
        let config = Config::new(Some(&cfg), None, &no_env()).unwrap();
        assert_eq!(
            config.search_types.unwrap(),
            vec![ChannelType::PublicChannel, ChannelType::Im]
        );
    }

    #[test]
    fn config_new_search_types_profile_overrides_default() {
        let mut cfg = minimal_config();
        cfg.default.search_types = Some(vec!["public_channel".to_string()]);
        cfg.profiles.insert(
            "work".to_string(),
            Profile {
                channel: None,
                max_file_size: None,
                confirm: None,
                output: None,
                search_types: Some(vec!["private_channel".to_string()]),
            },
        );
        let config = Config::new(Some(&cfg), Some("work"), &no_env()).unwrap();
        assert_eq!(
            config.search_types.unwrap(),
            vec![ChannelType::PrivateChannel]
        );
    }

    #[test]
    fn config_new_search_types_env_var_overrides() {
        let cfg = minimal_config();
        let env = Env {
            search_types: Some("im,mpim".to_string()),
            ..Env::default()
        };
        let config = Config::new(Some(&cfg), None, &env).unwrap();
        assert_eq!(
            config.search_types.unwrap(),
            vec![ChannelType::Im, ChannelType::Mpim]
        );
    }

    #[test]
    fn config_new_search_types_env_var_empty_falls_back() {
        let mut cfg = minimal_config();
        cfg.default.search_types = Some(vec!["public_channel".to_string()]);
        let config = Config::new(Some(&cfg), None, &no_env()).unwrap();
        assert_eq!(
            config.search_types.unwrap(),
            vec![ChannelType::PublicChannel]
        );
    }

    #[test]
    fn config_new_search_types_none_when_unset() {
        let cfg = minimal_config();
        let config = Config::new(Some(&cfg), None, &no_env()).unwrap();
        assert!(config.search_types.is_none());
    }

    // --- Config::new output tests ---

    #[test]
    fn config_new_output_env_var() {
        let cfg = minimal_config();
        let env = Env {
            output: Some("json".to_string()),
            ..Env::default()
        };
        let config = Config::new(Some(&cfg), None, &env).unwrap();
        assert_eq!(config.output.unwrap(), OutputFormat::Json);
    }

    #[test]
    fn config_new_output_env_var_empty_falls_back() {
        let mut cfg = minimal_config();
        cfg.default.output = Some("tsv".to_string());
        let config = Config::new(Some(&cfg), None, &no_env()).unwrap();
        assert_eq!(config.output.unwrap(), OutputFormat::Tsv);
    }

    #[test]
    fn config_new_output_from_default() {
        let mut cfg = minimal_config();
        cfg.default.output = Some("table".to_string());
        let config = Config::new(Some(&cfg), None, &no_env()).unwrap();
        assert_eq!(config.output.unwrap(), OutputFormat::Table);
    }

    #[test]
    fn config_new_output_profile_overrides_default() {
        let mut cfg = minimal_config();
        cfg.default.output = Some("table".to_string());
        cfg.profiles.insert(
            "work".to_string(),
            Profile {
                channel: None,
                max_file_size: None,
                confirm: None,
                output: Some("json".to_string()),
                search_types: None,
            },
        );
        let config = Config::new(Some(&cfg), Some("work"), &no_env()).unwrap();
        assert_eq!(config.output.unwrap(), OutputFormat::Json);
    }

    #[test]
    fn config_new_output_none_when_unset() {
        let cfg = minimal_config();
        let config = Config::new(Some(&cfg), None, &no_env()).unwrap();
        assert!(config.output.is_none());
    }

    // --- Additional Config::new tests ---

    #[test]
    fn config_new_profile_not_found() {
        let cfg = minimal_config();
        let err = Config::new(Some(&cfg), Some("nonexistent"), &no_env()).unwrap_err();
        assert!(err.to_string().contains("profile 'nonexistent' not found"));
    }

    #[test]
    fn config_new_max_file_size_env_overrides() {
        let mut cfg = minimal_config();
        cfg.default.max_file_size = Some("10MB".to_string());
        let env = Env {
            max_file_size: Some("20MB".to_string()),
            ..Env::default()
        };
        let config = Config::new(Some(&cfg), None, &env).unwrap();
        assert_eq!(config.max_file_size.unwrap(), "20MB");
    }

    #[test]
    fn config_new_confirm_env_overrides() {
        let mut cfg = minimal_config();
        cfg.default.confirm = Some(false);
        let env = Env {
            confirm: Some("true".to_string()),
            ..Env::default()
        };
        let config = Config::new(Some(&cfg), None, &env).unwrap();
        assert!(config.confirm);
    }
}
