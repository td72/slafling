use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;

// ── TokenStore enum ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TokenStore {
    Keychain,
    File,
}

impl TokenStore {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Keychain => "keychain",
            Self::File => "file",
        }
    }

    pub fn default_for_platform() -> Self {
        if cfg!(target_os = "macos") {
            Self::Keychain
        } else {
            Self::File
        }
    }
}

impl FromStr for TokenStore {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "keychain" => Ok(Self::Keychain),
            "file" => Ok(Self::File),
            _ => bail!("invalid token_store '{}' (valid: keychain, file)", s),
        }
    }
}

// ── TOML types ───────────────────────────────────────────────────────────────

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

// ── Config file I/O ──────────────────────────────────────────────────────────

pub fn config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home.join(".config").join("slafling").join("config.toml"))
}

pub fn generate_init_config() -> String {
    include_str!("../../config.template.toml").replace(
        "# token_store = \"keychain\"",
        &format!(
            "# token_store = \"{}\"",
            TokenStore::default_for_platform().as_str()
        ),
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

pub fn load_config() -> Result<ConfigFile> {
    let path = config_path()?;
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let config: ConfigFile =
        toml::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))?;
    validate_config(&config)?;
    Ok(config)
}

// ── Validation ───────────────────────────────────────────────────────────────

pub(super) fn validate_config(config: &ConfigFile) -> Result<()> {
    validate_section_values(
        "default",
        config.default.output.as_deref(),
        config.default.search_types.as_deref(),
    )?;

    if let Some(val) = &config.default.token_store {
        let store = val
            .parse::<TokenStore>()
            .map_err(|e| anyhow!("{} in [default]", e))?;
        if matches!(store, TokenStore::Keychain) && !cfg!(target_os = "macos") {
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
        val.parse::<crate::cli::OutputFormat>()
            .map_err(|e| anyhow!("{} in [{}]", e, section))?;
    }

    if let Some(types) = search_types {
        for val in types {
            val.parse::<crate::cli::SearchType>()
                .map_err(|e| anyhow!("{} in [{}]", e, section))?;
        }
    }

    Ok(())
}

pub fn resolve_token_store(config: &ConfigFile) -> TokenStore {
    config
        .default
        .token_store
        .as_deref()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(TokenStore::default_for_platform)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

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
        assert!(err.to_string().contains("invalid search type 'foo'"));
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
    fn token_store_from_str_valid() {
        assert_eq!("file".parse::<TokenStore>().unwrap(), TokenStore::File);
        assert_eq!(
            "keychain".parse::<TokenStore>().unwrap(),
            TokenStore::Keychain
        );
        assert_eq!("FILE".parse::<TokenStore>().unwrap(), TokenStore::File);
    }

    #[test]
    fn token_store_from_str_invalid() {
        let err = "redis".parse::<TokenStore>().unwrap_err();
        assert!(err.to_string().contains("invalid token_store 'redis'"));
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
        let template = include_str!("../../config.template.toml");
        assert!(
            template.contains("# token_store = \"keychain\""),
            "config.template.toml must contain the token_store needle for generate_init_config()"
        );
    }

    #[test]
    fn init_config_has_platform_default_token_store() {
        let content = generate_init_config();
        let expected = format!(
            "# token_store = \"{}\"",
            TokenStore::default_for_platform().as_str()
        );
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
    fn resolve_token_store_from_config() {
        let mut cfg = minimal_config();
        cfg.default.token_store = Some("file".to_string());
        assert_eq!(resolve_token_store(&cfg), TokenStore::File);
    }

    #[test]
    fn resolve_token_store_case_insensitive() {
        let mut cfg = minimal_config();
        cfg.default.token_store = Some("KEYCHAIN".to_string());
        assert_eq!(resolve_token_store(&cfg), TokenStore::Keychain);
    }

    #[test]
    fn resolve_token_store_default() {
        let cfg = minimal_config();
        assert_eq!(
            resolve_token_store(&cfg),
            TokenStore::default_for_platform()
        );
    }
}
