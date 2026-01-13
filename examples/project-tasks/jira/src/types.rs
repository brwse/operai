//! Type definitions for Jira API responses and requests.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Jira issue summary for search results
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IssueSummary {
    pub id: String,
    pub key: String,
    #[serde(default)]
    pub fields: Option<IssueFieldsSummary>,
}

/// Simplified issue fields for summary view
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IssueFieldsSummary {
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub status: Option<StatusInfo>,
    #[serde(default)]
    pub issuetype: Option<IssueType>,
    #[serde(default)]
    pub priority: Option<Priority>,
    #[serde(default)]
    pub assignee: Option<User>,
    #[serde(default)]
    pub reporter: Option<User>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
}

/// Full issue details
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Issue {
    pub id: String,
    pub key: String,
    #[serde(default)]
    pub fields: Option<IssueFields>,
}

/// Full issue fields
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IssueFields {
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: Option<StatusInfo>,
    #[serde(default)]
    pub issuetype: Option<IssueType>,
    #[serde(default)]
    pub priority: Option<Priority>,
    #[serde(default)]
    pub assignee: Option<User>,
    #[serde(default)]
    pub reporter: Option<User>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub comment: Option<CommentContainer>,
}

/// Status information
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StatusInfo {
    pub name: String,
    #[serde(default)]
    pub id: Option<String>,
}

/// Issue type information
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IssueType {
    pub name: String,
    #[serde(default)]
    pub id: Option<String>,
}

/// Priority information
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Priority {
    pub name: String,
    #[serde(default)]
    pub id: Option<String>,
}

/// User information
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct User {
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub email_address: Option<String>,
}

/// Comment container
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CommentContainer {
    #[serde(default)]
    pub comments: Vec<Comment>,
}

/// Comment details
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Comment {
    pub id: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub author: Option<User>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
}

/// Create issue request fields
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateIssueFields {
    pub project: ProjectReference,
    pub summary: String,
    pub issuetype: IssueTypeReference,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<PriorityReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<UserReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
}

/// Project reference for creation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectReference {
    pub key: String,
}

/// Issue type reference for creation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IssueTypeReference {
    pub name: String,
}

/// Priority reference for creation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PriorityReference {
    pub name: String,
}

/// User reference for assignment
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserReference {
    pub account_id: String,
}

/// Jira API search response
#[derive(Debug, Deserialize)]
pub struct SearchResponse {
    #[serde(default)]
    pub issues: Vec<IssueSummary>,
    #[serde(default)]
    pub total: Option<i64>,
}
