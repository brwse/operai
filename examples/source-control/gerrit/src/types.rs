//! Type definitions for Gerrit REST API responses and requests.

use std::collections::HashMap;

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Summary information about a Gerrit change.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ChangeSummary {
    /// Change identifier in the format "project~branch~change-id".
    pub id: String,
    /// Project name.
    pub project: String,
    /// Target branch name.
    pub branch: String,
    /// Change number.
    #[serde(rename = "_number")]
    pub number: i64,
    /// Subject line (commit message first line).
    #[serde(default)]
    pub subject: Option<String>,
    /// Change status (NEW, MERGED, ABANDONED).
    #[serde(default)]
    pub status: Option<String>,
    /// Owner of the change.
    #[serde(default)]
    pub owner: Option<AccountInfo>,
    /// Last updated timestamp.
    #[serde(default)]
    pub updated: Option<String>,
    /// Whether change is mergeable.
    #[serde(default)]
    pub mergeable: Option<bool>,
}

/// Information about a Gerrit account.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AccountInfo {
    /// Numeric account ID.
    #[serde(rename = "_account_id")]
    pub account_id: i64,
    /// Full name of the account.
    #[serde(default)]
    pub name: Option<String>,
    /// Email address.
    #[serde(default)]
    pub email: Option<String>,
    /// Username.
    #[serde(default)]
    pub username: Option<String>,
}

/// Request body for posting a review.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReviewInput {
    /// Review message/comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Tag for grouping review messages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Map of label names to vote values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, i32>>,
    /// Inline comments by file path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<HashMap<String, Vec<CommentInput>>>,
    /// Whether to mark the change as ready for review.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ready: Option<bool>,
    /// Notify handling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify: Option<String>,
    /// Account ID to post review on behalf of.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_behalf_of: Option<i64>,
    /// Reviewers to add to the change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewers: Option<Vec<ReviewerInput>>,
}

/// Input for adding a reviewer.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReviewerInput {
    /// Account ID of the reviewer.
    pub reviewer: i64,
    /// Review state (e.g., "REVIEWER", "CC").
    #[serde(default)]
    pub state: Option<String>,
}

/// Range for an inline comment.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CommentRange {
    /// Start line number (1-based).
    pub start_line: i32,
    /// Start character offset (0-based).
    pub start_character: i32,
    /// End line number (1-based).
    pub end_line: i32,
    /// End character offset (0-based).
    pub end_character: i32,
}

/// Input for an inline comment.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CommentInput {
    /// File path the comment applies to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Line number the comment applies to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<i32>,
    /// Range for the comment (more precise than line).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<CommentRange>,
    /// Comment message.
    pub message: String,
    /// ID of comment being replied to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<String>,
    /// Whether this is an unresolved comment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved: Option<bool>,
}

/// Result of posting a review.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReviewResult {
    /// Map of label names to new vote values.
    #[serde(default)]
    pub labels: HashMap<String, i32>,
    /// Whether the change is ready for review.
    #[serde(default)]
    pub ready: Option<bool>,
}

/// Request body for submitting a change.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SubmitInput {
    /// Account to submit on behalf of.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_behalf_of: Option<i64>,
}

/// Request body for abandoning a change.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AbandonInput {
    /// Abandon message explaining why.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
