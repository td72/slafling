use anyhow::Result;

const SERVICE: &str = "slafling";

fn account_name(profile: Option<&str>) -> &str {
    profile.unwrap_or("default")
}

#[cfg(target_os = "macos")]
pub fn get_token(profile: Option<&str>) -> Result<Option<String>> {
    let entry = keyring::Entry::new(SERVICE, account_name(profile))?;
    match entry.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[cfg(target_os = "macos")]
pub fn set_token(profile: Option<&str>, token: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE, account_name(profile))?;
    entry.set_password(token)?;
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn delete_token(profile: Option<&str>) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE, account_name(profile))?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn get_token(_profile: Option<&str>) -> Result<Option<String>> {
    Ok(None)
}

#[cfg(not(target_os = "macos"))]
pub fn set_token(_profile: Option<&str>, _token: &str) -> Result<()> {
    anyhow::bail!("Keychain is only supported on macOS")
}

#[cfg(not(target_os = "macos"))]
pub fn delete_token(_profile: Option<&str>) -> Result<()> {
    anyhow::bail!("Keychain is only supported on macOS")
}
