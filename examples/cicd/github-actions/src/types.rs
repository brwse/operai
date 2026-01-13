//! Type definitions for GitHub Actions API.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Status of a workflow run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunStatus {
    Queued,
    InProgress,
    Completed,
    Waiting,
}

/// Conclusion of a completed workflow run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunConclusion {
    Success,
    Failure,
    Neutral,
    Cancelled,
    Skipped,
    TimedOut,
    ActionRequired,
}

/// Summary information about a workflow.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkflowSummary {
    pub id: i64,
    pub node_id: String,
    pub name: String,
    pub path: String,
    pub state: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    pub url: String,
    pub html_url: String,
    pub badge_url: String,
}

/// Summary information about a workflow run.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkflowRunSummary {
    pub id: i64,
    pub name: String,
    pub status: WorkflowRunStatus,
    #[serde(default)]
    pub conclusion: Option<WorkflowRunConclusion>,
    pub workflow_id: i64,
    pub head_branch: String,
    pub head_sha: String,
    pub run_number: i64,
    pub event: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub run_started_at: Option<String>,
    pub html_url: String,
    pub path: String,
}

/// Detailed information about a workflow run.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorkflowRunDetail {
    pub id: i64,
    pub name: String,
    pub status: WorkflowRunStatus,
    #[serde(default)]
    pub conclusion: Option<WorkflowRunConclusion>,
    pub workflow_id: i64,
    pub head_branch: String,
    pub head_sha: String,
    pub run_number: i64,
    pub event: String,
    pub display_title: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub run_started_at: Option<String>,
    pub html_url: String,
    pub path: String,
    pub run_attempt: i64,
    pub referenced_workflows: Vec<ReferencedWorkflow>,
    pub actor: GitHubUser,
    pub triggering_actor: GitHubUser,
    pub jobs_url: String,
    pub logs_url: String,
    pub check_suite_url: String,
    pub artifacts_url: String,
    pub cancel_url: String,
    pub rerun_url: String,
    pub previous_attempt_url: Option<String>,
    pub workflow_url: String,
    pub head_commit: HeadCommit,
    pub repository: MinimalRepository,
    pub head_repository: Option<MinimalRepository>,
}

/// Information about an artifact.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Artifact {
    pub id: i64,
    pub node_id: String,
    pub name: String,
    pub size_in_bytes: i64,
    #[serde(default)]
    pub url: Option<String>,
    pub archive_download_url: String,
    pub expired: bool,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Internal response structure for list workflows.
#[derive(Debug, Deserialize)]
pub(crate) struct ListWorkflowsResponse {
    pub workflows: Vec<WorkflowSummary>,
}

/// Internal response structure for list workflow runs.
#[derive(Debug, Deserialize)]
pub(crate) struct ListWorkflowRunsResponse {
    pub workflow_runs: Vec<WorkflowRunSummary>,
}

/// Internal response structure for list artifacts.
#[derive(Debug, Deserialize)]
pub(crate) struct ListArtifactsResponse {
    pub artifacts: Vec<Artifact>,
}

/// GitHub user information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitHubUser {
    pub login: String,
    pub id: i64,
    pub node_id: String,
    pub avatar_url: String,
    pub gravatar_id: String,
    pub url: String,
    pub html_url: String,
    pub followers_url: String,
    pub following_url: String,
    pub gists_url: String,
    pub starred_url: String,
    pub subscriptions_url: String,
    pub organizations_url: String,
    pub repos_url: String,
    pub events_url: String,
    pub received_events_url: String,
    pub r#type: String,
    pub site_admin: bool,
}

/// Git commit information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HeadCommit {
    pub id: String,
    pub tree_id: String,
    pub message: String,
    pub timestamp: String,
    pub author: CommitAuthor,
    pub committer: CommitAuthor,
}

/// Commit author/committer information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CommitAuthor {
    pub name: String,
    pub email: String,
}

/// Minimal repository information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MinimalRepository {
    pub id: i64,
    pub node_id: String,
    pub name: String,
    pub full_name: String,
    pub private: bool,
    pub owner: GitHubUser,
    pub html_url: String,
    pub description: Option<String>,
    pub fork: bool,
    pub url: String,
}

/// Referenced workflow in a workflow run.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReferencedWorkflow {
    pub path: String,
    pub sha: String,
    #[serde(default)]
    #[serde(rename = "ref")]
    pub r#ref: Option<String>,
}
