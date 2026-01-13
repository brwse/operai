//! Type definitions for Google Drive API responses.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Google Drive file metadata.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DriveFile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub created_time: Option<String>,
    #[serde(default)]
    pub modified_time: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub web_view_link: Option<String>,
    #[serde(default)]
    pub web_content_link: Option<String>,
    #[serde(default)]
    pub parents: Vec<String>,
    #[serde(default)]
    pub shared: Option<bool>,
    #[serde(default)]
    pub owned_by_me: Option<bool>,
}

/// Response from the Drive files.list API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileListResponse {
    pub files: Vec<DriveFile>,
    #[serde(default)]
    pub next_page_token: Option<String>,
}

/// Permission role for sharing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PermissionRole {
    Owner,
    Organizer,
    FileOrganizer,
    Writer,
    Commenter,
    Reader,
}

/// Permission type for sharing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PermissionType {
    User,
    Group,
    Domain,
    Anyone,
}

/// Google Drive permission metadata.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: PermissionType,
    pub role: PermissionRole,
    #[serde(default)]
    pub email_address: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
}
