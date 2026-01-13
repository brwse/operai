//! cicd/jenkins integration for Operai Toolbox.
//!
//! Provides tools for interacting with Jenkins CI/CD server:
//! - `trigger_job`: Trigger a Jenkins job build
//! - `get_build_status`: Get the status of a specific build
//! - `fetch_console_log`: Fetch the console output of a build
//! - `download_artifact`: Download a build artifact

mod types;

use operai::{
    Context, JsonSchema, Result, define_system_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
use types::{BuildStatus, JobParameter};

define_system_credential! {
    JenkinsCredential("jenkins") {
        username: String,
        password: String,  // API token
        #[optional]
        endpoint: Option<String>,
    }
}

#[init]
async fn setup() -> Result<()> {
    info!("Jenkins integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Jenkins integration shutting down");
}

// ============================================================================
// Tool 1: Trigger Job
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TriggerJobInput {
    /// Job name or path (e.g., "my-job" or "folder/my-job").
    pub job_name: String,
    /// Optional job parameters as key-value pairs.
    #[serde(default)]
    pub parameters: Vec<JobParameter>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TriggerJobOutput {
    /// Whether the job was successfully triggered.
    pub triggered: bool,
    /// Queue item ID if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_id: Option<u64>,
}

/// # Trigger Jenkins Job
///
/// Triggers a new build of a Jenkins job. This tool initiates a build on the
/// Jenkins CI/CD server for a specified job. Use this when the user wants to
/// start a new build process, either manually or as part of an automated
/// workflow.
///
/// **When to use:**
/// - User wants to trigger a Jenkins job/build
/// - User needs to start a CI/CD pipeline
/// - User wants to run a Jenkins job with specific parameters
///
/// **Key behaviors:**
/// - For parameterized builds, provide the `parameters` list with key-value
///   pairs
/// - For non-parameterized builds, omit the `parameters` field or pass an empty
///   list
/// - Supports nested job paths using "/" separator (e.g.,
///   "folder/subfolder/job-name")
/// - Returns a queue ID when available, which can be used to track the build's
///   progress
/// - Jenkins will return 201 Created on success with a Location header pointing
///   to the queue item
///
/// **Job name format:**
/// - Simple job: "my-job"
/// - Nested job: "folder/my-job" or "folder/subfolder/my-job"
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - jenkins
/// - cicd
/// - build
///
/// # Errors
///
/// Returns an error if:
/// - The provided `job_name` is empty or contains only whitespace
/// - Jenkins credentials are missing or invalid (username/password required)
/// - The HTTP request to Jenkins fails (network error, timeout)
/// - Jenkins returns a non-success status code (4xx or 5xx)
/// - The response body cannot be parsed
#[tool]
pub async fn trigger_job(ctx: Context, input: TriggerJobInput) -> Result<TriggerJobOutput> {
    ensure!(
        !input.job_name.trim().is_empty(),
        "job_name must not be empty"
    );

    let client = JenkinsClient::from_ctx(&ctx)?;

    // Build the URL path: /job/{job_name}/buildWithParameters or
    // /job/{job_name}/build
    let job_path = format!("job/{}", input.job_name.replace('/', "/job/"));

    let endpoint = if input.parameters.is_empty() {
        format!("{job_path}/build")
    } else {
        format!("{job_path}/buildWithParameters")
    };

    // Convert parameters to query string
    let params: Vec<(String, String)> = input
        .parameters
        .into_iter()
        .map(|p| (p.name, p.value))
        .collect();

    // Jenkins returns 201 Created with Location header pointing to queue item
    let response = client.post(&endpoint, &params).await?;

    // Extract queue ID from Location header if present
    let queue_id = response
        .headers()
        .get("Location")
        .and_then(|loc| loc.to_str().ok())
        .and_then(|loc| {
            // Location is like: http://jenkins/queue/item/123/
            loc.split('/').rev().nth(1).and_then(|s| s.parse().ok())
        });

    Ok(TriggerJobOutput {
        triggered: true,
        queue_id,
    })
}

// ============================================================================
// Tool 2: Get Build Status
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetBuildStatusInput {
    /// Job name or path.
    pub job_name: String,
    /// Build number. Use "lastBuild" for the most recent build.
    pub build_number: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetBuildStatusOutput {
    pub status: BuildStatus,
}

/// # Get Jenkins Build Status
///
/// Retrieves the current status and details of a specific Jenkins build. Use
/// this tool when the user wants to check if a build is still running, has
/// completed, or has failed. This provides comprehensive information about the
/// build including its result, duration, and timestamp.
///
/// **When to use:**
/// - User wants to check the status of a Jenkins build
/// - User needs to know if a build has completed or is still running
/// - User wants to see build details (duration, result, timestamp)
/// - User needs to monitor a build's progress after triggering it
///
/// **Key information returned:**
/// - `building`: Whether the build is currently in progress
/// - `result`: The final build status (SUCCESS, FAILURE, UNSTABLE, ABORTED,
///   `NOT_BUILT`) when complete
/// - `duration`: How long the build took (in milliseconds)
/// - `timestamp`: When the build started (Unix epoch in milliseconds)
/// - `url`: Direct link to the build in Jenkins UI
///
/// **Build number formats:**
/// - Specific build: "42" (for build #42)
/// - Last build: "lastBuild" (most recent build)
/// - Special Jenkins identifiers: "lastStableBuild", "lastSuccessfulBuild",
///   "lastFailedBuild"
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - jenkins
/// - cicd
/// - build
/// - status
///
/// # Errors
///
/// Returns an error if:
/// - The provided `job_name` is empty or contains only whitespace
/// - The provided `build_number` is empty or contains only whitespace
/// - Jenkins credentials are missing or invalid
/// - The HTTP request to Jenkins fails
/// - Jenkins returns a non-success status code
/// - The response JSON cannot be parsed as `BuildStatus`
#[tool]
pub async fn get_build_status(
    ctx: Context,
    input: GetBuildStatusInput,
) -> Result<GetBuildStatusOutput> {
    ensure!(
        !input.job_name.trim().is_empty(),
        "job_name must not be empty"
    );
    ensure!(
        !input.build_number.trim().is_empty(),
        "build_number must not be empty"
    );

    let client = JenkinsClient::from_ctx(&ctx)?;

    let job_path = format!("job/{}", input.job_name.replace('/', "/job/"));
    let endpoint = format!("{}/{}/api/json", job_path, input.build_number);

    let status: BuildStatus = client.get_json(&endpoint).await?;

    Ok(GetBuildStatusOutput { status })
}

// ============================================================================
// Tool 3: Fetch Console Log
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FetchConsoleLogInput {
    /// Job name or path.
    pub job_name: String,
    /// Build number.
    pub build_number: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FetchConsoleLogOutput {
    /// Console log output as plain text.
    pub console_log: String,
}

/// # Fetch Jenkins Build Console Log
///
/// Retrieves the complete console output (log) from a Jenkins build. This tool
/// provides the full text output from the build execution, including all log
/// messages, errors, warnings, and build steps. Use this when the user wants to
/// debug a failed build, review build output, or understand what
/// happened during a build execution.
///
/// **When to use:**
/// - User wants to see the logs/output from a Jenkins build
/// - User needs to debug why a build failed
/// - User wants to review build execution details
/// - User needs to analyze build warnings or errors
///
/// **Key behaviors:**
/// - Returns the full console text as a single string
/// - Includes all output from build start to finish
/// - Useful for diagnosing build failures, test failures, or deployment issues
/// - Can be used on both running and completed builds
///
/// **Common use cases:**
/// - Diagnosing build failures after getting a FAILURE status from
///   `get_build_status`
/// - Checking test output and error messages
/// - Verifying build steps executed correctly
/// - Reviewing deployment logs
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - jenkins
/// - cicd
/// - build
/// - logs
///
/// # Errors
///
/// Returns an error if:
/// - The provided `job_name` is empty or contains only whitespace
/// - The provided `build_number` is empty or contains only whitespace
/// - Jenkins credentials are missing or invalid
/// - The HTTP request to Jenkins fails
/// - Jenkins returns a non-success status code
/// - The response text cannot be parsed
#[tool]
pub async fn fetch_console_log(
    ctx: Context,
    input: FetchConsoleLogInput,
) -> Result<FetchConsoleLogOutput> {
    ensure!(
        !input.job_name.trim().is_empty(),
        "job_name must not be empty"
    );
    ensure!(
        !input.build_number.trim().is_empty(),
        "build_number must not be empty"
    );

    let client = JenkinsClient::from_ctx(&ctx)?;

    let job_path = format!("job/{}", input.job_name.replace('/', "/job/"));
    let endpoint = format!("{}/{}/consoleText", job_path, input.build_number);

    let console_log = client.get_text(&endpoint).await?;

    Ok(FetchConsoleLogOutput { console_log })
}

// ============================================================================
// Tool 4: Download Artifact
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DownloadArtifactInput {
    /// Job name or path.
    pub job_name: String,
    /// Build number.
    pub build_number: String,
    /// Artifact relative path (from build's artifacts list).
    pub artifact_path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DownloadArtifactOutput {
    /// Artifact content as base64-encoded bytes.
    pub content: String,
    /// Artifact file name.
    pub file_name: String,
}

/// # Download Jenkins Build Artifact
///
/// Downloads a specific artifact file from a Jenkins build. Artifacts are files
/// generated during the build process, such as compiled binaries, JAR files,
/// test reports, documentation, or deployment packages. Use this tool when the
/// user wants to retrieve build outputs for deployment, testing,
/// or distribution.
///
/// **When to use:**
/// - User wants to download build outputs (binaries, libraries, packages)
/// - User needs to retrieve compiled artifacts from a Jenkins build
/// - User wants to deploy artifacts built by Jenkins
/// - User needs to access test reports or other build-generated files
///
/// **Key behaviors:**
/// - Returns artifact content as base64-encoded data for safe transmission
/// - Includes the original file name in the response
/// - Supports any file type (binaries, text files, archives, etc.)
/// - User must provide the exact relative path to the artifact as it appears in
///   Jenkins
///
/// **Artifact path format:**
/// - Use the relative path from the build's artifact root
/// - Examples: "target/app.jar", "dist/project.tar.gz",
///   "reports/test-report.html"
/// - For nested artifacts: "path/to/file.ext"
/// - Use Jenkins UI or API to discover available artifact paths if unknown
///
/// **Common artifact types:**
/// - Java: JAR files (target/*.jar)
/// - Node.js: Packaged tarballs (dist/*.tgz)
/// - Python: Wheel or source distributions (dist/*.whl)
/// - Reports: HTML/PDF test reports, coverage reports
/// - Deployables: Docker images, cloud formation templates, etc.
///
/// **Important notes:**
/// - The returned content is base64-encoded and must be decoded to get the
///   original file
/// - Large artifacts may be returned as significant data payloads
/// - Ensure the artifact exists before downloading (check build status first)
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - jenkins
/// - cicd
/// - build
/// - artifacts
///
/// # Errors
///
/// Returns an error if:
/// - The provided `job_name` is empty or contains only whitespace
/// - The provided `build_number` is empty or contains only whitespace
/// - The provided `artifact_path` is empty or contains only whitespace
/// - Jenkins credentials are missing or invalid
/// - The HTTP request to Jenkins fails
/// - Jenkins returns a non-success status code
/// - The response body cannot be read
#[tool]
pub async fn download_artifact(
    ctx: Context,
    input: DownloadArtifactInput,
) -> Result<DownloadArtifactOutput> {
    ensure!(
        !input.job_name.trim().is_empty(),
        "job_name must not be empty"
    );
    ensure!(
        !input.build_number.trim().is_empty(),
        "build_number must not be empty"
    );
    ensure!(
        !input.artifact_path.trim().is_empty(),
        "artifact_path must not be empty"
    );

    let client = JenkinsClient::from_ctx(&ctx)?;

    let job_path = format!("job/{}", input.job_name.replace('/', "/job/"));
    let endpoint = format!(
        "{}/{}/artifact/{}",
        job_path, input.build_number, input.artifact_path
    );

    let bytes = client.get_bytes(&endpoint).await?;

    // Extract file name from artifact_path
    let file_name = input
        .artifact_path
        .split('/')
        .next_back()
        .unwrap_or(&input.artifact_path)
        .to_string();

    // Encode as base64
    let content = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);

    Ok(DownloadArtifactOutput { content, file_name })
}

// ============================================================================
// Jenkins HTTP Client
// ============================================================================

#[derive(Debug, Clone)]
struct JenkinsClient {
    http: reqwest::Client,
    base_url: String,
    username: String,
    password: String,
}

impl JenkinsClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = JenkinsCredential::get(ctx)?;
        ensure!(
            !cred.username.trim().is_empty(),
            "username must not be empty"
        );
        ensure!(
            !cred.password.trim().is_empty(),
            "password must not be empty"
        );

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or("http://localhost:8080"))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            username: cred.username,
            password: cred.password,
        })
    }

    fn build_url(&self, path: &str) -> String {
        let trimmed_path = path.trim_start_matches('/');
        format!("{}/{}", self.base_url, trimmed_path)
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = self.build_url(path);
        let response = self
            .http
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?;

        Self::check_response_status(&response)?;
        Ok(response.json::<T>().await?)
    }

    async fn get_text(&self, path: &str) -> Result<String> {
        let url = self.build_url(path);
        let response = self
            .http
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?;

        Self::check_response_status(&response)?;
        Ok(response.text().await?)
    }

    async fn get_bytes(&self, path: &str) -> Result<Vec<u8>> {
        let url = self.build_url(path);
        let response = self
            .http
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?;

        Self::check_response_status(&response)?;
        Ok(response.bytes().await?.to_vec())
    }

    async fn post(&self, path: &str, params: &[(String, String)]) -> Result<reqwest::Response> {
        let url = self.build_url(path);
        let response = self
            .http
            .post(&url)
            .basic_auth(&self.username, Some(&self.password))
            .query(params)
            .send()
            .await?;

        Self::check_response_status(&response)?;
        Ok(response)
    }

    fn check_response_status(response: &reqwest::Response) -> Result<()> {
        let status = response.status();
        if status.is_success() || status.as_u16() == 201 {
            Ok(())
        } else {
            Err(operai::anyhow::anyhow!(
                "Jenkins request failed with status {status}"
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

    use types::BuildResult;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{basic_auth, method, path},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut jenkins_values = HashMap::new();
        jenkins_values.insert("username".to_string(), "test-user".to_string());
        jenkins_values.insert("password".to_string(), "test-token".to_string());
        jenkins_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_system_credential("jenkins", jenkins_values)
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_build_result_serialization_roundtrip() {
        for variant in [
            BuildResult::Success,
            BuildResult::Failure,
            BuildResult::Unstable,
            BuildResult::Aborted,
            BuildResult::NotBuilt,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: BuildResult = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_build_result_deserializes_from_jenkins_format() {
        assert_eq!(
            serde_json::from_str::<BuildResult>("\"SUCCESS\"").unwrap(),
            BuildResult::Success
        );
        assert_eq!(
            serde_json::from_str::<BuildResult>("\"FAILURE\"").unwrap(),
            BuildResult::Failure
        );
        assert_eq!(
            serde_json::from_str::<BuildResult>("\"NOT_BUILT\"").unwrap(),
            BuildResult::NotBuilt
        );
    }

    #[test]
    fn test_job_parameter_serialization_roundtrip() {
        let param = JobParameter {
            name: "BRANCH".to_string(),
            value: "main".to_string(),
        };
        let json = serde_json::to_string(&param).unwrap();
        let parsed: JobParameter = serde_json::from_str(&json).unwrap();
        assert_eq!(param.name, parsed.name);
        assert_eq!(param.value, parsed.value);
    }

    #[test]
    fn test_build_status_serialization_roundtrip() {
        let status = BuildStatus {
            number: 42,
            display_name: Some("Build #42".to_string()),
            full_display_name: None,
            id: None,
            url: "http://jenkins/job/test/42/".to_string(),
            building: false,
            result: Some(BuildResult::Success),
            duration: 120_000,
            estimated_duration: Some(115_000),
            timestamp: 1_704_067_200_000,
            actions: vec![],
        };
        let json = serde_json::to_string(&status).unwrap();
        let parsed: BuildStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status.number, parsed.number);
        assert_eq!(status.building, parsed.building);
        assert_eq!(status.result, parsed.result);
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("http://jenkins.example.com/").unwrap();
        assert_eq!(result, "http://jenkins.example.com");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  http://jenkins.example.com  ").unwrap();
        assert_eq!(result, "http://jenkins.example.com");
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
    async fn test_trigger_job_empty_job_name_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = trigger_job(
            ctx,
            TriggerJobInput {
                job_name: "   ".to_string(),
                parameters: vec![],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("job_name must not be empty")
        );
    }

    #[tokio::test]
    async fn test_get_build_status_empty_job_name_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = get_build_status(
            ctx,
            GetBuildStatusInput {
                job_name: "  ".to_string(),
                build_number: "1".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("job_name must not be empty")
        );
    }

    #[tokio::test]
    async fn test_get_build_status_empty_build_number_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = get_build_status(
            ctx,
            GetBuildStatusInput {
                job_name: "my-job".to_string(),
                build_number: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("build_number must not be empty")
        );
    }

    #[tokio::test]
    async fn test_fetch_console_log_empty_job_name_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = fetch_console_log(
            ctx,
            FetchConsoleLogInput {
                job_name: "  ".to_string(),
                build_number: "1".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("job_name must not be empty")
        );
    }

    #[tokio::test]
    async fn test_download_artifact_empty_artifact_path_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = download_artifact(
            ctx,
            DownloadArtifactInput {
                job_name: "my-job".to_string(),
                build_number: "1".to_string(),
                artifact_path: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("artifact_path must not be empty")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_trigger_job_without_parameters_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/job/my-job/build"))
            .and(basic_auth("test-user", "test-token"))
            .respond_with(
                ResponseTemplate::new(201)
                    .insert_header("Location", format!("{}/queue/item/123/", server.uri())),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = trigger_job(
            ctx,
            TriggerJobInput {
                job_name: "my-job".to_string(),
                parameters: vec![],
            },
        )
        .await
        .unwrap();

        assert!(output.triggered);
        assert_eq!(output.queue_id, Some(123));
    }

    #[tokio::test]
    async fn test_trigger_job_with_parameters_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/job/my-job/buildWithParameters"))
            .and(basic_auth("test-user", "test-token"))
            .respond_with(ResponseTemplate::new(201))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = trigger_job(
            ctx,
            TriggerJobInput {
                job_name: "my-job".to_string(),
                parameters: vec![JobParameter {
                    name: "BRANCH".to_string(),
                    value: "main".to_string(),
                }],
            },
        )
        .await
        .unwrap();

        assert!(output.triggered);
    }

    #[tokio::test]
    async fn test_trigger_job_with_nested_job_path() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/job/folder/job/my-job/build"))
            .and(basic_auth("test-user", "test-token"))
            .respond_with(ResponseTemplate::new(201))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = trigger_job(
            ctx,
            TriggerJobInput {
                job_name: "folder/my-job".to_string(),
                parameters: vec![],
            },
        )
        .await
        .unwrap();

        assert!(output.triggered);
    }

    #[tokio::test]
    async fn test_get_build_status_success() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "number": 42,
          "displayName": "Build #42",
          "url": "http://jenkins/job/my-job/42/",
          "building": false,
          "result": "SUCCESS",
          "duration": 120000,
          "estimatedDuration": 115000,
          "timestamp": 1704067200000
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/job/my-job/42/api/json"))
            .and(basic_auth("test-user", "test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = get_build_status(
            ctx,
            GetBuildStatusInput {
                job_name: "my-job".to_string(),
                build_number: "42".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.status.number, 42);
        assert!(!output.status.building);
        assert_eq!(output.status.result, Some(BuildResult::Success));
        assert_eq!(output.status.duration, 120_000);
    }

    #[tokio::test]
    async fn test_get_build_status_building() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "number": 43,
          "url": "http://jenkins/job/my-job/43/",
          "building": true,
          "result": null,
          "duration": 0,
          "timestamp": 1704067200000
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/job/my-job/lastBuild/api/json"))
            .and(basic_auth("test-user", "test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = get_build_status(
            ctx,
            GetBuildStatusInput {
                job_name: "my-job".to_string(),
                build_number: "lastBuild".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.status.number, 43);
        assert!(output.status.building);
        assert_eq!(output.status.result, None);
    }

    #[tokio::test]
    async fn test_fetch_console_log_success() {
        let server = MockServer::start().await;

        let console_output = "Started by user admin\nBuilding in workspace\nFinished: SUCCESS";

        Mock::given(method("GET"))
            .and(path("/job/my-job/42/consoleText"))
            .and(basic_auth("test-user", "test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(console_output, "text/plain"))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = fetch_console_log(
            ctx,
            FetchConsoleLogInput {
                job_name: "my-job".to_string(),
                build_number: "42".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.console_log, console_output);
    }

    #[tokio::test]
    async fn test_download_artifact_success() {
        let server = MockServer::start().await;

        let artifact_content = b"artifact file content";

        Mock::given(method("GET"))
            .and(path("/job/my-job/42/artifact/target/app.jar"))
            .and(basic_auth("test-user", "test-token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw(artifact_content.to_vec(), "application/octet-stream"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = download_artifact(
            ctx,
            DownloadArtifactInput {
                job_name: "my-job".to_string(),
                build_number: "42".to_string(),
                artifact_path: "target/app.jar".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.file_name, "app.jar");
        // Decode base64 and verify
        let decoded =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &output.content)
                .unwrap();
        assert_eq!(decoded, artifact_content);
    }

    #[tokio::test]
    async fn test_jenkins_client_handles_401_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/job/my-job/42/api/json"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let result = get_build_status(
            ctx,
            GetBuildStatusInput {
                job_name: "my-job".to_string(),
                build_number: "42".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("401"));
    }

    #[tokio::test]
    async fn test_jenkins_client_handles_404_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/job/missing-job/1/api/json"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let result = get_build_status(
            ctx,
            GetBuildStatusInput {
                job_name: "missing-job".to_string(),
                build_number: "1".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("404"));
    }

    // --- Additional edge case tests ---

    #[tokio::test]
    async fn test_trigger_job_empty_username_returns_error() {
        let server = MockServer::start().await;

        let mut jenkins_values = HashMap::new();
        jenkins_values.insert("username".to_string(), String::new());
        jenkins_values.insert("password".to_string(), "test-token".to_string());
        jenkins_values.insert("endpoint".to_string(), server.uri());

        let ctx = Context::with_metadata("req-123", "sess-456", "user-789")
            .with_system_credential("jenkins", jenkins_values);

        let result = trigger_job(
            ctx,
            TriggerJobInput {
                job_name: "my-job".to_string(),
                parameters: vec![],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("username must not be empty")
        );
    }

    #[tokio::test]
    async fn test_trigger_job_empty_password_returns_error() {
        let server = MockServer::start().await;

        let mut jenkins_values = HashMap::new();
        jenkins_values.insert("username".to_string(), "test-user".to_string());
        jenkins_values.insert("password".to_string(), String::new());
        jenkins_values.insert("endpoint".to_string(), server.uri());

        let ctx = Context::with_metadata("req-123", "sess-456", "user-789")
            .with_system_credential("jenkins", jenkins_values);

        let result = trigger_job(
            ctx,
            TriggerJobInput {
                job_name: "my-job".to_string(),
                parameters: vec![],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("password must not be empty")
        );
    }

    #[tokio::test]
    async fn test_trigger_job_without_location_header() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/job/my-job/build"))
            .and(basic_auth("test-user", "test-token"))
            .respond_with(ResponseTemplate::new(201))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = trigger_job(
            ctx,
            TriggerJobInput {
                job_name: "my-job".to_string(),
                parameters: vec![],
            },
        )
        .await
        .unwrap();

        assert!(output.triggered);
        assert_eq!(output.queue_id, None);
    }

    #[tokio::test]
    async fn test_trigger_job_with_malformed_location_header() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/job/my-job/build"))
            .and(basic_auth("test-user", "test-token"))
            .respond_with(
                ResponseTemplate::new(201)
                    .insert_header("Location", "http://jenkins/queue/invalid/"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = trigger_job(
            ctx,
            TriggerJobInput {
                job_name: "my-job".to_string(),
                parameters: vec![],
            },
        )
        .await
        .unwrap();

        assert!(output.triggered);
        assert_eq!(output.queue_id, None);
    }

    #[tokio::test]
    async fn test_get_build_status_with_actions_field() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "number": 42,
          "displayName": "Build #42",
          "fullDisplayName": "my-project » my-job #42",
          "id": "42",
          "url": "http://jenkins/job/my-job/42/",
          "building": false,
          "result": "SUCCESS",
          "duration": 120000,
          "estimatedDuration": 115000,
          "timestamp": 1704067200000,
          "actions": [
            {
              "_class": "hudson.model.ParametersAction",
              "parameters": [
                {"name": "BRANCH", "value": "main"},
                {"name": "ENVIRONMENT", "value": "production"}
              ]
            },
            {
              "_class": "hudson.model.CauseAction",
              "causes": [
                {"shortDescription": "Started by user admin", "userId": "admin"}
              ]
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/job/my-job/42/api/json"))
            .and(basic_auth("test-user", "test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = get_build_status(
            ctx,
            GetBuildStatusInput {
                job_name: "my-job".to_string(),
                build_number: "42".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.status.number, 42);
        assert_eq!(
            output.status.full_display_name,
            Some("my-project » my-job #42".to_string())
        );
        assert_eq!(output.status.id, Some("42".to_string()));
        assert!(!output.status.building);
        assert_eq!(output.status.result, Some(BuildResult::Success));
        assert!(!output.status.actions.is_empty());
    }

    #[test]
    fn test_build_status_serialization_with_new_fields() {
        let status = BuildStatus {
            number: 42,
            display_name: Some("Build #42".to_string()),
            full_display_name: Some("my-project » my-job #42".to_string()),
            id: Some("42".to_string()),
            url: "http://jenkins/job/test/42/".to_string(),
            building: false,
            result: Some(BuildResult::Success),
            duration: 120_000,
            estimated_duration: Some(115_000),
            timestamp: 1_704_067_200_000,
            actions: vec![],
        };
        let json = serde_json::to_string(&status).unwrap();
        let parsed: BuildStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status.number, parsed.number);
        assert_eq!(status.full_display_name, parsed.full_display_name);
        assert_eq!(status.id, parsed.id);
    }
}
