use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

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

// --- File upload (3-step) ---

#[derive(Deserialize)]
struct GetUploadUrlResponse {
    ok: bool,
    error: Option<String>,
    upload_url: Option<String>,
    file_id: Option<String>,
}

#[derive(Deserialize)]
struct CompleteUploadResponse {
    ok: bool,
    error: Option<String>,
}

fn get_upload_url(token: &str, filename: &str, length: u64) -> Result<(String, String)> {
    let length_str = length.to_string();
    let mut resp = ureq::post("https://slack.com/api/files.getUploadURLExternal")
        .header("Authorization", &format!("Bearer {token}"))
        .send_form([("filename", filename), ("length", &length_str)])
        .context("failed to call files.getUploadURLExternal")?;

    let body: GetUploadUrlResponse = resp
        .body_mut()
        .read_json()
        .context("failed to parse getUploadURLExternal response")?;

    if !body.ok {
        let error = body.error.as_deref().unwrap_or("unknown error");
        bail!("Slack API error (getUploadURLExternal): {error}");
    }

    let upload_url = body.upload_url.context("missing upload_url in response")?;
    let file_id = body.file_id.context("missing file_id in response")?;
    Ok((upload_url, file_id))
}

fn upload_file_content(upload_url: &str, data: &[u8]) -> Result<()> {
    ureq::post(upload_url)
        .content_type("application/octet-stream")
        .send(data)
        .context("failed to upload file content")?;
    Ok(())
}

#[derive(Serialize)]
struct FileEntry {
    id: String,
    title: String,
}

#[derive(Serialize)]
struct CompleteUploadBody {
    files: Vec<FileEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    initial_comment: Option<String>,
}

fn complete_upload(
    token: &str,
    file_id: &str,
    title: &str,
    channel: &str,
    initial_comment: Option<&str>,
) -> Result<()> {
    let body = CompleteUploadBody {
        files: vec![FileEntry {
            id: file_id.to_string(),
            title: title.to_string(),
        }],
        channel_id: Some(channel.to_string()),
        initial_comment: initial_comment.map(String::from),
    };

    let mut resp = ureq::post("https://slack.com/api/files.completeUploadExternal")
        .header("Authorization", &format!("Bearer {token}"))
        .send_json(&body)
        .context("failed to call files.completeUploadExternal")?;

    let result: CompleteUploadResponse = resp
        .body_mut()
        .read_json()
        .context("failed to parse completeUploadExternal response")?;

    if !result.ok {
        let error = result.error.as_deref().unwrap_or("unknown error");
        bail!("Slack API error (completeUploadExternal): {error}");
    }

    Ok(())
}

pub fn upload_file_bytes(
    token: &str,
    channel: &str,
    filename: &str,
    data: &[u8],
    initial_comment: Option<&str>,
) -> Result<()> {
    let (upload_url, file_id) = get_upload_url(token, filename, data.len() as u64)?;
    upload_file_content(&upload_url, data)?;
    complete_upload(token, &file_id, filename, channel, initial_comment)?;

    Ok(())
}

// --- Channel search ---

#[derive(Deserialize)]
struct ConversationsListResponse {
    ok: bool,
    error: Option<String>,
    #[serde(default)]
    channels: Vec<Channel>,
    response_metadata: Option<ResponseMetadata>,
}

#[derive(Deserialize)]
struct Channel {
    id: String,
    name: String,
}

#[derive(Deserialize)]
struct ResponseMetadata {
    next_cursor: Option<String>,
}

pub fn search_channels(token: &str, query: &str) -> Result<Vec<(String, String)>> {
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();
    let mut cursor = String::new();

    loop {
        let mut params = vec![
            ("limit".to_string(), "200".to_string()),
            ("exclude_archived".to_string(), "true".to_string()),
        ];
        if !cursor.is_empty() {
            params.push(("cursor".to_string(), cursor.clone()));
        }

        let mut resp = ureq::post("https://slack.com/api/conversations.list")
            .header("Authorization", &format!("Bearer {token}"))
            .send_form(params)
            .context("failed to call conversations.list")?;

        let body: ConversationsListResponse = resp
            .body_mut()
            .read_json()
            .context("failed to parse conversations.list response")?;

        if !body.ok {
            let error = body.error.as_deref().unwrap_or("unknown error");
            bail!("Slack API error (conversations.list): {error}");
        }

        for ch in &body.channels {
            if ch.name.to_lowercase().contains(&query_lower) {
                results.push((ch.name.clone(), ch.id.clone()));
            }
        }

        match body
            .response_metadata
            .and_then(|m| m.next_cursor)
            .filter(|c| !c.is_empty())
        {
            Some(next) => cursor = next,
            None => break,
        }
    }

    results.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(results)
}
