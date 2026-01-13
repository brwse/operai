//! Type definitions for Azure Repos integration.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub default_branch: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub remote_url: Option<String>,
    #[serde(default)]
    pub web_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    #[serde(rename = "pullRequestId")]
    pub id: i32,
    pub repository: Option<RepositoryRef>,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub source_ref_name: String,
    pub target_ref_name: String,
    pub status: PullRequestStatus,
    #[serde(default)]
    pub created_by: Option<Identity>,
    #[serde(default)]
    pub creation_date: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum PullRequestStatus {
    NotSet,
    Active,
    Abandoned,
    Completed,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub display_name: String,
    pub unique_name: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub id: i32,
    pub content: String,
    #[serde(default)]
    pub author: Option<Identity>,
    #[serde(default)]
    pub published_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommentThread {
    pub id: i32,
    #[serde(default)]
    pub comments: Vec<Comment>,
    pub status: CommentThreadStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum CommentThreadStatus {
    Unknown,
    Active,
    Fixed,
    WontFix,
    Closed,
    ByDesign,
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Reviewer {
    pub id: String,
    pub display_name: String,
    pub vote: i32,
}
