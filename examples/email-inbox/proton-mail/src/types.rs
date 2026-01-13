//! Type definitions for Proton Mail API.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Email address in Proton Mail
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub struct EmailAddress {
    pub address: String,
    #[serde(default)]
    pub name: Option<String>,
}

/// Recipient information
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub struct Recipient {
    pub address: String,
    #[serde(default)]
    pub name: Option<String>,
}

/// Message summary (for list/search operations)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub struct MessageSummary {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub sender: Option<EmailAddress>,
    #[serde(default)]
    pub time: Option<i64>,
    #[serde(default)]
    pub size: Option<i64>,
    #[serde(default)]
    pub unread: Option<i32>,
    #[serde(default)]
    pub starred: Option<i32>,
    #[serde(rename = "LabelIDs", default)]
    pub label_ids: Vec<String>,
}

/// Full message details
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "PascalCase")]
pub struct Message {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub sender: Option<EmailAddress>,
    #[serde(rename = "ToList", default)]
    pub to_list: Vec<Recipient>,
    #[serde(rename = "CCList", default)]
    pub cc_list: Vec<Recipient>,
    #[serde(rename = "BCCList", default)]
    pub bcc_list: Vec<Recipient>,
    #[serde(default)]
    pub time: Option<i64>,
    #[serde(default)]
    pub size: Option<i64>,
    #[serde(default)]
    pub unread: Option<i32>,
    #[serde(default)]
    pub starred: Option<i32>,
    #[serde(rename = "LabelIDs", default)]
    pub label_ids: Vec<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(rename = "MIMEType", default)]
    pub mime_type: Option<String>,
}

/// Proton API list response wrapper
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ProtonListResponse<T> {
    pub total: Option<i32>,
    pub messages: Vec<T>,
}

/// Proton API single item response wrapper
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ProtonResponse<T> {
    pub message: T,
}

/// Request to send an email
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SendMessageRequest {
    pub message: SendMessage,
}

/// Message to send
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SendMessage {
    pub subject: String,
    pub body: String,
    #[serde(rename = "ToList")]
    pub to_list: Vec<Recipient>,
    #[serde(rename = "CCList", skip_serializing_if = "Vec::is_empty")]
    pub cc_list: Vec<Recipient>,
    #[serde(rename = "BCCList", skip_serializing_if = "Vec::is_empty")]
    pub bcc_list: Vec<Recipient>,
    #[serde(rename = "MIMEType")]
    pub mime_type: String,
}

/// Request to label/unlabel messages
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct LabelRequest {
    #[serde(rename = "LabelID")]
    pub label_id: String,
}

/// Request to move message to folder
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct MoveRequest {
    #[serde(rename = "LabelID")]
    pub label_id: String,
}
