//! Type definitions for Slack API.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Represents a Slack channel.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Channel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub is_private: bool,
    #[serde(default)]
    pub is_archived: bool,
    #[serde(default)]
    pub num_members: Option<u32>,
}

/// Represents a Slack message.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Message {
    #[serde(default)]
    pub ts: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub thread_ts: Option<String>,
    #[serde(default)]
    pub reply_count: Option<u32>,
}

/// Represents a Slack file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct File {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub mimetype: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub permalink: Option<String>,
}

// Internal API response types

#[derive(Debug, Deserialize)]
pub(crate) struct SlackResponse<T> {
    pub ok: bool,
    #[serde(flatten)]
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChannelsData {
    pub channels: Vec<SlackChannel>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SlackChannel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub is_private: bool,
    #[serde(default)]
    pub is_archived: bool,
    #[serde(default)]
    pub num_members: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MessagesData {
    pub messages: Vec<SlackMessage>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SlackMessage {
    #[serde(default)]
    pub ts: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub thread_ts: Option<String>,
    #[serde(default)]
    pub reply_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PostMessageData {
    pub ts: String,
    pub channel: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GetUploadURLData {
    pub upload_url: String,
    pub file_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CompleteUploadData {
    pub files: Vec<SlackFile>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SlackFile {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub mimetype: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub permalink: Option<String>,
}

// Conversion functions

pub(crate) fn map_channel(channel: SlackChannel) -> Channel {
    Channel {
        id: channel.id,
        name: channel.name,
        is_private: channel.is_private,
        is_archived: channel.is_archived,
        num_members: channel.num_members,
    }
}

pub(crate) fn map_message(message: SlackMessage) -> Message {
    Message {
        ts: message.ts,
        text: message.text,
        user: message.user,
        thread_ts: message.thread_ts,
        reply_count: message.reply_count,
    }
}

pub(crate) fn map_file(file: SlackFile) -> File {
    File {
        id: file.id,
        name: file.name,
        mimetype: file.mimetype,
        size: file.size,
        permalink: file.permalink,
    }
}
