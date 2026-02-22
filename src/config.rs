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

#[derive(Debug)]
pub struct Config {
    pub headless: bool,
    pub profile: Option<String>,
    pub token_store: String,   // normal mode (empty in headless)
    token_env: Option<String>, // headless only (private)
    pub channel: Option<String>,
    pub max_file_size: Option<String>,
    pub confirm: bool,
    pub output: Option<String>,
    pub search_types: Option<String>,
}

impl Config {
    pub fn new(file: Option<&ConfigFile>, profile: Option<&str>, env: &Env) -> Result<Self> {
        match file {
            Some(f) => Self::from_file(f, profile, env),
            None => Ok(Self::from_env(env)),
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
        let mut output = file.default.output.clone();
        let mut search_types: Option<Vec<String>> = file.default.search_types.clone();

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
            if p.output.is_some() {
                output = p.output.clone();
            }
            if p.search_types.is_some() {
                search_types = p.search_types.clone();
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
            output = Some(val.clone());
        }
        let search_types_str = if let Some(ref val) = env.search_types {
            Some(val.clone())
        } else {
            search_types.map(|v| v.join(","))
        };

        Ok(Self {
            headless: false,
            profile: profile.map(|s| s.to_string()),
            token_store,
            token_env: None,
            channel,
            max_file_size,
            confirm,
            output,
            search_types: search_types_str,
        })
    }

    fn from_env(env: &Env) -> Self {
        Self {
            headless: true,
            profile: None,
            token_store: String::new(),
            token_env: env.token.clone(),
            channel: env.channel.clone(),
            max_file_size: env.max_file_size.clone(),
            confirm: env.confirm.as_deref().map(is_truthy).unwrap_or(false),
            output: env.output.clone(),
            search_types: env.search_types.clone(),
        }
    }

    pub fn resolve_token(&self) -> Result<String> {
        if self.headless {
            self.token_env
                .clone()
                .context("in headless mode, SLAFLING_TOKEN must be set")
        } else {
            resolve_token(&self.token_store, self.profile.as_deref())
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

/// All environment variables read at startup, in one place.
#[derive(Debug, Default)]
pub struct Env {
    pub headless: bool,
    pub profile: Option<String>,       // normal mode only
    pub token: Option<String>,         // headless only
    pub channel: Option<String>,       // headless only
    pub output: Option<String>,        // both modes
    pub max_file_size: Option<String>, // both modes
    pub confirm: Option<String>,       // both modes
    pub search_types: Option<String>,  // both modes
}

impl Env {
    pub fn load() -> Self {
        fn opt(key: &str) -> Option<String> {
            std::env::var(key).ok().filter(|s| !s.is_empty())
        }
        Self {
            headless: opt("SLAFLING_HEADLESS")
                .map(|v| is_truthy(&v))
                .unwrap_or(false),
            profile: opt("SLAFLING_PROFILE"),
            token: opt("SLAFLING_TOKEN"),
            channel: opt("SLAFLING_CHANNEL"),
            output: opt("SLAFLING_OUTPUT"),
            max_file_size: opt("SLAFLING_MAX_FILE_SIZE"),
            confirm: opt("SLAFLING_CONFIRM"),
            search_types: opt("SLAFLING_SEARCH_TYPES"),
        }
    }
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

/// Resolve token from token_store backend (keychain or file).
pub fn resolve_token(token_store: &str, profile_name: Option<&str>) -> Result<String> {
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

    bail!("token is not configured (use `slafling token set`)")
}

/// Describe where the token is currently resolved from
pub fn describe_token_source(
    token_store: &str,
    profile_name: Option<&str>,
) -> Result<(&'static str, String)> {
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

    bail!("token is not configured (use `slafling token set`)")
}

pub fn resolve_token_store(config: &ConfigFile) -> String {
    config
        .default
        .token_store
        .as_deref()
        .unwrap_or(default_token_store())
        .to_lowercase()
}

fn is_truthy(s: &str) -> bool {
    matches!(s.to_lowercase().as_str(), "1" | "true" | "yes")
}

/// Validate an output format string (from env var).
pub fn validate_output_str(s: &str) -> Result<()> {
    let lower = s.to_lowercase();
    if !VALID_OUTPUT_VALUES.contains(&lower.as_str()) {
        bail!(
            "invalid output '{}' (valid: {})",
            s,
            VALID_OUTPUT_VALUES.join(", ")
        );
    }
    Ok(())
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
    use serial_test::serial;

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

    // --- Env struct tests ---

    #[test]
    fn env_default_is_all_none() {
        let env = Env::default();
        assert!(!env.headless);
        assert!(env.profile.is_none());
        assert!(env.token.is_none());
        assert!(env.channel.is_none());
        assert!(env.output.is_none());
        assert!(env.max_file_size.is_none());
        assert!(env.confirm.is_none());
        assert!(env.search_types.is_none());
    }

    #[test]
    #[serial]
    fn env_load_reads_all_vars() {
        let keys = [
            ("SLAFLING_HEADLESS", "1"),
            ("SLAFLING_PROFILE", "work"),
            ("SLAFLING_TOKEN", "xoxb-test"),
            ("SLAFLING_CHANNEL", "#general"),
            ("SLAFLING_OUTPUT", "json"),
            ("SLAFLING_MAX_FILE_SIZE", "50MB"),
            ("SLAFLING_CONFIRM", "true"),
            ("SLAFLING_SEARCH_TYPES", "im,mpim"),
        ];
        let prev: Vec<_> = keys
            .iter()
            .map(|(k, _)| (*k, std::env::var(k).ok()))
            .collect();
        for (k, v) in &keys {
            std::env::set_var(k, v);
        }

        let env = Env::load();

        for (k, p) in prev {
            match p {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }

        assert!(env.headless);
        assert_eq!(env.profile.as_deref(), Some("work"));
        assert_eq!(env.token.as_deref(), Some("xoxb-test"));
        assert_eq!(env.channel.as_deref(), Some("#general"));
        assert_eq!(env.output.as_deref(), Some("json"));
        assert_eq!(env.max_file_size.as_deref(), Some("50MB"));
        assert_eq!(env.confirm.as_deref(), Some("true"));
        assert_eq!(env.search_types.as_deref(), Some("im,mpim"));
    }

    #[test]
    #[serial]
    fn env_load_filters_empty_strings() {
        let keys = [
            "SLAFLING_TOKEN",
            "SLAFLING_CHANNEL",
            "SLAFLING_OUTPUT",
            "SLAFLING_MAX_FILE_SIZE",
            "SLAFLING_CONFIRM",
            "SLAFLING_SEARCH_TYPES",
            "SLAFLING_PROFILE",
        ];
        let prev: Vec<_> = keys.iter().map(|k| (*k, std::env::var(k).ok())).collect();
        for k in &keys {
            std::env::set_var(k, "");
        }
        std::env::set_var("SLAFLING_HEADLESS", "0");

        let env = Env::load();

        for (k, p) in prev {
            match p {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }
        std::env::remove_var("SLAFLING_HEADLESS");

        assert!(!env.headless);
        assert!(env.token.is_none());
        assert!(env.channel.is_none());
        assert!(env.output.is_none());
        assert!(env.max_file_size.is_none());
        assert!(env.confirm.is_none());
        assert!(env.search_types.is_none());
        assert!(env.profile.is_none());
    }

    #[test]
    fn is_truthy_values() {
        for val in &["1", "true", "yes", "TRUE", "Yes", "YES"] {
            assert!(is_truthy(val), "expected '{val}' to be truthy");
        }
        for val in &["0", "false", "no", "", "maybe"] {
            assert!(!is_truthy(val), "expected '{val}' to be falsy");
        }
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

    #[test]
    fn validate_output_str_valid() {
        assert!(validate_output_str("table").is_ok());
        assert!(validate_output_str("tsv").is_ok());
        assert!(validate_output_str("json").is_ok());
        assert!(validate_output_str("JSON").is_ok());
        assert!(validate_output_str("Table").is_ok());
    }

    #[test]
    fn validate_output_str_invalid() {
        let err = validate_output_str("yaml").unwrap_err();
        assert!(err.to_string().contains("invalid output 'yaml'"));
        assert!(err.to_string().contains("table, tsv, json"));
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

    // --- parse_file_size tests ---

    #[test]
    fn parse_file_size_bytes() {
        assert_eq!(parse_file_size("100B").unwrap(), 100);
    }

    #[test]
    fn parse_file_size_kb() {
        assert_eq!(parse_file_size("1KB").unwrap(), KB);
    }

    #[test]
    fn parse_file_size_mb() {
        assert_eq!(parse_file_size("50MB").unwrap(), 50 * MB);
    }

    #[test]
    fn parse_file_size_gb() {
        assert_eq!(parse_file_size("2GB").unwrap(), 2 * GB);
    }

    #[test]
    fn parse_file_size_short_units() {
        assert_eq!(parse_file_size("1K").unwrap(), KB);
        assert_eq!(parse_file_size("1M").unwrap(), MB);
        assert_eq!(parse_file_size("1G").unwrap(), GB);
    }

    #[test]
    fn parse_file_size_case_insensitive() {
        assert_eq!(parse_file_size("100mb").unwrap(), 100 * MB);
        assert_eq!(parse_file_size("100Mb").unwrap(), 100 * MB);
    }

    #[test]
    fn parse_file_size_decimal() {
        assert_eq!(parse_file_size("1.5MB").unwrap(), (1.5 * MB as f64) as u64);
    }

    #[test]
    fn parse_file_size_no_unit() {
        assert_eq!(parse_file_size("1024").unwrap(), 1024);
    }

    #[test]
    fn parse_file_size_whitespace() {
        assert_eq!(parse_file_size(" 100 MB ").unwrap(), 100 * MB);
    }

    #[test]
    fn parse_file_size_zero() {
        assert_eq!(parse_file_size("0MB").unwrap(), 0);
    }

    #[test]
    fn parse_file_size_invalid_unit() {
        let err = parse_file_size("1TB").unwrap_err();
        assert!(err.to_string().contains("unknown file size unit"));
    }

    #[test]
    fn parse_file_size_invalid_number() {
        let err = parse_file_size("abcMB").unwrap_err();
        assert!(err.to_string().contains("invalid number"));
    }

    // --- format_size tests ---

    #[test]
    fn format_size_zero() {
        assert_eq!(format_size(0), "0B");
    }

    #[test]
    fn format_size_bytes() {
        assert_eq!(format_size(512), "512B");
    }

    #[test]
    fn format_size_below_kb() {
        assert_eq!(format_size(1023), "1023B");
    }

    #[test]
    fn format_size_kb() {
        assert_eq!(format_size(KB), "1.0KB");
    }

    #[test]
    fn format_size_mb() {
        assert_eq!(format_size(MB), "1.0MB");
    }

    #[test]
    fn format_size_gb() {
        assert_eq!(format_size(GB), "1.0GB");
    }

    // --- resolve_token_store tests ---

    #[test]
    fn resolve_token_store_from_config() {
        let mut cfg = minimal_config();
        cfg.default.token_store = Some("file".to_string());
        assert_eq!(resolve_token_store(&cfg), "file");
    }

    #[test]
    fn resolve_token_store_case_insensitive() {
        let mut cfg = minimal_config();
        cfg.default.token_store = Some("KEYCHAIN".to_string());
        assert_eq!(resolve_token_store(&cfg), "keychain");
    }

    #[test]
    fn resolve_token_store_default() {
        let cfg = minimal_config();
        assert_eq!(resolve_token_store(&cfg), default_token_store());
    }

    // --- Config::new search_types tests ---

    #[test]
    fn config_new_search_types_from_default() {
        let mut cfg = minimal_config();
        cfg.default.search_types = Some(vec!["public_channel".to_string(), "im".to_string()]);
        let config = Config::new(Some(&cfg), None, &no_env()).unwrap();
        assert_eq!(config.search_types.unwrap(), "public_channel,im");
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
        assert_eq!(config.search_types.unwrap(), "private_channel");
    }

    #[test]
    fn config_new_search_types_env_var_overrides() {
        let cfg = minimal_config();
        let env = Env {
            search_types: Some("im,mpim".to_string()),
            ..Env::default()
        };
        let config = Config::new(Some(&cfg), None, &env).unwrap();
        assert_eq!(config.search_types.unwrap(), "im,mpim");
    }

    #[test]
    fn config_new_search_types_env_var_empty_falls_back() {
        let mut cfg = minimal_config();
        cfg.default.search_types = Some(vec!["public_channel".to_string()]);
        // empty string is filtered by Env::load(), so no env var = fallback to config
        let config = Config::new(Some(&cfg), None, &no_env()).unwrap();
        assert_eq!(config.search_types.unwrap(), "public_channel");
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
        assert_eq!(config.output.unwrap(), "json");
    }

    #[test]
    fn config_new_output_env_var_empty_falls_back() {
        let mut cfg = minimal_config();
        cfg.default.output = Some("tsv".to_string());
        // empty output filtered by Env::load(), so no env output = fallback to config
        let config = Config::new(Some(&cfg), None, &no_env()).unwrap();
        assert_eq!(config.output.unwrap(), "tsv");
    }

    #[test]
    fn config_new_output_from_default() {
        let mut cfg = minimal_config();
        cfg.default.output = Some("table".to_string());
        let config = Config::new(Some(&cfg), None, &no_env()).unwrap();
        assert_eq!(config.output.unwrap(), "table");
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
        assert_eq!(config.output.unwrap(), "json");
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
