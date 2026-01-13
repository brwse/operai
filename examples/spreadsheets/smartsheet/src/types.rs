//! Type definitions for Smartsheet API responses and requests.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Summary information about a sheet.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SheetSummary {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub access_level: Option<String>,
    #[serde(default)]
    pub permalink: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub modified_at: Option<String>,
}

/// Column definition in a sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    pub id: i64,
    pub title: String,
    #[serde(default)]
    pub index: Option<i32>,
    #[serde(default)]
    pub primary: Option<bool>,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub column_type: Option<String>,
    #[serde(default)]
    pub options: Option<Vec<String>>,
}

/// Row in a sheet.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Row {
    pub id: i64,
    #[serde(default)]
    pub row_number: Option<i32>,
    #[serde(default)]
    pub cells: Vec<Cell>,
    #[serde(default)]
    pub expanded: Option<bool>,
    #[serde(default)]
    pub parent_id: Option<i64>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub modified_at: Option<String>,
}

/// Cell in a row.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Cell {
    pub column_id: i64,
    #[serde(default)]
    pub value: Option<serde_json::Value>,
    #[serde(default)]
    pub display_value: Option<String>,
    #[serde(default)]
    pub formula: Option<String>,
    #[serde(default)]
    pub hyperlink: Option<CellHyperlink>,
}

/// Hyperlink information for a cell.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CellHyperlink {
    #[serde(default)]
    pub url: Option<String>,
}

/// API list response wrapper for GET /sheets endpoint.
/// According to Smartsheet API 2.0, GET /sheets returns an array directly
/// with optional totalPages and totalCount in the response.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetsListResponse {
    pub sheets: Vec<SheetSummary>,
    #[serde(default)]
    pub total_count: Option<i32>,
}

/// Full sheet response from GET /sheets/{sheetId}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetResponse {
    pub name: String,
    #[serde(default)]
    pub columns: Option<Vec<Column>>,
    #[serde(default)]
    pub rows: Option<Vec<Row>>,
    #[serde(default)]
    pub total_row_count: Option<i32>,
}

/// Discussion object for comments
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Discussion {
    pub id: i64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub comment_id: Option<i64>,
    #[serde(default)]
    pub comments: Option<Vec<Comment>>,
}

/// Comment object within discussions
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: i64,
    pub text: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub modified_at: Option<String>,
    #[serde(default)]
    pub created_by: Option<CreatorInfo>,
}

/// Creator information for comments
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatorInfo {
    pub name: String,
    pub email: String,
}

/// Attachment object
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub attachment_type: Option<String>,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub size_in_kb: Option<i64>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}
