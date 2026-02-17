use anyhow::{bail, Context, Result};
use serde::Serialize;

#[derive(Serialize)]
struct PostMessageBody<'a> {
    channel: &'a str,
    text: &'a str,
}

pub fn post_message(token: &str, channel: &str, text: &str) -> Result<()> {
    let body = PostMessageBody { channel, text };

    let mut response = ureq::post("https://slack.com/api/chat.postMessage")
        .header("Authorization", &format!("Bearer {token}"))
        .send_json(&body)
        .context("failed to call Slack API")?;

    let json: serde_json::Value = response
        .body_mut()
        .read_json()
        .context("failed to parse Slack API response")?;

    if json.get("ok") != Some(&serde_json::Value::Bool(true)) {
        let error = json["error"].as_str().unwrap_or("unknown error");
        bail!("Slack API error: {error}");
    }

    Ok(())
}
