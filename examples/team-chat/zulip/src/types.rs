//! Types for Zulip API responses and internal models.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

// API Response wrapper
#[derive(Debug, Deserialize)]
pub struct ZulipResponse<T> {
    pub result: String,
    #[serde(default)]
    pub msg: String,
    #[serde(flatten)]
    pub data: Option<T>,
}

// Stream/Channel types
#[derive(Debug, Deserialize)]
pub struct ZulipStream {
    #[serde(rename = "stream_id")]
    pub id: i64,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub is_web_public: bool,
    #[serde(default)]
    pub is_announcement_only: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Stream {
    #[serde(rename = "stream_id")]
    pub id: i64,
    pub name: String,
    pub description: String,
    pub is_web_public: bool,
    pub is_announcement_only: bool,
}

#[derive(Debug, Deserialize)]
pub struct StreamsData {
    pub streams: Vec<ZulipStream>,
}

pub fn map_stream(s: ZulipStream) -> Stream {
    Stream {
        id: s.id,
        name: s.name,
        description: s.description,
        is_web_public: s.is_web_public,
        is_announcement_only: s.is_announcement_only,
    }
}

// Message types
#[derive(Debug, Deserialize)]
pub struct ZulipMessage {
    pub id: i64,
    pub sender_id: i64,
    pub sender_full_name: String,
    pub sender_email: String,
    pub timestamp: i64,
    pub content: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub stream_id: Option<i64>,
    #[serde(default)]
    pub subject: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Message {
    pub id: i64,
    pub sender_id: i64,
    pub sender_full_name: String,
    pub sender_email: String,
    pub timestamp: i64,
    pub content: String,
    #[serde(rename = "message_type")]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MessagesData {
    pub messages: Vec<ZulipMessage>,
}

pub fn map_message(m: ZulipMessage) -> Message {
    Message {
        id: m.id,
        sender_id: m.sender_id,
        sender_full_name: m.sender_full_name,
        sender_email: m.sender_email,
        timestamp: m.timestamp,
        content: m.content,
        type_: m.type_,
        stream_id: m.stream_id,
        topic: m.subject,
    }
}

// Send message response
#[derive(Debug, Deserialize)]
pub struct SendMessageData {
    pub id: i64,
}

// Topic data
#[derive(Debug, Deserialize)]
pub struct ZulipTopic {
    pub name: String,
    pub max_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct TopicsData {
    pub topics: Vec<ZulipTopic>,
}
