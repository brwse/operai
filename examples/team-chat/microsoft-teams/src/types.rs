//! Type definitions for Microsoft Teams integration.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Represents a Microsoft Teams team.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub id: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Represents a Microsoft Teams channel.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub web_url: Option<String>,
}

/// Content type for message body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum BodyContentType {
    Text,
    Html,
}

/// Message body content.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemBody {
    pub content_type: BodyContentType,
    pub content: String,
}

/// Identity of a user who performed an action.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
}

/// Information about who sent or created a message.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdentitySet {
    #[serde(default)]
    pub user: Option<Identity>,
}

/// Represents a chat message in a channel.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: String,
    #[serde(default)]
    pub created_date_time: Option<String>,
    #[serde(default)]
    pub last_modified_date_time: Option<String>,
    #[serde(default)]
    pub from: Option<IdentitySet>,
    #[serde(default)]
    pub body: Option<ItemBody>,
    #[serde(default)]
    pub web_url: Option<String>,
}
