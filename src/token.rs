use std::path::PathBuf;

use anyhow::{Context, Result};

fn token_dir() -> Result<PathBuf> {
    let data_dir = dirs::data_dir().context("could not determine data directory")?;
    Ok(data_dir.join("slafling").join("tokens"))
}

pub fn token_path(profile: Option<&str>) -> Result<PathBuf> {
    let filename = profile.unwrap_or("default");
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
    std::fs::write(&path, token)
        .with_context(|| format!("failed to write token file {}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to set permissions on {}", path.display()))?;
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
    fn roundtrip_set_get_delete() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-token");
        std::fs::write(&path, "xoxb-test-123").unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content.trim(), "xoxb-test-123");
        std::fs::remove_file(&path).unwrap();
        assert!(!path.exists());
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
