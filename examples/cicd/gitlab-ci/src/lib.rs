//! cicd/gitlab-ci integration for Operai Toolbox.

mod types;

use std::collections::HashMap;

use operai::{
    Context, JsonSchema, Result, define_system_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
use types::{Pipeline, PipelineDetailed, PipelineStatus};

define_system_credential! {
    GitLabCredential("gitlab") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_GITLAB_ENDPOINT: &str = "https://gitlab.com";

#[init]
async fn setup() -> Result<()> {
    info!("GitLab CI integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("GitLab CI integration shutting down");
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TriggerPipelineInput {
    /// GitLab project ID or path (e.g., "group/project").
    pub project: String,
    /// Branch or tag name to trigger the pipeline on.
    #[serde(rename = "ref")]
    pub ref_name: String,
    /// Pipeline trigger token.
    pub trigger_token: String,
    /// Optional variables to pass to the pipeline.
    #[serde(default)]
    pub variables: HashMap<String, String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TriggerPipelineOutput {
    pub pipeline_id: u64,
    pub project_id: u64,
    pub status: PipelineStatus,
    pub ref_name: String,
    pub sha: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_url: Option<String>,
}

/// # Trigger GitLab CI Pipeline
///
/// Triggers a new CI/CD pipeline run for a specified GitLab project using a
/// pipeline trigger token.
///
/// Use this tool when a user wants to:
/// - Start a CI/CD pipeline on a specific branch or tag
/// - Automate pipeline execution as part of a workflow
/// - Trigger builds, tests, or deployments in GitLab
///
/// ## Important Notes
/// - Requires a GitLab pipeline trigger token (not the same as a personal
///   access token)
/// - The trigger token must be configured in the project's CI/CD settings
/// - Optionally accepts custom variables to pass to the pipeline execution
/// - Returns the pipeline ID, status, and web URL for tracking
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - cicd
/// - gitlab
/// - pipeline
///
/// # Errors
///
/// Returns an error if:
/// - The `project` field is empty or contains only whitespace
/// - The `ref_name` field is empty or contains only whitespace
/// - The `trigger_token` field is empty or contains only whitespace
/// - No GitLab credential is configured in the context
/// - The configured access token is empty or contains only whitespace
/// - The GitLab API endpoint URL is invalid
/// - The HTTP request to GitLab fails
/// - The GitLab API returns a non-success status code
/// - The response body cannot be parsed as a `Pipeline`
#[tool]
pub async fn trigger_pipeline(
    ctx: Context,
    input: TriggerPipelineInput,
) -> Result<TriggerPipelineOutput> {
    ensure!(
        !input.project.trim().is_empty(),
        "project must not be empty"
    );
    ensure!(!input.ref_name.trim().is_empty(), "ref must not be empty");
    ensure!(
        !input.trigger_token.trim().is_empty(),
        "trigger_token must not be empty"
    );

    let client = GitLabClient::from_ctx(&ctx)?;
    let encoded_project = urlencoding::encode(&input.project);

    let mut form_data = vec![
        ("token", input.trigger_token.clone()),
        ("ref", input.ref_name.clone()),
    ];

    let mut variables_json = HashMap::new();
    for (key, value) in &input.variables {
        variables_json.insert(format!("variables[{key}]"), value.clone());
    }

    for (key, value) in &variables_json {
        form_data.push((key.as_str(), value.clone()));
    }

    let url = client.url_with_path(&format!(
        "api/v4/projects/{encoded_project}/trigger/pipeline"
    ))?;

    let pipeline: Pipeline = client.post_form(url, &form_data).await?;

    Ok(TriggerPipelineOutput {
        pipeline_id: pipeline.id,
        project_id: pipeline.project_id,
        status: pipeline.status,
        ref_name: pipeline.ref_name,
        sha: pipeline.sha,
        web_url: pipeline.web_url,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetPipelineStatusInput {
    /// GitLab project ID or path (e.g., "group/project").
    pub project: String,
    /// Pipeline ID to query.
    pub pipeline_id: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetPipelineStatusOutput {
    pub pipeline_id: u64,
    pub project_id: u64,
    pub status: PipelineStatus,
    pub ref_name: String,
    pub sha: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u64>,
}

/// # Get GitLab Pipeline Status
///
/// Retrieves detailed status information for a specific GitLab CI/CD pipeline,
/// including its current state, timestamps, and duration.
///
/// Use this tool when a user wants to:
/// - Check if a pipeline has completed, failed, or is still running
/// - Monitor pipeline progress and execution time
/// - Get detailed metadata about a pipeline run (created, updated, started,
///   finished times)
/// - Determine if a CI/CD job was successful before proceeding with downstream
///   tasks
///
/// ## Important Notes
/// - Requires the pipeline ID (obtained from triggering a pipeline or listing
///   pipelines)
/// - Returns comprehensive timing information (created, updated, started,
///   finished timestamps)
/// - Includes duration in seconds if the pipeline has completed
/// - Provides a web URL to view the pipeline in the GitLab UI
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - cicd
/// - gitlab
/// - pipeline
///
/// # Errors
///
/// Returns an error if:
/// - The `project` field is empty or contains only whitespace
/// - No GitLab credential is configured in the context
/// - The configured access token is empty or contains only whitespace
/// - The GitLab API endpoint URL is invalid
/// - The HTTP request to GitLab fails
/// - The GitLab API returns a non-success status code (e.g., 404 if pipeline
///   not found)
/// - The response body cannot be parsed as a `PipelineDetailed`
#[tool]
pub async fn get_pipeline_status(
    ctx: Context,
    input: GetPipelineStatusInput,
) -> Result<GetPipelineStatusOutput> {
    ensure!(
        !input.project.trim().is_empty(),
        "project must not be empty"
    );

    let client = GitLabClient::from_ctx(&ctx)?;
    let encoded_project = urlencoding::encode(&input.project);

    let url = client.url_with_path(&format!(
        "api/v4/projects/{}/pipelines/{}",
        encoded_project, input.pipeline_id
    ))?;

    let pipeline: PipelineDetailed = client.get_json(url).await?;

    Ok(GetPipelineStatusOutput {
        pipeline_id: pipeline.id,
        project_id: pipeline.project_id,
        status: pipeline.status,
        ref_name: pipeline.ref_name,
        sha: pipeline.sha,
        web_url: pipeline.web_url,
        created_at: pipeline.created_at,
        updated_at: pipeline.updated_at,
        started_at: pipeline.started_at,
        finished_at: pipeline.finished_at,
        duration: pipeline.duration,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FetchJobLogsInput {
    /// GitLab project ID or path (e.g., "group/project").
    pub project: String,
    /// Job ID to fetch logs from.
    pub job_id: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FetchJobLogsOutput {
    pub job_id: u64,
    pub logs: String,
}

/// # Fetch GitLab Job Logs
///
/// Retrieves the complete console output (trace logs) from a specific GitLab
/// CI/CD job execution.
///
/// Use this tool when a user wants to:
/// - Debug a failed CI/CD job by examining its output
/// - Review build logs, test results, or deployment output
/// - Analyze job execution to identify errors or warnings
/// - Capture logs for auditing or diagnostic purposes
///
/// ## Important Notes
/// - Requires a specific job ID (not pipeline ID) - obtain from pipeline
///   details or job listing
/// - Returns the full raw log output as plain text
/// - Logs are only available after the job has started running
/// - For very large logs, consider the response size may be substantial
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - cicd
/// - gitlab
/// - logs
///
/// # Errors
///
/// Returns an error if:
/// - The `project` field is empty or contains only whitespace
/// - No GitLab credential is configured in the context
/// - The configured access token is empty or contains only whitespace
/// - The GitLab API endpoint URL is invalid
/// - The HTTP request to GitLab fails
/// - The GitLab API returns a non-success status code
/// - The response body cannot be read as text
#[tool]
pub async fn fetch_job_logs(ctx: Context, input: FetchJobLogsInput) -> Result<FetchJobLogsOutput> {
    ensure!(
        !input.project.trim().is_empty(),
        "project must not be empty"
    );

    let client = GitLabClient::from_ctx(&ctx)?;
    let encoded_project = urlencoding::encode(&input.project);

    let url = client.url_with_path(&format!(
        "api/v4/projects/{}/jobs/{}/trace",
        encoded_project, input.job_id
    ))?;

    let logs = client.get_text(url).await?;

    Ok(FetchJobLogsOutput {
        job_id: input.job_id,
        logs,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DownloadArtifactsInput {
    /// GitLab project ID or path (e.g., "group/project").
    pub project: String,
    /// Branch, tag, or commit SHA to download artifacts from.
    #[serde(rename = "ref")]
    pub ref_name: String,
    /// Job name that produced the artifacts.
    pub job: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DownloadArtifactsOutput {
    pub project: String,
    pub ref_name: String,
    pub job: String,
    /// Base64-encoded ZIP archive of artifacts.
    pub artifacts_base64: String,
    pub size_bytes: usize,
}

/// # Download GitLab Job Artifacts
///
/// Downloads artifacts (build outputs, test reports, deployment packages, etc.)
/// produced by a successful GitLab CI/CD job.
///
/// Use this tool when a user wants to:
/// - Retrieve build outputs such as compiled binaries, libraries, or packages
/// - Download test reports, coverage reports, or documentation generated during
///   CI/CD
/// - Obtain deployment artifacts for staging or production releases
/// - Access any files archived by a GitLab job for later use
///
/// ## Important Notes
/// - Artifacts are returned as a base64-encoded ZIP archive
/// - The job must have completed successfully and archived artifacts
/// - Requires the job name (not job ID) and reference (branch/tag/SHA)
/// - Artifacts must be enabled in the job's `.gitlab-ci.yml` configuration
/// - Large artifacts may result in substantial base64 responses
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - cicd
/// - gitlab
/// - artifacts
///
/// # Errors
///
/// Returns an error if:
/// - The `project` field is empty or contains only whitespace
/// - The `ref_name` field is empty or contains only whitespace
/// - The `job` field is empty or contains only whitespace
/// - No GitLab credential is configured in the context
/// - The configured access token is empty or contains only whitespace
/// - The GitLab API endpoint URL is invalid
/// - The HTTP request to GitLab fails
/// - The GitLab API returns a non-success status code
/// - The response body cannot be read as bytes
#[tool]
pub async fn download_artifacts(
    ctx: Context,
    input: DownloadArtifactsInput,
) -> Result<DownloadArtifactsOutput> {
    ensure!(
        !input.project.trim().is_empty(),
        "project must not be empty"
    );
    ensure!(!input.ref_name.trim().is_empty(), "ref must not be empty");
    ensure!(!input.job.trim().is_empty(), "job must not be empty");

    let client = GitLabClient::from_ctx(&ctx)?;
    let encoded_project = urlencoding::encode(&input.project);

    let url = client.url_with_path(&format!(
        "api/v4/projects/{}/jobs/artifacts/{}/download",
        encoded_project, input.ref_name
    ))?;

    let query = [("job", input.job.as_str())];
    let artifacts_bytes = client.get_bytes(url, &query).await?;
    let size = artifacts_bytes.len();
    let artifacts_base64 = base64_encode(&artifacts_bytes);

    Ok(DownloadArtifactsOutput {
        project: input.project,
        ref_name: input.ref_name,
        job: input.job,
        artifacts_base64,
        size_bytes: size,
    })
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

#[derive(Debug, Clone)]
struct GitLabClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl GitLabClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = GitLabCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_GITLAB_ENDPOINT))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            access_token: cred.access_token,
        })
    }

    fn url_with_path(&self, path: &str) -> Result<reqwest::Url> {
        let url_str = format!("{}/{}", self.base_url, path.trim_start_matches('/'));
        Ok(reqwest::Url::parse(&url_str)?)
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, url: reqwest::Url) -> Result<T> {
        let response = self.send_request(self.http.get(url)).await?;
        Ok(response.json::<T>().await?)
    }

    async fn get_text(&self, url: reqwest::Url) -> Result<String> {
        let response = self.send_request(self.http.get(url)).await?;
        Ok(response.text().await?)
    }

    async fn get_bytes(&self, url: reqwest::Url, query: &[(&str, &str)]) -> Result<Vec<u8>> {
        let response = self.send_request(self.http.get(url).query(query)).await?;
        Ok(response.bytes().await?.to_vec())
    }

    async fn post_form<T: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        form: &[(&str, String)],
    ) -> Result<T> {
        let response = self.send_request(self.http.post(url).form(form)).await?;
        Ok(response.json::<T>().await?)
    }

    async fn send_request(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let response = request
            .header("PRIVATE-TOKEN", &self.access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "GitLab API request failed ({status}): {body}"
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

    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path, query_param},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut gitlab_values = HashMap::new();
        gitlab_values.insert("access_token".to_string(), "test-token".to_string());
        gitlab_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_system_credential("gitlab", gitlab_values)
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_pipeline_status_serialization_roundtrip() {
        // Test all status variants to ensure proper serde serialization
        let test_cases = [
            (PipelineStatus::Created, "created"),
            (PipelineStatus::WaitingForResource, "waiting_for_resource"),
            (PipelineStatus::Preparing, "preparing"),
            (PipelineStatus::Pending, "pending"),
            (PipelineStatus::Running, "running"),
            (PipelineStatus::Success, "success"),
            (PipelineStatus::Failed, "failed"),
            (PipelineStatus::Canceled, "canceled"),
            (PipelineStatus::Skipped, "skipped"),
            (PipelineStatus::Manual, "manual"),
            (PipelineStatus::Scheduled, "scheduled"),
        ];

        for (status, expected_json) in test_cases {
            // Test serialization
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, format!("\"{expected_json}\""));

            // Test deserialization
            let parsed: PipelineStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, parsed);
        }
    }

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://gitlab.com/").unwrap();
        assert_eq!(result, "https://gitlab.com");
    }

    #[test]
    fn test_normalize_base_url_empty_returns_error() {
        let result = normalize_base_url("");
        assert!(result.is_err());
    }

    // --- Input validation tests ---

    #[tokio::test]
    async fn test_trigger_pipeline_empty_project_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = trigger_pipeline(
            ctx,
            TriggerPipelineInput {
                project: "  ".to_string(),
                ref_name: "main".to_string(),
                trigger_token: "token".to_string(),
                variables: HashMap::new(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("project must not be empty")
        );
    }

    #[tokio::test]
    async fn test_trigger_pipeline_empty_ref_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = trigger_pipeline(
            ctx,
            TriggerPipelineInput {
                project: "test/repo".to_string(),
                ref_name: "  ".to_string(),
                trigger_token: "token".to_string(),
                variables: HashMap::new(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("ref must not be empty")
        );
    }

    #[tokio::test]
    async fn test_trigger_pipeline_empty_token_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = trigger_pipeline(
            ctx,
            TriggerPipelineInput {
                project: "test/repo".to_string(),
                ref_name: "main".to_string(),
                trigger_token: "  ".to_string(),
                variables: HashMap::new(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("trigger_token must not be empty")
        );
    }

    #[tokio::test]
    async fn test_get_pipeline_status_empty_project_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = get_pipeline_status(
            ctx,
            GetPipelineStatusInput {
                project: "  ".to_string(),
                pipeline_id: 123,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("project must not be empty")
        );
    }

    #[tokio::test]
    async fn test_fetch_job_logs_empty_project_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = fetch_job_logs(
            ctx,
            FetchJobLogsInput {
                project: "  ".to_string(),
                job_id: 456,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("project must not be empty")
        );
    }

    #[tokio::test]
    async fn test_download_artifacts_empty_project_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = download_artifacts(
            ctx,
            DownloadArtifactsInput {
                project: "  ".to_string(),
                ref_name: "main".to_string(),
                job: "build".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("project must not be empty")
        );
    }

    #[tokio::test]
    async fn test_download_artifacts_empty_ref_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = download_artifacts(
            ctx,
            DownloadArtifactsInput {
                project: "test/repo".to_string(),
                ref_name: "  ".to_string(),
                job: "build".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("ref must not be empty")
        );
    }

    #[tokio::test]
    async fn test_download_artifacts_empty_job_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = download_artifacts(
            ctx,
            DownloadArtifactsInput {
                project: "test/repo".to_string(),
                ref_name: "main".to_string(),
                job: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("job must not be empty")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_trigger_pipeline_success() {
        let server = MockServer::start().await;
        let response_body = r#"{
            "id": 123,
            "iid": 10,
            "project_id": 1,
            "status": "pending",
            "ref": "main",
            "sha": "abc123",
            "web_url": "https://gitlab.com/test/repo/-/pipelines/123"
        }"#;

        Mock::given(method("POST"))
            .and(path("/api/v4/projects/test%2Frepo/trigger/pipeline"))
            .and(header("PRIVATE-TOKEN", "test-token"))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = trigger_pipeline(
            ctx,
            TriggerPipelineInput {
                project: "test/repo".to_string(),
                ref_name: "main".to_string(),
                trigger_token: "trigger-token-123".to_string(),
                variables: HashMap::new(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.pipeline_id, 123);
        assert_eq!(output.status, PipelineStatus::Pending);
    }

    #[tokio::test]
    async fn test_get_pipeline_status_success() {
        let server = MockServer::start().await;
        let response_body = r#"{
            "id": 123,
            "iid": 10,
            "project_id": 1,
            "status": "success",
            "ref": "main",
            "sha": "abc123",
            "web_url": "https://gitlab.com/test/repo/-/pipelines/123",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:05:00Z",
            "started_at": "2024-01-01T00:01:00Z",
            "finished_at": "2024-01-01T00:05:00Z",
            "duration": 240
        }"#;

        Mock::given(method("GET"))
            .and(path("/api/v4/projects/test%2Frepo/pipelines/123"))
            .and(header("PRIVATE-TOKEN", "test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = get_pipeline_status(
            ctx,
            GetPipelineStatusInput {
                project: "test/repo".to_string(),
                pipeline_id: 123,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.pipeline_id, 123);
        assert_eq!(output.status, PipelineStatus::Success);
        assert_eq!(output.duration, Some(240));
    }

    #[tokio::test]
    async fn test_fetch_job_logs_success() {
        let server = MockServer::start().await;
        let log_content = "Running tests...\nAll tests passed!\n";

        Mock::given(method("GET"))
            .and(path("/api/v4/projects/test%2Frepo/jobs/456/trace"))
            .and(header("PRIVATE-TOKEN", "test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(log_content, "text/plain"))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = fetch_job_logs(
            ctx,
            FetchJobLogsInput {
                project: "test/repo".to_string(),
                job_id: 456,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.job_id, 456);
        assert!(output.logs.contains("All tests passed"));
    }

    #[tokio::test]
    async fn test_download_artifacts_success() {
        let server = MockServer::start().await;
        let artifact_data = b"PK\x03\x04mock zip data";

        Mock::given(method("GET"))
            .and(path(
                "/api/v4/projects/test%2Frepo/jobs/artifacts/main/download",
            ))
            .and(query_param("job", "build"))
            .and(header("PRIVATE-TOKEN", "test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(artifact_data))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = download_artifacts(
            ctx,
            DownloadArtifactsInput {
                project: "test/repo".to_string(),
                ref_name: "main".to_string(),
                job: "build".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.project, "test/repo");
        assert_eq!(output.size_bytes, artifact_data.len());
        assert!(!output.artifacts_base64.is_empty());
    }

    #[tokio::test]
    async fn test_get_pipeline_status_not_found_returns_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v4/projects/test%2Frepo/pipelines/999"))
            .respond_with(ResponseTemplate::new(404).set_body_raw(
                r#"{"message": "404 Pipeline Not Found"}"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let result = get_pipeline_status(
            ctx,
            GetPipelineStatusInput {
                project: "test/repo".to_string(),
                pipeline_id: 999,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("404"));
    }
}
