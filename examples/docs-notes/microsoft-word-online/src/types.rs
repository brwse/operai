//! Type definitions for Microsoft Word Online integration.
//!
//! These types represent Microsoft Graph API responses for Word document
//! operations.

use serde::{Deserialize, Serialize};

// ============================================================================
// Drive Item (Document Metadata)
// ============================================================================

/// Represents a Word document in OneDrive/SharePoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveItem {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub web_url: Option<String>,
    #[serde(default)]
    pub size: Option<i64>,
    #[serde(default)]
    pub created_date_time: Option<String>,
    #[serde(default)]
    pub last_modified_date_time: Option<String>,
    #[serde(default)]
    pub last_modified_by: Option<IdentitySet>,
    #[serde(default)]
    pub file: Option<FileMetadata>,
}

/// File-specific metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMetadata {
    #[serde(default)]
    pub mime_type: Option<String>,
}

/// Identity information for users.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentitySet {
    #[serde(default)]
    pub user: Option<Identity>,
}

/// User identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
}
