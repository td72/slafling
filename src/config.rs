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
    pub channel: Option<String>,
    pub max_file_size: Option<String>,
    pub confirm: Option<bool>,
    pub output: Option<String>,
    pub search_types: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct Profile {
    pub token: Option<String>,
    pub channel: Option<String>,
    pub max_file_size: Option<String>,
    pub confirm: Option<bool>,
    pub output: Option<String>,
    pub search_types: Option<Vec<String>>,
}

pub struct ResolvedConfig {
    pub token: String,
    pub channel: String,
    pub max_file_size: u64,
    pub confirm: bool,
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

pub fn generate_init_config(token: &str) -> String {
    format!(
        "\
[default]
token = \"{token}\"
# channel = \"#general\"
"
    )
}

pub fn write_init_config(path: &std::path::Path, token: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    let content = generate_init_config(token);
    std::fs::write(path, &content)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home.join(".config").join("slafling").join("config.toml"))
}

pub fn load_config() -> Result<ConfigFile> {
    let path = config_path()?;
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let config: ConfigFile =
        toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))?;
    validate_config(&config)?;
    Ok(config)
}

const VALID_OUTPUT_VALUES: &[&str] = &["table", "tsv", "json"];
const VALID_SEARCH_TYPES: &[&str] = &["public_channel", "private_channel", "im", "mpim"];

fn validate_config(config: &ConfigFile) -> Result<()> {
    validate_section_values(
        "default",
        config.default.output.as_deref(),
        config.default.search_types.as_deref(),
    )?;

    for (name, profile) in &config.profiles {
        validate_section_values(
            &format!("profiles.{name}"),
            profile.output.as_deref(),
            profile.search_types.as_deref(),
        )?;
    }

    Ok(())
}

fn validate_section_values(
    section: &str,
    output: Option<&str>,
    search_types: Option<&[String]>,
) -> Result<()> {
    if let Some(val) = output {
        let lower = val.to_lowercase();
        if !VALID_OUTPUT_VALUES.contains(&lower.as_str()) {
            bail!(
                "invalid output '{}' in [{}] (valid: {})",
                val,
                section,
                VALID_OUTPUT_VALUES.join(", ")
            );
        }
    }

    if let Some(types) = search_types {
        for val in types {
            let lower = val.to_lowercase();
            if !VALID_SEARCH_TYPES.contains(&lower.as_str()) {
                bail!(
                    "invalid search_types '{}' in [{}] (valid: {})",
                    val,
                    section,
                    VALID_SEARCH_TYPES.join(", ")
                );
            }
        }
    }

    Ok(())
}

pub fn resolve(config: &ConfigFile, profile_name: Option<&str>) -> Result<ResolvedConfig> {
    let mut token = config.default.token.clone();
    let mut channel = config.default.channel.clone();
    let mut max_file_size_str = config.default.max_file_size.clone();
    let mut confirm = config.default.confirm.unwrap_or(false);

    if let Some(name) = profile_name {
        let profile = config
            .profiles
            .get(name)
            .with_context(|| format!("profile '{}' not found in config", name))?;
        if let Some(t) = &profile.token {
            token = t.clone();
        }
        if let Some(c) = &profile.channel {
            channel = Some(c.clone());
        }
        if profile.max_file_size.is_some() {
            max_file_size_str = profile.max_file_size.clone();
        }
        if let Some(c) = profile.confirm {
            confirm = c;
        }
    }

    if token.is_empty() {
        bail!("token is not configured");
    }
    let channel = match channel {
        Some(c) if !c.is_empty() => c,
        _ => bail!("channel is not configured"),
    };

    let max_file_size = match max_file_size_str {
        Some(s) => parse_file_size(&s)?,
        None => DEFAULT_MAX_FILE_SIZE,
    };

    Ok(ResolvedConfig {
        token,
        channel,
        max_file_size,
        confirm,
    })
}

pub fn resolve_token(config: &ConfigFile, profile_name: Option<&str>) -> Result<String> {
    let mut token = config.default.token.clone();

    if let Some(name) = profile_name {
        let profile = config
            .profiles
            .get(name)
            .with_context(|| format!("profile '{}' not found in config", name))?;
        if let Some(t) = &profile.token {
            token = t.clone();
        }
    }

    if token.is_empty() {
        bail!("token is not configured");
    }

    Ok(token)
}

pub fn resolve_search_types(config: &ConfigFile, profile_name: Option<&str>) -> Option<String> {
    let mut search_types = config.default.search_types.clone();

    if let Some(name) = profile_name {
        if let Some(profile) = config.profiles.get(name) {
            if profile.search_types.is_some() {
                search_types = profile.search_types.clone();
            }
        }
    }

    search_types.map(|v| v.join(","))
}

pub fn resolve_output(config: &ConfigFile, profile_name: Option<&str>) -> Option<String> {
    if let Ok(val) = std::env::var("SLAFLING_OUTPUT") {
        return Some(val);
    }

    let mut output = config.default.output.clone();

    if let Some(name) = profile_name {
        if let Some(profile) = config.profiles.get(name) {
            if profile.output.is_some() {
                output = profile.output.clone();
            }
        }
    }

    output
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

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_config() -> ConfigFile {
        ConfigFile {
            default: DefaultConfig {
                token: "xoxb-test".to_string(),
                channel: Some("#general".to_string()),
                max_file_size: None,
                confirm: None,
                output: None,
                search_types: None,
            },
            profiles: HashMap::new(),
        }
    }

    #[test]
    fn valid_output_values() {
        for val in &["table", "tsv", "json", "JSON", "Table"] {
            let mut cfg = minimal_config();
            cfg.default.output = Some(val.to_string());
            assert!(
                validate_config(&cfg).is_ok(),
                "expected '{val}' to be valid"
            );
        }
    }

    #[test]
    fn invalid_output_value() {
        let mut cfg = minimal_config();
        cfg.default.output = Some("yaml".to_string());
        let err = validate_config(&cfg).unwrap_err();
        assert!(err.to_string().contains("invalid output 'yaml'"));
    }

    #[test]
    fn valid_search_types() {
        let mut cfg = minimal_config();
        cfg.default.search_types = Some(vec![
            "public_channel".to_string(),
            "private_channel".to_string(),
            "im".to_string(),
            "mpim".to_string(),
        ]);
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn invalid_search_type() {
        let mut cfg = minimal_config();
        cfg.default.search_types = Some(vec!["public_channel".to_string(), "foo".to_string()]);
        let err = validate_config(&cfg).unwrap_err();
        assert!(err.to_string().contains("invalid search_types 'foo'"));
    }

    #[test]
    fn invalid_profile_output() {
        let mut cfg = minimal_config();
        cfg.profiles.insert(
            "work".to_string(),
            Profile {
                token: None,
                channel: None,
                max_file_size: None,
                confirm: None,
                output: Some("xml".to_string()),
                search_types: None,
            },
        );
        let err = validate_config(&cfg).unwrap_err();
        assert!(err.to_string().contains("profiles.work"));
    }

    #[test]
    fn none_values_are_valid() {
        let cfg = minimal_config();
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn init_generates_valid_toml() {
        let toml_str = generate_init_config("xoxb-test-token");
        let parsed: ConfigFile = toml::from_str(&toml_str).expect("generated TOML should parse");
        assert_eq!(parsed.default.token, "xoxb-test-token");
    }

    #[test]
    fn init_writes_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        write_init_config(&path, "xoxb-abc").unwrap();
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("xoxb-abc"));
    }

    #[test]
    fn init_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a").join("b").join("config.toml");
        write_init_config(&path, "xoxb-nested").unwrap();
        assert!(path.exists());
    }

    #[test]
    fn init_overwrites_existing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        write_init_config(&path, "xoxb-old").unwrap();
        write_init_config(&path, "xoxb-new").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("xoxb-new"));
        assert!(!content.contains("xoxb-old"));
    }

    #[test]
    fn resolve_without_channel_fails() {
        let cfg = ConfigFile {
            default: DefaultConfig {
                token: "xoxb-test".to_string(),
                channel: None,
                max_file_size: None,
                confirm: None,
                output: None,
                search_types: None,
            },
            profiles: HashMap::new(),
        };
        assert!(resolve(&cfg, None).is_err());
    }

    #[test]
    fn resolve_token_without_channel_ok() {
        let cfg = ConfigFile {
            default: DefaultConfig {
                token: "xoxb-test".to_string(),
                channel: None,
                max_file_size: None,
                confirm: None,
                output: None,
                search_types: None,
            },
            profiles: HashMap::new(),
        };
        let token = resolve_token(&cfg, None).unwrap();
        assert_eq!(token, "xoxb-test");
    }

    #[test]
    fn toml_without_channel_parses() {
        let toml_str = generate_init_config("xoxb-tok");
        let parsed: ConfigFile = toml::from_str(&toml_str).expect("should parse without channel");
        assert!(parsed.default.channel.is_none());
    }
}
