//! Type definitions for Coda API responses and requests.

use serde::{Deserialize, Serialize};

/// A Coda document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodaDoc {
    pub id: String,
    #[serde(rename = "type")]
    pub doc_type: String,
    pub href: String,
    pub browser_link: String,
    pub name: String,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub owner_name: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// A row in a Coda table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodaRow {
    pub id: String,
    #[serde(rename = "type")]
    pub row_type: String,
    pub href: String,
    pub name: String,
    pub index: i32,
    pub browser_link: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    pub values: serde_json::Map<String, serde_json::Value>,
}

/// A comment in a Coda doc.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodaComment {
    pub id: String,
    #[serde(rename = "type")]
    pub comment_type: String,
    pub href: String,
    pub created_at: String,
    pub modified_at: String,
    pub content: String,
    #[serde(default)]
    pub parent: Option<CodaCommentParent>,
}

/// Parent reference for a comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodaCommentParent {
    pub id: String,
    #[serde(rename = "type")]
    pub parent_type: String,
    pub href: String,
}

/// API list response wrapper.
#[derive(Debug, Deserialize)]
pub struct CodaListResponse<T> {
    pub items: Vec<T>,
}

/// Row upsert request.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertRowsRequest {
    pub rows: Vec<UpsertRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_columns: Option<Vec<String>>,
}

/// A single row to upsert.
#[derive(Debug, Serialize)]
pub struct UpsertRow {
    pub cells: Vec<CellValue>,
}

/// A cell value in a row.
#[derive(Debug, Serialize)]
pub struct CellValue {
    pub column: String,
    pub value: serde_json::Value,
}

/// Row upsert response.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertRowsResponse {
    pub request_id: String,
    #[serde(default)]
    pub added_row_ids: Vec<String>,
}

/// Comment creation request.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCommentRequest {
    pub content: String,
}
