use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

fn token_dir() -> Result<PathBuf> {
    let data_dir = dirs::data_dir().context("could not determine data directory")?;
    Ok(data_dir.join("slafling").join("tokens"))
}

fn validate_profile_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name.contains('\0')
    {
        bail!("invalid profile name '{name}' (must not be empty or contain /, \\, .., or null)");
    }
    Ok(())
}

fn profile_path(dir: &Path, profile: Option<&str>) -> Result<PathBuf> {
    let filename = profile.unwrap_or("default");
    validate_profile_name(filename)?;
    Ok(dir.join(filename))
}

pub fn token_path(profile: Option<&str>) -> Result<PathBuf> {
    profile_path(&token_dir()?, profile)
}

fn read_token(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let token = content.trim().to_string();
            if token.is_empty() {
                Ok(None)
            } else {
                Ok(Some(token))
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("failed to read token file {}", path.display())),
    }
}

fn write_token(path: &Path, token: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        use std::os::unix::fs::PermissionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .with_context(|| format!("failed to create token file {}", path.display()))?;
        file.write_all(token.as_bytes())
            .with_context(|| format!("failed to write token file {}", path.display()))?;
        // Ensure permissions are 0o600 even when overwriting an existing file
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to set permissions on {}", path.display()))?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(path, token)
            .with_context(|| format!("failed to write token file {}", path.display()))?;
    }

    Ok(())
}

fn remove_token(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("failed to delete token file {}", path.display())),
    }
}

pub fn get_token(profile: Option<&str>) -> Result<Option<String>> {
    read_token(&token_path(profile)?)
}

pub fn set_token(profile: Option<&str>, token: &str) -> Result<()> {
    write_token(&token_path(profile)?, token)
}

pub fn delete_token(profile: Option<&str>) -> Result<()> {
    remove_token(&token_path(profile)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let tokens = dir.path().join("tokens");
        (dir, tokens)
    }

    #[test]
    fn token_path_default() {
        let path = token_path(None).unwrap();
        assert!(path.ends_with("slafling/tokens/default"));
    }

    #[test]
    fn token_path_named_profile() {
        let path = token_path(Some("work")).unwrap();
        assert!(path.ends_with("slafling/tokens/work"));
    }

    #[test]
    fn roundtrip_write_read_delete() {
        let (_dir, tokens) = test_dir();
        let path = profile_path(&tokens, Some("test-profile")).unwrap();

        // write
        write_token(&path, "xoxb-test-123").unwrap();
        assert!(path.exists());

        // read
        let token = read_token(&path).unwrap();
        assert_eq!(token, Some("xoxb-test-123".to_string()));

        // delete
        remove_token(&path).unwrap();
        assert!(!path.exists());

        // read after delete
        let token = read_token(&path).unwrap();
        assert_eq!(token, None);
    }

    #[cfg(unix)]
    #[test]
    fn write_token_creates_file_with_restricted_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let (_dir, tokens) = test_dir();
        let path = profile_path(&tokens, Some("perm-test")).unwrap();

        write_token(&path, "xoxb-secret").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn write_token_enforces_permissions_on_overwrite() {
        use std::os::unix::fs::PermissionsExt;

        let (_dir, tokens) = test_dir();
        let path = profile_path(&tokens, Some("overwrite-test")).unwrap();

        // Create file with loose permissions
        write_token(&path, "initial").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        // Overwrite — should restore 0o600
        write_token(&path, "updated").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn read_token_missing_file_returns_none() {
        let (_dir, tokens) = test_dir();
        let path = profile_path(&tokens, Some("nonexistent")).unwrap();
        assert_eq!(read_token(&path).unwrap(), None);
    }

    #[test]
    fn read_token_empty_file_returns_none() {
        let (_dir, tokens) = test_dir();
        let path = profile_path(&tokens, Some("empty")).unwrap();
        write_token(&path, "").unwrap();
        assert_eq!(read_token(&path).unwrap(), None);
    }

    #[test]
    fn rejects_path_traversal_profiles() {
        let (_dir, tokens) = test_dir();
        assert!(profile_path(&tokens, Some("../evil")).is_err());
        assert!(profile_path(&tokens, Some("foo/bar")).is_err());
        assert!(profile_path(&tokens, Some("foo\\bar")).is_err());
        assert!(profile_path(&tokens, Some("..")).is_err());
    }

    #[test]
    fn rejects_empty_profile_name() {
        let (_dir, tokens) = test_dir();
        assert!(profile_path(&tokens, Some("")).is_err());
    }

    #[test]
    fn delete_nonexistent_is_ok() {
        let (_dir, tokens) = test_dir();
        let path = profile_path(&tokens, Some("ghost")).unwrap();
        assert!(remove_token(&path).is_ok());
    }
}
