//! Type definitions for Google Chat API integration.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// A Google Chat space (room or conversation).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Space {
    /// Resource name: spaces/{space}
    pub name: String,
    /// Display name of the space
    #[serde(default)]
    pub display_name: Option<String>,
    /// Type of space
    #[serde(rename = "spaceType", default)]
    pub kind: Option<SpaceType>,
}

/// Type of Google Chat space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SpaceType {
    Space,
    GroupChat,
    DirectMessage,
}

/// A message in Google Chat.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    /// Resource name: spaces/{space}/messages/{message}
    pub name: String,
    /// Message text content
    #[serde(default)]
    pub text: Option<String>,
    /// Message sender
    #[serde(default)]
    pub sender: Option<User>,
    /// Message creation time
    #[serde(default)]
    pub create_time: Option<String>,
    /// Thread information
    #[serde(default)]
    pub thread: Option<Thread>,
}

/// A Google Chat user.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct User {
    /// Resource name: users/{user}
    pub name: String,
    /// Display name
    #[serde(default)]
    pub display_name: Option<String>,
    /// User type
    #[serde(rename = "type", default)]
    pub kind: Option<UserType>,
}

/// Type of Google Chat user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UserType {
    Human,
    Bot,
}

/// Thread information for organizing messages.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    /// Resource name: spaces/{space}/threads/{thread}
    pub name: String,
}

/// Attachment metadata for uploaded files.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    /// Resource name
    pub name: String,
    /// Content type (MIME type)
    #[serde(default)]
    pub content_type: Option<String>,
    /// Attachment source
    #[serde(rename = "attachmentDataRef", default)]
    pub data_ref: Option<AttachmentDataRef>,
}

/// Reference to attachment data.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentDataRef {
    /// Resource name for downloading
    #[serde(default)]
    pub resource_name: Option<String>,
    /// Upload token
    pub attachment_upload_token: String,
}
