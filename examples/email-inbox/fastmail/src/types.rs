//! Type definitions for JMAP Email operations.

use std::collections::HashMap;

use operai::{schemars, schemars::JsonSchema};
use serde::{Deserialize, Serialize};

/// Represents an email address with optional display name.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EmailAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub email: String,
}

/// Summary information about an email message.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EmailSummary {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub from: Vec<EmailAddress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub received_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub keywords: HashMap<String, bool>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub mailbox_ids: HashMap<String, bool>,
}

/// Full email message with body content.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Email {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub from: Vec<EmailAddress>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub to: Vec<EmailAddress>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cc: Vec<EmailAddress>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bcc: Vec<EmailAddress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub received_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sent_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub html_body: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub keywords: HashMap<String, bool>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub mailbox_ids: HashMap<String, bool>,
}

/// JMAP API request structure.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JmapRequest {
    pub using: Vec<String>,
    pub method_calls: Vec<(String, serde_json::Value, String)>,
}

/// JMAP API response structure.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JmapResponse {
    pub method_responses: Vec<serde_json::Value>,
}

/// Email/query filter condition.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailQueryFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_mailbox: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
}

/// Email/query arguments.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailQueryArgs {
    pub account_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<EmailQueryFilter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// Email/get arguments.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailGetArgs {
    pub account_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Vec<String>>,
}

/// Email/set create structure.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailSetCreate {
    pub mailbox_ids: HashMap<String, bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keywords: Option<HashMap<String, bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<Vec<EmailAddress>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<Vec<EmailAddress>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cc: Option<Vec<EmailAddress>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bcc: Option<Vec<EmailAddress>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_body: Option<Vec<EmailBodyPart>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html_body: Option<Vec<EmailBodyPart>>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub body_values: HashMap<String, EmailBodyValue>,
}

/// Email body value for text/html content.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailBodyValue {
    #[serde(rename = "value")]
    pub content: String,
}

/// Email body part for creating emails.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailBodyPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_id: Option<String>,
    #[serde(rename = "type")]
    pub part_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disposition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
}

/// Email/set arguments.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailSetArgs {
    pub account_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create: Option<HashMap<String, EmailSetCreate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update: Option<HashMap<String, serde_json::Value>>,
}

/// EmailSubmission/set create structure.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailSubmissionCreate {
    pub identity_id: String,
    pub email_id: String,
}

/// EmailSubmission/set arguments.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailSubmissionSetArgs {
    pub account_id: String,
    pub create: HashMap<String, EmailSubmissionCreate>,
}

/// Response from Email/query.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailQueryResponse {
    pub ids: Vec<String>,
}

/// Response from Email/get.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailGetResponse {
    pub list: Vec<serde_json::Value>,
}

/// Response from Email/set.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailSetResponse {
    #[serde(default)]
    pub created: Option<HashMap<String, serde_json::Value>>,
}

/// Identity object for sending emails.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub email: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<Vec<EmailAddress>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bcc: Option<Vec<EmailAddress>>,
    #[serde(default)]
    pub text_signature: String,
    #[serde(default)]
    pub html_signature: String,
    #[serde(rename = "mayDelete")]
    pub may_delete: bool,
}

/// Identity/get arguments.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityGetArgs {
    pub account_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ids: Option<Vec<String>>,
}

/// Response from Identity/get.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityGetResponse {
    pub list: Vec<Identity>,
}
