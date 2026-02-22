use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::{keychain, token};

#[derive(Deserialize)]
pub struct ConfigFile {
    pub default: DefaultConfig,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

#[derive(Deserialize)]
pub struct DefaultConfig {
    pub channel: Option<String>,
    pub max_file_size: Option<String>,
    pub confirm: Option<bool>,
    pub output: Option<String>,
    pub search_types: Option<Vec<String>>,
    pub token_store: Option<String>,
}

#[derive(Deserialize)]
pub struct Profile {
    pub channel: Option<String>,
    pub max_file_size: Option<String>,
    pub confirm: Option<bool>,
    pub output: Option<String>,
    pub search_types: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct ResolvedConfig {
    pub token: String,
    pub channel: String,
    pub max_file_size: u64,
    pub confirm: bool,
}

const KB: u64 = 1_024;
const MB: u64 = 1_048_576;
const GB: u64 = 1_073_741_824;

const DEFAULT_MAX_FILE_SIZE: u64 = 100 * MB; // Slack API max: 1GB

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

pub fn generate_init_config() -> String {
    include_str!("../config.template.toml").replace(
        "# token_store = \"keychain\"",
        &format!("# token_store = \"{}\"", default_token_store()),
    )
}

pub fn write_init_config(path: &std::path::Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    let content = generate_init_config();
    std::fs::write(path, &content)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home.join(".config").join("slafling").join("config.toml"))
}

/// Return the platform default for token_store: "keychain" on macOS, "file" elsewhere.
pub fn default_token_store() -> &'static str {
    if cfg!(target_os = "macos") {
        "keychain"
    } else {
        "file"
    }
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
const VALID_TOKEN_STORE_VALUES: &[&str] = &["keychain", "file"];

fn validate_config(config: &ConfigFile) -> Result<()> {
    validate_section_values(
        "default",
        config.default.output.as_deref(),
        config.default.search_types.as_deref(),
    )?;

    if let Some(val) = &config.default.token_store {
        let lower = val.to_lowercase();
        if !VALID_TOKEN_STORE_VALUES.contains(&lower.as_str()) {
            bail!(
                "invalid token_store '{}' in [default] (valid: {})",
                val,
                VALID_TOKEN_STORE_VALUES.join(", ")
            );
        }
        if lower == "keychain" && !cfg!(target_os = "macos") {
            bail!("token_store 'keychain' is only supported on macOS");
        }
    }

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

/// Resolve token from: 1) SLAFLING_TOKEN env  2) token_store backend
pub fn resolve_token(token_store: &str, profile_name: Option<&str>) -> Result<String> {
    // 1. Environment variable (highest priority — for CI/CD and temporary overrides)
    if let Ok(t) = std::env::var("SLAFLING_TOKEN") {
        if !t.is_empty() {
            return Ok(t);
        }
    }

    // 2. token_store backend
    match token_store {
        "keychain" => {
            if let Some(t) = keychain::get_token(profile_name)? {
                return Ok(t);
            }
        }
        "file" => {
            if let Some(t) = token::get_token(profile_name)? {
                return Ok(t);
            }
        }
        _ => bail!("invalid token_store '{token_store}'"),
    }

    bail!("token is not configured (use `slafling token set` or set SLAFLING_TOKEN)")
}

/// Describe where the token is currently resolved from
pub fn describe_token_source(
    token_store: &str,
    profile_name: Option<&str>,
) -> Result<(&'static str, String)> {
    // 1. Env var
    if let Ok(t) = std::env::var("SLAFLING_TOKEN") {
        if !t.is_empty() {
            return Ok(("env", "SLAFLING_TOKEN".to_string()));
        }
    }

    // 2. token_store backend
    match token_store {
        "keychain" => {
            if keychain::get_token(profile_name)?.is_some() {
                return Ok(("keychain", "macOS Keychain".to_string()));
            }
        }
        "file" => {
            let path = token::token_path(profile_name)?;
            if token::get_token(profile_name)?.is_some() {
                return Ok(("file", path.display().to_string()));
            }
        }
        _ => bail!("invalid token_store '{token_store}'"),
    }

    bail!("token is not configured (use `slafling token set` or set SLAFLING_TOKEN)")
}

pub fn resolve_token_store(config: &ConfigFile) -> String {
    config
        .default
        .token_store
        .as_deref()
        .unwrap_or(default_token_store())
        .to_lowercase()
}

pub fn resolve(config: &ConfigFile, profile_name: Option<&str>) -> Result<ResolvedConfig> {
    let token_store = resolve_token_store(config);
    let token = resolve_token(&token_store, profile_name)?;
    let mut channel = config.default.channel.clone();
    let mut max_file_size_str = config.default.max_file_size.clone();
    let mut confirm = config.default.confirm.unwrap_or(false);

    if let Some(name) = profile_name {
        let profile = config
            .profiles
            .get(name)
            .with_context(|| format!("profile '{}' not found in config", name))?;
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

fn is_truthy(s: &str) -> bool {
    matches!(s.to_lowercase().as_str(), "1" | "true" | "yes")
}

/// Check if headless mode is enabled via SLAFLING_HEADLESS env var.
pub fn is_headless_env() -> bool {
    std::env::var("SLAFLING_HEADLESS")
        .map(|v| is_truthy(&v))
        .unwrap_or(false)
}

/// Resolve all settings from environment variables (headless mode, for send).
pub fn resolve_from_env() -> Result<ResolvedConfig> {
    let token = resolve_token_from_env()?;

    let channel = std::env::var("SLAFLING_CHANNEL")
        .ok()
        .filter(|s| !s.is_empty())
        .context("in headless mode, SLAFLING_CHANNEL must be set")?;

    let max_file_size = match std::env::var("SLAFLING_MAX_FILE_SIZE")
        .ok()
        .filter(|s| !s.is_empty())
    {
        Some(s) => parse_file_size(&s)
            .with_context(|| format!("in headless mode, invalid SLAFLING_MAX_FILE_SIZE: '{s}'"))?,
        None => DEFAULT_MAX_FILE_SIZE,
    };

    let confirm = std::env::var("SLAFLING_CONFIRM")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|v| is_truthy(&v))
        .unwrap_or(false);

    Ok(ResolvedConfig {
        token,
        channel,
        max_file_size,
        confirm,
    })
}

/// Resolve token from SLAFLING_TOKEN env var (headless mode).
pub fn resolve_token_from_env() -> Result<String> {
    std::env::var("SLAFLING_TOKEN")
        .ok()
        .filter(|s| !s.is_empty())
        .context("in headless mode, SLAFLING_TOKEN must be set")
}

/// Resolve search_types from SLAFLING_SEARCH_TYPES env var (headless mode).
pub fn resolve_search_types_from_env() -> Option<String> {
    std::env::var("SLAFLING_SEARCH_TYPES")
        .ok()
        .filter(|s| !s.is_empty())
}

/// Validate a comma-separated search_types string.
pub fn validate_search_types_str(s: &str) -> Result<()> {
    for val in s.split(',') {
        let trimmed = val.trim();
        if !VALID_SEARCH_TYPES.contains(&trimmed) {
            bail!(
                "invalid search type '{}' (valid: {})",
                trimmed,
                VALID_SEARCH_TYPES.join(", ")
            );
        }
    }
    Ok(())
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
    fn valid_token_store_file() {
        for val in &["file", "FILE"] {
            let mut cfg = minimal_config();
            cfg.default.token_store = Some(val.to_string());
            assert!(
                validate_config(&cfg).is_ok(),
                "expected '{val}' to be valid"
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn valid_token_store_keychain() {
        for val in &["keychain", "Keychain"] {
            let mut cfg = minimal_config();
            cfg.default.token_store = Some(val.to_string());
            assert!(
                validate_config(&cfg).is_ok(),
                "expected '{val}' to be valid"
            );
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn keychain_rejected_on_non_macos() {
        let mut cfg = minimal_config();
        cfg.default.token_store = Some("keychain".to_string());
        let err = validate_config(&cfg).unwrap_err();
        assert!(err.to_string().contains("only supported on macOS"));
    }

    #[test]
    fn invalid_token_store_value() {
        let mut cfg = minimal_config();
        cfg.default.token_store = Some("redis".to_string());
        let err = validate_config(&cfg).unwrap_err();
        assert!(err.to_string().contains("invalid token_store 'redis'"));
    }

    #[test]
    fn default_token_store_returns_valid_value() {
        let val = default_token_store();
        assert!(
            VALID_TOKEN_STORE_VALUES.contains(&val),
            "default_token_store() returned '{val}' which is not valid"
        );
    }

    #[test]
    fn none_values_are_valid() {
        let cfg = minimal_config();
        assert!(validate_config(&cfg).is_ok());
    }

    #[test]
    fn init_generates_valid_toml() {
        let toml_str = generate_init_config();
        let parsed: ConfigFile = toml::from_str(&toml_str).expect("generated TOML should parse");
        assert!(parsed.default.channel.is_none());
    }

    #[test]
    fn init_config_template_has_token_store_needle() {
        let template = include_str!("../config.template.toml");
        assert!(
            template.contains("# token_store = \"keychain\""),
            "config.template.toml must contain the token_store needle for generate_init_config()"
        );
    }

    #[test]
    fn init_config_has_platform_default_token_store() {
        let content = generate_init_config();
        let expected = format!("# token_store = \"{}\"", default_token_store());
        assert!(
            content.contains(&expected),
            "generated config should contain '{expected}'"
        );
    }

    #[test]
    fn init_writes_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        write_init_config(&path).unwrap();
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("[default]"));
        assert!(!content.contains("token ="));
    }

    #[test]
    fn init_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a").join("b").join("config.toml");
        write_init_config(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn init_overwrites_existing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "old content").unwrap();
        write_init_config(&path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("[default]"));
        assert!(!content.contains("old content"));
    }

    #[test]
    fn init_config_has_no_token_field() {
        let content = generate_init_config();
        assert!(
            !content.contains("token ="),
            "config should not contain token field"
        );
    }

    #[test]
    fn toml_without_channel_parses() {
        let toml_str = generate_init_config();
        let parsed: ConfigFile = toml::from_str(&toml_str).expect("should parse without channel");
        assert!(parsed.default.channel.is_none());
    }

    #[test]
    fn resolve_token_invalid_store() {
        let err = resolve_token("redis", None).unwrap_err();
        assert!(err.to_string().contains("invalid token_store 'redis'"));
    }

    // Note: env var tests are not thread-safe; they may flake under parallel execution.
    #[test]
    fn resolve_token_env_takes_priority() {
        let prev = std::env::var("SLAFLING_TOKEN").ok();
        std::env::set_var("SLAFLING_TOKEN", "xoxb-env-test");
        let result = resolve_token("file", None);
        match prev {
            Some(v) => std::env::set_var("SLAFLING_TOKEN", v),
            None => std::env::remove_var("SLAFLING_TOKEN"),
        }
        assert_eq!(result.unwrap(), "xoxb-env-test");
    }

    #[test]
    fn resolve_token_empty_env_is_ignored() {
        let prev = std::env::var("SLAFLING_TOKEN").ok();
        std::env::set_var("SLAFLING_TOKEN", "");
        let result = resolve_token("file", Some("_nonexistent_test_profile_"));
        match prev {
            Some(v) => std::env::set_var("SLAFLING_TOKEN", v),
            None => std::env::remove_var("SLAFLING_TOKEN"),
        }
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("token is not configured"));
    }

    #[test]
    fn describe_token_source_env() {
        let prev = std::env::var("SLAFLING_TOKEN").ok();
        std::env::set_var("SLAFLING_TOKEN", "xoxb-env-test");
        let result = describe_token_source("file", None);
        match prev {
            Some(v) => std::env::set_var("SLAFLING_TOKEN", v),
            None => std::env::remove_var("SLAFLING_TOKEN"),
        }
        let (source, location) = result.unwrap();
        assert_eq!(source, "env");
        assert_eq!(location, "SLAFLING_TOKEN");
    }

    #[test]
    fn validate_search_types_str_valid() {
        assert!(validate_search_types_str("public_channel").is_ok());
        assert!(validate_search_types_str("public_channel,private_channel").is_ok());
        assert!(validate_search_types_str("public_channel,private_channel,im,mpim").is_ok());
    }

    #[test]
    fn validate_search_types_str_invalid() {
        let err = validate_search_types_str("public_channel,foo").unwrap_err();
        assert!(err.to_string().contains("invalid search type 'foo'"));
    }

    #[test]
    fn is_headless_env_values() {
        let prev = std::env::var("SLAFLING_HEADLESS").ok();

        for val in &["1", "true", "yes", "TRUE", "Yes"] {
            std::env::set_var("SLAFLING_HEADLESS", val);
            assert!(is_headless_env(), "expected '{val}' to enable headless");
        }

        for val in &["0", "false", "no", ""] {
            std::env::set_var("SLAFLING_HEADLESS", val);
            assert!(
                !is_headless_env(),
                "expected '{val}' to not enable headless"
            );
        }

        std::env::remove_var("SLAFLING_HEADLESS");
        assert!(!is_headless_env(), "expected unset to not enable headless");

        match prev {
            Some(v) => std::env::set_var("SLAFLING_HEADLESS", v),
            None => std::env::remove_var("SLAFLING_HEADLESS"),
        }
    }

    #[test]
    fn resolve_from_env_success() {
        let prev_token = std::env::var("SLAFLING_TOKEN").ok();
        let prev_channel = std::env::var("SLAFLING_CHANNEL").ok();
        let prev_max = std::env::var("SLAFLING_MAX_FILE_SIZE").ok();
        let prev_confirm = std::env::var("SLAFLING_CONFIRM").ok();

        std::env::set_var("SLAFLING_TOKEN", "xoxb-headless");
        std::env::set_var("SLAFLING_CHANNEL", "#test");
        std::env::set_var("SLAFLING_MAX_FILE_SIZE", "50MB");
        std::env::set_var("SLAFLING_CONFIRM", "true");

        let result = resolve_from_env();

        // Restore
        for (key, prev) in [
            ("SLAFLING_TOKEN", prev_token),
            ("SLAFLING_CHANNEL", prev_channel),
            ("SLAFLING_MAX_FILE_SIZE", prev_max),
            ("SLAFLING_CONFIRM", prev_confirm),
        ] {
            match prev {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }

        let cfg = result.unwrap();
        assert_eq!(cfg.token, "xoxb-headless");
        assert_eq!(cfg.channel, "#test");
        assert_eq!(cfg.max_file_size, 50 * MB);
        assert!(cfg.confirm);
    }

    #[test]
    fn resolve_from_env_missing_channel() {
        let prev_token = std::env::var("SLAFLING_TOKEN").ok();
        let prev_channel = std::env::var("SLAFLING_CHANNEL").ok();

        std::env::set_var("SLAFLING_TOKEN", "xoxb-test");
        std::env::remove_var("SLAFLING_CHANNEL");

        let result = resolve_from_env();

        match prev_token {
            Some(v) => std::env::set_var("SLAFLING_TOKEN", v),
            None => std::env::remove_var("SLAFLING_TOKEN"),
        }
        match prev_channel {
            Some(v) => std::env::set_var("SLAFLING_CHANNEL", v),
            None => std::env::remove_var("SLAFLING_CHANNEL"),
        }

        let err = result.unwrap_err();
        assert!(err.to_string().contains("SLAFLING_CHANNEL must be set"));
    }

    #[test]
    fn resolve_from_env_missing_token() {
        let prev = std::env::var("SLAFLING_TOKEN").ok();
        std::env::remove_var("SLAFLING_TOKEN");

        let result = resolve_from_env();

        match prev {
            Some(v) => std::env::set_var("SLAFLING_TOKEN", v),
            None => std::env::remove_var("SLAFLING_TOKEN"),
        }

        let err = result.unwrap_err();
        assert!(err.to_string().contains("SLAFLING_TOKEN must be set"));
    }

    #[test]
    fn resolve_from_env_defaults() {
        let prev_token = std::env::var("SLAFLING_TOKEN").ok();
        let prev_channel = std::env::var("SLAFLING_CHANNEL").ok();
        let prev_max = std::env::var("SLAFLING_MAX_FILE_SIZE").ok();
        let prev_confirm = std::env::var("SLAFLING_CONFIRM").ok();

        std::env::set_var("SLAFLING_TOKEN", "xoxb-test");
        std::env::set_var("SLAFLING_CHANNEL", "#general");
        std::env::remove_var("SLAFLING_MAX_FILE_SIZE");
        std::env::remove_var("SLAFLING_CONFIRM");

        let result = resolve_from_env();

        for (key, prev) in [
            ("SLAFLING_TOKEN", prev_token),
            ("SLAFLING_CHANNEL", prev_channel),
            ("SLAFLING_MAX_FILE_SIZE", prev_max),
            ("SLAFLING_CONFIRM", prev_confirm),
        ] {
            match prev {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }

        let cfg = result.unwrap();
        assert_eq!(cfg.max_file_size, DEFAULT_MAX_FILE_SIZE);
        assert!(!cfg.confirm);
    }
}
