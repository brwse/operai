//! Type definitions for GitHub API.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Represents a GitHub issue.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    pub state: IssueState,
    pub html_url: String,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Represents a GitHub pull request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    pub state: PullRequestState,
    pub html_url: String,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub head: Option<PullRequestRef>,
    #[serde(default)]
    pub base: Option<PullRequestRef>,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub mergeable: Option<bool>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Represents a GitHub comment.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Comment {
    pub id: u64,
    pub body: String,
    #[serde(default)]
    pub user: Option<User>,
    pub html_url: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Represents a GitHub user.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct User {
    pub login: String,
    pub id: u64,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
}

/// Represents a GitHub label.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Label {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Represents a GitHub pull request ref (head or base).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PullRequestRef {
    pub ref_field: String,
    pub sha: String,
    #[serde(default)]
    pub label: Option<String>,
}

/// Issue state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum IssueState {
    Open,
    Closed,
}

/// Pull request state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PullRequestState {
    Open,
    Closed,
}

/// Search filter for issues and pull requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SearchFilter {
    Issue,
    Pr,
    All,
}

// Internal API response types from Octocrab

#[derive(Debug, Deserialize)]
pub(crate) struct OctoIssue {
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    pub state: String,
    pub html_url: String,
    #[serde(default)]
    pub user: Option<OctoUser>,
    #[serde(default)]
    pub labels: Vec<OctoLabel>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub pull_request: Option<OctoPullRequestMarker>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OctoPullRequest {
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    pub state: String,
    pub html_url: String,
    #[serde(default)]
    pub user: Option<OctoUser>,
    #[serde(default)]
    pub head: Option<OctoPullRequestRef>,
    #[serde(default)]
    pub base: Option<OctoPullRequestRef>,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub mergeable: Option<bool>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OctoComment {
    pub id: u64,
    pub body: String,
    #[serde(default)]
    pub user: Option<OctoUser>,
    pub html_url: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OctoUser {
    pub login: String,
    pub id: u64,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OctoLabel {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OctoPullRequestRef {
    #[serde(rename = "ref")]
    pub ref_field: String,
    pub sha: String,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OctoPullRequestMarker {}

// Conversion functions

pub(crate) fn map_user(user: OctoUser) -> User {
    User {
        login: user.login,
        id: user.id,
        avatar_url: user.avatar_url,
        html_url: user.html_url,
    }
}

pub(crate) fn map_label(label: OctoLabel) -> Label {
    Label {
        name: label.name,
        color: label.color,
        description: label.description,
    }
}

pub(crate) fn map_pull_request_ref(pr_ref: OctoPullRequestRef) -> PullRequestRef {
    PullRequestRef {
        ref_field: pr_ref.ref_field,
        sha: pr_ref.sha,
        label: pr_ref.label,
    }
}

pub(crate) fn map_issue(issue: OctoIssue) -> Issue {
    let state = match issue.state.as_str() {
        "closed" => IssueState::Closed,
        _ => IssueState::Open,
    };

    Issue {
        number: issue.number,
        title: issue.title,
        body: issue.body,
        state,
        html_url: issue.html_url,
        user: issue.user.map(map_user),
        labels: issue.labels.into_iter().map(map_label).collect(),
        created_at: issue.created_at,
        updated_at: issue.updated_at,
    }
}

pub(crate) fn map_pull_request(pr: OctoPullRequest) -> PullRequest {
    let state = match pr.state.as_str() {
        "closed" => PullRequestState::Closed,
        _ => PullRequestState::Open,
    };

    PullRequest {
        number: pr.number,
        title: pr.title,
        body: pr.body,
        state,
        html_url: pr.html_url,
        user: pr.user.map(map_user),
        head: pr.head.map(map_pull_request_ref),
        base: pr.base.map(map_pull_request_ref),
        draft: pr.draft,
        mergeable: pr.mergeable,
        created_at: pr.created_at,
        updated_at: pr.updated_at,
    }
}

pub(crate) fn map_comment(comment: OctoComment) -> Comment {
    Comment {
        id: comment.id,
        body: comment.body,
        user: comment.user.map(map_user),
        html_url: comment.html_url,
        created_at: comment.created_at,
        updated_at: comment.updated_at,
    }
}
