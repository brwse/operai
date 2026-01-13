//! Type definitions for Box API responses and requests.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxFile {
    pub id: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub name: String,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub path_collection: Option<PathCollection>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub modified_at: Option<String>,
    #[serde(default)]
    pub shared_link: Option<SharedLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoxFolder {
    pub id: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub name: String,
    #[serde(default)]
    pub path_collection: Option<PathCollection>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub modified_at: Option<String>,
    #[serde(default)]
    pub shared_link: Option<SharedLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathCollection {
    pub total_count: u32,
    pub entries: Vec<PathEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedLink {
    pub url: String,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub access: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BoxItem {
    File(BoxFile),
    Folder(BoxFolder),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResults {
    pub total_count: u32,
    pub entries: Vec<BoxItem>,
    #[serde(default)]
    pub offset: Option<u32>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFolderRequest {
    pub name: String,
    pub parent: ParentReference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParentReference {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedLinkRequest {
    pub shared_link: SharedLinkAccess,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedLinkAccess {
    pub access: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationRequest {
    pub item: CollaborationItem,
    pub accessible_by: AccessibleBy,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationItem {
    #[serde(rename = "type")]
    pub item_type: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibleBy {
    #[serde(rename = "type")]
    pub user_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collaboration {
    pub id: String,
    #[serde(rename = "type")]
    pub collab_type: String,
    pub role: String,
    #[serde(default)]
    pub accessible_by: Option<AccessibleBy>,
    #[serde(default)]
    pub item: Option<CollaborationItem>,
}
