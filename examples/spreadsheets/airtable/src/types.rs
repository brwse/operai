//! Type definitions for Airtable API.

use std::collections::HashMap;

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Represents an Airtable base summary.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Base {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub permission_level: Option<String>,
}

/// Represents an Airtable table metadata.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Table {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub primary_field_id: Option<String>,
}

/// Represents an Airtable record.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Record {
    pub id: String,
    pub fields: HashMap<String, serde_json::Value>,
    #[serde(default, rename = "createdTime")]
    pub created_time: Option<String>,
}

/// Airtable API response for listing bases.
#[derive(Debug, Deserialize)]
pub struct ListBasesResponse {
    pub bases: Vec<AirtableBase>,
    #[serde(default)]
    pub offset: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AirtableBase {
    pub id: String,
    pub name: String,
    #[serde(default, rename = "permissionLevel")]
    pub permission_level: Option<String>,
}

/// Airtable API response for getting base schema.
#[derive(Debug, Deserialize)]
pub struct BaseSchemaResponse {
    pub tables: Vec<AirtableTable>,
}

#[derive(Debug, Deserialize)]
pub struct AirtableTable {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "primaryFieldId")]
    pub primary_field_id: Option<String>,
}

/// Airtable API response for listing records.
#[derive(Debug, Deserialize)]
pub struct ListRecordsResponse {
    pub records: Vec<AirtableRecord>,
    #[serde(default)]
    pub offset: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AirtableRecord {
    pub id: String,
    pub fields: HashMap<String, serde_json::Value>,
    #[serde(default, rename = "createdTime")]
    pub created_time: Option<String>,
}

/// Request body for creating a record.
#[derive(Debug, Serialize)]
pub struct CreateRecordRequest {
    pub fields: HashMap<String, serde_json::Value>,
}

/// Request body for updating a record.
#[derive(Debug, Serialize)]
pub struct UpdateRecordRequest {
    pub fields: HashMap<String, serde_json::Value>,
}

/// Response when creating or updating a record.
#[derive(Debug, Deserialize)]
pub struct RecordResponse {
    pub id: String,
    pub fields: HashMap<String, serde_json::Value>,
    #[serde(default, rename = "createdTime")]
    pub created_time: Option<String>,
}

/// Represents an attachment to add to an attachment field.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Attachment {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
}
