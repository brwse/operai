//! cicd/buildkite integration for Operai Toolbox.

mod types;

use std::collections::HashMap;

use operai::{
    Context, JsonSchema, Result, anyhow, define_system_credential, ensure, info, init, schemars,
    shutdown, tool,
};
use serde::{Deserialize, Serialize};
pub use types::*;

define_system_credential! {
    BuildkiteCredential("buildkite") {
        api_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_API_ENDPOINT: &str = "https://api.buildkite.com/v2";

#[init]
async fn setup() -> Result<()> {
    info!("Buildkite integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Buildkite integration shutting down");
}

// ============================================================================
// Tool: trigger_build
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TriggerBuildInput {
    /// Organization slug
    pub organization: String,
    /// Pipeline slug
    pub pipeline: String,
    /// Git commit SHA, reference, or tag to build
    pub commit: String,
    /// Branch containing the commit
    pub branch: String,
    /// Optional build message/description
    #[serde(default)]
    pub message: Option<String>,
    /// Optional author information
    #[serde(default)]
    pub author: Option<Author>,
    /// Optional environment variables for the build
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    /// Optional metadata for the build
    #[serde(default)]
    pub meta_data: Option<HashMap<String, String>>,
    /// Force a clean checkout
    #[serde(default)]
    pub clean_checkout: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TriggerBuildOutput {
    pub build: Build,
}

/// # Trigger Buildkite Build
///
/// Triggers a new build in a Buildkite pipeline for a specific commit and
/// branch. Use this tool when the user wants to start a new CI/CD build in
/// Buildkite, such as after pushing code, creating a pull request, or manually
/// triggering a pipeline.
///
/// Requires the organization slug, pipeline slug, commit SHA/ref/tag, and
/// branch name. Optionally supports custom build messages, author information,
/// environment variables, metadata, and forcing a clean checkout.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - ci
/// - buildkite
/// - build
///
/// # Errors
///
/// Returns an error if:
/// - The organization, pipeline, commit, or branch fields are empty or contain
///   only whitespace
/// - The Buildkite credential is not configured or the API token is empty
/// - The configured API endpoint URL is invalid
/// - The Buildkite API request fails (network error, timeout, or server error)
/// - The Buildkite API returns a non-success status code (e.g., 404 for not
///   found, 422 for validation errors)
/// - The API response cannot be parsed as a `Build` object
#[tool]
pub async fn trigger_build(ctx: Context, input: TriggerBuildInput) -> Result<TriggerBuildOutput> {
    ensure!(
        !input.organization.trim().is_empty(),
        "organization must not be empty"
    );
    ensure!(
        !input.pipeline.trim().is_empty(),
        "pipeline must not be empty"
    );
    ensure!(!input.commit.trim().is_empty(), "commit must not be empty");
    ensure!(!input.branch.trim().is_empty(), "branch must not be empty");

    let client = BuildkiteClient::from_ctx(&ctx)?;

    let request = CreateBuildRequest {
        commit: input.commit,
        branch: input.branch,
        message: input.message,
        author: input.author,
        env: input.env,
        meta_data: input.meta_data,
        clean_checkout: input.clean_checkout,
    };

    let build: Build = client
        .post_json(
            client.url_with_segments(&[
                "organizations",
                &input.organization,
                "pipelines",
                &input.pipeline,
                "builds",
            ])?,
            &request,
        )
        .await?;

    Ok(TriggerBuildOutput { build })
}

// ============================================================================
// Tool: get_build_status
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetBuildStatusInput {
    /// Organization slug
    pub organization: String,
    /// Pipeline slug
    pub pipeline: String,
    /// Build number (not ID)
    pub build_number: u64,
    /// Include all retried jobs
    #[serde(default)]
    pub include_retried_jobs: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetBuildStatusOutput {
    pub build: Build,
}

/// # Get Buildkite Build Status
///
/// Retrieves detailed status information about a specific Buildkite build.
/// Use this tool when the user wants to check the current state of a build,
/// including whether it passed, failed, is still running, or was canceled.
/// Returns complete build details including job states, commit info, and build
/// metadata.
///
/// Requires the organization slug, pipeline slug, and build number (not the
/// build ID). Optionally include all retried jobs in the response.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - ci
/// - buildkite
/// - build
///
/// # Errors
///
/// Returns an error if:
/// - The organization or pipeline fields are empty or contain only whitespace
/// - The Buildkite credential is not configured or the API token is empty
/// - The configured API endpoint URL is invalid
/// - The Buildkite API request fails (network error, timeout, or server error)
/// - The Buildkite API returns a non-success status code (e.g., 404 for not
///   found)
/// - The API response cannot be parsed as a `Build` object
#[tool]
pub async fn get_build_status(
    ctx: Context,
    input: GetBuildStatusInput,
) -> Result<GetBuildStatusOutput> {
    ensure!(
        !input.organization.trim().is_empty(),
        "organization must not be empty"
    );
    ensure!(
        !input.pipeline.trim().is_empty(),
        "pipeline must not be empty"
    );

    let client = BuildkiteClient::from_ctx(&ctx)?;

    let mut query = vec![];
    if input.include_retried_jobs {
        query.push(("include_retried_jobs", "true".to_string()));
    }

    let build: Build = client
        .get_json(
            client.url_with_segments(&[
                "organizations",
                &input.organization,
                "pipelines",
                &input.pipeline,
                "builds",
                &input.build_number.to_string(),
            ])?,
            &query,
        )
        .await?;

    Ok(GetBuildStatusOutput { build })
}

// ============================================================================
// Tool: fetch_job_logs
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FetchJobLogsInput {
    /// Organization slug
    pub organization: String,
    /// Pipeline slug
    pub pipeline: String,
    /// Build number (not ID)
    pub build_number: u64,
    /// Job ID
    pub job_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FetchJobLogsOutput {
    pub log: JobLog,
}

/// # Fetch Buildkite Job Logs
///
/// Retrieves the complete log output for a specific job within a Buildkite
/// build. Use this tool when the user wants to debug a failed build, review job
/// output, or analyze the execution logs of a specific CI/CD job.
///
/// Requires the organization slug, pipeline slug, build number (not the build
/// ID), and the job ID (available from the build status response). Returns the
/// full log content, size, and metadata.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - ci
/// - buildkite
/// - logs
///
/// # Errors
///
/// Returns an error if:
/// - The organization, pipeline, or `job_id` fields are empty or contain only
///   whitespace
/// - The Buildkite credential is not configured or the API token is empty
/// - The configured API endpoint URL is invalid
/// - The Buildkite API request fails (network error, timeout, or server error)
/// - The Buildkite API returns a non-success status code (e.g., 404 for not
///   found)
/// - The API response cannot be parsed as a `JobLog` object
#[tool]
pub async fn fetch_job_logs(ctx: Context, input: FetchJobLogsInput) -> Result<FetchJobLogsOutput> {
    ensure!(
        !input.organization.trim().is_empty(),
        "organization must not be empty"
    );
    ensure!(
        !input.pipeline.trim().is_empty(),
        "pipeline must not be empty"
    );
    ensure!(!input.job_id.trim().is_empty(), "job_id must not be empty");

    let client = BuildkiteClient::from_ctx(&ctx)?;

    let log: JobLog = client
        .get_json(
            client.url_with_segments(&[
                "organizations",
                &input.organization,
                "pipelines",
                &input.pipeline,
                "builds",
                &input.build_number.to_string(),
                "jobs",
                &input.job_id,
                "log",
            ])?,
            &[],
        )
        .await?;

    Ok(FetchJobLogsOutput { log })
}

// ============================================================================
// Tool: annotate_build
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AnnotateBuildInput {
    /// Organization slug
    pub organization: String,
    /// Pipeline slug
    pub pipeline: String,
    /// Build number (not ID)
    pub build_number: u64,
    /// Annotation body (Markdown or HTML)
    pub body: String,
    /// Visual style of the annotation
    #[serde(default)]
    pub style: Option<AnnotationStyle>,
    /// Context identifier for grouping/updating
    #[serde(default)]
    pub context: Option<String>,
    /// Whether to append to existing annotation
    #[serde(default)]
    pub append: bool,
    /// Display priority (1-10, default 3)
    #[serde(default)]
    pub priority: Option<u8>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AnnotateBuildOutput {
    pub annotation: Annotation,
}

/// # Annotate Buildkite Build
///
/// Creates a visual annotation on a Buildkite build to display messages,
/// warnings, errors, or other contextual information directly on the build
/// page. Use this tool when the user wants to add commentary, highlight test
/// results, document decisions, or provide rich context (Markdown/HTML) for
/// build viewers.
///
/// Requires the organization slug, pipeline slug, build number, and annotation
/// body. Supports visual styles (success, info, warning, error), context for
/// grouping/updating annotations, appending to existing annotations, and
/// priority (1-10) for display order.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - ci
/// - buildkite
/// - annotation
///
/// # Errors
///
/// Returns an error if:
/// - The organization, pipeline, or body fields are empty or contain only
///   whitespace
/// - The priority value is provided but not between 1 and 10 (inclusive)
/// - The Buildkite credential is not configured or the API token is empty
/// - The configured API endpoint URL is invalid
/// - The Buildkite API request fails (network error, timeout, or server error)
/// - The Buildkite API returns a non-success status code (e.g., 403 for
///   insufficient permissions)
/// - The API response cannot be parsed as an `Annotation` object
#[tool]
pub async fn annotate_build(
    ctx: Context,
    input: AnnotateBuildInput,
) -> Result<AnnotateBuildOutput> {
    ensure!(
        !input.organization.trim().is_empty(),
        "organization must not be empty"
    );
    ensure!(
        !input.pipeline.trim().is_empty(),
        "pipeline must not be empty"
    );
    ensure!(!input.body.trim().is_empty(), "body must not be empty");

    if let Some(priority) = input.priority {
        ensure!(
            (1..=10).contains(&priority),
            "priority must be between 1 and 10"
        );
    }

    let client = BuildkiteClient::from_ctx(&ctx)?;

    let request = CreateAnnotationRequest {
        body: input.body,
        style: input.style,
        context: input.context,
        append: Some(input.append),
        priority: input.priority,
    };

    let annotation: Annotation = client
        .post_json(
            client.url_with_segments(&[
                "organizations",
                &input.organization,
                "pipelines",
                &input.pipeline,
                "builds",
                &input.build_number.to_string(),
                "annotations",
            ])?,
            &request,
        )
        .await?;

    Ok(AnnotateBuildOutput { annotation })
}

// ============================================================================
// HTTP Client
// ============================================================================

#[derive(Debug, Clone)]
struct BuildkiteClient {
    http: reqwest::Client,
    base_url: String,
    api_token: String,
}

impl BuildkiteClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = BuildkiteCredential::get(ctx)?;
        ensure!(
            !cred.api_token.trim().is_empty(),
            "api_token must not be empty"
        );

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_API_ENDPOINT))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            api_token: cred.api_token,
        })
    }

    fn url_with_segments(&self, segments: &[&str]) -> Result<reqwest::Url> {
        let mut url = reqwest::Url::parse(&self.base_url)?;
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|()| anyhow::anyhow!("base_url must be an absolute URL"))?;
            for segment in segments {
                path.push(segment);
            }
        }
        Ok(url)
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        query: &[(&str, String)],
    ) -> Result<T> {
        let response = self
            .http
            .get(url)
            .query(query)
            .bearer_auth(&self.api_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.json::<T>().await?)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Buildkite API request failed ({status}): {body}"
            ))
        }
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
    ) -> Result<TRes> {
        let response = self
            .http
            .post(url)
            .json(body)
            .bearer_auth(&self.api_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.json::<TRes>().await?)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Buildkite API request failed ({status}): {body}"
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
    use std::collections::HashMap as StdHashMap;

    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_string_contains, header, method, path},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut buildkite_values = StdHashMap::new();
        buildkite_values.insert("api_token".to_string(), "test-token".to_string());
        buildkite_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_system_credential("buildkite", buildkite_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        format!("{}/v2", server.uri())
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_build_state_serialization_roundtrip() {
        for variant in [
            BuildState::Scheduled,
            BuildState::Running,
            BuildState::Passed,
            BuildState::Failed,
            BuildState::Failing,
            BuildState::Blocked,
            BuildState::Canceled,
            BuildState::Canceling,
            BuildState::Skipped,
            BuildState::NotRun,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: BuildState = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_annotation_style_serialization_roundtrip() {
        for variant in [
            AnnotationStyle::Success,
            AnnotationStyle::Info,
            AnnotationStyle::Warning,
            AnnotationStyle::Error,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: AnnotationStyle = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_author_serialization_roundtrip() {
        let author = Author {
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
        };
        let json = serde_json::to_string(&author).unwrap();
        let parsed: Author = serde_json::from_str(&json).unwrap();
        assert_eq!(author.name, parsed.name);
        assert_eq!(author.email, parsed.email);
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://api.buildkite.com/v2/").unwrap();
        assert_eq!(result, "https://api.buildkite.com/v2");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://api.buildkite.com/v2  ").unwrap();
        assert_eq!(result, "https://api.buildkite.com/v2");
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

    #[test]
    fn test_normalize_base_url_whitespace_only_returns_error() {
        let result = normalize_base_url("   ");
        assert!(result.is_err());
    }

    // --- Input validation tests ---

    #[tokio::test]
    async fn test_trigger_build_empty_organization_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = trigger_build(
            ctx,
            TriggerBuildInput {
                organization: "  ".to_string(),
                pipeline: "my-pipeline".to_string(),
                commit: "abc123".to_string(),
                branch: "main".to_string(),
                message: None,
                author: None,
                env: None,
                meta_data: None,
                clean_checkout: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("organization must not be empty")
        );
    }

    #[tokio::test]
    async fn test_trigger_build_empty_pipeline_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = trigger_build(
            ctx,
            TriggerBuildInput {
                organization: "my-org".to_string(),
                pipeline: "  ".to_string(),
                commit: "abc123".to_string(),
                branch: "main".to_string(),
                message: None,
                author: None,
                env: None,
                meta_data: None,
                clean_checkout: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("pipeline must not be empty")
        );
    }

    #[tokio::test]
    async fn test_trigger_build_empty_commit_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = trigger_build(
            ctx,
            TriggerBuildInput {
                organization: "my-org".to_string(),
                pipeline: "my-pipeline".to_string(),
                commit: "  ".to_string(),
                branch: "main".to_string(),
                message: None,
                author: None,
                env: None,
                meta_data: None,
                clean_checkout: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("commit must not be empty")
        );
    }

    #[tokio::test]
    async fn test_trigger_build_empty_branch_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = trigger_build(
            ctx,
            TriggerBuildInput {
                organization: "my-org".to_string(),
                pipeline: "my-pipeline".to_string(),
                commit: "abc123".to_string(),
                branch: "  ".to_string(),
                message: None,
                author: None,
                env: None,
                meta_data: None,
                clean_checkout: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("branch must not be empty")
        );
    }

    #[tokio::test]
    async fn test_get_build_status_empty_organization_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = get_build_status(
            ctx,
            GetBuildStatusInput {
                organization: "  ".to_string(),
                pipeline: "my-pipeline".to_string(),
                build_number: 1,
                include_retried_jobs: false,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("organization must not be empty")
        );
    }

    #[tokio::test]
    async fn test_get_build_status_empty_pipeline_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = get_build_status(
            ctx,
            GetBuildStatusInput {
                organization: "my-org".to_string(),
                pipeline: "  ".to_string(),
                build_number: 1,
                include_retried_jobs: false,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("pipeline must not be empty")
        );
    }

    #[tokio::test]
    async fn test_fetch_job_logs_empty_job_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = fetch_job_logs(
            ctx,
            FetchJobLogsInput {
                organization: "my-org".to_string(),
                pipeline: "my-pipeline".to_string(),
                build_number: 1,
                job_id: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("job_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_annotate_build_empty_body_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = annotate_build(
            ctx,
            AnnotateBuildInput {
                organization: "my-org".to_string(),
                pipeline: "my-pipeline".to_string(),
                build_number: 1,
                body: "  ".to_string(),
                style: None,
                context: None,
                append: false,
                priority: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("body must not be empty")
        );
    }

    #[tokio::test]
    async fn test_annotate_build_invalid_priority_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = annotate_build(
            ctx,
            AnnotateBuildInput {
                organization: "my-org".to_string(),
                pipeline: "my-pipeline".to_string(),
                build_number: 1,
                body: "Test".to_string(),
                style: None,
                context: None,
                append: false,
                priority: Some(11),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("priority must be between 1 and 10")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_trigger_build_success_returns_build() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "build-id-123",
          "number": 42,
          "state": "scheduled",
          "message": "Test build",
          "commit": "abc123",
          "branch": "main",
          "env": {},
          "jobs": [],
          "url": "https://api.buildkite.com/v2/organizations/my-org/pipelines/my-pipeline/builds/42",
          "web_url": "https://buildkite.com/my-org/my-pipeline/builds/42"
        }
        "#;

        Mock::given(method("POST"))
            .and(path(
                "/v2/organizations/my-org/pipelines/my-pipeline/builds",
            ))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("\"commit\":\"abc123\""))
            .and(body_string_contains("\"branch\":\"main\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = trigger_build(
            ctx,
            TriggerBuildInput {
                organization: "my-org".to_string(),
                pipeline: "my-pipeline".to_string(),
                commit: "abc123".to_string(),
                branch: "main".to_string(),
                message: Some("Test build".to_string()),
                author: None,
                env: None,
                meta_data: None,
                clean_checkout: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.build.id, "build-id-123");
        assert_eq!(output.build.number, 42);
        assert_eq!(output.build.state, BuildState::Scheduled);
        assert_eq!(output.build.commit, "abc123");
        assert_eq!(output.build.branch, "main");
    }

    #[tokio::test]
    async fn test_trigger_build_error_response_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("POST"))
            .and(path(
                "/v2/organizations/my-org/pipelines/my-pipeline/builds",
            ))
            .respond_with(
                ResponseTemplate::new(422)
                    .set_body_raw(r#"{"message":"Pipeline not found"}"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = trigger_build(
            ctx,
            TriggerBuildInput {
                organization: "my-org".to_string(),
                pipeline: "my-pipeline".to_string(),
                commit: "abc123".to_string(),
                branch: "main".to_string(),
                message: None,
                author: None,
                env: None,
                meta_data: None,
                clean_checkout: None,
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("422"));
    }

    #[tokio::test]
    async fn test_get_build_status_success_returns_build() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "build-id-123",
          "number": 42,
          "state": "passed",
          "message": "Test build",
          "commit": "abc123",
          "branch": "main",
          "env": {},
          "jobs": [
            {
              "id": "job-1",
              "type": "script",
              "name": "Test",
              "state": "passed"
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path(
                "/v2/organizations/my-org/pipelines/my-pipeline/builds/42",
            ))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = get_build_status(
            ctx,
            GetBuildStatusInput {
                organization: "my-org".to_string(),
                pipeline: "my-pipeline".to_string(),
                build_number: 42,
                include_retried_jobs: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.build.id, "build-id-123");
        assert_eq!(output.build.number, 42);
        assert_eq!(output.build.state, BuildState::Passed);
        assert_eq!(output.build.jobs.len(), 1);
        assert_eq!(output.build.jobs[0].id, "job-1");
    }

    #[tokio::test]
    async fn test_get_build_status_not_found_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path(
                "/v2/organizations/my-org/pipelines/my-pipeline/builds/999",
            ))
            .respond_with(
                ResponseTemplate::new(404)
                    .set_body_raw(r#"{"message":"Build not found"}"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = get_build_status(
            ctx,
            GetBuildStatusInput {
                organization: "my-org".to_string(),
                pipeline: "my-pipeline".to_string(),
                build_number: 999,
                include_retried_jobs: false,
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("404"));
    }

    #[tokio::test]
    async fn test_fetch_job_logs_success_returns_log() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "url": "https://api.buildkite.com/v2/organizations/my-org/pipelines/my-pipeline/builds/42/jobs/job-1/log",
          "content": "This is the job log output\nLine 2\nLine 3",
          "size": 42,
          "header_times": []
        }
        "#;

        Mock::given(method("GET"))
            .and(path(
                "/v2/organizations/my-org/pipelines/my-pipeline/builds/42/jobs/job-1/log",
            ))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = fetch_job_logs(
            ctx,
            FetchJobLogsInput {
                organization: "my-org".to_string(),
                pipeline: "my-pipeline".to_string(),
                build_number: 42,
                job_id: "job-1".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.log.size, 42);
        assert!(output.log.content.contains("This is the job log output"));
    }

    #[tokio::test]
    async fn test_fetch_job_logs_not_found_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path(
                "/v2/organizations/my-org/pipelines/my-pipeline/builds/42/jobs/missing/log",
            ))
            .respond_with(
                ResponseTemplate::new(404)
                    .set_body_raw(r#"{"message":"Job not found"}"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = fetch_job_logs(
            ctx,
            FetchJobLogsInput {
                organization: "my-org".to_string(),
                pipeline: "my-pipeline".to_string(),
                build_number: 42,
                job_id: "missing".to_string(),
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("404"));
    }

    #[tokio::test]
    async fn test_annotate_build_success_returns_annotation() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "annotation-1",
          "context": "test-context",
          "style": "info",
          "body_html": "<p>Test annotation</p>",
          "created_at": "2024-01-01T00:00:00Z",
          "updated_at": "2024-01-01T00:00:00Z"
        }
        "#;

        Mock::given(method("POST"))
            .and(path(
                "/v2/organizations/my-org/pipelines/my-pipeline/builds/42/annotations",
            ))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("\"body\":\"Test annotation\""))
            .and(body_string_contains("\"style\":\"info\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = annotate_build(
            ctx,
            AnnotateBuildInput {
                organization: "my-org".to_string(),
                pipeline: "my-pipeline".to_string(),
                build_number: 42,
                body: "Test annotation".to_string(),
                style: Some(AnnotationStyle::Info),
                context: Some("test-context".to_string()),
                append: false,
                priority: Some(5),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.annotation.id, "annotation-1");
        assert_eq!(output.annotation.context, Some("test-context".to_string()));
        assert_eq!(output.annotation.style, Some(AnnotationStyle::Info));
    }

    #[tokio::test]
    async fn test_annotate_build_error_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("POST"))
            .and(path(
                "/v2/organizations/my-org/pipelines/my-pipeline/builds/42/annotations",
            ))
            .respond_with(ResponseTemplate::new(403).set_body_raw(
                r#"{"message":"Insufficient permissions"}"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = annotate_build(
            ctx,
            AnnotateBuildInput {
                organization: "my-org".to_string(),
                pipeline: "my-pipeline".to_string(),
                build_number: 42,
                body: "Test annotation".to_string(),
                style: None,
                context: None,
                append: false,
                priority: None,
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("403"));
    }
}
