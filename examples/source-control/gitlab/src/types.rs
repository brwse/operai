//! GitLab API types

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Author {
    pub id: u64,
    pub username: String,
    pub name: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MergeRequestState {
    Opened,
    Closed,
    Locked,
    Merged,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MergeRequestSummary {
    pub id: u64,
    pub iid: u64,
    pub project_id: u64,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub state: MergeRequestState,
    pub created_at: String,
    pub updated_at: String,
    pub merged_at: Option<String>,
    pub closed_at: Option<String>,
    pub author: Author,
    pub source_branch: String,
    pub target_branch: String,
    pub web_url: String,
    #[serde(default)]
    pub merge_status: Option<String>,
    #[serde(default)]
    pub draft: Option<bool>,
    #[serde(default)]
    pub has_conflicts: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IssueState {
    Opened,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IssueSummary {
    pub id: u64,
    pub iid: u64,
    pub project_id: u64,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub state: IssueState,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
    pub author: Author,
    #[serde(default)]
    pub assignees: Vec<Author>,
    #[serde(default)]
    pub labels: Vec<String>,
    pub web_url: String,
    #[serde(default)]
    pub confidential: Option<bool>,
    #[serde(default)]
    pub issue_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Note {
    pub id: u64,
    pub body: String,
    pub author: Author,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateIssueRequest {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee_ids: Option<Vec<u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateMergeRequestRequest {
    pub source_branch: String,
    pub target_branch: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee_ids: Option<Vec<u64>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateNoteRequest {
    pub body: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct UpdateMergeRequestRequest {
    pub state_event: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct UpdateIssueRequest {
    pub state_event: String,
}
