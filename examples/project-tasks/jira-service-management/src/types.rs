//! Type definitions for Jira Service Management API

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct User {
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub email_address: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequestType {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub status_category: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequestSummary {
    #[serde(default)]
    pub issue_id: String,
    #[serde(default)]
    pub issue_key: String,
    #[serde(default)]
    pub request_type: Option<RequestType>,
    #[serde(default)]
    pub current_status: Option<Status>,
    #[serde(default)]
    pub reporter: Option<User>,
    #[serde(default)]
    pub created_date: Option<Timestamp>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequestDetail {
    #[serde(default)]
    pub issue_id: String,
    #[serde(default)]
    pub issue_key: String,
    #[serde(default)]
    pub request_type: Option<RequestType>,
    #[serde(default)]
    pub current_status: Option<Status>,
    #[serde(default)]
    pub reporter: Option<User>,
    #[serde(default)]
    pub created_date: Option<Timestamp>,
    #[serde(default)]
    pub request_field_values: Vec<RequestFieldValue>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequestFieldValue {
    #[serde(default)]
    pub field_id: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub author: Option<User>,
    #[serde(default)]
    pub created: Option<Timestamp>,
    #[serde(default)]
    pub public: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Timestamp {
    #[serde(default)]
    pub epoch_millis: Option<i64>,
    #[serde(default)]
    pub friendly: Option<String>,
    #[serde(default)]
    pub iso8601: Option<String>,
    #[serde(default)]
    pub jira: Option<String>,
}

// Internal API types (not exposed in tool output)

#[derive(Debug, Deserialize)]
pub(crate) struct PagedResponse<T> {
    #[serde(default)]
    #[serde(bound(deserialize = "T: Deserialize<'de>"))]
    pub values: Vec<T>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AddCommentRequest {
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public: Option<bool>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PerformTransitionRequest {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_comment: Option<AddCommentRequest>,
}
