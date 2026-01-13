//! Type definitions for CircleCI API v2.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Status of a pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PipelineState {
    Created,
    Errored,
    SetupPending,
    Setup,
    Pending,
}

/// Status of a workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStatus {
    Success,
    Running,
    NotRun,
    Failed,
    Error,
    Failing,
    OnHold,
    Canceled,
    Unauthorized,
}

/// Status of a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Success,
    Running,
    NotRun,
    Failed,
    Retried,
    Queued,
    NotRunning,
    InfrastructureFail,
    Timedout,
    OnHold,
    Terminated,
    Blocked,
    Canceled,
    Unauthorized,
}

/// VCS commit information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Commit {
    pub subject: String,
    pub body: String,
}

/// VCS information for a pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Vcs {
    pub provider_name: String,
    pub target_repository_url: String,
    pub branch: Option<String>,
    pub review_id: Option<String>,
    pub review_url: Option<String>,
    pub revision: String,
    pub tag: Option<String>,
    pub commit: Option<Commit>,
    pub origin_repository_url: String,
}

/// Actor information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Actor {
    pub login: String,
    pub avatar_url: String,
}

/// Pipeline trigger information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Trigger {
    #[serde(rename = "type")]
    pub trigger_type: String,
    pub received_at: String,
    pub actor: Actor,
}

/// Pipeline summary returned from API.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Pipeline {
    pub id: String,
    pub project_slug: String,
    pub number: i64,
    pub state: PipelineState,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub vcs: Option<Vcs>,
    pub trigger: Trigger,
    pub errors: Vec<PipelineError>,
}

/// Error in pipeline configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PipelineError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

/// Workflow summary.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub project_slug: String,
    pub pipeline_id: String,
    pub pipeline_number: i64,
    pub status: WorkflowStatus,
    pub created_at: String,
    pub stopped_at: Option<String>,
}

/// Job details with additional information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobDetails {
    pub id: String,
    pub name: String,
    pub project_slug: String,
    #[serde(rename = "job_number")]
    pub number: Option<i64>,
    pub status: JobStatus,
    pub started_at: Option<String>,
    pub stopped_at: Option<String>,
    #[serde(rename = "type")]
    pub type_: String,
    pub web_url: String,
    pub organization: JobOrganization,
    pub pipeline: JobPipeline,
    /// Parallel runs information
    #[serde(default)]
    pub parallel_runs: Vec<ParallelRun>,
    /// Job messages
    #[serde(default)]
    pub messages: Vec<JobMessage>,
    /// Contexts used by the job
    #[serde(default)]
    pub contexts: Vec<JobContext>,
    /// Queued timestamp
    #[serde(rename = "queued_at", default)]
    pub queued_at: Option<String>,
    /// Project details
    pub project: Option<JobProject>,
    /// Latest workflow info
    #[serde(rename = "latest_workflow")]
    pub latest_workflow: Option<JobLatestWorkflow>,
    /// Executor information
    pub executor: Option<JobExecutor>,
    /// Duration in seconds
    pub duration: Option<i64>,
    /// Created timestamp
    #[serde(default)]
    pub created_at: Option<String>,
}

/// Parallel run information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ParallelRun {
    pub index: i64,
    pub status: String,
}

/// Job message.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobMessage {
    pub r#type: String,
    pub message: String,
    pub reason: Option<String>,
}

/// Job context.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobContext {
    pub name: String,
}

/// Job project details.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobProject {
    pub id: String,
    pub slug: String,
    pub name: String,
    #[serde(rename = "external_url")]
    pub external_url: String,
}

/// Latest workflow in job response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobLatestWorkflow {
    pub id: String,
    pub name: String,
}

/// Job executor information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobExecutor {
    #[serde(rename = "resource_class")]
    pub resource_class: String,
    pub r#type: String,
}

/// Organization details in job response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobOrganization {
    pub name: String,
}

/// Pipeline details in job response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobPipeline {
    pub id: String,
}

/// API response for triggering a pipeline.
#[derive(Debug, Deserialize)]
pub(crate) struct TriggerPipelineResponse {
    pub id: String,
    pub number: i64,
    pub state: PipelineState,
    pub created_at: String,
}

/// API response for getting pipeline workflows.
#[derive(Debug, Deserialize)]
pub(crate) struct WorkflowsResponse {
    pub items: Vec<Workflow>,
}

/// API response for rerunning a workflow.
#[derive(Debug, Deserialize)]
pub(crate) struct RerunWorkflowResponse {
    pub workflow_id: String,
}
