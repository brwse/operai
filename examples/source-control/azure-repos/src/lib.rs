//! source-control/azure-repos integration for Operai Toolbox.

mod types;

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
use types::{CommentThread, PullRequest, PullRequestStatus, Repository, Reviewer};

define_user_credential! {
    AzureReposCredential("azure_repos") {
        access_token: String,
        organization: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_API_VERSION: &str = "7.1";

/// Initializes the Azure Repos integration.
///
/// # Errors
///
/// This function currently does not return any errors but uses the Result type
/// for consistency with the `operai` framework.
#[init]
async fn setup() -> Result<()> {
    info!("Azure Repos integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Azure Repos integration shutting down");
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListReposInput {
    /// Project name or ID.
    pub project: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListReposOutput {
    pub repositories: Vec<Repository>,
}

/// # List Azure Repos Repositories
///
/// Lists all Git repositories in a specified Azure DevOps project.
/// Use this tool when a user needs to browse, discover, or enumerate
/// repositories within their Azure DevOps organization. This is useful for
/// exploring available repositories before performing operations on them.
///
/// Requires the project name or ID. Returns a list of repositories with
/// metadata including repository ID, name, default branch, and URLs.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - source-control
/// - azure-repos
/// - azure-devops
///
/// # Errors
///
/// Returns an error if:
/// - The project name is empty or contains only whitespace
/// - No Azure Repos credentials are configured or the access token/organization
///   is empty
/// - The HTTP request to the Azure DevOps API fails
/// - The API response cannot be parsed as a list of repositories
#[tool]
pub async fn list_repos(ctx: Context, input: ListReposInput) -> Result<ListReposOutput> {
    ensure!(
        !input.project.trim().is_empty(),
        "project must not be empty"
    );

    let client = AzureReposClient::from_ctx(&ctx)?;
    let response: AzureListResponse<Repository> = client
        .get_json(
            client.url_with_segments(&[&input.project, "_apis", "git", "repositories"])?,
            &[("api-version", DEFAULT_API_VERSION.to_string())],
        )
        .await?;

    Ok(ListReposOutput {
        repositories: response.value,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreatePrInput {
    /// Project name or ID.
    pub project: String,
    /// Repository ID or name.
    pub repository_id: String,
    /// Title of the pull request.
    pub title: String,
    /// Optional description of the pull request.
    #[serde(default)]
    pub description: Option<String>,
    /// Source branch ref (e.g., "refs/heads/feature-branch").
    pub source_ref_name: String,
    /// Target branch ref (e.g., "refs/heads/main").
    pub target_ref_name: String,
    /// Optional reviewers to add (user IDs).
    #[serde(default)]
    pub reviewers: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreatePrOutput {
    pub pull_request: PullRequest,
}

/// # Create Azure Repos Pull Request
///
/// Creates a new pull request in an Azure Repos repository to propose merging
/// changes from a source branch into a target branch.
/// Use this tool when a user wants to initiate a code review process or merge
/// changes from one branch to another in their Azure DevOps repository.
///
/// Requires project name, repository ID, pull request title, source branch,
/// and target branch. Optionally accepts a description and a list of reviewer
/// IDs to add as reviewers. Branch references must be in the format
/// "refs/heads/branch-name".
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - source-control
/// - azure-repos
/// - azure-devops
///
/// # Errors
///
/// Returns an error if:
/// - The project name, repository ID, title, source ref, or target ref is empty
///   or contains only whitespace
/// - No Azure Repos credentials are configured or the access token/organization
///   is empty
/// - The HTTP request to the Azure DevOps API fails
/// - The API response cannot be parsed as a pull request
#[tool]
pub async fn create_pr(ctx: Context, input: CreatePrInput) -> Result<CreatePrOutput> {
    ensure!(
        !input.project.trim().is_empty(),
        "project must not be empty"
    );
    ensure!(
        !input.repository_id.trim().is_empty(),
        "repository_id must not be empty"
    );
    ensure!(!input.title.trim().is_empty(), "title must not be empty");
    ensure!(
        !input.source_ref_name.trim().is_empty(),
        "source_ref_name must not be empty"
    );
    ensure!(
        !input.target_ref_name.trim().is_empty(),
        "target_ref_name must not be empty"
    );

    let client = AzureReposClient::from_ctx(&ctx)?;

    let request = CreatePrRequest {
        source_ref_name: input.source_ref_name,
        target_ref_name: input.target_ref_name,
        title: input.title,
        description: input.description.unwrap_or_default(),
        reviewers: input
            .reviewers
            .into_iter()
            .map(|id| ReviewerRequest { id })
            .collect(),
    };

    let pull_request: PullRequest = client
        .post_json(
            client.url_with_segments(&[
                &input.project,
                "_apis",
                "git",
                "repositories",
                &input.repository_id,
                "pullrequests",
            ])?,
            &request,
            &[("api-version", DEFAULT_API_VERSION.to_string())],
        )
        .await?;

    Ok(CreatePrOutput { pull_request })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CommentInput {
    /// Project name or ID.
    pub project: String,
    /// Repository ID or name.
    pub repository_id: String,
    /// Pull request ID.
    pub pull_request_id: i32,
    /// Comment text.
    pub comment: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CommentOutput {
    pub thread: CommentThread,
}

/// # Comment on Azure Repos Pull Request
///
/// Adds a new comment thread to an existing Azure Repos pull request.
/// Use this tool when a user wants to provide feedback, ask questions, or
/// discuss specific aspects of a pull request. This creates a new comment
/// thread that can be replied to by other reviewers.
///
/// Requires project name, repository ID, pull request ID, and comment text.
/// Returns the created comment thread with its ID and metadata.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - source-control
/// - azure-repos
/// - azure-devops
///
/// # Errors
///
/// Returns an error if:
/// - The project name, repository ID, or comment text is empty or contains only
///   whitespace
/// - The pull request ID is not positive
/// - No Azure Repos credentials are configured or the access token/organization
///   is empty
/// - The HTTP request to the Azure DevOps API fails
/// - The API response cannot be parsed as a comment thread
#[tool]
pub async fn comment(ctx: Context, input: CommentInput) -> Result<CommentOutput> {
    ensure!(
        !input.project.trim().is_empty(),
        "project must not be empty"
    );
    ensure!(
        !input.repository_id.trim().is_empty(),
        "repository_id must not be empty"
    );
    ensure!(
        input.pull_request_id > 0,
        "pull_request_id must be positive"
    );
    ensure!(
        !input.comment.trim().is_empty(),
        "comment must not be empty"
    );

    let client = AzureReposClient::from_ctx(&ctx)?;

    let request = CreateThreadRequest {
        comments: vec![CommentRequest {
            parent_comment_id: 0,
            content: input.comment,
            comment_type: 1,
        }],
        status: 1,
    };

    let thread: CommentThread = client
        .post_json(
            client.url_with_segments(&[
                &input.project,
                "_apis",
                "git",
                "repositories",
                &input.repository_id,
                "pullRequests",
                &input.pull_request_id.to_string(),
                "threads",
            ])?,
            &request,
            &[("api-version", DEFAULT_API_VERSION.to_string())],
        )
        .await?;

    Ok(CommentOutput { thread })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ApproveInput {
    /// Project name or ID.
    pub project: String,
    /// Repository ID or name.
    pub repository_id: String,
    /// Pull request ID.
    pub pull_request_id: i32,
    /// Vote: 10 = approved, 5 = approved with suggestions, 0 = no vote, -5 =
    /// waiting for author, -10 = rejected.
    pub vote: i32,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ApproveOutput {
    pub reviewer: Reviewer,
}

/// # Approve Azure Repos Pull Request
///
/// Submits an approval vote on an Azure Repos pull request.
/// Use this tool when a user wants to approve, reject, or provide feedback on a
/// pull request as a reviewer. The vote system supports granular feedback from
/// full approval to rejection with various intermediate states.
///
/// Requires project name, repository ID, pull request ID, and vote value.
/// Vote values range from -10 (rejected) to 10 (approved):
/// - 10: Approved
/// - 5: Approved with suggestions
/// - 0: No vote / Reset vote
/// - -5: Waiting for author
/// - -10: Rejected
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - source-control
/// - azure-repos
/// - azure-devops
///
/// # Errors
///
/// Returns an error if:
/// - The project name or repository ID is empty or contains only whitespace
/// - The pull request ID is not positive
/// - The vote value is not between -10 and 10
/// - No Azure Repos credentials are configured or the access token/organization
///   is empty
/// - The HTTP request to the Azure DevOps API fails
/// - The API response cannot be parsed as a reviewer
#[tool]
pub async fn approve(ctx: Context, input: ApproveInput) -> Result<ApproveOutput> {
    ensure!(
        !input.project.trim().is_empty(),
        "project must not be empty"
    );
    ensure!(
        !input.repository_id.trim().is_empty(),
        "repository_id must not be empty"
    );
    ensure!(
        input.pull_request_id > 0,
        "pull_request_id must be positive"
    );
    ensure!(
        (-10..=10).contains(&input.vote),
        "vote must be between -10 and 10"
    );

    let client = AzureReposClient::from_ctx(&ctx)?;
    let reviewer_id = "me"; // Azure DevOps uses "me" to refer to the authenticated user

    let request = ReviewerVoteRequest { vote: input.vote };

    let reviewer: Reviewer = client
        .put_json(
            client.url_with_segments(&[
                &input.project,
                "_apis",
                "git",
                "repositories",
                &input.repository_id,
                "pullRequests",
                &input.pull_request_id.to_string(),
                "reviewers",
                reviewer_id,
            ])?,
            &request,
            &[("api-version", DEFAULT_API_VERSION.to_string())],
        )
        .await?;

    Ok(ApproveOutput { reviewer })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MergeInput {
    /// Project name or ID.
    pub project: String,
    /// Repository ID or name.
    pub repository_id: String,
    /// Pull request ID.
    pub pull_request_id: i32,
    /// Merge commit message (optional).
    #[serde(default)]
    pub commit_message: Option<String>,
    /// Whether to delete source branch after merge.
    #[serde(default)]
    pub delete_source_branch: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MergeOutput {
    pub pull_request: PullRequest,
}

/// # Merge Azure Repos Pull Request
///
/// Completes and merges an Azure Repos pull request into its target branch.
/// Use this tool when a user wants to finalize a pull request after it has been
/// reviewed and approved. This will merge the source branch into the target
/// branch using the configured merge strategy.
///
/// Requires project name, repository ID, and pull request ID. Optionally
/// accepts a custom merge commit message and a flag to delete the source branch
/// after merge. If no commit message is provided, a default message will be
/// generated.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - source-control
/// - azure-repos
/// - azure-devops
///
/// # Errors
///
/// Returns an error if:
/// - The project name or repository ID is empty or contains only whitespace
/// - The pull request ID is not positive
/// - No Azure Repos credentials are configured or the access token/organization
///   is empty
/// - The HTTP request to the Azure DevOps API fails
/// - The API response cannot be parsed as a pull request
#[tool]
pub async fn merge(ctx: Context, input: MergeInput) -> Result<MergeOutput> {
    ensure!(
        !input.project.trim().is_empty(),
        "project must not be empty"
    );
    ensure!(
        !input.repository_id.trim().is_empty(),
        "repository_id must not be empty"
    );
    ensure!(
        input.pull_request_id > 0,
        "pull_request_id must be positive"
    );

    let client = AzureReposClient::from_ctx(&ctx)?;

    let mut completion_options = CompletionOptions {
        delete_source_branch: input.delete_source_branch,
        merge_commit_message: input.commit_message.clone(),
    };

    // Set default message if not provided
    if completion_options.merge_commit_message.is_none() {
        completion_options.merge_commit_message =
            Some(format!("Merged PR {}", input.pull_request_id));
    }

    let request = MergePrRequest {
        status: PullRequestStatus::Completed,
        completion_options: Some(completion_options),
    };

    let pull_request: PullRequest = client
        .patch_json(
            client.url_with_segments(&[
                &input.project,
                "_apis",
                "git",
                "repositories",
                &input.repository_id,
                "pullRequests",
                &input.pull_request_id.to_string(),
            ])?,
            &request,
            &[("api-version", DEFAULT_API_VERSION.to_string())],
        )
        .await?;

    Ok(MergeOutput { pull_request })
}

// Internal API request/response types

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreatePrRequest {
    source_ref_name: String,
    target_ref_name: String,
    title: String,
    description: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    reviewers: Vec<ReviewerRequest>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewerRequest {
    id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateThreadRequest {
    comments: Vec<CommentRequest>,
    status: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommentRequest {
    parent_comment_id: i32,
    content: String,
    comment_type: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewerVoteRequest {
    vote: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MergePrRequest {
    status: PullRequestStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    completion_options: Option<CompletionOptions>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompletionOptions {
    delete_source_branch: bool,
    merge_commit_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AzureListResponse<T> {
    value: Vec<T>,
}

// HTTP Client

#[derive(Debug, Clone)]
struct AzureReposClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl AzureReposClient {
    /// Creates a new Azure Repos client from the tool context.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No Azure Repos credentials are configured in the context
    /// - The access token or organization name is empty
    /// - The configured endpoint URL is invalid
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = AzureReposCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );
        ensure!(
            !cred.organization.trim().is_empty(),
            "organization must not be empty"
        );

        let base_url = if let Some(endpoint) = cred.endpoint {
            normalize_base_url(&endpoint)?
        } else {
            format!("https://dev.azure.com/{}", cred.organization)
        };

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            access_token: cred.access_token,
        })
    }

    /// Builds a URL by appending path segments to the base URL.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The base URL cannot be parsed as a valid URL
    /// - The base URL is not an absolute URL (cannot be a relative URL)
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

    /// Sends a GET request and deserializes the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails
    /// - The response status is not a success code (2xx)
    /// - The response body cannot be deserialized into the target type
    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        query: &[(&str, String)],
    ) -> Result<T> {
        let response = self.send_request(self.http.get(url).query(query)).await?;
        Ok(response.json::<T>().await?)
    }

    /// Sends a POST request with JSON body and deserializes the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails
    /// - The response status is not a success code (2xx)
    /// - The response body cannot be deserialized into the target type
    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
        query: &[(&str, String)],
    ) -> Result<TRes> {
        let response = self
            .send_request(self.http.post(url).query(query).json(body))
            .await?;
        Ok(response.json::<TRes>().await?)
    }

    /// Sends a PUT request with JSON body and deserializes the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails
    /// - The response status is not a success code (2xx)
    /// - The response body cannot be deserialized into the target type
    async fn put_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
        query: &[(&str, String)],
    ) -> Result<TRes> {
        let response = self
            .send_request(self.http.put(url).query(query).json(body))
            .await?;
        Ok(response.json::<TRes>().await?)
    }

    /// Sends a PATCH request with JSON body and deserializes the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails
    /// - The response status is not a success code (2xx)
    /// - The response body cannot be deserialized into the target type
    async fn patch_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
        query: &[(&str, String)],
    ) -> Result<TRes> {
        let response = self
            .send_request(self.http.patch(url).query(query).json(body))
            .await?;
        Ok(response.json::<TRes>().await?)
    }

    /// Sends an HTTP request with authentication headers.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails to complete
    /// - The response status is not a success code (2xx)
    async fn send_request(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let response = request
            .basic_auth("", Some(&self.access_token))
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Azure DevOps API request failed ({status}): {body}"
            ))
        }
    }
}

/// Normalizes a base URL by trimming whitespace and trailing slashes.
///
/// # Errors
///
/// Returns an error if the endpoint is empty or contains only whitespace.
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

    use types::CommentThreadStatus;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_string_contains, method, path, query_param},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut azure_values = HashMap::new();
        azure_values.insert("access_token".to_string(), "test-token".to_string());
        azure_values.insert("organization".to_string(), "test-org".to_string());
        azure_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("azure_repos", azure_values)
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_pull_request_status_serialization_roundtrip() {
        for variant in [
            PullRequestStatus::NotSet,
            PullRequestStatus::Active,
            PullRequestStatus::Abandoned,
            PullRequestStatus::Completed,
            PullRequestStatus::All,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: PullRequestStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_comment_thread_status_serialization_roundtrip() {
        for variant in [
            CommentThreadStatus::Unknown,
            CommentThreadStatus::Active,
            CommentThreadStatus::Fixed,
            CommentThreadStatus::WontFix,
            CommentThreadStatus::Closed,
            CommentThreadStatus::ByDesign,
            CommentThreadStatus::Pending,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: CommentThreadStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_repository_serialization_roundtrip() {
        let repo = Repository {
            id: "repo-1".to_string(),
            name: "test-repo".to_string(),
            default_branch: Some("refs/heads/main".to_string()),
            url: Some(
                "https://dev.azure.com/org/project/_apis/git/repositories/repo-1".to_string(),
            ),
            remote_url: Some("https://dev.azure.com/org/project/_git/test-repo".to_string()),
            web_url: Some("https://dev.azure.com/org/project/_git/test-repo".to_string()),
        };
        let json = serde_json::to_string(&repo).unwrap();
        let parsed: Repository = serde_json::from_str(&json).unwrap();
        assert_eq!(repo.id, parsed.id);
        assert_eq!(repo.name, parsed.name);
        assert_eq!(repo.default_branch, parsed.default_branch);
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://dev.azure.com/org/").unwrap();
        assert_eq!(result, "https://dev.azure.com/org");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://dev.azure.com/org  ").unwrap();
        assert_eq!(result, "https://dev.azure.com/org");
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
    async fn test_list_repos_empty_project_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = list_repos(
            ctx,
            ListReposInput {
                project: "   ".to_string(),
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
    async fn test_create_pr_empty_project_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = create_pr(
            ctx,
            CreatePrInput {
                project: "  ".to_string(),
                repository_id: "repo-1".to_string(),
                title: "Test PR".to_string(),
                description: None,
                source_ref_name: "refs/heads/feature".to_string(),
                target_ref_name: "refs/heads/main".to_string(),
                reviewers: vec![],
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
    async fn test_create_pr_empty_title_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = create_pr(
            ctx,
            CreatePrInput {
                project: "test-project".to_string(),
                repository_id: "repo-1".to_string(),
                title: "  ".to_string(),
                description: None,
                source_ref_name: "refs/heads/feature".to_string(),
                target_ref_name: "refs/heads/main".to_string(),
                reviewers: vec![],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("title must not be empty")
        );
    }

    #[tokio::test]
    async fn test_comment_empty_comment_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = comment(
            ctx,
            CommentInput {
                project: "test-project".to_string(),
                repository_id: "repo-1".to_string(),
                pull_request_id: 1,
                comment: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("comment must not be empty")
        );
    }

    #[tokio::test]
    async fn test_approve_invalid_vote_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = approve(
            ctx,
            ApproveInput {
                project: "test-project".to_string(),
                repository_id: "repo-1".to_string(),
                pull_request_id: 1,
                vote: 15, // Invalid vote
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("vote must be between -10 and 10")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_list_repos_success_returns_repositories() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "value": [
            {
              "id": "repo-1",
              "name": "test-repo",
              "defaultBranch": "refs/heads/main",
              "url": "https://dev.azure.com/org/project/_apis/git/repositories/repo-1",
              "remoteUrl": "https://dev.azure.com/org/project/_git/test-repo",
              "webUrl": "https://dev.azure.com/org/project/_git/test-repo"
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/test-project/_apis/git/repositories"))
            .and(query_param("api-version", "7.1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = list_repos(
            ctx,
            ListReposInput {
                project: "test-project".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.repositories.len(), 1);
        assert_eq!(output.repositories[0].id, "repo-1");
        assert_eq!(output.repositories[0].name, "test-repo");
    }

    #[tokio::test]
    async fn test_create_pr_success_returns_pull_request() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "pullRequestId": 42,
          "title": "Test PR",
          "description": "Test description",
          "sourceRefName": "refs/heads/feature",
          "targetRefName": "refs/heads/main",
          "status": "active"
        }
        "#;

        Mock::given(method("POST"))
            .and(path(
                "/test-project/_apis/git/repositories/repo-1/pullrequests",
            ))
            .and(query_param("api-version", "7.1"))
            .and(body_string_contains("\"title\":\"Test PR\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = create_pr(
            ctx,
            CreatePrInput {
                project: "test-project".to_string(),
                repository_id: "repo-1".to_string(),
                title: "Test PR".to_string(),
                description: Some("Test description".to_string()),
                source_ref_name: "refs/heads/feature".to_string(),
                target_ref_name: "refs/heads/main".to_string(),
                reviewers: vec![],
            },
        )
        .await
        .unwrap();

        assert_eq!(output.pull_request.id, 42);
        assert_eq!(output.pull_request.title, "Test PR");
    }

    #[tokio::test]
    async fn test_comment_success_returns_thread() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "id": 123,
          "comments": [
            {
              "id": 1,
              "content": "Test comment"
            }
          ],
          "status": "active"
        }
        "#;

        Mock::given(method("POST"))
            .and(path(
                "/test-project/_apis/git/repositories/repo-1/pullRequests/1/threads",
            ))
            .and(query_param("api-version", "7.1"))
            .and(body_string_contains("\"content\":\"Test comment\""))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = comment(
            ctx,
            CommentInput {
                project: "test-project".to_string(),
                repository_id: "repo-1".to_string(),
                pull_request_id: 1,
                comment: "Test comment".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.thread.id, 123);
        assert_eq!(output.thread.comments.len(), 1);
    }

    #[tokio::test]
    async fn test_approve_success_returns_reviewer() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "id": "user-123",
          "displayName": "Test User",
          "vote": 10
        }
        "#;

        Mock::given(method("PUT"))
            .and(path(
                "/test-project/_apis/git/repositories/repo-1/pullRequests/1/reviewers/me",
            ))
            .and(query_param("api-version", "7.1"))
            .and(body_string_contains("\"vote\":10"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = approve(
            ctx,
            ApproveInput {
                project: "test-project".to_string(),
                repository_id: "repo-1".to_string(),
                pull_request_id: 1,
                vote: 10,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.reviewer.id, "user-123");
        assert_eq!(output.reviewer.vote, 10);
    }

    #[tokio::test]
    async fn test_merge_success_returns_completed_pr() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "pullRequestId": 1,
          "title": "Test PR",
          "sourceRefName": "refs/heads/feature",
          "targetRefName": "refs/heads/main",
          "status": "completed"
        }
        "#;

        Mock::given(method("PATCH"))
            .and(path(
                "/test-project/_apis/git/repositories/repo-1/pullRequests/1",
            ))
            .and(query_param("api-version", "7.1"))
            .and(body_string_contains("\"status\":\"completed\""))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = merge(
            ctx,
            MergeInput {
                project: "test-project".to_string(),
                repository_id: "repo-1".to_string(),
                pull_request_id: 1,
                commit_message: Some("Merge PR".to_string()),
                delete_source_branch: true,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.pull_request.id, 1);
        assert_eq!(output.pull_request.status, PullRequestStatus::Completed);
    }

    #[tokio::test]
    async fn test_list_repos_auth_error_returns_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/test-project/_apis/git/repositories"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_raw(r#"{"message": "Unauthorized"}"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let result = list_repos(
            ctx,
            ListReposInput {
                project: "test-project".to_string(),
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("401"));
    }
}
