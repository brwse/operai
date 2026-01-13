//! Type definitions for Dropbox API responses and requests.

use serde::{Deserialize, Serialize};

// Internal API types (not exposed)

// ============================================================================
// Search API types
// ============================================================================

#[derive(Debug, Serialize)]
pub(crate) struct SearchRequest {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchResponse {
    pub matches: Vec<SearchMatch>,
    #[serde(default)]
    pub has_more: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchMatch {
    pub metadata: SearchMetadata,
}

#[derive(Debug, Deserialize)]
#[expect(
    dead_code,
    reason = "fields used for deserialization from API responses"
)]
pub(crate) struct SearchMetadata {
    #[serde(rename = ".tag")]
    pub tag: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub path_display: Option<String>,
    #[serde(default)]
    pub client_modified: Option<String>,
    #[serde(default)]
    pub server_modified: Option<String>,
    #[serde(default)]
    pub sharing_info: Option<SharingInfo>,
}

// ============================================================================
// File metadata API types
// ============================================================================

#[derive(Debug, Serialize)]
pub(crate) struct GetMetadataRequest {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_media_info: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[expect(
    dead_code,
    reason = "fields used for deserialization from API responses"
)]
pub(crate) struct FileMetadataResponse {
    #[serde(rename = ".tag")]
    pub tag: String,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub path_display: Option<String>,
    #[serde(default)]
    pub rev: Option<String>,
    #[serde(default)]
    pub client_modified: Option<String>,
    #[serde(default)]
    pub server_modified: Option<String>,
    #[serde(default)]
    pub parent_shared_folder_id: Option<String>,
    #[serde(default)]
    pub sharer_info: Option<SharerInfo>,
    #[serde(default)]
    pub sharing_info: Option<SharingInfo>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SharerInfo {
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SharingInfo {
    #[serde(default)]
    pub read_only: Option<bool>,
}

// ============================================================================
// Download API types
// ============================================================================

#[derive(Debug, Serialize)]
pub(crate) struct DownloadRequest {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_format: Option<String>,
}

// ============================================================================
// Upload API types
// ============================================================================

#[derive(Debug, Serialize)]
pub(crate) struct UploadRequest {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autorename: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_modified: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mute: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict_conflict: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import_format: Option<String>,
}

#[derive(Debug, Deserialize)]
#[expect(
    dead_code,
    reason = "fields used for deserialization from API responses"
)]
pub(crate) struct FileMetadata {
    #[serde(rename = ".tag")]
    pub tag: String,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub path_display: Option<String>,
    #[serde(default)]
    pub rev: Option<String>,
}

// ============================================================================
// Share folder API types
// ============================================================================

#[derive(Debug, Serialize)]
pub(crate) struct CreateSharedLinkRequest {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<SharedLinkSettings>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SharedLinkSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_visibility: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audience: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access: Option<String>,
}

#[derive(Debug, Deserialize)]
#[expect(
    dead_code,
    reason = "tag field used for deserialization from API responses"
)]
pub(crate) struct SharedLinkResponse {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(rename = ".tag")]
    pub tag: String,
}
