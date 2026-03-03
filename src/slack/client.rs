use anyhow::{bail, Result};
use serde::Deserialize;

pub(super) const API_BASE: &str = "https://slack.com/api";

pub(super) fn slack_post(
    token: &str,
    endpoint: &str,
) -> ureq::RequestBuilder<ureq::typestate::WithBody> {
    ureq::post(&format!("{API_BASE}/{endpoint}")).header("Authorization", format!("Bearer {token}"))
}

pub(super) fn check_ok(ok: bool, error: Option<&str>, api: &str) -> Result<()> {
    if !ok {
        bail!(
            "Slack API error ({}): {}",
            api,
            error.unwrap_or("unknown error")
        );
    }
    Ok(())
}

#[derive(Deserialize)]
pub(super) struct OkResponse {
    pub ok: bool,
    pub error: Option<String>,
}
