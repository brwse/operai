//! Type definitions for Google Sheets API.

use serde::{Deserialize, Serialize};

// API request/response types for internal use with Google Sheets API

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetsValueRange {
    pub range: String,
    pub values: Vec<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetsValueRangeInput {
    pub range: String,
    pub major_dimension: String,
    pub values: Vec<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetsAppendResponse {
    pub spreadsheet_id: String,
    pub updates: UpdateValuesResponse,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateValuesResponse {
    pub spreadsheet_id: String,
    pub updated_range: String,
    pub updated_rows: u32,
    pub updated_columns: u32,
    pub updated_cells: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddSheetRequest {
    pub requests: Vec<Request>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub add_sheet: AddSheet,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddSheet {
    pub properties: SheetProperties,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetProperties {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_properties: Option<GridProperties>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GridProperties {
    pub row_count: u32,
    pub column_count: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchUpdateResponse {
    pub spreadsheet_id: String,
    pub replies: Vec<Response>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub add_sheet: Option<AddSheetResponse>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddSheetResponse {
    pub properties: AddedSheetProperties,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddedSheetProperties {
    pub sheet_id: u32,
    pub title: String,
    pub index: u32,
}
