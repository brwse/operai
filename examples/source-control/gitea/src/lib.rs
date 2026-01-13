//! source-control/gitea integration for Operai Toolbox.

mod types;

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
use types::{
    Comment, CreateCommentRequest, CreatePullRequestRequest, CreateReviewRequest,
    MergePullRequestRequest, MergePullRequestResponse, PullRequest, Repository, Review,
};

define_user_credential! {
    GiteaCredential("gitea") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_GITEA_ENDPOINT: &str = "https://gitea.com";

#[init]
async fn setup() -> Result<()> {
    info!("Gitea integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Gitea integration shutting down");
}

// ============================================================================
// Tool Inputs and Outputs
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListReposInput {
    /// Owner/organization name.
    pub owner: String,
    /// Maximum number of results (1-100). Defaults to 30.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListReposOutput {
    pub repositories: Vec<RepositorySummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RepositorySummary {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub private: bool,
    #[serde(default)]
    pub html_url: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreatePullRequestInput {
    /// Owner/organization name.
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Pull request title.
    pub title: String,
    /// Pull request body/description.
    #[serde(default)]
    pub body: Option<String>,
    /// Head branch (source branch).
    pub head: String,
    /// Base branch (target branch).
    pub base: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreatePullRequestOutput {
    pub pull_request: PullRequestSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PullRequestSummary {
    pub id: u64,
    pub number: u64,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CommentInput {
    /// Owner/organization name.
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Pull request number.
    pub pr_number: u64,
    /// Comment text.
    pub body: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CommentOutput {
    pub comment_id: u64,
    pub created: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ApproveInput {
    /// Owner/organization name.
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Pull request number.
    pub pr_number: u64,
    /// Optional review comment.
    #[serde(default)]
    pub body: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ApproveOutput {
    pub review_id: u64,
    pub approved: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MergeInput {
    /// Owner/organization name.
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Pull request number.
    pub pr_number: u64,
    /// Merge method: "merge", "rebase", "rebase-merge", or "squash". Defaults
    /// to "merge".
    #[serde(default)]
    pub merge_method: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MergeOutput {
    pub merged: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CloseInput {
    /// Owner/organization name.
    pub owner: String,
    /// Repository name.
    pub repo: String,
    /// Pull request number.
    pub pr_number: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CloseOutput {
    pub closed: bool,
}

// ============================================================================
// Tool Implementations
// ============================================================================

/// # List Gitea Repositories
///
/// Retrieves a list of repositories for a specified owner or organization from
/// Gitea.
///
/// Use this tool when the user wants to browse, search, or discover
/// repositories owned by a specific user or organization on a Gitea instance.
/// This is useful for:
/// - Listing all repositories owned by a user or organization
/// - Getting repository metadata (names, descriptions, visibility)
/// - Finding repositories to perform further operations on
///
/// The results can be limited using the `limit` parameter (1-100, defaults to
/// 30). Only returns basic repository information; use other tools for detailed
/// operations.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - git
/// - gitea
/// - repository
///
/// # Errors
///
/// Returns an error if:
/// - The `owner` field is empty or contains only whitespace
/// - The `limit` is not between 1 and 100
/// - Gitea credentials are not configured or the access token is empty
/// - The HTTP request to the Gitea API fails
/// - The API response cannot be parsed as a JSON array of repositories
#[tool]
pub async fn list_repos(ctx: Context, input: ListReposInput) -> Result<ListReposOutput> {
    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    let limit = input.limit.unwrap_or(30);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let client = GiteaClient::from_ctx(&ctx)?;
    let url = client.url_with_segments(&["orgs", &input.owner, "repos"])?;

    let query = [("limit", limit.to_string())];

    let repositories: Vec<Repository> = client.get_json(url, &query).await?;

    Ok(ListReposOutput {
        repositories: repositories.into_iter().map(map_repo_summary).collect(),
    })
}

/// # Create Gitea Pull Request
///
/// Creates a new pull request in a Gitea repository to propose changes from a
/// head branch to a base branch.
///
/// Use this tool when the user wants to:
/// - Open a new pull request for code review
/// - Propose merging changes from one branch to another
/// - Submit a feature branch for consideration
///
/// Requires specifying the source branch (`head`) and target branch (`base`).
/// The pull request will be created in the specified repository owned by
/// `owner`. An optional body/description can be provided to explain the
/// changes.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - git
/// - gitea
/// - pull-request
///
/// # Errors
///
/// Returns an error if:
/// - The `owner`, `repo`, `title`, `head`, or `base` fields are empty or
///   contain only whitespace
/// - Gitea credentials are not configured or the access token is empty
/// - The HTTP request to the Gitea API fails
/// - The API response cannot be parsed as a JSON pull request object
#[tool]
pub async fn create_pr(
    ctx: Context,
    input: CreatePullRequestInput,
) -> Result<CreatePullRequestOutput> {
    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    ensure!(!input.repo.trim().is_empty(), "repo must not be empty");
    ensure!(!input.title.trim().is_empty(), "title must not be empty");
    ensure!(!input.head.trim().is_empty(), "head must not be empty");
    ensure!(!input.base.trim().is_empty(), "base must not be empty");

    let client = GiteaClient::from_ctx(&ctx)?;
    let url = client.url_with_segments(&["repos", &input.owner, &input.repo, "pulls"])?;

    let request = CreatePullRequestRequest {
        title: input.title,
        body: input.body,
        head: input.head,
        base: input.base,
        assignee: None,
        assignees: None,
        milestone: None,
        labels: None,
    };

    let pr: PullRequest = client.post_json(url, &request).await?;

    Ok(CreatePullRequestOutput {
        pull_request: map_pr_summary(pr),
    })
}

/// # Comment on Gitea Pull Request
///
/// Adds a new comment to an existing pull request in a Gitea repository.
///
/// Use this tool when the user wants to:
/// - Provide feedback on a pull request
/// - Ask questions about proposed changes
/// - Leave review comments or suggestions
/// - Communicate with the pull request author or reviewers
///
/// The comment will be posted as a general comment on the pull request
/// (not a specific code review comment). Requires the pull request number.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - git
/// - gitea
/// - pull-request
/// - comment
///
/// # Errors
///
/// Returns an error if:
/// - The `owner` or `repo` fields are empty or contain only whitespace
/// - The `pr_number` is 0
/// - The `body` field is empty or contains only whitespace
/// - Gitea credentials are not configured or the access token is empty
/// - The HTTP request to the Gitea API fails
/// - The API response cannot be parsed as a JSON comment object
#[tool]
pub async fn comment(ctx: Context, input: CommentInput) -> Result<CommentOutput> {
    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    ensure!(!input.repo.trim().is_empty(), "repo must not be empty");
    ensure!(input.pr_number > 0, "pr_number must be greater than 0");
    ensure!(!input.body.trim().is_empty(), "body must not be empty");

    let client = GiteaClient::from_ctx(&ctx)?;
    let url = client.url_with_segments(&[
        "repos",
        &input.owner,
        &input.repo,
        "issues",
        &input.pr_number.to_string(),
        "comments",
    ])?;

    let request = CreateCommentRequest { body: input.body };

    let comment: Comment = client.post_json(url, &request).await?;

    Ok(CommentOutput {
        comment_id: comment.id,
        created: true,
    })
}

/// # Approve Gitea Pull Request
///
/// Submits an approval review for a pull request in a Gitea repository.
///
/// Use this tool when the user wants to:
/// - Approve a pull request for merging
/// - Signal that the code has been reviewed and is acceptable
/// - Provide positive feedback on proposed changes
///
/// An optional review comment can be included to explain the approval
/// or provide additional context. The approval is recorded as a formal
/// review with event type "APPROVED".
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - git
/// - gitea
/// - pull-request
/// - review
///
/// # Errors
///
/// Returns an error if:
/// - The `owner` or `repo` fields are empty or contain only whitespace
/// - The `pr_number` is 0
/// - Gitea credentials are not configured or the access token is empty
/// - The HTTP request to the Gitea API fails
/// - The API response cannot be parsed as a JSON review object
#[tool]
pub async fn approve(ctx: Context, input: ApproveInput) -> Result<ApproveOutput> {
    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    ensure!(!input.repo.trim().is_empty(), "repo must not be empty");
    ensure!(input.pr_number > 0, "pr_number must be greater than 0");

    let client = GiteaClient::from_ctx(&ctx)?;
    let url = client.url_with_segments(&[
        "repos",
        &input.owner,
        &input.repo,
        "pulls",
        &input.pr_number.to_string(),
        "reviews",
    ])?;

    let request = CreateReviewRequest {
        body: input.body,
        event: "APPROVED".to_string(),
    };

    let review: Review = client.post_json(url, &request).await?;

    Ok(ApproveOutput {
        review_id: review.id,
        approved: true,
    })
}

/// # Merge Gitea Pull Request
///
/// Merges a pull request into its target branch using the specified merge
/// method.
///
/// Use this tool when the user wants to:
/// - Merge an approved pull request into the base branch
/// - Complete the pull request workflow
/// - Integrate changes from one branch to another
///
/// Supports four merge methods:
/// - "merge": Create a merge commit (default)
/// - "rebase": Rebase commits onto the base branch
/// - "rebase-merge": Rebase and create a merge commit
/// - "squash": Squash all commits into a single merge commit
///
/// The pull request must be in a mergeable state (e.g., approved, no
/// conflicts).
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - git
/// - gitea
/// - pull-request
/// - merge
///
/// # Errors
///
/// Returns an error if:
/// - The `owner` or `repo` fields are empty or contain only whitespace
/// - The `pr_number` is 0
/// - The `merge_method` is not one of: merge, rebase, rebase-merge, or squash
/// - Gitea credentials are not configured or the access token is empty
/// - The HTTP request to the Gitea API fails
/// - The API response cannot be parsed as a JSON merge response object
#[tool]
pub async fn merge(ctx: Context, input: MergeInput) -> Result<MergeOutput> {
    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    ensure!(!input.repo.trim().is_empty(), "repo must not be empty");
    ensure!(input.pr_number > 0, "pr_number must be greater than 0");

    let merge_method = input.merge_method.unwrap_or_else(|| "merge".to_string());
    ensure!(
        matches!(
            merge_method.as_str(),
            "merge" | "rebase" | "rebase-merge" | "squash"
        ),
        "merge_method must be one of: merge, rebase, rebase-merge, squash"
    );

    let client = GiteaClient::from_ctx(&ctx)?;
    let url = client.url_with_segments(&[
        "repos",
        &input.owner,
        &input.repo,
        "pulls",
        &input.pr_number.to_string(),
        "merge",
    ])?;

    let request = MergePullRequestRequest {
        merge_method,
        merge_message: None,
        merge_title: None,
    };

    let response: MergePullRequestResponse = client.post_json(url, &request).await?;

    Ok(MergeOutput {
        merged: response.merged,
    })
}

/// # Close Gitea Pull Request
///
/// Closes a pull request without merging it, effectively abandoning the
/// proposed changes.
///
/// Use this tool when the user wants to:
/// - Reject a pull request
/// - Close an outdated or no longer needed pull request
/// - Abandon a pull request that won't be merged
///
/// This operation does not merge any changes. The pull request state is
/// set to "closed" and it will remain visible in the repository but marked
/// as closed. Use this instead of merging when the changes should not be
/// integrated into the target branch.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - git
/// - gitea
/// - pull-request
///
/// # Errors
///
/// Returns an error if:
/// - The `owner` or `repo` fields are empty or contain only whitespace
/// - The `pr_number` is 0
/// - Gitea credentials are not configured or the access token is empty
/// - The HTTP request to the Gitea API fails
/// - The API response cannot be parsed as a JSON pull request object
#[tool]
pub async fn close(ctx: Context, input: CloseInput) -> Result<CloseOutput> {
    #[derive(Serialize)]
    struct UpdatePRRequest {
        state: String,
    }

    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    ensure!(!input.repo.trim().is_empty(), "repo must not be empty");
    ensure!(input.pr_number > 0, "pr_number must be greater than 0");

    let client = GiteaClient::from_ctx(&ctx)?;
    let url = client.url_with_segments(&[
        "repos",
        &input.owner,
        &input.repo,
        "pulls",
        &input.pr_number.to_string(),
    ])?;

    let request = UpdatePRRequest {
        state: "closed".to_string(),
    };

    let _pr: PullRequest = client.patch_json(url, &request).await?;

    Ok(CloseOutput { closed: true })
}

// ============================================================================
// HTTP Client
// ============================================================================

#[derive(Debug, Clone)]
struct GiteaClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl GiteaClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = GiteaCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_GITEA_ENDPOINT))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url: format!("{base_url}/api/v1"),
            access_token: cred.access_token,
        })
    }

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

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        query: &[(&str, String)],
    ) -> Result<T> {
        let request = self.http.get(url).query(query);
        let response = self.send_request(request).await?;
        Ok(response.json::<T>().await?)
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
    ) -> Result<TRes> {
        let request = self.http.post(url).json(body);
        let response = self.send_request(request).await?;
        Ok(response.json::<TRes>().await?)
    }

    async fn patch_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
    ) -> Result<TRes> {
        let request = self.http.patch(url).json(body);
        let response = self.send_request(request).await?;
        Ok(response.json::<TRes>().await?)
    }

    async fn send_request(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let response = request
            .header("Authorization", format!("token {}", self.access_token))
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Gitea API request failed ({status}): {body}"
            ))
        }
    }
}

fn normalize_base_url(endpoint: &str) -> Result<String> {
    let trimmed = endpoint.trim();
    ensure!(!trimmed.is_empty(), "endpoint must not be empty");
    Ok(trimmed.trim_end_matches('/').to_string())
}

fn map_repo_summary(repo: Repository) -> RepositorySummary {
    RepositorySummary {
        id: repo.id,
        name: repo.name,
        full_name: repo.full_name,
        description: repo.description,
        private: repo.private,
        html_url: repo.html_url,
    }
}

fn map_pr_summary(pr: PullRequest) -> PullRequestSummary {
    PullRequestSummary {
        id: pr.id,
        number: pr.number,
        title: pr.title,
        state: pr.state,
        html_url: pr.html_url,
    }
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_string_contains, header, method, path, query_param},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut gitea_values = HashMap::new();
        gitea_values.insert("access_token".to_string(), "test-token".to_string());
        gitea_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("gitea", gitea_values)
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_repository_summary_serialization_roundtrip() {
        let repo = RepositorySummary {
            id: 123,
            name: "test-repo".to_string(),
            full_name: "owner/test-repo".to_string(),
            description: Some("A test repo".to_string()),
            private: false,
            html_url: Some("https://gitea.com/owner/test-repo".to_string()),
        };
        let json = serde_json::to_string(&repo).unwrap();
        let parsed: RepositorySummary = serde_json::from_str(&json).unwrap();
        assert_eq!(repo.id, parsed.id);
        assert_eq!(repo.name, parsed.name);
    }

    #[test]
    fn test_pull_request_summary_serialization_roundtrip() {
        let pr = PullRequestSummary {
            id: 456,
            number: 1,
            title: Some("Fix bug".to_string()),
            state: Some("open".to_string()),
            html_url: Some("https://gitea.com/owner/repo/pulls/1".to_string()),
        };
        let json = serde_json::to_string(&pr).unwrap();
        let parsed: PullRequestSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(pr.id, parsed.id);
        assert_eq!(pr.number, parsed.number);
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://gitea.com/").unwrap();
        assert_eq!(result, "https://gitea.com");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://gitea.com  ").unwrap();
        assert_eq!(result, "https://gitea.com");
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
    async fn test_list_repos_empty_owner_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = list_repos(
            ctx,
            ListReposInput {
                owner: "   ".to_string(),
                limit: None,
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
    async fn test_list_repos_limit_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = list_repos(
            ctx,
            ListReposInput {
                owner: "owner".to_string(),
                limit: Some(0),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("limit must be between 1 and 100")
        );
    }

    #[tokio::test]
    async fn test_list_repos_limit_exceeds_max_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = list_repos(
            ctx,
            ListReposInput {
                owner: "owner".to_string(),
                limit: Some(101),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("limit must be between 1 and 100")
        );
    }

    #[tokio::test]
    async fn test_create_pr_empty_owner_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = create_pr(
            ctx,
            CreatePullRequestInput {
                owner: "  ".to_string(),
                repo: "repo".to_string(),
                title: "Title".to_string(),
                body: None,
                head: "feature".to_string(),
                base: "main".to_string(),
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
    async fn test_create_pr_empty_title_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = create_pr(
            ctx,
            CreatePullRequestInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                title: "  ".to_string(),
                body: None,
                head: "feature".to_string(),
                base: "main".to_string(),
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
    async fn test_comment_empty_body_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = comment(
            ctx,
            CommentInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                pr_number: 1,
                body: "  ".to_string(),
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
    async fn test_approve_zero_pr_number_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = approve(
            ctx,
            ApproveInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                pr_number: 0,
                body: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("pr_number must be greater than 0")
        );
    }

    #[tokio::test]
    async fn test_merge_invalid_method_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = merge(
            ctx,
            MergeInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                pr_number: 1,
                merge_method: Some("invalid".to_string()),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("merge_method must be one of")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_list_repos_success_returns_repositories() {
        let server = MockServer::start().await;

        let response_body = r#"[
            {
                "id": 1,
                "name": "repo1",
                "full_name": "owner/repo1",
                "description": "First repo",
                "private": false,
                "fork": false,
                "html_url": "https://gitea.com/owner/repo1"
            },
            {
                "id": 2,
                "name": "repo2",
                "full_name": "owner/repo2",
                "description": null,
                "private": true,
                "fork": false,
                "html_url": "https://gitea.com/owner/repo2"
            }
        ]"#;

        Mock::given(method("GET"))
            .and(path("/api/v1/orgs/owner/repos"))
            .and(header("authorization", "token test-token"))
            .and(query_param("limit", "30"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = list_repos(
            ctx,
            ListReposInput {
                owner: "owner".to_string(),
                limit: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.repositories.len(), 2);
        assert_eq!(output.repositories[0].name, "repo1");
        assert_eq!(output.repositories[1].name, "repo2");
        assert!(output.repositories[1].private);
    }

    #[tokio::test]
    async fn test_create_pr_success_returns_pull_request() {
        let server = MockServer::start().await;

        let response_body = r#"{
            "id": 123,
            "number": 5,
            "title": "Fix bug",
            "body": "This fixes the bug",
            "state": "open",
            "html_url": "https://gitea.com/owner/repo/pulls/5"
        }"#;

        Mock::given(method("POST"))
            .and(path("/api/v1/repos/owner/repo/pulls"))
            .and(body_string_contains("\"title\":\"Fix bug\""))
            .and(body_string_contains("\"head\":\"feature\""))
            .and(body_string_contains("\"base\":\"main\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = create_pr(
            ctx,
            CreatePullRequestInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                title: "Fix bug".to_string(),
                body: Some("This fixes the bug".to_string()),
                head: "feature".to_string(),
                base: "main".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.pull_request.number, 5);
        assert_eq!(output.pull_request.title.as_deref(), Some("Fix bug"));
        assert_eq!(output.pull_request.state.as_deref(), Some("open"));
    }

    #[tokio::test]
    async fn test_comment_success_creates_comment() {
        let server = MockServer::start().await;

        let response_body = r#"{
            "id": 789,
            "body": "Thanks for the PR!",
            "created_at": "2024-01-01T00:00:00Z"
        }"#;

        Mock::given(method("POST"))
            .and(path("/api/v1/repos/owner/repo/issues/5/comments"))
            .and(body_string_contains("\"body\":\"Thanks for the PR!\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = comment(
            ctx,
            CommentInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                pr_number: 5,
                body: "Thanks for the PR!".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.comment_id, 789);
        assert!(output.created);
    }

    #[tokio::test]
    async fn test_approve_success_approves_pr() {
        let server = MockServer::start().await;

        let response_body = r#"{
            "id": 456,
            "state": "APPROVED",
            "submitted_at": "2024-01-01T00:00:00Z"
        }"#;

        Mock::given(method("POST"))
            .and(path("/api/v1/repos/owner/repo/pulls/5/reviews"))
            .and(body_string_contains("\"event\":\"APPROVED\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = approve(
            ctx,
            ApproveInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                pr_number: 5,
                body: Some("Looks good!".to_string()),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.review_id, 456);
        assert!(output.approved);
    }

    #[tokio::test]
    async fn test_merge_success_merges_pr() {
        let server = MockServer::start().await;

        let response_body = r#"{
            "merged": true
        }"#;

        Mock::given(method("POST"))
            .and(path("/api/v1/repos/owner/repo/pulls/5/merge"))
            .and(body_string_contains("\"Do\":\"squash\""))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = merge(
            ctx,
            MergeInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                pr_number: 5,
                merge_method: Some("squash".to_string()),
            },
        )
        .await
        .unwrap();

        assert!(output.merged);
    }

    #[tokio::test]
    async fn test_close_success_closes_pr() {
        let server = MockServer::start().await;

        let response_body = r#"{
            "id": 123,
            "number": 5,
            "state": "closed"
        }"#;

        Mock::given(method("PATCH"))
            .and(path("/api/v1/repos/owner/repo/pulls/5"))
            .and(body_string_contains("\"state\":\"closed\""))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = close(
            ctx,
            CloseInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                pr_number: 5,
            },
        )
        .await
        .unwrap();

        assert!(output.closed);
    }

    #[tokio::test]
    async fn test_gitea_api_error_returns_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v1/orgs/owner/repos"))
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
                owner: "owner".to_string(),
                limit: None,
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("401"));
    }
}
