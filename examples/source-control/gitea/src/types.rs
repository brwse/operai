//! Type definitions for Gitea API responses and requests.

use serde::{Deserialize, Serialize};

/// Repository information from Gitea API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub private: bool,
    pub fork: bool,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub clone_url: Option<String>,
    #[serde(default)]
    pub ssh_url: Option<String>,
    #[serde(default)]
    pub default_branch: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Pull request information from Gitea API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub id: u64,
    pub number: u64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub head: Option<PRBranchInfo>,
    #[serde(default)]
    pub base: Option<PRBranchInfo>,
    #[serde(default)]
    pub mergeable: Option<bool>,
    #[serde(default)]
    pub merged: Option<bool>,
    #[serde(default)]
    pub merged_at: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// User information from Gitea API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub login: String,
    #[serde(default)]
    pub full_name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

/// Pull request branch information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PRBranchInfo {
    #[serde(rename = "ref")]
    pub ref_name: String,
    #[serde(default)]
    pub sha: Option<String>,
    #[serde(default)]
    pub repo: Option<Repository>,
}

/// Comment on an issue or pull request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: u64,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Pull request review information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub id: u64,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub submitted_at: Option<String>,
}

/// Request payload for creating a pull request.
#[derive(Debug, Serialize)]
pub struct CreatePullRequestRequest {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub head: String,
    pub base: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignees: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestone: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<u64>>,
}

/// Request payload for creating a comment.
#[derive(Debug, Serialize)]
pub struct CreateCommentRequest {
    pub body: String,
}

/// Request payload for approving or requesting changes to a PR.
#[derive(Debug, Serialize)]
pub struct CreateReviewRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub event: String, // APPROVED, REQUEST_CHANGES, COMMENT
}

/// Request payload for merging a pull request.
#[derive(Debug, Serialize)]
pub struct MergePullRequestRequest {
    #[serde(rename = "Do")]
    pub merge_method: String, // merge, rebase, rebase-merge, squash
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "MergeMessageField")]
    pub merge_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "MergeTitleField")]
    pub merge_title: Option<String>,
}

/// Response from merging a pull request.
#[derive(Debug, Deserialize)]
pub struct MergePullRequestResponse {
    pub merged: bool,
}
