//! Type definitions for Gmail API integration.

use serde::{Deserialize, Serialize};

/// Gmail API message list response.
#[derive(Debug, Deserialize)]
#[expect(
    dead_code,
    reason = "fields used for deserialization from API responses"
)]
#[serde(rename_all = "camelCase")]
pub struct ListMessagesResponse {
    #[serde(default)]
    pub messages: Vec<MessageRef>,
    #[serde(default)]
    pub next_page_token: Option<String>,
    #[serde(default)]
    pub result_size_estimate: u32,
}

/// Gmail message reference (ID and thread ID).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageRef {
    pub id: String,
    #[serde(default)]
    pub thread_id: Option<String>,
}

/// Gmail API full message.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GmailMessage {
    pub id: String,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub label_ids: Vec<String>,
    #[serde(default)]
    pub snippet: Option<String>,
    #[serde(default)]
    pub payload: Option<MessagePart>,
}

/// Gmail message part (MIME structure).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagePart {
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub headers: Vec<MessageHeader>,
    #[serde(default)]
    pub body: Option<MessagePartBody>,
    #[serde(default)]
    pub parts: Vec<MessagePart>,
}

/// Gmail message header.
#[derive(Debug, Deserialize)]
pub struct MessageHeader {
    pub name: String,
    pub value: String,
}

/// Gmail message part body.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagePartBody {
    #[serde(default)]
    pub data: Option<String>,
}

/// Request to send an email.
#[derive(Debug, Serialize)]
pub struct SendMessageRequest {
    pub raw: String,
}

/// Request to modify message labels.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModifyMessageRequest {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub add_label_ids: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub remove_label_ids: Vec<String>,
}
