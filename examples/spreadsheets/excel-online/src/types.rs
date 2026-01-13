//! Type definitions for Excel Online Microsoft Graph API.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Represents a range of cells in a worksheet.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub address_local: Option<String>,
    #[serde(default)]
    pub cell_count: Option<i32>,
    #[serde(default)]
    pub column_count: Option<i32>,
    #[serde(default)]
    pub column_index: Option<i32>,
    #[serde(default)]
    pub row_count: Option<i32>,
    #[serde(default)]
    pub row_index: Option<i32>,
    #[serde(default)]
    pub values: Option<Vec<Vec<serde_json::Value>>>,
    #[serde(default)]
    pub text: Option<Vec<Vec<String>>>,
    #[serde(default)]
    pub formulas: Option<Vec<Vec<serde_json::Value>>>,
    #[serde(default)]
    pub number_format: Option<Vec<Vec<String>>>,
}

/// Represents a table in a worksheet.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub show_headers: Option<bool>,
    #[serde(default)]
    pub show_totals: Option<bool>,
    #[serde(default)]
    pub style: Option<String>,
}
