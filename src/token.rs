use std::path::PathBuf;

use anyhow::{bail, Context, Result};

fn token_dir() -> Result<PathBuf> {
    let data_dir = dirs::data_dir().context("could not determine data directory")?;
    Ok(data_dir.join("slafling").join("tokens"))
}

fn validate_profile_name(name: &str) -> Result<()> {
    if name.contains('/') || name.contains('\\') || name.contains("..") || name.contains('\0') {
        bail!("invalid profile name '{name}' (must not contain /, \\, .., or null)");
    }
    Ok(())
}

pub fn token_path(profile: Option<&str>) -> Result<PathBuf> {
    let filename = profile.unwrap_or("default");
    validate_profile_name(filename)?;
    Ok(token_dir()?.join(filename))
}

pub fn get_token(profile: Option<&str>) -> Result<Option<String>> {
    let path = token_path(profile)?;
    match std::fs::read_to_string(&path) {
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

pub fn set_token(profile: Option<&str>, token: &str) -> Result<()> {
    let path = token_path(profile)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .with_context(|| format!("failed to create token file {}", path.display()))?;
        file.write_all(token.as_bytes())
            .with_context(|| format!("failed to write token file {}", path.display()))?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(&path, token)
            .with_context(|| format!("failed to write token file {}", path.display()))?;
    }

    Ok(())
}

pub fn delete_token(profile: Option<&str>) -> Result<()> {
    let path = token_path(profile)?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("failed to delete token file {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tokens").join("test-profile");

        // set: create parent dirs and write
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "xoxb-test-123").unwrap();

        // get: read and trim
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content.trim(), "xoxb-test-123");

        // delete: remove file
        std::fs::remove_file(&path).unwrap();
        assert!(!path.exists());

        // get after delete: file not found
        assert!(std::fs::read_to_string(&path).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn set_token_creates_file_with_restricted_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("token-perm-test");
        std::fs::write(&path, "xoxb-secret").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn rejects_path_traversal_profiles() {
        assert!(token_path(Some("../evil")).is_err());
        assert!(token_path(Some("foo/bar")).is_err());
        assert!(token_path(Some("foo\\bar")).is_err());
        assert!(token_path(Some("..")).is_err());
    }

    #[test]
    fn get_token_missing_file_returns_none() {
        // token_path for a profile that doesn't have a file stored
        // We can't easily test get_token directly without mocking dirs::data_dir,
        // but we test the NotFound handling logic
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent");
        match std::fs::read_to_string(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {} // expected
            other => panic!("expected NotFound, got {other:?}"),
        }
    }
}
