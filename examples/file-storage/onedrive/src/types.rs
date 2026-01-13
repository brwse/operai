//! Type definitions for OneDrive integration.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// File or folder item from OneDrive
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DriveItem {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(rename = "createdDateTime", default)]
    pub created_date_time: Option<String>,
    #[serde(rename = "lastModifiedDateTime", default)]
    pub last_modified_date_time: Option<String>,
    #[serde(rename = "webUrl", default)]
    pub web_url: Option<String>,
    #[serde(default)]
    pub folder: Option<FolderFacet>,
    #[serde(default)]
    pub file: Option<FileFacet>,
    #[serde(rename = "@microsoft.graph.downloadUrl", default)]
    pub download_url: Option<String>,
    #[serde(rename = "parentReference", default)]
    pub parent_reference: Option<ItemReference>,
}

/// Folder facet indicating an item is a folder
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FolderFacet {
    #[serde(rename = "childCount", default)]
    pub child_count: Option<u32>,
}

/// File facet indicating an item is a file
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FileFacet {
    #[serde(rename = "mimeType", default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub hashes: Option<Hashes>,
}

/// File hashes
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Hashes {
    #[serde(rename = "sha1Hash", default)]
    pub sha1_hash: Option<String>,
    #[serde(rename = "quickXorHash", default)]
    pub quick_xor_hash: Option<String>,
}

/// Reference to another item
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ItemReference {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(rename = "driveId", default)]
    pub drive_id: Option<String>,
}

/// Sharing link information
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SharingLink {
    #[serde(rename = "type")]
    pub link_type: String,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(rename = "webUrl")]
    pub web_url: String,
}

/// Permission information
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Permission {
    pub id: String,
    #[serde(default)]
    pub link: Option<SharingLink>,
}
