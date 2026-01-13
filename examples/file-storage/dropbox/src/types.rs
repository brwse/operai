//! Type definitions for Dropbox API.

use serde::Deserialize;

// ============================================================================
// Search Tool Types
// ============================================================================

#[derive(Deserialize)]
pub struct SearchResponse {
    pub matches: Vec<SearchMatch>,
    pub has_more: bool,
    pub cursor: Option<String>,
}

#[derive(Deserialize)]
pub struct SearchMatch {
    pub metadata: SearchMetadata,
}

#[derive(Deserialize)]
#[serde(tag = ".tag")]
pub enum SearchMetadata {
    #[serde(rename = "file")]
    File {
        name: String,
        path_display: String,
        path_lower: String,
        id: String,
        #[serde(default)]
        size: Option<u64>,
        #[serde(default)]
        server_modified: Option<String>,
        #[serde(default)]
        content_hash: Option<String>,
    },
    #[serde(rename = "folder")]
    Folder {
        name: String,
        path_display: String,
        path_lower: String,
        id: String,
    },
    #[serde(other)]
    Other,
}

// ============================================================================
// Download Tool Types
// ============================================================================

#[derive(Deserialize)]
pub struct DropboxDownloadMetadata {
    pub name: String,
    pub path_display: String,
    pub path_lower: String,
    pub id: String,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub server_modified: Option<String>,
    #[serde(default)]
    pub content_hash: Option<String>,
}

// ============================================================================
// Upload Tool Types
// ============================================================================

#[derive(Deserialize)]
pub struct DropboxFileMetadata {
    pub name: String,
    pub path_display: String,
    pub path_lower: String,
    pub id: String,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub server_modified: Option<String>,
    #[serde(default)]
    pub content_hash: Option<String>,
    pub rev: String,
}

// ============================================================================
// Share Link Tool Types
// ============================================================================

#[derive(Deserialize)]
pub struct SharedLinkResponse {
    pub url: String,
    #[serde(default)]
    pub expires: Option<String>,
}

// ============================================================================
// Move/Rename Tool Types
// ============================================================================

#[derive(Deserialize)]
pub struct MoveResponse {
    pub metadata: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(tag = ".tag")]
pub enum MovedMetadata {
    #[serde(rename = "file")]
    File {
        name: String,
        path_display: String,
        path_lower: String,
        id: String,
        #[serde(default)]
        size: Option<u64>,
        #[serde(default)]
        server_modified: Option<String>,
        #[serde(default)]
        content_hash: Option<String>,
    },
    #[serde(rename = "folder")]
    Folder {
        name: String,
        path_display: String,
        path_lower: String,
        id: String,
    },
}
