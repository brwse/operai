//! cicd/github-actions integration for Operai Toolbox.

mod types;

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
use types::{
    Artifact, ListArtifactsResponse, ListWorkflowRunsResponse, ListWorkflowsResponse,
    WorkflowRunDetail, WorkflowRunStatus, WorkflowRunSummary, WorkflowSummary,
};

define_user_credential! {
    GitHubActionsCredential("github_actions") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_GITHUB_API_ENDPOINT: &str = "https://api.github.com";

#[init]
async fn setup() -> Result<()> {
    info!("GitHub Actions integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("GitHub Actions integration shutting down");
}

// ============================================================================
// Tools
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListWorkflowsInput {
    /// Repository owner (username or organization).
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Maximum number of workflows to return (1-100). Defaults to 30.
    #[serde(default)]
    pub per_page: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListWorkflowsOutput {
    pub workflows: Vec<WorkflowSummary>,
}

/// # List GitHub Actions Workflows
///
/// Lists all GitHub Actions workflows in a repository. Use this tool when you
/// need to discover what workflows are available in a GitHub repository, view
/// their configuration details, or get workflow metadata like names, paths, and
/// states.
///
/// This tool returns workflow summaries including:
/// - Workflow ID and name
/// - File path (e.g., `.github/workflows/ci.yml`)
/// - Current state (active/inactive)
/// - URLs to the workflow in GitHub UI and API
/// - Badge URL for workflow status display
///
/// Use cases:
/// - Discovering available workflows in a repository before triggering one
/// - Auditing workflow configurations across a repository
/// - Getting workflow IDs for programmatic access
/// - Checking workflow states for health monitoring
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - ci-cd
/// - github
/// - workflows
///
/// # Errors
///
/// Returns an error if:
/// - The owner or repo fields are empty or contain only whitespace
/// - The `per_page` value is outside the valid range (1-100)
/// - Failed to retrieve GitHub credentials from the context
/// - The access token in credentials is empty
/// - The endpoint URL is invalid or cannot be parsed
/// - The GitHub API request fails (network error, timeout, etc.)
/// - The API returns a non-success status code (e.g., 401 Unauthorized, 404 Not
///   Found)
/// - The API response cannot be parsed as valid JSON
/// - The response JSON does not match the expected schema
#[tool]
pub async fn list_workflows(
    ctx: Context,
    input: ListWorkflowsInput,
) -> Result<ListWorkflowsOutput> {
    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    ensure!(!input.repo.trim().is_empty(), "repo must not be empty");

    let per_page = input.per_page.unwrap_or(30);
    ensure!(
        (1..=100).contains(&per_page),
        "per_page must be between 1 and 100"
    );

    let client = GitHubClient::from_ctx(&ctx)?;
    let url =
        client.url_with_segments(&["repos", &input.owner, &input.repo, "actions", "workflows"])?;

    let query = [("per_page", per_page.to_string())];

    let response: ListWorkflowsResponse = client.get_json(url, &query).await?;

    Ok(ListWorkflowsOutput {
        workflows: response.workflows,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TriggerWorkflowInput {
    /// Repository owner (username or organization).
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Workflow ID or filename (e.g., "main.yml" or workflow ID).
    pub workflow_id: String,
    /// Git reference (branch or tag name) for the workflow.
    pub git_ref: String,
    /// Optional inputs to pass to the workflow (JSON object as key-value
    /// pairs).
    #[serde(default)]
    pub inputs: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TriggerWorkflowOutput {
    pub triggered: bool,
}

/// # Trigger GitHub Actions Workflow
///
/// Manually triggers a GitHub Actions workflow run using the
/// `workflow_dispatch` event. Use this tool when you need to start a workflow
/// run programmatically, such as for deployment pipelines, custom CI triggers,
/// or automated testing workflows.
///
/// **Important**: The target workflow must have `workflow_dispatch` event
/// configured in its YAML file (under `.github/workflows/`). Not all workflows
/// support manual triggering.
///
/// Parameters:
/// - `workflow_id`: Can be the workflow filename (e.g., `ci.yml`) or numeric
///   workflow ID
/// - `git_ref`: The branch or tag to run the workflow against (e.g., `main`,
///   `v1.0.0`)
/// - `inputs`: Optional key-value pairs to pass to the workflow's `inputs`
///   section
///
/// Use cases:
/// - Triggering deployment workflows after code review approval
/// - Running scheduled workflows on-demand
/// - Starting CI/CD pipelines from external systems
/// - Testing workflows with different inputs
///
/// This tool returns immediately after the workflow is queued; use
/// `get_run_status` to monitor execution progress.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - ci-cd
/// - github
/// - workflows
/// - trigger
///
/// # Errors
///
/// Returns an error if:
/// - The owner, repo, `workflow_id`, or `git_ref` fields are empty or contain
///   only whitespace
/// - Failed to retrieve GitHub credentials from the context
/// - The access token in credentials is empty
/// - The endpoint URL is invalid or cannot be parsed
/// - The GitHub API request fails (network error, timeout, etc.)
/// - The API returns a non-success status code (e.g., 401 Unauthorized, 404 Not
///   Found)
/// - The workflow does not support `workflow_dispatch` events
/// - The specified `git_ref` does not exist
#[tool]
pub async fn trigger_workflow(
    ctx: Context,
    input: TriggerWorkflowInput,
) -> Result<TriggerWorkflowOutput> {
    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    ensure!(!input.repo.trim().is_empty(), "repo must not be empty");
    ensure!(
        !input.workflow_id.trim().is_empty(),
        "workflow_id must not be empty"
    );
    ensure!(
        !input.git_ref.trim().is_empty(),
        "git_ref must not be empty"
    );

    let client = GitHubClient::from_ctx(&ctx)?;
    let url = client.url_with_segments(&[
        "repos",
        &input.owner,
        &input.repo,
        "actions",
        "workflows",
        &input.workflow_id,
        "dispatches",
    ])?;

    let body = TriggerWorkflowRequest {
        git_ref: input.git_ref,
        inputs: input
            .inputs
            .unwrap_or(serde_json::Value::Object(serde_json::Map::default())),
    };

    client.post_empty(url, &body).await?;

    Ok(TriggerWorkflowOutput { triggered: true })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetRunStatusInput {
    /// Repository owner (username or organization).
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Workflow run ID.
    pub run_id: i64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetRunStatusOutput {
    pub run: WorkflowRunDetail,
}

/// # Get GitHub Actions Workflow Run Status
///
/// Retrieves detailed status and metadata for a specific GitHub Actions
/// workflow run. Use this tool when you need to check the current state,
/// results, or details of a workflow execution, such as after triggering a
/// workflow or monitoring CI/CD health.
///
/// This tool returns comprehensive workflow run information including:
/// - Run status (queued, `in_progress`, completed, waiting)
/// - Conclusion (success, failure, neutral, cancelled, skipped, `timed_out`,
///   `action_required`)
/// - Run number and attempt count
/// - Triggering event type (push, `pull_request`, `workflow_dispatch`, etc.)
/// - Branch/tag reference and commit SHA
/// - Actor information (who triggered the run)
/// - Timestamps (created, updated, started)
/// - URLs to the run in GitHub UI, logs, jobs, and artifacts
///
/// Use cases:
/// - Monitoring workflow execution after triggering with `trigger_workflow`
/// - Checking CI/CD pipeline status before proceeding with deployments
/// - Debugging failed workflow runs by examining details
/// - Retrieving commit and actor information for audit trails
/// - Getting artifact and job URLs for further investigation
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - ci-cd
/// - github
/// - workflows
/// - status
///
/// # Errors
///
/// Returns an error if:
/// - The owner or repo fields are empty or contain only whitespace
/// - The `run_id` is not positive
/// - Failed to retrieve GitHub credentials from the context
/// - The access token in credentials is empty
/// - The endpoint URL is invalid or cannot be parsed
/// - The GitHub API request fails (network error, timeout, etc.)
/// - The API returns a non-success status code (e.g., 401 Unauthorized, 404 Not
///   Found)
/// - The workflow run does not exist
/// - The API response cannot be parsed as valid JSON
/// - The response JSON does not match the expected schema
#[tool]
pub async fn get_run_status(ctx: Context, input: GetRunStatusInput) -> Result<GetRunStatusOutput> {
    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    ensure!(!input.repo.trim().is_empty(), "repo must not be empty");
    ensure!(input.run_id > 0, "run_id must be positive");

    let client = GitHubClient::from_ctx(&ctx)?;
    let url = client.url_with_segments(&[
        "repos",
        &input.owner,
        &input.repo,
        "actions",
        "runs",
        &input.run_id.to_string(),
    ])?;

    let run: WorkflowRunDetail = client.get_json(url, &[]).await?;

    Ok(GetRunStatusOutput { run })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListWorkflowRunsInput {
    /// Repository owner (username or organization).
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Optional workflow ID or filename to filter runs.
    #[serde(default)]
    pub workflow_id: Option<String>,
    /// Optional status filter (queued, `in_progress`, completed).
    #[serde(default)]
    pub status: Option<WorkflowRunStatus>,
    /// Maximum number of runs to return (1-100). Defaults to 30.
    #[serde(default)]
    pub per_page: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListWorkflowRunsOutput {
    pub runs: Vec<WorkflowRunSummary>,
}

/// # List GitHub Actions Workflow Runs
///
/// Lists GitHub Actions workflow runs for a repository, with optional filtering
/// by specific workflow and run status. Use this tool when you need to browse
/// workflow execution history, find recent runs, or filter runs by status for
/// debugging and monitoring.
///
/// This tool returns workflow run summaries including:
/// - Run ID and workflow name
/// - Current status (queued, `in_progress`, completed, waiting)
/// - Conclusion result if completed
/// - Run number and attempt count
/// - Event type that triggered the run
/// - Branch/tag and commit SHA
/// - Timestamps (created, updated, started)
/// - URL to view the run in GitHub UI
///
/// **Filtering options:**
/// - `workflow_id`: Scope results to a specific workflow (filename or ID)
/// - `status`: Filter by run status (queued, `in_progress`, completed, waiting)
/// - `per_page`: Control pagination (1-100, default 30)
///
/// Use cases:
/// - Viewing recent CI/CD activity across a repository
/// - Finding failed runs for debugging
/// - Checking workflow execution history for a specific branch
/// - Monitoring all runs for a particular workflow
/// - Identifying long-running or queued workflows
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - ci-cd
/// - github
/// - workflows
/// - runs
///
/// # Errors
///
/// Returns an error if:
/// - The owner or repo fields are empty or contain only whitespace
/// - The `workflow_id` is provided but empty or contains only whitespace
/// - The `per_page` value is outside the valid range (1-100)
/// - Failed to retrieve GitHub credentials from the context
/// - The access token in credentials is empty
/// - The endpoint URL is invalid or cannot be parsed
/// - The GitHub API request fails (network error, timeout, etc.)
/// - The API returns a non-success status code (e.g., 401 Unauthorized, 404 Not
///   Found)
/// - The API response cannot be parsed as valid JSON
/// - The response JSON does not match the expected schema
#[tool]
pub async fn list_workflow_runs(
    ctx: Context,
    input: ListWorkflowRunsInput,
) -> Result<ListWorkflowRunsOutput> {
    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    ensure!(!input.repo.trim().is_empty(), "repo must not be empty");

    let per_page = input.per_page.unwrap_or(30);
    ensure!(
        (1..=100).contains(&per_page),
        "per_page must be between 1 and 100"
    );

    let client = GitHubClient::from_ctx(&ctx)?;

    let url = if let Some(workflow_id) = &input.workflow_id {
        ensure!(
            !workflow_id.trim().is_empty(),
            "workflow_id must not be empty when provided"
        );
        client.url_with_segments(&[
            "repos",
            &input.owner,
            &input.repo,
            "actions",
            "workflows",
            workflow_id,
            "runs",
        ])?
    } else {
        client.url_with_segments(&["repos", &input.owner, &input.repo, "actions", "runs"])?
    };

    let mut query = vec![("per_page", per_page.to_string())];
    if let Some(status) = input.status {
        let status_str = match status {
            WorkflowRunStatus::Queued => "queued",
            WorkflowRunStatus::InProgress => "in_progress",
            WorkflowRunStatus::Completed => "completed",
            WorkflowRunStatus::Waiting => "waiting",
        };
        query.push(("status", status_str.to_string()));
    }

    let response: ListWorkflowRunsResponse = client.get_json(url, &query).await?;

    Ok(ListWorkflowRunsOutput {
        runs: response.workflow_runs,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FetchLogsInput {
    /// Repository owner (username or organization).
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Workflow run ID.
    pub run_id: i64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FetchLogsOutput {
    /// Base64-encoded zip archive containing the logs.
    pub logs_base64: String,
    /// Size of the encoded logs in bytes.
    pub size_bytes: usize,
}

/// # Fetch GitHub Actions Workflow Run Logs
///
/// Downloads complete logs for a GitHub Actions workflow run as a
/// base64-encoded zip archive. Use this tool when you need to retrieve detailed
/// execution logs for debugging, auditing, or analyzing workflow failures.
///
/// **Output format**: Returns a base64-encoded string containing a zip archive.
/// The zip file contains individual log files for each job and step in the
/// workflow run. Decode the base64 string and extract the zip to access plain
/// text log files.
///
/// **Log availability**: Logs are available for completed runs and may be
/// expired based on repository settings. GitHub retains logs for a configurable
/// retention period.
///
/// Use cases:
/// - Debugging failed workflow runs by examining step-by-step logs
/// - Archiving CI/CD execution logs for compliance or audit purposes
/// - Analyzing performance issues or error patterns in workflows
/// - Extracting specific job logs for detailed troubleshooting
/// - Building custom log analysis or monitoring tools
///
/// **Size considerations**: Large workflow runs with many jobs can produce
/// substantial log archives. The response includes the archive size in bytes to
/// help manage memory usage.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - ci-cd
/// - github
/// - workflows
/// - logs
///
/// # Errors
///
/// Returns an error if:
/// - The owner or repo fields are empty or contain only whitespace
/// - The `run_id` is not positive
/// - Failed to retrieve GitHub credentials from the context
/// - The access token in credentials is empty
/// - The endpoint URL is invalid or cannot be parsed
/// - The GitHub API request fails (network error, timeout, etc.)
/// - The API returns a non-success status code (e.g., 401 Unauthorized, 404 Not
///   Found)
/// - The workflow run does not exist or has no logs
/// - The log archive cannot be downloaded
/// - The log data cannot be encoded as base64
#[tool]
pub async fn fetch_logs(ctx: Context, input: FetchLogsInput) -> Result<FetchLogsOutput> {
    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    ensure!(!input.repo.trim().is_empty(), "repo must not be empty");
    ensure!(input.run_id > 0, "run_id must be positive");

    let client = GitHubClient::from_ctx(&ctx)?;
    let url = client.url_with_segments(&[
        "repos",
        &input.owner,
        &input.repo,
        "actions",
        "runs",
        &input.run_id.to_string(),
        "logs",
    ])?;

    let logs_bytes = client.get_bytes(url).await?;
    let logs_base64 =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &logs_bytes);
    let size_bytes = logs_bytes.len();

    Ok(FetchLogsOutput {
        logs_base64,
        size_bytes,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListArtifactsInput {
    /// Repository owner (username or organization).
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Workflow run ID.
    pub run_id: i64,
    /// Maximum number of artifacts to return (1-100). Defaults to 30.
    #[serde(default)]
    pub per_page: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListArtifactsOutput {
    pub artifacts: Vec<Artifact>,
}

/// # List GitHub Actions Workflow Artifacts
///
/// Lists all artifacts produced by a specific GitHub Actions workflow run. Use
/// this tool when you need to discover what build outputs, test results, or
/// other files were generated by a workflow execution.
///
/// This tool returns artifact metadata including:
/// - Artifact ID and name
/// - File size in bytes
/// - Expiration status and date
/// - Creation and update timestamps
/// - URLs to download or view the artifact in GitHub UI
///
/// **Artifact lifecycle**: Artifacts are stored after workflow completion and
/// expire based on repository retention settings (typically 90 days by
/// default). Expired artifacts are automatically deleted.
///
/// Use cases:
/// - Discovering available build outputs after a CI run completes
/// - Finding specific artifacts (e.g., test reports, binaries) by name
/// - Checking artifact expiration dates before automated downloads
/// - Auditing what files are being generated by workflows
/// - Getting artifact IDs for use with `download_artifact`
///
/// **Workflow context**: Artifacts are scoped to a specific workflow run. Use
/// `list_workflow_runs` first to find the appropriate `run_id`, then use this
/// tool to discover artifacts from that run.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - ci-cd
/// - github
/// - workflows
/// - artifacts
///
/// # Errors
///
/// Returns an error if:
/// - The owner or repo fields are empty or contain only whitespace
/// - The `run_id` is not positive
/// - The `per_page` value is outside the valid range (1-100)
/// - Failed to retrieve GitHub credentials from the context
/// - The access token in credentials is empty
/// - The endpoint URL is invalid or cannot be parsed
/// - The GitHub API request fails (network error, timeout, etc.)
/// - The API returns a non-success status code (e.g., 401 Unauthorized, 404 Not
///   Found)
/// - The workflow run does not exist
/// - The API response cannot be parsed as valid JSON
/// - The response JSON does not match the expected schema
#[tool]
pub async fn list_artifacts(
    ctx: Context,
    input: ListArtifactsInput,
) -> Result<ListArtifactsOutput> {
    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    ensure!(!input.repo.trim().is_empty(), "repo must not be empty");
    ensure!(input.run_id > 0, "run_id must be positive");

    let per_page = input.per_page.unwrap_or(30);
    ensure!(
        (1..=100).contains(&per_page),
        "per_page must be between 1 and 100"
    );

    let client = GitHubClient::from_ctx(&ctx)?;
    let url = client.url_with_segments(&[
        "repos",
        &input.owner,
        &input.repo,
        "actions",
        "runs",
        &input.run_id.to_string(),
        "artifacts",
    ])?;

    let query = [("per_page", per_page.to_string())];

    let response: ListArtifactsResponse = client.get_json(url, &query).await?;

    Ok(ListArtifactsOutput {
        artifacts: response.artifacts,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DownloadArtifactInput {
    /// Repository owner (username or organization).
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Artifact ID.
    pub artifact_id: i64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DownloadArtifactOutput {
    /// Base64-encoded artifact archive.
    pub artifact_base64: String,
    /// Size of the encoded artifact in bytes.
    pub size_bytes: usize,
}

/// # Download GitHub Actions Workflow Artifact
///
/// Downloads a specific artifact from a GitHub Actions workflow run as
/// base64-encoded data. Use this tool when you need to retrieve build outputs,
/// test reports, binaries, or other files generated by a workflow execution.
///
/// **Output format**: Returns a base64-encoded string containing the artifact
/// archive (typically a zip file). Decode the base64 string and extract the
/// archive to access the actual files.
///
/// **Prerequisite**: Use `list_artifacts` first to discover available artifacts
/// and obtain their artifact IDs. Artifacts are identified by unique numeric
/// IDs, not names.
///
/// Use cases:
/// - Downloading build binaries (e.g., `.exe`, `.dmg`, `.apk`) after CI
///   completion
/// - Retrieving test reports or coverage reports for analysis
/// - Fetching deployment packages for distribution
/// - Extracting generated documentation or other assets
/// - Archiving build outputs for long-term storage
///
/// **Size considerations**: Artifacts can be large (up to several GB). The
/// response includes the file size in bytes to help manage memory and storage
/// usage.
///
/// **Expiration**: Artifacts expire based on repository retention settings.
/// Attempting to download an expired artifact will return a 404 error.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - ci-cd
/// - github
/// - workflows
/// - artifacts
///
/// # Errors
///
/// Returns an error if:
/// - The owner or repo fields are empty or contain only whitespace
/// - The `artifact_id` is not positive
/// - Failed to retrieve GitHub credentials from the context
/// - The access token in credentials is empty
/// - The endpoint URL is invalid or cannot be parsed
/// - The GitHub API request fails (network error, timeout, etc.)
/// - The API returns a non-success status code (e.g., 401 Unauthorized, 404 Not
///   Found)
/// - The artifact does not exist
/// - The artifact archive cannot be downloaded
/// - The artifact data cannot be encoded as base64
#[tool]
pub async fn download_artifact(
    ctx: Context,
    input: DownloadArtifactInput,
) -> Result<DownloadArtifactOutput> {
    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    ensure!(!input.repo.trim().is_empty(), "repo must not be empty");
    ensure!(input.artifact_id > 0, "artifact_id must be positive");

    let client = GitHubClient::from_ctx(&ctx)?;
    let url = client.url_with_segments(&[
        "repos",
        &input.owner,
        &input.repo,
        "actions",
        "artifacts",
        &input.artifact_id.to_string(),
        "zip",
    ])?;

    let artifact_bytes = client.get_bytes(url).await?;
    let artifact_base64 =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &artifact_bytes);
    let size_bytes = artifact_bytes.len();

    Ok(DownloadArtifactOutput {
        artifact_base64,
        size_bytes,
    })
}

// ============================================================================
// Internal GitHub API client
// ============================================================================

#[derive(Debug, Serialize)]
struct TriggerWorkflowRequest {
    #[serde(rename = "ref")]
    git_ref: String,
    inputs: serde_json::Value,
}

#[derive(Debug, Clone)]
struct GitHubClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl GitHubClient {
    /// Creates a new `GitHubClient` from the provided context.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Failed to retrieve GitHub credentials from the context
    /// - The access token in credentials is empty
    /// - The endpoint URL is invalid or cannot be normalized
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = GitHubActionsCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url = normalize_base_url(
            cred.endpoint
                .as_deref()
                .unwrap_or(DEFAULT_GITHUB_API_ENDPOINT),
        )?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            access_token: cred.access_token,
        })
    }

    /// Constructs a URL by appending path segments to the base URL.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `base_url` is not an absolute URL (cannot be a relative URL or
    ///   have an invalid scheme)
    fn url_with_segments(&self, segments: &[&str]) -> Result<reqwest::Url> {
        let mut url = reqwest::Url::parse(&self.base_url)?;
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|()| operai::anyhow::anyhow!("base_url must be an absolute URL"))?;
            for segment in segments {
                path.push(segment);
            }
        }
        Ok(url)
    }

    /// Sends a GET request and parses the response as JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails (network error, timeout, etc.)
    /// - The API returns a non-success status code
    /// - The response body cannot be read
    /// - The response cannot be parsed as the expected JSON type
    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        query: &[(&str, String)],
    ) -> Result<T> {
        let response = self
            .http
            .get(url)
            .query(query)
            .bearer_auth(&self.access_token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "operai-github-actions/0.1.0")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.json::<T>().await?)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "GitHub API request failed ({status}): {body}"
            ))
        }
    }

    /// Sends a GET request and returns the response body as bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails (network error, timeout, etc.)
    /// - The API returns a non-success status code
    /// - The response body cannot be read
    async fn get_bytes(&self, url: reqwest::Url) -> Result<Vec<u8>> {
        let response = self
            .http
            .get(url)
            .bearer_auth(&self.access_token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "operai-github-actions/0.1.0")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.bytes().await?.to_vec())
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "GitHub API request failed ({status}): {body}"
            ))
        }
    }

    /// Sends a POST request with a JSON body and ignores the response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The request body cannot be serialized as JSON
    /// - The HTTP request fails (network error, timeout, etc.)
    /// - The API returns a non-success status code
    async fn post_empty<TReq: Serialize>(&self, url: reqwest::Url, body: &TReq) -> Result<()> {
        let response = self
            .http
            .post(url)
            .json(body)
            .bearer_auth(&self.access_token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "operai-github-actions/0.1.0")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            let body_text = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "GitHub API request failed ({status}): {body_text}"
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

    use types::WorkflowRunConclusion;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_string_contains, header, method, path, query_param},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut github_values = HashMap::new();
        github_values.insert("access_token".to_string(), "ghp_test_token_123".to_string());
        github_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("github_actions", github_values)
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_workflow_run_status_serialization_roundtrip() {
        for variant in [
            WorkflowRunStatus::Queued,
            WorkflowRunStatus::InProgress,
            WorkflowRunStatus::Completed,
            WorkflowRunStatus::Waiting,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: WorkflowRunStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_workflow_run_conclusion_serialization_roundtrip() {
        for variant in [
            WorkflowRunConclusion::Success,
            WorkflowRunConclusion::Failure,
            WorkflowRunConclusion::Neutral,
            WorkflowRunConclusion::Cancelled,
            WorkflowRunConclusion::Skipped,
            WorkflowRunConclusion::TimedOut,
            WorkflowRunConclusion::ActionRequired,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: WorkflowRunConclusion = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_workflow_summary_serialization_roundtrip() {
        let summary = WorkflowSummary {
            id: 123,
            node_id: "W_kwDOA1".to_string(),
            name: "CI".to_string(),
            path: ".github/workflows/ci.yml".to_string(),
            state: "active".to_string(),
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: Some("2024-01-02T00:00:00Z".to_string()),
            url: "https://api.github.com/repos/test-owner/test-repo/actions/workflows/123"
                .to_string(),
            html_url:
                "https://github.com/test-owner/test-repo/blob/master/.github/workflows/ci.yml"
                    .to_string(),
            badge_url: "https://github.com/test-owner/test-repo/workflows/CI/badge.svg".to_string(),
        };
        let json = serde_json::to_string(&summary).unwrap();
        let parsed: WorkflowSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(summary.id, parsed.id);
        assert_eq!(summary.name, parsed.name);
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://api.github.com/").unwrap();
        assert_eq!(result, "https://api.github.com");
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
    async fn test_list_workflows_empty_owner_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = list_workflows(
            ctx,
            ListWorkflowsInput {
                owner: "  ".to_string(),
                repo: "test-repo".to_string(),
                per_page: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("owner must not be empty")
        );
    }

    #[tokio::test]
    async fn test_list_workflows_empty_repo_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = list_workflows(
            ctx,
            ListWorkflowsInput {
                owner: "test-owner".to_string(),
                repo: "  ".to_string(),
                per_page: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("repo must not be empty")
        );
    }

    #[tokio::test]
    async fn test_list_workflows_invalid_per_page_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = list_workflows(
            ctx,
            ListWorkflowsInput {
                owner: "test-owner".to_string(),
                repo: "test-repo".to_string(),
                per_page: Some(101),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("per_page must be between 1 and 100")
        );
    }

    #[tokio::test]
    async fn test_trigger_workflow_empty_git_ref_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = trigger_workflow(
            ctx,
            TriggerWorkflowInput {
                owner: "test-owner".to_string(),
                repo: "test-repo".to_string(),
                workflow_id: "ci.yml".to_string(),
                git_ref: "  ".to_string(),
                inputs: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("git_ref must not be empty")
        );
    }

    #[tokio::test]
    async fn test_get_run_status_invalid_run_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = get_run_status(
            ctx,
            GetRunStatusInput {
                owner: "test-owner".to_string(),
                repo: "test-repo".to_string(),
                run_id: 0,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("run_id must be positive")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_list_workflows_success_returns_workflows() {
        let server = MockServer::start().await;
        let endpoint = server.uri();

        let response_body = r#"
          {
            "total_count": 1,
            "workflows": [
              {
                "id": 123,
                "node_id": "MDQ6V29ya2Zsb3cxMjM=",
                "name": "CI",
                "path": ".github/workflows/ci.yml",
                "state": "active",
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z",
                "url": "https://api.github.com/repos/test-owner/test-repo/actions/workflows/123",
                "html_url": "https://github.com/test-owner/test-repo/blob/master/.github/workflows/ci.yml",
                "badge_url": "https://github.com/test-owner/test-repo/workflows/CI/badge.svg"
              }
            ]
          }
        "#;

        Mock::given(method("GET"))
            .and(path("/repos/test-owner/test-repo/actions/workflows"))
            .and(header("authorization", "Bearer ghp_test_token_123"))
            .and(query_param("per_page", "30"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_workflows(
            ctx,
            ListWorkflowsInput {
                owner: "test-owner".to_string(),
                repo: "test-repo".to_string(),
                per_page: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.workflows.len(), 1);
        assert_eq!(output.workflows[0].id, 123);
        assert_eq!(output.workflows[0].name, "CI");
    }

    #[tokio::test]
    async fn test_trigger_workflow_success_returns_triggered() {
        let server = MockServer::start().await;
        let endpoint = server.uri();

        Mock::given(method("POST"))
            .and(path(
                "/repos/test-owner/test-repo/actions/workflows/ci.yml/dispatches",
            ))
            .and(header("authorization", "Bearer ghp_test_token_123"))
            .and(body_string_contains("\"ref\":\"main\""))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = trigger_workflow(
            ctx,
            TriggerWorkflowInput {
                owner: "test-owner".to_string(),
                repo: "test-repo".to_string(),
                workflow_id: "ci.yml".to_string(),
                git_ref: "main".to_string(),
                inputs: None,
            },
        )
        .await
        .unwrap();

        assert!(output.triggered);
    }

    #[tokio::test]
    async fn test_get_run_status_success_returns_run_detail() {
        let server = MockServer::start().await;
        let endpoint = server.uri();

        let response_body = r#"
          {
            "id": 456,
            "name": "CI",
            "status": "completed",
            "conclusion": "success",
            "workflow_id": 123,
            "head_branch": "main",
            "head_sha": "abc123",
            "run_number": 42,
            "event": "push",
            "display_title": "Update README",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:05:00Z",
            "run_started_at": "2024-01-01T00:00:10Z",
            "html_url": "https://github.com/test-owner/test-repo/actions/runs/456",
            "path": ".github/workflows/ci.yml@main",
            "run_attempt": 1,
            "referenced_workflows": [],
            "actor": {
              "login": "octocat",
              "id": 1,
              "node_id": "MDQ6VXNlcjE=",
              "avatar_url": "https://github.com/images/error/octocat_happy.gif",
              "gravatar_id": "",
              "url": "https://api.github.com/users/octocat",
              "html_url": "https://github.com/octocat",
              "followers_url": "https://api.github.com/users/octocat/followers",
              "following_url": "https://api.github.com/users/octocat/following{/other_user}",
              "gists_url": "https://api.github.com/users/octocat/gists{/gist_id}",
              "starred_url": "https://api.github.com/users/octocat/starred{/owner}{/repo}",
              "subscriptions_url": "https://api.github.com/users/octocat/subscriptions",
              "organizations_url": "https://api.github.com/users/octocat/orgs",
              "repos_url": "https://api.github.com/users/octocat/repos",
              "events_url": "https://api.github.com/users/octocat/events",
              "received_events_url": "https://api.github.com/users/octocat/received_events",
              "type": "User",
              "site_admin": false
            },
            "triggering_actor": {
              "login": "octocat",
              "id": 1,
              "node_id": "MDQ6VXNlcjE=",
              "avatar_url": "https://github.com/images/error/octocat_happy.gif",
              "gravatar_id": "",
              "url": "https://api.github.com/users/octocat",
              "html_url": "https://github.com/octocat",
              "followers_url": "https://api.github.com/users/octocat/followers",
              "following_url": "https://api.github.com/users/octocat/following{/other_user}",
              "gists_url": "https://api.github.com/users/octocat/gists{/gist_id}",
              "starred_url": "https://api.github.com/users/octocat/starred{/owner}{/repo}",
              "subscriptions_url": "https://api.github.com/users/octocat/subscriptions",
              "organizations_url": "https://api.github.com/users/octocat/orgs",
              "repos_url": "https://api.github.com/users/octocat/repos",
              "events_url": "https://api.github.com/users/octocat/events",
              "received_events_url": "https://api.github.com/users/octocat/received_events",
              "type": "User",
              "site_admin": false
            },
            "jobs_url": "https://api.github.com/repos/test-owner/test-repo/actions/runs/456/jobs",
            "logs_url": "https://api.github.com/repos/test-owner/test-repo/actions/runs/456/logs",
            "check_suite_url": "https://api.github.com/repos/test-owner/test-repo/check-suites/414944374",
            "artifacts_url": "https://api.github.com/repos/test-owner/test-repo/actions/runs/456/artifacts",
            "cancel_url": "https://api.github.com/repos/test-owner/test-repo/actions/runs/456/cancel",
            "rerun_url": "https://api.github.com/repos/test-owner/test-repo/actions/runs/456/rerun",
            "workflow_url": "https://api.github.com/repos/test-owner/test-repo/actions/workflows/123",
            "head_commit": {
              "id": "abc123",
              "tree_id": "xyz789",
              "message": "Update README",
              "timestamp": "2024-01-01T00:00:00Z",
              "author": {
                "name": "Octo Cat",
                "email": "octocat@github.com"
              },
              "committer": {
                "name": "GitHub",
                "email": "noreply@github.com"
              }
            },
            "repository": {
              "id": 12345,
              "node_id": "MDEwOlJlcG9zaXRvcnkxMjM0NQ==",
              "name": "test-repo",
              "full_name": "test-owner/test-repo",
              "private": false,
              "owner": {
                "login": "test-owner",
                "id": 1,
                "node_id": "MDQ6VXNlcjE=",
                "avatar_url": "https://github.com/images/error/octocat_happy.gif",
                "gravatar_id": "",
                "url": "https://api.github.com/users/test-owner",
                "html_url": "https://github.com/test-owner",
                "followers_url": "https://api.github.com/users/test-owner/followers",
                "following_url": "https://api.github.com/users/test-owner/following{/other_user}",
                "gists_url": "https://api.github.com/users/test-owner/gists{/gist_id}",
                "starred_url": "https://api.github.com/users/test-owner/starred{/owner}{/repo}",
                "subscriptions_url": "https://api.github.com/users/test-owner/subscriptions",
                "organizations_url": "https://api.github.com/users/test-owner/orgs",
                "repos_url": "https://api.github.com/users/test-owner/repos",
                "events_url": "https://api.github.com/users/test-owner/events",
                "received_events_url": "https://api.github.com/users/test-owner/received_events",
                "type": "User",
                "site_admin": false
              },
              "html_url": "https://github.com/test-owner/test-repo",
              "description": "Test repo",
              "fork": false,
              "url": "https://api.github.com/repos/test-owner/test-repo"
            }
          }
        "#;

        Mock::given(method("GET"))
            .and(path("/repos/test-owner/test-repo/actions/runs/456"))
            .and(header("authorization", "Bearer ghp_test_token_123"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = get_run_status(
            ctx,
            GetRunStatusInput {
                owner: "test-owner".to_string(),
                repo: "test-repo".to_string(),
                run_id: 456,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.run.id, 456);
        assert_eq!(output.run.status, WorkflowRunStatus::Completed);
        assert_eq!(output.run.conclusion, Some(WorkflowRunConclusion::Success));
        assert_eq!(output.run.run_number, 42);
    }

    #[tokio::test]
    async fn test_list_workflow_runs_success_returns_runs() {
        let server = MockServer::start().await;
        let endpoint = server.uri();

        let response_body = r#"
          {
            "total_count": 1,
            "workflow_runs": [
              {
                "id": 456,
                "name": "CI",
                "status": "in_progress",
                "workflow_id": 123,
                "head_branch": "main",
                "head_sha": "abc123",
                "run_number": 42,
                "event": "push",
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:05:00Z",
                "run_started_at": "2024-01-01T00:00:10Z",
                "html_url": "https://github.com/test-owner/test-repo/actions/runs/456",
                "path": ".github/workflows/ci.yml@main"
              }
            ]
          }
        "#;

        Mock::given(method("GET"))
            .and(path("/repos/test-owner/test-repo/actions/runs"))
            .and(query_param("per_page", "30"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_workflow_runs(
            ctx,
            ListWorkflowRunsInput {
                owner: "test-owner".to_string(),
                repo: "test-repo".to_string(),
                workflow_id: None,
                status: None,
                per_page: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.runs.len(), 1);
        assert_eq!(output.runs[0].id, 456);
        assert_eq!(output.runs[0].status, WorkflowRunStatus::InProgress);
        assert!(output.runs[0].conclusion.is_none());
    }

    #[tokio::test]
    async fn test_list_artifacts_success_returns_artifacts() {
        let server = MockServer::start().await;
        let endpoint = server.uri();

        let response_body = r#"
          {
            "total_count": 1,
            "artifacts": [
              {
                "id": 789,
                "node_id": "MDQ6QXJ0aWZhY3Q3ODk=",
                "name": "build-output",
                "size_in_bytes": 1024,
                "url": "https://api.github.com/repos/test-owner/test-repo/actions/artifacts/789",
                "archive_download_url": "https://api.github.com/repos/test-owner/test-repo/actions/artifacts/789/zip",
                "expired": false,
                "created_at": "2024-01-01T00:00:00Z",
                "expires_at": "2024-01-31T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z"
              }
            ]
          }
        "#;

        Mock::given(method("GET"))
            .and(path(
                "/repos/test-owner/test-repo/actions/runs/456/artifacts",
            ))
            .and(query_param("per_page", "30"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_artifacts(
            ctx,
            ListArtifactsInput {
                owner: "test-owner".to_string(),
                repo: "test-repo".to_string(),
                run_id: 456,
                per_page: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.artifacts.len(), 1);
        assert_eq!(output.artifacts[0].id, 789);
        assert_eq!(output.artifacts[0].name, "build-output");
        assert!(!output.artifacts[0].expired);
    }

    #[tokio::test]
    async fn test_fetch_logs_success_returns_base64_encoded_logs() {
        let server = MockServer::start().await;
        let endpoint = server.uri();

        let fake_zip_data = b"PK\x03\x04fake-zip-content";

        Mock::given(method("GET"))
            .and(path("/repos/test-owner/test-repo/actions/runs/456/logs"))
            .and(header("authorization", "Bearer ghp_test_token_123"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(fake_zip_data))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = fetch_logs(
            ctx,
            FetchLogsInput {
                owner: "test-owner".to_string(),
                repo: "test-repo".to_string(),
                run_id: 456,
            },
        )
        .await
        .unwrap();

        assert!(!output.logs_base64.is_empty());
        assert_eq!(output.size_bytes, fake_zip_data.len());

        // Verify we can decode it back
        let decoded = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &output.logs_base64,
        )
        .unwrap();
        assert_eq!(decoded, fake_zip_data);
    }

    #[tokio::test]
    async fn test_download_artifact_success_returns_base64_encoded_artifact() {
        let server = MockServer::start().await;
        let endpoint = server.uri();

        let fake_artifact_data = b"artifact-content-here";

        Mock::given(method("GET"))
            .and(path(
                "/repos/test-owner/test-repo/actions/artifacts/789/zip",
            ))
            .and(header("authorization", "Bearer ghp_test_token_123"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(fake_artifact_data))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = download_artifact(
            ctx,
            DownloadArtifactInput {
                owner: "test-owner".to_string(),
                repo: "test-repo".to_string(),
                artifact_id: 789,
            },
        )
        .await
        .unwrap();

        assert!(!output.artifact_base64.is_empty());
        assert_eq!(output.size_bytes, fake_artifact_data.len());

        // Verify we can decode it back
        let decoded = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &output.artifact_base64,
        )
        .unwrap();
        assert_eq!(decoded, fake_artifact_data);
    }

    #[tokio::test]
    async fn test_list_workflows_api_error_returns_error() {
        let server = MockServer::start().await;
        let endpoint = server.uri();

        Mock::given(method("GET"))
            .and(path("/repos/test-owner/test-repo/actions/workflows"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_raw(r#"{"message":"Bad credentials"}"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = list_workflows(
            ctx,
            ListWorkflowsInput {
                owner: "test-owner".to_string(),
                repo: "test-repo".to_string(),
                per_page: None,
            },
        )
        .await;

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("401"));
    }
}
