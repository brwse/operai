//! Type definitions for Mattermost API responses and requests.

use serde::{Deserialize, Serialize};

// =============================================================================
// Mattermost API Response Types
// =============================================================================

/// Mattermost channel as returned by the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MattermostChannel {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default, rename = "type")]
    pub channel_type: String,
    #[serde(default)]
    pub team_id: String,
    #[serde(default)]
    pub header: String,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub total_msg_count: u64,
}

/// Mattermost post/message as returned by the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MattermostPost {
    pub id: String,
    pub channel_id: String,
    pub user_id: String,
    pub message: String,
    pub create_at: i64,
    pub update_at: i64,
    #[serde(default)]
    pub root_id: String,
    #[serde(default)]
    pub file_ids: Vec<String>,
}

/// Mattermost user as returned by the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MattermostUser {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub first_name: String,
    #[serde(default)]
    pub last_name: String,
}

/// Mattermost file info as returned by the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MattermostFileInfo {
    pub id: String,
    pub name: String,
    pub extension: String,
    pub size: i64,
    pub mime_type: String,
    #[serde(default)]
    pub has_preview_image: bool,
}

/// Response from GET /`channels/{channel_id}/posts`.
#[derive(Debug, Deserialize)]
pub struct MattermostPostsResponse {
    pub order: Vec<String>,
    pub posts: std::collections::HashMap<String, MattermostPost>,
}

// =============================================================================
// Mattermost API Request Types
// =============================================================================

/// Request to create a post.
#[derive(Debug, Serialize)]
pub struct CreatePostRequest {
    pub channel_id: String,
    pub message: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub root_id: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub file_ids: Vec<String>,
}
