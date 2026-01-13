//! cicd/circleci integration for Operai Toolbox.

mod types;

use operai::{
    Context, JsonSchema, Result, define_system_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
use types::{
    JobDetails, Pipeline, RerunWorkflowResponse, TriggerPipelineResponse, Workflow,
    WorkflowsResponse,
};

define_system_credential! {
    CircleCiCredential("circleci") {
        api_key: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_API_ENDPOINT: &str = "https://circleci.com/api/v2";

#[init]
async fn setup() -> Result<()> {
    info!("CircleCI integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("CircleCI integration shutting down");
}

// ============================================================================
// Tools
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TriggerPipelineInput {
    /// Project slug in the format: vcs-slug/org-name/repo-name (e.g.,
    /// "gh/myorg/myrepo")
    pub project_slug: String,
    /// Branch to build (optional, defaults to project's default branch)
    #[serde(default)]
    pub branch: Option<String>,
    /// Tag to build (optional)
    #[serde(default)]
    pub tag: Option<String>,
    /// Pipeline parameters as JSON key-value pairs (optional)
    #[serde(default)]
    pub parameters: Option<std::collections::HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TriggerPipelineOutput {
    pub pipeline_id: String,
    pub pipeline_number: u64,
    pub state: types::PipelineState,
    pub created_at: String,
}

/// # Trigger CircleCI Pipeline
///
/// Triggers a new pipeline run for a CircleCI project using the CircleCI
/// API v2.
///
/// Use this tool when a user wants to start a new CI/CD pipeline build in
/// CircleCI. This is commonly used to:
/// - Deploy code to staging or production environments
/// - Run automated tests on a specific branch or tag
/// - Trigger release builds with custom pipeline parameters
/// - Start workflows manually instead of waiting for webhooks or git pushes
///
/// ## Key Inputs
/// - **project_slug**: Must be in format "vcs-slug/org-name/repo-name" (e.g.,
///   "gh/myorg/myrepo")
/// - **branch**: Optional specific branch to build (defaults to project's
///   default branch)
/// - **tag**: Optional git tag to build (mutually exclusive with branch)
/// - **parameters**: Optional JSON key-value pairs for pipeline variables
///
/// ## Constraints
/// - Branch and tag parameters are mutually exclusive - you cannot specify both
/// - The project must exist in CircleCI and be accessible with the configured
///   credentials
/// - Returns pipeline ID, number, state, and creation timestamp for tracking
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - cicd
/// - circleci
/// - pipeline
///
/// # Errors
///
/// This function will return an error if:
/// - The provided `project_slug` is empty or contains only whitespace
/// - The CircleCI credential is not configured or the `API` key is empty
/// - The HTTP request to the CircleCI `API` fails (network errors, timeouts,
///   etc.)
/// - The CircleCI `API` returns a non-success status code (e.g., 404 for
///   project not found, 401 for authentication failures)
/// - The response body cannot be parsed as JSON
#[tool]
pub async fn trigger_pipeline(
    ctx: Context,
    input: TriggerPipelineInput,
) -> Result<TriggerPipelineOutput> {
    ensure!(
        !input.project_slug.trim().is_empty(),
        "project_slug must not be empty"
    );

    let client = CircleCiClient::from_ctx(&ctx)?;

    let mut body = serde_json::Map::new();

    // Branch and tag are mutually exclusive according to API spec
    if let Some(branch) = &input.branch {
        body.insert(
            "branch".to_string(),
            serde_json::Value::String(branch.clone()),
        );
    } else if let Some(tag) = &input.tag {
        body.insert("tag".to_string(), serde_json::Value::String(tag.clone()));
    }

    // Add parameters if provided
    if let Some(params) = &input.parameters {
        body.insert("parameters".to_string(), serde_json::to_value(params)?);
    }

    let response: TriggerPipelineResponse = client
        .post_json(&format!("/project/{}/pipeline", input.project_slug), &body)
        .await?;

    Ok(TriggerPipelineOutput {
        pipeline_id: response.id,
        pipeline_number: u64::try_from(response.number).unwrap_or(0),
        state: response.state,
        created_at: response.created_at,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetPipelineStatusInput {
    /// Pipeline ID (UUID)
    pub pipeline_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetPipelineStatusOutput {
    pub pipeline: Pipeline,
    pub workflows: Vec<Workflow>,
}

/// # Get CircleCI Pipeline Status
///
/// Retrieves detailed status information about a CircleCI pipeline and all
/// its associated workflows.
///
/// Use this tool when a user wants to check the progress, state, or results of
/// a pipeline run. This is essential for:
/// - Monitoring the progress of running CI/CD builds
/// - Determining if a deployment pipeline succeeded or failed
/// - Getting detailed information about workflows within a pipeline
/// - Troubleshooting failed builds by examining workflow states
///
/// ## Key Inputs
/// - **pipeline_id**: The UUID of the pipeline to query (obtained from
///   `trigger_pipeline` or CircleCI dashboard)
///
/// ## Outputs
/// - **Pipeline details**: ID, project slug, number, state, creation timestamp,
///   trigger information
/// - **Workflows list**: All workflows associated with this pipeline including
///   their IDs, names, statuses
///
/// ## Constraints
/// - The pipeline must exist and be accessible with configured credentials
/// - Returns comprehensive workflow details that can be used to track
///   individual job progress
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - cicd
/// - circleci
/// - pipeline
/// - status
///
/// # Errors
///
/// This function will return an error if:
/// - The provided `pipeline_id` is empty or contains only whitespace
/// - The CircleCI credential is not configured or the `API` key is empty
/// - The HTTP request to the CircleCI `API` fails (network errors, timeouts,
///   etc.)
/// - The CircleCI `API` returns a non-success status code (e.g., 404 for
///   pipeline not found, 401 for authentication failures)
/// - The response body cannot be parsed as JSON
#[tool]
pub async fn get_pipeline_status(
    ctx: Context,
    input: GetPipelineStatusInput,
) -> Result<GetPipelineStatusOutput> {
    ensure!(
        !input.pipeline_id.trim().is_empty(),
        "pipeline_id must not be empty"
    );

    let client = CircleCiClient::from_ctx(&ctx)?;

    let pipeline: Pipeline = client
        .get_json(&format!("/pipeline/{}", input.pipeline_id))
        .await?;

    let workflows_response: WorkflowsResponse = client
        .get_json(&format!("/pipeline/{}/workflow", input.pipeline_id))
        .await?;

    Ok(GetPipelineStatusOutput {
        pipeline,
        workflows: workflows_response.items,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetJobLogsInput {
    /// Project slug in the format: vcs-slug/org-name/repo-name
    pub project_slug: String,
    /// Job number
    pub job_number: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetJobLogsOutput {
    pub job: JobDetails,
    pub log_url: String,
}

/// # Get CircleCI Job Logs
///
/// Retrieves job details and a log URL for a specific CircleCI job using the
/// CircleCI API v2.
///
/// Use this tool when a user wants to access logs or details from a completed
/// or running job. This is useful for:
/// - Debugging failed builds by accessing job logs
/// - Viewing the output of specific jobs in a workflow
/// - Getting job metadata (status, duration, executor info)
/// - Obtaining the web URL to view full logs in the CircleCI dashboard
///
/// ## Key Inputs
/// - **project_slug**: Must be in format "vcs-slug/org-name/repo-name" (e.g.,
///   "gh/myorg/myrepo")
/// - **job_number**: The numeric job identifier (not the job ID)
///
/// ## Outputs
/// - **Job details**: Name, status, duration, executor type, timestamps,
///   project info
/// - **`log_url`**: Web URL to view the full job logs in the CircleCI dashboard
///
/// ## Constraints
/// - CircleCI API v2 does not return direct log content - only a web URL is
///   provided
/// - The job must exist and be accessible with configured credentials
/// - Job number must be greater than 0
/// - Use the web URL to view or download the actual log contents
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - cicd
/// - circleci
/// - job
/// - logs
///
/// # Errors
///
/// This function will return an error if:
/// - The provided `project_slug` is empty or contains only whitespace
/// - The provided `job_number` is zero
/// - The CircleCI credential is not configured or the `API` key is empty
/// - The HTTP request to the CircleCI `API` fails (network errors, timeouts,
///   etc.)
/// - The CircleCI `API` returns a non-success status code (e.g., 404 for job
///   not found, 401 for authentication failures)
/// - The response body cannot be parsed as JSON
#[tool]
pub async fn get_job_logs(ctx: Context, input: GetJobLogsInput) -> Result<GetJobLogsOutput> {
    ensure!(
        !input.project_slug.trim().is_empty(),
        "project_slug must not be empty"
    );
    ensure!(input.job_number > 0, "job_number must be greater than 0");

    let client = CircleCiClient::from_ctx(&ctx)?;

    let job: JobDetails = client
        .get_json(&format!(
            "/project/{}/job/{}",
            input.project_slug, input.job_number
        ))
        .await?;

    // CircleCI doesn't provide direct log content in API v2, but provides web_url
    let log_url = job.web_url.clone();

    Ok(GetJobLogsOutput { job, log_url })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RerunJobInput {
    /// Project slug in the format: vcs-slug/org-name/repo-name
    pub project_slug: String,
    /// Job number to rerun
    pub job_number: u64,
    /// Whether to rerun from failed jobs only
    #[serde(default)]
    pub from_failed: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RerunJobOutput {
    pub success: bool,
    pub message: String,
}

/// # Rerun CircleCI Job
///
/// Reruns a CircleCI job or workflow using the CircleCI API v2.
///
/// Use this tool when a user wants to retry a failed job or re-run a workflow.
/// This is commonly needed for:
/// - Retrying failed builds after fixing configuration issues
/// - Re-running workflows from failed jobs to skip already-successful jobs
/// - Re-triggering jobs without pushing new commits
/// - Recovering from transient infrastructure failures
///
/// ## Key Inputs
/// - **project_slug**: Must be in format "vcs-slug/org-name/repo-name" (e.g.,
///   "gh/myorg/myrepo")
/// - **job_number**: The numeric job identifier to rerun
/// - **from_failed**: If true, only reruns failed jobs in the workflow
///   (efficient for large workflows)
///
/// ## How It Works
/// - Finds the workflow associated with the given job
/// - Triggers a workflow rerun via CircleCI API
/// - When `from_failed=true`, only failed jobs in the workflow are re-executed
/// - Returns the new workflow ID for tracking the rerun
///
/// ## Constraints
/// - Job number must be greater than 0
/// - The job and workflow must exist and be accessible
/// - A new workflow is created (the original workflow remains unchanged)
/// - Rerun is triggered at the workflow level, not individual job level
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - cicd
/// - circleci
/// - job
/// - rerun
///
/// # Errors
///
/// This function will return an error if:
/// - The provided `project_slug` is empty or contains only whitespace
/// - The provided `job_number` is zero
/// - The CircleCI credential is not configured or the `API` key is empty
/// - The HTTP request to the CircleCI `API` fails (network errors, timeouts,
///   etc.)
/// - The CircleCI `API` returns a non-success status code (e.g., 404 for job
///   not found, 401 for authentication failures)
/// - No workflows are found for the job's pipeline
/// - The response body cannot be parsed as JSON
#[tool]
pub async fn rerun_job(ctx: Context, input: RerunJobInput) -> Result<RerunJobOutput> {
    ensure!(
        !input.project_slug.trim().is_empty(),
        "project_slug must not be empty"
    );
    ensure!(input.job_number > 0, "job_number must be greater than 0");

    let client = CircleCiClient::from_ctx(&ctx)?;

    // Get job details first to get the workflow ID
    let job: JobDetails = client
        .get_json(&format!(
            "/project/{}/job/{}",
            input.project_slug, input.job_number
        ))
        .await?;

    // Get workflows for this pipeline to find the workflow ID
    let workflows: WorkflowsResponse = client
        .get_json(&format!("/pipeline/{}/workflow", job.pipeline.id))
        .await?;

    ensure!(
        !workflows.items.is_empty(),
        "No workflows found for this job"
    );

    let workflow_id = &workflows.items[0].id;

    // Rerun the workflow with the from_failed parameter
    let endpoint = format!("/workflow/{workflow_id}/rerun");

    let mut body = serde_json::Map::new();
    body.insert(
        "from_failed".to_string(),
        serde_json::Value::Bool(input.from_failed),
    );

    let response: RerunWorkflowResponse = client.post_json(&endpoint, &body).await?;

    Ok(RerunJobOutput {
        success: true,
        message: format!(
            "Workflow rerun started. New workflow ID: {}",
            response.workflow_id
        ),
    })
}

// ============================================================================
// HTTP Client
// ============================================================================

#[derive(Debug, Clone)]
struct CircleCiClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl CircleCiClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = CircleCiCredential::get(ctx)?;
        ensure!(!cred.api_key.trim().is_empty(), "api_key must not be empty");

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_API_ENDPOINT))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            api_key: cred.api_key,
        })
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .get(&url)
            .header("Circle-Token", &self.api_key)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        self.handle_response(response).await
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &TReq,
    ) -> Result<TRes> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .post(&url)
            .header("Circle-Token", &self.api_key)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::ACCEPT, "application/json")
            .json(body)
            .send()
            .await?;

        self.handle_response(response).await
    }

    async fn handle_response<T: for<'de> Deserialize<'de>>(
        &self,
        response: reqwest::Response,
    ) -> Result<T> {
        let status = response.status();
        if status.is_success() {
            Ok(response.json::<T>().await?)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "CircleCI API request failed ({status}): {body}"
            ))
        }
    }
}

fn normalize_base_url(endpoint: &str) -> Result<String> {
    let trimmed = endpoint.trim();
    ensure!(!trimmed.is_empty(), "endpoint must not be empty");
    Ok(trimmed.trim_end_matches('/').to_string())
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use operai::Context;
    use types::{JobStatus, PipelineState, WorkflowStatus};
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut circleci_values = HashMap::new();
        circleci_values.insert("api_key".to_string(), "test-token".to_string());
        circleci_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_system_credential("circleci", circleci_values)
    }

    // --- Serialization tests ---

    #[test]
    fn test_pipeline_state_serialization_roundtrip() {
        for variant in [
            PipelineState::Created,
            PipelineState::Errored,
            PipelineState::SetupPending,
            PipelineState::Setup,
            PipelineState::Pending,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: PipelineState = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_workflow_status_serialization_roundtrip() {
        for variant in [
            WorkflowStatus::Success,
            WorkflowStatus::Running,
            WorkflowStatus::Failed,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: WorkflowStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_job_status_serialization_roundtrip() {
        for variant in [JobStatus::Success, JobStatus::Running, JobStatus::Failed] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: JobStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://circleci.com/api/v2/").unwrap();
        assert_eq!(result, "https://circleci.com/api/v2");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://circleci.com/api/v2  ").unwrap();
        assert_eq!(result, "https://circleci.com/api/v2");
    }

    #[test]
    fn test_normalize_base_url_empty_returns_error() {
        let result = normalize_base_url("");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must not be empty")
        );
    }

    // --- Input validation tests ---

    #[tokio::test]
    async fn test_trigger_pipeline_empty_project_slug_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = trigger_pipeline(
            ctx,
            TriggerPipelineInput {
                project_slug: "  ".to_string(),
                branch: None,
                tag: None,
                parameters: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("project_slug must not be empty")
        );
    }

    #[tokio::test]
    async fn test_get_pipeline_status_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = get_pipeline_status(
            ctx,
            GetPipelineStatusInput {
                pipeline_id: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("pipeline_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_get_job_logs_empty_project_slug_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = get_job_logs(
            ctx,
            GetJobLogsInput {
                project_slug: "  ".to_string(),
                job_number: 123,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("project_slug must not be empty")
        );
    }

    #[tokio::test]
    async fn test_get_job_logs_zero_job_number_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = get_job_logs(
            ctx,
            GetJobLogsInput {
                project_slug: "gh/org/repo".to_string(),
                job_number: 0,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("job_number must be greater than 0")
        );
    }

    #[tokio::test]
    async fn test_rerun_job_empty_project_slug_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = rerun_job(
            ctx,
            RerunJobInput {
                project_slug: "  ".to_string(),
                job_number: 123,
                from_failed: false,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("project_slug must not be empty")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_trigger_pipeline_success() {
        let server = MockServer::start().await;

        let response_body = r#"{
            "id": "pipeline-123",
            "number": 42,
            "state": "pending",
            "created_at": "2024-01-01T00:00:00Z"
        }"#;

        Mock::given(method("POST"))
            .and(path("/project/gh/myorg/myrepo/pipeline"))
            .and(header("Circle-Token", "test-token"))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = trigger_pipeline(
            ctx,
            TriggerPipelineInput {
                project_slug: "gh/myorg/myrepo".to_string(),
                branch: Some("main".to_string()),
                tag: None,
                parameters: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.pipeline_id, "pipeline-123");
        assert_eq!(output.pipeline_number, 42);
    }

    #[tokio::test]
    async fn test_get_pipeline_status_success() {
        let server = MockServer::start().await;

        let pipeline_response = r#"{
            "id": "pipeline-123",
            "project_slug": "gh/myorg/myrepo",
            "number": 42,
            "state": "pending",
            "created_at": "2024-01-01T00:00:00Z",
            "trigger": {
                "type": "api",
                "received_at": "2024-01-01T00:00:00Z",
                "actor": {
                    "login": "testuser",
                    "avatar_url": "https://example.com/avatar.png"
                }
            },
            "errors": []
        }"#;

        let workflows_response = r#"{
            "items": [{
                "id": "workflow-456",
                "name": "build-and-test",
                "project_slug": "gh/myorg/myrepo",
                "pipeline_id": "pipeline-123",
                "pipeline_number": 42,
                "status": "running",
                "created_at": "2024-01-01T00:00:00Z"
            }],
            "next_page_token": null
        }"#;

        Mock::given(method("GET"))
            .and(path("/pipeline/pipeline-123"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(pipeline_response, "application/json"),
            )
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/pipeline/pipeline-123/workflow"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(workflows_response, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = get_pipeline_status(
            ctx,
            GetPipelineStatusInput {
                pipeline_id: "pipeline-123".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.pipeline.id, "pipeline-123");
        assert_eq!(output.workflows.len(), 1);
        assert_eq!(output.workflows[0].name, "build-and-test");
    }

    #[tokio::test]
    async fn test_get_job_logs_success() {
        let server = MockServer::start().await;

        let job_response = r#"{
            "id": "job-789",
            "name": "build",
            "project_slug": "gh/myorg/myrepo",
            "job_number": 123,
            "status": "success",
            "started_at": "2024-01-01T00:00:00Z",
            "stopped_at": "2024-01-01T00:05:00Z",
            "type": "build",
            "web_url": "https://app.circleci.com/jobs/gh/myorg/myrepo/123",
            "organization": {
                "name": "myorg"
            },
            "pipeline": {
                "id": "pipeline-123"
            },
            "parallel_runs": [],
            "messages": [],
            "contexts": [],
            "project": {
                "id": "project-123",
                "slug": "gh/myorg/myrepo",
                "name": "myrepo",
                "external_url": "https://github.com/myorg/myrepo"
            },
            "latest_workflow": {
                "id": "workflow-456",
                "name": "build-and-test"
            },
            "executor": {
                "resource_class": "medium",
                "type": "docker"
            },
            "duration": 300,
            "created_at": "2024-01-01T00:00:00Z",
            "queued_at": "2024-01-01T00:00:00Z"
        }"#;

        Mock::given(method("GET"))
            .and(path("/project/gh/myorg/myrepo/job/123"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(job_response, "application/json"))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = get_job_logs(
            ctx,
            GetJobLogsInput {
                project_slug: "gh/myorg/myrepo".to_string(),
                job_number: 123,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.job.name, "build");
        assert_eq!(
            output.log_url,
            "https://app.circleci.com/jobs/gh/myorg/myrepo/123"
        );
    }

    #[tokio::test]
    async fn test_trigger_pipeline_error_response() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/project/gh/myorg/myrepo/pipeline"))
            .respond_with(
                ResponseTemplate::new(404)
                    .set_body_raw(r#"{"message": "Project not found"}"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let result = trigger_pipeline(
            ctx,
            TriggerPipelineInput {
                project_slug: "gh/myorg/myrepo".to_string(),
                branch: None,
                tag: None,
                parameters: None,
            },
        )
        .await;

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("404"));
    }
}
