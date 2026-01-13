//! Type definitions for Aircall API responses and requests.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Call direction indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CallDirection {
    Inbound,
    Outbound,
}

/// Call status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CallStatus {
    Initial,
    Ringing,
    Answered,
    Done,
    Abandoned,
}

/// User information within a call.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct User {
    pub id: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
}

/// Team information within a call.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Team {
    pub id: i64,
    #[serde(default)]
    pub name: Option<String>,
}

/// Contact information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Contact {
    pub id: i64,
    #[serde(default)]
    pub first_name: Option<String>,
    #[serde(default)]
    pub last_name: Option<String>,
    #[serde(default)]
    pub company_name: Option<String>,
    #[serde(default)]
    pub emails: Vec<String>,
}

/// Aircall call object (summary for listings).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CallSummary {
    pub id: i64,
    pub direct_link: String,
    #[serde(default)]
    pub started_at: Option<i64>,
    #[serde(default)]
    pub answered_at: Option<i64>,
    #[serde(default)]
    pub ended_at: Option<i64>,
    #[serde(default)]
    pub duration: Option<i64>,
    #[serde(default)]
    pub direction: Option<CallDirection>,
    #[serde(default)]
    pub status: Option<CallStatus>,
    #[serde(default)]
    pub raw_digits: Option<String>,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub assigned_to: Option<User>,
    #[serde(default)]
    pub teams: Vec<Team>,
    #[serde(default)]
    pub contact: Option<Contact>,
    #[serde(default)]
    pub comments: Vec<Comment>,
    #[serde(default)]
    pub tags: Vec<Tag>,
}

/// Full call object with recording links.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Call {
    pub id: i64,
    pub direct_link: String,
    #[serde(default)]
    pub started_at: Option<i64>,
    #[serde(default)]
    pub answered_at: Option<i64>,
    #[serde(default)]
    pub ended_at: Option<i64>,
    #[serde(default)]
    pub duration: Option<i64>,
    #[serde(default)]
    pub direction: Option<CallDirection>,
    #[serde(default)]
    pub status: Option<CallStatus>,
    #[serde(default)]
    pub raw_digits: Option<String>,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub assigned_to: Option<User>,
    #[serde(default)]
    pub teams: Vec<Team>,
    #[serde(default)]
    pub contact: Option<Contact>,
    #[serde(default)]
    pub comments: Vec<Comment>,
    #[serde(default)]
    pub tags: Vec<Tag>,
    #[serde(default)]
    pub recording: Option<String>,
    #[serde(default)]
    pub recording_short_url: Option<String>,
    #[serde(default)]
    pub voicemail: Option<String>,
    #[serde(default)]
    pub voicemail_short_url: Option<String>,
    #[serde(default)]
    pub asset: Option<String>,
}

/// Comment on a call.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Comment {
    pub id: i64,
    pub content: String,
    #[serde(default)]
    pub posted_at: Option<i64>,
    #[serde(default)]
    pub posted_by: Option<User>,
}

/// Tag on a call.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Tag {
    pub id: i64,
    pub name: String,
}

/// Pagination metadata from Aircall API.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Meta {
    #[serde(default)]
    pub total: Option<i64>,
    #[serde(default)]
    pub count: Option<i64>,
    #[serde(default)]
    pub current_page: Option<i64>,
    #[serde(default)]
    pub per_page: Option<i64>,
    #[serde(default)]
    pub next_page_link: Option<String>,
    #[serde(default)]
    pub previous_page_link: Option<String>,
}
