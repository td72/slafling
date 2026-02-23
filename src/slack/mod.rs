mod client;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::cli::ChannelType;
use client::{check_ok, slack_post, OkResponse};

// --- chat.postMessage ---

#[derive(Serialize)]
struct PostMessageBody<'a> {
    channel: &'a str,
    text: &'a str,
}

pub fn post_message(token: &str, channel: &str, text: &str) -> Result<()> {
    let body = PostMessageBody { channel, text };
    let mut resp = slack_post(token, "chat.postMessage")
        .send_json(&body)
        .context("failed to call chat.postMessage")?;
    let result: OkResponse = resp
        .body_mut()
        .read_json()
        .context("failed to parse chat.postMessage response")?;
    check_ok(result.ok, result.error.as_deref(), "chat.postMessage")
}

// --- File upload (3-step) ---

#[derive(Deserialize)]
struct GetUploadUrlResponse {
    ok: bool,
    error: Option<String>,
    upload_url: Option<String>,
    file_id: Option<String>,
}

fn get_upload_url(token: &str, filename: &str, length: u64) -> Result<(String, String)> {
    let length_str = length.to_string();
    let mut resp = slack_post(token, "files.getUploadURLExternal")
        .send_form([("filename", filename), ("length", &length_str)])
        .context("failed to call files.getUploadURLExternal")?;
    let body: GetUploadUrlResponse = resp
        .body_mut()
        .read_json()
        .context("failed to parse files.getUploadURLExternal response")?;
    check_ok(body.ok, body.error.as_deref(), "files.getUploadURLExternal")?;
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
    let mut resp = slack_post(token, "files.completeUploadExternal")
        .send_json(&body)
        .context("failed to call files.completeUploadExternal")?;
    let result: OkResponse = resp
        .body_mut()
        .read_json()
        .context("failed to parse files.completeUploadExternal response")?;
    check_ok(
        result.ok,
        result.error.as_deref(),
        "files.completeUploadExternal",
    )
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
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    is_im: bool,
    #[serde(default)]
    is_mpim: bool,
    #[serde(default)]
    is_private: bool,
    user: Option<String>,
}

impl Channel {
    fn channel_type(&self) -> ChannelType {
        if self.is_im {
            ChannelType::Im
        } else if self.is_mpim {
            ChannelType::Mpim
        } else if self.is_private {
            ChannelType::PrivateChannel
        } else {
            ChannelType::PublicChannel
        }
    }
}

#[derive(Deserialize)]
struct ResponseMetadata {
    next_cursor: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct ChannelInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub channel_type: ChannelType,
    pub channel_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

pub fn search_channels(
    token: &str,
    query: &str,
    types: &[ChannelType],
) -> Result<Vec<ChannelInfo>> {
    let query_lower = query.to_lowercase();
    let types_str = crate::cli::channel_types_to_api_string(types);
    let mut results = Vec::new();
    let mut cursor = String::new();

    loop {
        let mut params = vec![
            ("limit".to_string(), "200".to_string()),
            ("exclude_archived".to_string(), "true".to_string()),
            ("types".to_string(), types_str.clone()),
        ];
        if !cursor.is_empty() {
            params.push(("cursor".to_string(), cursor.clone()));
        }

        let mut resp = slack_post(token, "conversations.list")
            .send_form(params)
            .context("failed to call conversations.list")?;
        let body: ConversationsListResponse = resp
            .body_mut()
            .read_json()
            .context("failed to parse conversations.list response")?;
        check_ok(body.ok, body.error.as_deref(), "conversations.list")?;

        for ch in &body.channels {
            let display_name = ch
                .name
                .clone()
                .or_else(|| ch.user.clone())
                .unwrap_or_else(|| ch.id.clone());

            if display_name.to_lowercase().contains(&query_lower) {
                results.push(ChannelInfo {
                    name: display_name,
                    channel_type: ch.channel_type(),
                    channel_id: ch.id.clone(),
                    user_id: ch.user.clone(),
                });
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

    results.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(results)
}
