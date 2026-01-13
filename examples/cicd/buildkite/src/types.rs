use std::collections::HashMap;

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Build state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum BuildState {
    Scheduled,
    Running,
    Passed,
    Failed,
    Failing,
    Blocked,
    Canceled,
    Canceling,
    Skipped,
    NotRun,
}

/// Annotation style enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AnnotationStyle {
    Success,
    Info,
    Warning,
    Error,
}

/// Job state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Scheduled,
    Assigned,
    Accepted,
    Running,
    Passed,
    Failed,
    Canceled,
    Skipped,
    Broken,
    #[serde(rename = "timed_out")]
    TimedOut,
    Waiting,
    WaitingFailed,
    Blocked,
    Unblocked,
    Limiting,
}

/// Author information for builds
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Author {
    pub name: String,
    pub email: String,
}

/// Build object returned by the API
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Build {
    pub id: String,
    pub number: u64,
    pub state: BuildState,
    pub message: Option<String>,
    pub commit: String,
    pub branch: String,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub jobs: Vec<Job>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub web_url: Option<String>,
}

/// Job object within a build
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Job {
    pub id: String,
    #[serde(rename = "type")]
    pub job_type: Option<String>,
    pub name: Option<String>,
    pub state: Option<JobState>,
    #[serde(default)]
    pub log_url: Option<String>,
    #[serde(default)]
    pub raw_log_url: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
}

/// Annotation object
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Annotation {
    pub id: String,
    pub context: Option<String>,
    pub style: Option<AnnotationStyle>,
    pub body_html: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Job log response
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobLog {
    pub url: String,
    pub content: String,
    pub size: u64,
    #[serde(default)]
    pub header_times: Vec<String>,
}

/// Internal request for creating a build
#[derive(Debug, Serialize)]
pub(crate) struct CreateBuildRequest {
    pub commit: String,
    pub branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<Author>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta_data: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clean_checkout: Option<bool>,
}

/// Internal request for creating an annotation
#[derive(Debug, Serialize)]
pub(crate) struct CreateAnnotationRequest {
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<AnnotationStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub append: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
}
