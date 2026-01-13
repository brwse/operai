//! Type definitions for Bitbucket API responses and requests.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Bitbucket user representation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct User {
    pub display_name: String,
    pub uuid: String,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub account_id: Option<String>,
}

/// Bitbucket repository representation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Repository {
    pub uuid: String,
    pub name: String,
    pub full_name: String,
}

/// Bitbucket branch representation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Branch {
    pub name: String,
}

/// Pull request state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum PullRequestState {
    Open,
    Merged,
    Declined,
    Superseded,
}

/// Pull request representation (summary)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PullRequestSummary {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub state: PullRequestState,
    pub author: User,
    pub source: BranchInfo,
    pub destination: BranchInfo,
    pub created_on: String,
    pub updated_on: String,
    #[serde(default)]
    pub comment_count: Option<u32>,
    #[serde(default)]
    pub task_count: Option<u32>,
}

/// Branch information in pull request
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BranchInfo {
    pub branch: Branch,
    pub repository: Repository,
}

/// Participant role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum ParticipantRole {
    Reviewer,
    Participant,
}

/// Participant in a pull request
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Participant {
    pub user: User,
    pub role: ParticipantRole,
    pub approved: bool,
    #[serde(default)]
    pub participated_on: Option<String>,
}

/// Paginated response from Bitbucket API
#[derive(Debug, Deserialize)]
pub struct PaginatedResponse<T> {
    pub values: Vec<T>,
}

/// Internal representation of a pull request from Bitbucket API
#[derive(Debug, Deserialize)]
pub(crate) struct BitbucketPullRequest {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub state: PullRequestState,
    pub author: User,
    pub source: BranchInfo,
    pub destination: BranchInfo,
    pub created_on: String,
    pub updated_on: String,
    #[serde(default)]
    pub comment_count: Option<u32>,
    #[serde(default)]
    pub task_count: Option<u32>,
    #[serde(default)]
    pub participants: Vec<Participant>,
}

/// Internal representation of a comment from Bitbucket API
#[derive(Debug, Deserialize)]
pub(crate) struct BitbucketComment {
    pub id: u64,
    pub created_on: String,
}

/// Request body for creating a pull request
#[derive(Debug, Serialize)]
pub(crate) struct CreatePullRequestRequest {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub source: BranchRef,
    pub destination: BranchRef,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reviewers: Vec<ReviewerRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub close_source_branch: Option<bool>,
}

/// Branch reference for creating a pull request
#[derive(Debug, Serialize)]
pub(crate) struct BranchRef {
    pub branch: BranchName,
}

/// Branch name
#[derive(Debug, Serialize)]
pub(crate) struct BranchName {
    pub name: String,
}

/// Reviewer reference for creating a pull request
#[derive(Debug, Serialize)]
pub(crate) struct ReviewerRef {
    pub uuid: String,
}

/// Request body for creating a comment
#[derive(Debug, Serialize)]
pub(crate) struct CreateCommentRequest {
    pub content: CommentContentInput,
}

/// Comment content for input
#[derive(Debug, Serialize)]
pub(crate) struct CommentContentInput {
    pub raw: String,
}

/// Request body for merging a pull request
#[derive(Debug, Serialize)]
pub(crate) struct MergePullRequestRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub close_source_branch: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_strategy: Option<String>,
}

/// Request body for declining a pull request
#[derive(Debug, Serialize)]
pub(crate) struct DeclinePullRequestRequest {}
