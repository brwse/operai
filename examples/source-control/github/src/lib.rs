//! source-control/github integration for Operai Toolbox.

mod types;

use operai::{
    Context, JsonSchema, Result, anyhow, define_user_credential, ensure, info, init, schemars,
    shutdown, tool,
};
use serde::{Deserialize, Serialize};
use types::{Comment, Issue, PullRequest, SearchFilter, map_comment, map_issue, map_pull_request};

define_user_credential! {
    GitHubCredential("github") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_GITHUB_API: &str = "https://api.github.com";

#[init]
async fn setup() -> Result<()> {
    info!("GitHub integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("GitHub integration shutting down");
}

// ============================================================================
// Tool 1: Search Issues/PRs
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchIssuesInput {
    /// Repository owner (e.g., "octocat")
    pub owner: String,
    /// Repository name (e.g., "Hello-World")
    pub repo: String,
    /// Search query string
    pub query: String,
    /// Filter by type: "issue", "pr", or "all". Defaults to "all".
    #[serde(default)]
    pub filter: Option<SearchFilter>,
    /// Maximum number of results (1-100). Defaults to 30.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchIssuesOutput {
    pub issues: Vec<Issue>,
    pub pull_requests: Vec<PullRequest>,
}

/// # Search GitHub Issues and Pull Requests
///
/// Searches for issues and pull requests in a GitHub repository using GitHub's
/// search syntax.
///
/// Use this tool when you need to find specific issues or pull requests in a
/// repository. The search query supports GitHub's advanced search syntax,
/// allowing you to filter by state (open/closed), labels, authors, assignees,
/// and more. Results can be filtered to show only issues, only pull requests,
/// or both.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - github
/// - issues
/// - pull-requests
/// - search
///
/// # Errors
///
/// Returns an error if:
/// - `owner` is empty or contains only whitespace
/// - `repo` is empty or contains only whitespace
/// - `query` is empty or contains only whitespace
/// - `limit` is not between 1 and 100
/// - GitHub credentials are missing or invalid
/// - The GitHub API request fails (network errors, rate limiting,
///   authentication failures)
/// - The response from GitHub cannot be parsed
#[tool]
pub async fn search_issues_prs(
    ctx: Context,
    input: SearchIssuesInput,
) -> Result<SearchIssuesOutput> {
    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    ensure!(!input.repo.trim().is_empty(), "repo must not be empty");
    ensure!(!input.query.trim().is_empty(), "query must not be empty");

    let limit = input.limit.unwrap_or(30);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let filter = input.filter.unwrap_or(SearchFilter::All);

    let client = GitHubClient::from_ctx(&ctx)?;

    // Search in the repo context
    let query_str = format!(
        "{} repo:{}/{}",
        input.query.trim(),
        input.owner.trim(),
        input.repo.trim()
    );

    let search_results: GitHubSearchResponse = client
        .get_json(
            format!("{}/search/issues", client.base_url),
            &[("q", query_str.as_str()), ("per_page", &limit.to_string())],
        )
        .await?;

    let mut issues = Vec::new();
    let mut pull_requests = Vec::new();

    for item in search_results.items {
        let is_pr = item.pull_request.is_some();

        match filter {
            SearchFilter::Issue if is_pr => continue,
            SearchFilter::Pr if !is_pr => continue,
            _ => {}
        }

        if is_pr {
            // Fetch full PR details
            let pr_details: types::OctoPullRequest = client
                .get_json(
                    format!(
                        "{}/repos/{}/{}/pulls/{}",
                        client.base_url, input.owner, input.repo, item.number
                    ),
                    &[],
                )
                .await?;
            pull_requests.push(map_pull_request(pr_details));
        } else {
            issues.push(map_issue(item));
        }
    }

    Ok(SearchIssuesOutput {
        issues,
        pull_requests,
    })
}

// ============================================================================
// Tool 2: Create Issue
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateIssueInput {
    /// Repository owner
    pub owner: String,
    /// Repository name
    pub repo: String,
    /// Issue title
    pub title: String,
    /// Issue body (optional)
    #[serde(default)]
    pub body: Option<String>,
    /// Labels to apply (optional)
    #[serde(default)]
    pub labels: Vec<String>,
    /// Assignees (optional)
    #[serde(default)]
    pub assignees: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateIssueOutput {
    pub issue: Issue,
}

/// # Create GitHub Issue
///
/// Creates a new issue in a GitHub repository with an optional description,
/// labels, and assignees.
///
/// Use this tool when a user wants to file a new bug report, feature request,
/// or task in a GitHub repository. The issue will be created in the open state.
/// You can optionally add labels for categorization (e.g., "bug",
/// "enhancement") and assign specific users to work on the issue.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - github
/// - issues
///
/// # Errors
///
/// Returns an error if:
/// - `owner` is empty or contains only whitespace
/// - `repo` is empty or contains only whitespace
/// - `title` is empty or contains only whitespace
/// - GitHub credentials are missing or invalid
/// - The GitHub API request fails (network errors, rate limiting,
///   authentication failures)
/// - The response from GitHub cannot be parsed
#[tool]
pub async fn create_issue(ctx: Context, input: CreateIssueInput) -> Result<CreateIssueOutput> {
    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    ensure!(!input.repo.trim().is_empty(), "repo must not be empty");
    ensure!(!input.title.trim().is_empty(), "title must not be empty");

    let client = GitHubClient::from_ctx(&ctx)?;

    let request = CreateIssueRequest {
        title: input.title,
        body: input.body,
        labels: if input.labels.is_empty() {
            None
        } else {
            Some(input.labels)
        },
        assignees: if input.assignees.is_empty() {
            None
        } else {
            Some(input.assignees)
        },
    };

    let issue_response: types::OctoIssue = client
        .post_json(
            format!(
                "{}/repos/{}/{}/issues",
                client.base_url, input.owner, input.repo
            ),
            &request,
        )
        .await?;

    Ok(CreateIssueOutput {
        issue: map_issue(issue_response),
    })
}

// ============================================================================
// Tool 3: Comment on Issue/PR
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CommentInput {
    /// Repository owner
    pub owner: String,
    /// Repository name
    pub repo: String,
    /// Issue or PR number
    pub issue_number: u64,
    /// Comment body
    pub body: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CommentOutput {
    pub comment: Comment,
}

/// # Comment on GitHub Issue or Pull Request
///
/// Adds a comment to an existing GitHub issue or pull request discussion.
///
/// Use this tool when a user wants to reply to or participate in a discussion
/// on an existing issue or pull request. Comments are public and visible to all
/// repository collaborators. This is the appropriate tool for asking questions,
/// providing feedback, or sharing additional information in the context of an
/// issue or PR.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - github
/// - issues
/// - pull-requests
/// - comments
///
/// # Errors
///
/// Returns an error if:
/// - `owner` is empty or contains only whitespace
/// - `repo` is empty or contains only whitespace
/// - `issue_number` is not positive
/// - `body` is empty or contains only whitespace
/// - GitHub credentials are missing or invalid
/// - The GitHub API request fails (network errors, rate limiting,
///   authentication failures)
/// - The response from GitHub cannot be parsed
#[tool]
pub async fn comment(ctx: Context, input: CommentInput) -> Result<CommentOutput> {
    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    ensure!(!input.repo.trim().is_empty(), "repo must not be empty");
    ensure!(input.issue_number > 0, "issue_number must be positive");
    ensure!(!input.body.trim().is_empty(), "body must not be empty");

    let client = GitHubClient::from_ctx(&ctx)?;

    let request = CommentRequest { body: input.body };

    let comment_response: types::OctoComment = client
        .post_json(
            format!(
                "{}/repos/{}/{}/issues/{}/comments",
                client.base_url, input.owner, input.repo, input.issue_number
            ),
            &request,
        )
        .await?;

    Ok(CommentOutput {
        comment: map_comment(comment_response),
    })
}

// ============================================================================
// Tool 4: Open Pull Request
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OpenPullRequestInput {
    /// Repository owner
    pub owner: String,
    /// Repository name
    pub repo: String,
    /// PR title
    pub title: String,
    /// PR body (optional)
    #[serde(default)]
    pub body: Option<String>,
    /// Head branch (source branch to merge from)
    pub head: String,
    /// Base branch (target branch to merge into)
    pub base: String,
    /// Create as draft PR (optional)
    #[serde(default)]
    pub draft: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct OpenPullRequestOutput {
    pub pull_request: PullRequest,
}

/// # Open GitHub Pull Request
///
/// Creates a new pull request to merge changes from a head branch into a base
/// branch.
///
/// Use this tool when a user wants to propose code changes for review and merge
/// into a target branch. The pull request will be created in the open state,
/// triggering GitHub's review workflow. You can optionally create it as a draft
/// PR for work-in-progress changes that aren't ready for review.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - github
/// - pull-requests
///
/// # Errors
///
/// Returns an error if:
/// - `owner` is empty or contains only whitespace
/// - `repo` is empty or contains only whitespace
/// - `title` is empty or contains only whitespace
/// - `head` is empty or contains only whitespace
/// - `base` is empty or contains only whitespace
/// - GitHub credentials are missing or invalid
/// - The GitHub API request fails (network errors, rate limiting,
///   authentication failures)
/// - The response from GitHub cannot be parsed
#[tool]
pub async fn open_pull_request(
    ctx: Context,
    input: OpenPullRequestInput,
) -> Result<OpenPullRequestOutput> {
    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    ensure!(!input.repo.trim().is_empty(), "repo must not be empty");
    ensure!(!input.title.trim().is_empty(), "title must not be empty");
    ensure!(!input.head.trim().is_empty(), "head must not be empty");
    ensure!(!input.base.trim().is_empty(), "base must not be empty");

    let client = GitHubClient::from_ctx(&ctx)?;

    let request = CreatePullRequestRequest {
        title: input.title,
        body: input.body,
        head: input.head,
        base: input.base,
        draft: input.draft,
    };

    let pr_response: types::OctoPullRequest = client
        .post_json(
            format!(
                "{}/repos/{}/{}/pulls",
                client.base_url, input.owner, input.repo
            ),
            &request,
        )
        .await?;

    Ok(OpenPullRequestOutput {
        pull_request: map_pull_request(pr_response),
    })
}

// ============================================================================
// Tool 5: Request Review
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RequestReviewInput {
    /// Repository owner
    pub owner: String,
    /// Repository name
    pub repo: String,
    /// Pull request number
    pub pull_number: u64,
    /// Reviewers (usernames) to request
    pub reviewers: Vec<String>,
    /// Team reviewers (optional)
    #[serde(default)]
    pub team_reviewers: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RequestReviewOutput {
    pub requested: bool,
}

/// # Request GitHub Pull Request Review
///
/// Requests reviews from specific users or teams for a pull request.
///
/// Use this tool when a user wants to request code reviews from collaborators.
/// You can request reviews from individual users (by their GitHub usernames) or
/// from entire teams. Reviewers will receive a notification and the PR will be
/// marked as awaiting their review. At least one reviewer or team must be
/// specified.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - github
/// - pull-requests
/// - reviews
///
/// # Errors
///
/// Returns an error if:
/// - `owner` is empty or contains only whitespace
/// - `repo` is empty or contains only whitespace
/// - `pull_number` is not positive
/// - Both `reviewers` and `team_reviewers` are empty
/// - GitHub credentials are missing or invalid
/// - The GitHub API request fails (network errors, rate limiting,
///   authentication failures)
/// - The response from GitHub cannot be parsed
#[tool]
pub async fn request_review(
    ctx: Context,
    input: RequestReviewInput,
) -> Result<RequestReviewOutput> {
    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    ensure!(!input.repo.trim().is_empty(), "repo must not be empty");
    ensure!(input.pull_number > 0, "pull_number must be positive");
    ensure!(
        !input.reviewers.is_empty() || !input.team_reviewers.is_empty(),
        "at least one reviewer or team_reviewer must be specified"
    );

    let client = GitHubClient::from_ctx(&ctx)?;

    let request = RequestReviewersRequest {
        reviewers: if input.reviewers.is_empty() {
            None
        } else {
            Some(input.reviewers)
        },
        team_reviewers: if input.team_reviewers.is_empty() {
            None
        } else {
            Some(input.team_reviewers)
        },
    };

    let _: serde_json::Value = client
        .post_json(
            format!(
                "{}/repos/{}/{}/pulls/{}/requested_reviewers",
                client.base_url, input.owner, input.repo, input.pull_number
            ),
            &request,
        )
        .await?;

    Ok(RequestReviewOutput { requested: true })
}

// ============================================================================
// Tool 6: Merge Pull Request
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MergePullRequestInput {
    /// Repository owner
    pub owner: String,
    /// Repository name
    pub repo: String,
    /// Pull request number
    pub pull_number: u64,
    /// Commit message (optional)
    #[serde(default)]
    pub commit_message: Option<String>,
    /// Merge method: "merge", "squash", or "rebase". Defaults to "merge".
    #[serde(default)]
    pub merge_method: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MergePullRequestOutput {
    pub merged: bool,
    pub sha: String,
}

/// # Merge GitHub Pull Request
///
/// Merges a pull request into its base branch using the specified merge method.
///
/// Use this tool when a user wants to merge an approved pull request. The merge
/// can be performed using three different strategies: "merge" (creates a merge
/// commit), "squash" (combines all commits into one), or "rebase" (replays
/// commits on top). The PR must be mergeable (no conflicts, reviews approved if
/// required) for the merge to succeed. Returns the SHA of the merge commit.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - github
/// - pull-requests
///
/// # Errors
///
/// Returns an error if:
/// - `owner` is empty or contains only whitespace
/// - `repo` is empty or contains only whitespace
/// - `pull_number` is not positive
/// - `merge_method` is not one of "merge", "squash", or "rebase"
/// - GitHub credentials are missing or invalid
/// - The GitHub API request fails (network errors, rate limiting,
///   authentication failures)
/// - The response from GitHub cannot be parsed
#[tool]
pub async fn merge_pull_request(
    ctx: Context,
    input: MergePullRequestInput,
) -> Result<MergePullRequestOutput> {
    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    ensure!(!input.repo.trim().is_empty(), "repo must not be empty");
    ensure!(input.pull_number > 0, "pull_number must be positive");

    let merge_method = input.merge_method.as_deref().unwrap_or("merge");
    ensure!(
        matches!(merge_method, "merge" | "squash" | "rebase"),
        "merge_method must be 'merge', 'squash', or 'rebase'"
    );

    let client = GitHubClient::from_ctx(&ctx)?;

    let request = MergePullRequestRequest {
        commit_message: input.commit_message,
        merge_method: Some(merge_method.to_string()),
    };

    let response: MergePullRequestResponse = client
        .put_json(
            format!(
                "{}/repos/{}/{}/pulls/{}/merge",
                client.base_url, input.owner, input.repo, input.pull_number
            ),
            &request,
        )
        .await?;

    Ok(MergePullRequestOutput {
        merged: response.merged,
        sha: response.sha,
    })
}

// ============================================================================
// Tool 7: Close Issue/PR
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CloseInput {
    /// Repository owner
    pub owner: String,
    /// Repository name
    pub repo: String,
    /// Issue or PR number
    pub number: u64,
    /// Whether this is a pull request (default: false)
    #[serde(default)]
    pub is_pull_request: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CloseOutput {
    pub closed: bool,
}

/// # Close GitHub Issue or Pull Request
///
/// Closes an open issue or pull request, changing its state to "closed".
///
/// Use this tool when a user wants to close an issue that has been resolved or
/// a pull request that has been merged or is no longer needed. Closing an issue
/// or PR is different from merging a PR - this simply changes the state without
/// merging any code changes. The issue/PR number must exist and the user must
/// have write permissions to the repository.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - github
/// - issues
/// - pull-requests
///
/// # Errors
///
/// Returns an error if:
/// - `owner` is empty or contains only whitespace
/// - `repo` is empty or contains only whitespace
/// - `number` is not positive
/// - GitHub credentials are missing or invalid
/// - The GitHub API request fails (network errors, rate limiting,
///   authentication failures)
/// - The response from GitHub cannot be parsed
#[tool]
pub async fn close(ctx: Context, input: CloseInput) -> Result<CloseOutput> {
    ensure!(!input.owner.trim().is_empty(), "owner must not be empty");
    ensure!(!input.repo.trim().is_empty(), "repo must not be empty");
    ensure!(input.number > 0, "number must be positive");

    let client = GitHubClient::from_ctx(&ctx)?;

    let request = CloseRequest {
        state: "closed".to_string(),
    };

    let endpoint = if input.is_pull_request {
        format!(
            "{}/repos/{}/{}/pulls/{}",
            client.base_url, input.owner, input.repo, input.number
        )
    } else {
        format!(
            "{}/repos/{}/{}/issues/{}",
            client.base_url, input.owner, input.repo, input.number
        )
    };

    let _: serde_json::Value = client.patch_json(endpoint, &request).await?;

    Ok(CloseOutput { closed: true })
}

// ============================================================================
// HTTP Client
// ============================================================================

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
    /// - GitHub credentials are not found in the context
    /// - The `access_token` in credentials is empty or contains only whitespace
    /// - The `endpoint` in credentials (if provided) is empty after trimming
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = GitHubCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url = normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_GITHUB_API))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            access_token: cred.access_token,
        })
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: String,
        query: &[(&str, &str)],
    ) -> Result<T> {
        let response = self
            .http
            .get(&url)
            .query(query)
            .bearer_auth(&self.access_token)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.json::<T>().await?)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "GitHub API request failed ({status}): {body}"
            ))
        }
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: String,
        body: &TReq,
    ) -> Result<TRes> {
        let response = self
            .http
            .post(&url)
            .json(body)
            .bearer_auth(&self.access_token)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.json::<TRes>().await?)
        } else {
            let body_text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "GitHub API request failed ({status}): {body_text}"
            ))
        }
    }

    async fn put_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: String,
        body: &TReq,
    ) -> Result<TRes> {
        let response = self
            .http
            .put(&url)
            .json(body)
            .bearer_auth(&self.access_token)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.json::<TRes>().await?)
        } else {
            let body_text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "GitHub API request failed ({status}): {body_text}"
            ))
        }
    }

    async fn patch_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: String,
        body: &TReq,
    ) -> Result<TRes> {
        let response = self
            .http
            .patch(&url)
            .json(body)
            .bearer_auth(&self.access_token)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.json::<TRes>().await?)
        } else {
            let body_text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "GitHub API request failed ({status}): {body_text}"
            ))
        }
    }
}

/// Normalizes a GitHub API endpoint URL by trimming whitespace and trailing
/// slashes.
///
/// # Errors
///
/// Returns an error if the endpoint is empty or contains only whitespace.
fn normalize_base_url(endpoint: &str) -> Result<String> {
    let trimmed = endpoint.trim();
    ensure!(!trimmed.is_empty(), "endpoint must not be empty");
    Ok(trimmed.trim_end_matches('/').to_string())
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct GitHubSearchResponse {
    items: Vec<types::OctoIssue>,
}

#[derive(Debug, Serialize)]
struct CreateIssueRequest {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    labels: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assignees: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct CommentRequest {
    body: String,
}

#[derive(Debug, Serialize)]
struct CreatePullRequestRequest {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    head: String,
    base: String,
    draft: bool,
}

#[derive(Debug, Serialize)]
struct RequestReviewersRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    reviewers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    team_reviewers: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct MergePullRequestRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    merge_method: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MergePullRequestResponse {
    merged: bool,
    sha: String,
}

#[derive(Debug, Serialize)]
struct CloseRequest {
    state: String,
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use types::{IssueState, PullRequestState};
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_string_contains, header, method, path, query_param},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut github_values = HashMap::new();
        github_values.insert("access_token".to_string(), "test-token".to_string());
        github_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("github", github_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        server.uri()
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_issue_state_serialization_roundtrip() {
        for variant in [IssueState::Open, IssueState::Closed] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: IssueState = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_pull_request_state_serialization_roundtrip() {
        for variant in [PullRequestState::Open, PullRequestState::Closed] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: PullRequestState = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_search_filter_serialization_roundtrip() {
        for variant in [SearchFilter::Issue, SearchFilter::Pr, SearchFilter::All] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: SearchFilter = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://api.github.com/").unwrap();
        assert_eq!(result, "https://api.github.com");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://api.github.com  ").unwrap();
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
    async fn test_search_issues_prs_empty_owner_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = search_issues_prs(
            ctx,
            SearchIssuesInput {
                owner: "  ".to_string(),
                repo: "repo".to_string(),
                query: "bug".to_string(),
                filter: None,
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
    async fn test_create_issue_empty_title_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = create_issue(
            ctx,
            CreateIssueInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                title: "  ".to_string(),
                body: None,
                labels: vec![],
                assignees: vec![],
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
        let ctx = test_ctx(&endpoint_for(&server));

        let result = comment(
            ctx,
            CommentInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                issue_number: 1,
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
    async fn test_open_pull_request_empty_head_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = open_pull_request(
            ctx,
            OpenPullRequestInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                title: "PR".to_string(),
                body: None,
                head: "  ".to_string(),
                base: "main".to_string(),
                draft: false,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("head must not be empty")
        );
    }

    #[tokio::test]
    async fn test_request_review_empty_reviewers_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = request_review(
            ctx,
            RequestReviewInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                pull_number: 1,
                reviewers: vec![],
                team_reviewers: vec![],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("at least one reviewer")
        );
    }

    #[tokio::test]
    async fn test_merge_pull_request_invalid_merge_method_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = merge_pull_request(
            ctx,
            MergePullRequestInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                pull_number: 1,
                commit_message: None,
                merge_method: Some("invalid".to_string()),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("merge_method"));
    }

    #[tokio::test]
    async fn test_close_zero_number_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = close(
            ctx,
            CloseInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                number: 0,
                is_pull_request: false,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("number must be positive")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_search_issues_prs_success_returns_results() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "items": [
            {
              "number": 1,
              "title": "Bug report",
              "body": "This is a bug",
              "state": "open",
              "html_url": "https://github.com/owner/repo/issues/1",
              "user": { "login": "alice", "id": 100 },
              "labels": [],
              "created_at": "2024-01-01T00:00:00Z"
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/search/issues"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("q", "bug repo:owner/repo"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = search_issues_prs(
            ctx,
            SearchIssuesInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                query: "bug".to_string(),
                filter: Some(SearchFilter::Issue),
                limit: Some(30),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.issues.len(), 1);
        assert_eq!(output.issues[0].number, 1);
        assert_eq!(output.issues[0].title, "Bug report");
    }

    #[tokio::test]
    async fn test_create_issue_success_returns_issue() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "number": 2,
          "title": "New issue",
          "body": "Issue description",
          "state": "open",
          "html_url": "https://github.com/owner/repo/issues/2",
          "user": { "login": "alice", "id": 100 },
          "labels": [],
          "created_at": "2024-01-01T00:00:00Z"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/repos/owner/repo/issues"))
            .and(body_string_contains("\"title\":\"New issue\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = create_issue(
            ctx,
            CreateIssueInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                title: "New issue".to_string(),
                body: Some("Issue description".to_string()),
                labels: vec![],
                assignees: vec![],
            },
        )
        .await
        .unwrap();

        assert_eq!(output.issue.number, 2);
        assert_eq!(output.issue.title, "New issue");
    }

    #[tokio::test]
    async fn test_comment_success_returns_comment() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": 1,
          "body": "Thanks for reporting",
          "user": { "login": "alice", "id": 100 },
          "html_url": "https://github.com/owner/repo/issues/1#issuecomment-1",
          "created_at": "2024-01-01T00:00:00Z"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/repos/owner/repo/issues/1/comments"))
            .and(body_string_contains("\"body\":\"Thanks for reporting\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = comment(
            ctx,
            CommentInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                issue_number: 1,
                body: "Thanks for reporting".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.comment.id, 1);
        assert_eq!(output.comment.body, "Thanks for reporting");
    }

    #[tokio::test]
    async fn test_close_issue_success_returns_closed() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "number": 1,
          "title": "Bug report",
          "state": "closed",
          "html_url": "https://github.com/owner/repo/issues/1"
        }
        "#;

        Mock::given(method("PATCH"))
            .and(path("/repos/owner/repo/issues/1"))
            .and(body_string_contains("\"state\":\"closed\""))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = close(
            ctx,
            CloseInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                number: 1,
                is_pull_request: false,
            },
        )
        .await
        .unwrap();

        assert!(output.closed);
    }

    #[tokio::test]
    async fn test_search_issues_prs_filters_pull_requests() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "items": [
            {
              "number": 1,
              "title": "Bug fix",
              "body": "Fixes a bug",
              "state": "open",
              "html_url": "https://github.com/owner/repo/pull/1",
              "user": { "login": "alice", "id": 100 },
              "labels": [],
              "created_at": "2024-01-01T00:00:00Z",
              "pull_request": {}
            },
            {
              "number": 2,
              "title": "Feature request",
              "body": "Add feature",
              "state": "open",
              "html_url": "https://github.com/owner/repo/issues/2",
              "user": { "login": "bob", "id": 101 },
              "labels": [],
              "created_at": "2024-01-01T00:00:00Z"
            }
          ]
        }
        "#;

        let pr_details_body = r#"
        {
          "number": 1,
          "title": "Bug fix",
          "body": "Fixes a bug",
          "state": "open",
          "html_url": "https://github.com/owner/repo/pull/1",
          "user": { "login": "alice", "id": 100 },
          "head": { "ref": "feature-branch", "sha": "abc123" },
          "base": { "ref": "main", "sha": "def456" },
          "draft": false,
          "created_at": "2024-01-01T00:00:00Z"
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/search/issues"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("q", "bug repo:owner/repo"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/pulls/1"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(pr_details_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = search_issues_prs(
            ctx,
            SearchIssuesInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                query: "bug".to_string(),
                filter: Some(SearchFilter::Pr),
                limit: Some(30),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.issues.len(), 0);
        assert_eq!(output.pull_requests.len(), 1);
        assert_eq!(output.pull_requests[0].number, 1);
        assert_eq!(output.pull_requests[0].title, "Bug fix");
    }

    #[tokio::test]
    async fn test_open_pull_request_success_returns_pull_request() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "number": 3,
          "title": "New feature",
          "body": "Feature description",
          "state": "open",
          "html_url": "https://github.com/owner/repo/pull/3",
          "user": { "login": "alice", "id": 100 },
          "head": { "ref": "feature", "sha": "abc123" },
          "base": { "ref": "main", "sha": "def456" },
          "draft": false,
          "created_at": "2024-01-01T00:00:00Z"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/repos/owner/repo/pulls"))
            .and(body_string_contains("\"title\":\"New feature\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = open_pull_request(
            ctx,
            OpenPullRequestInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                title: "New feature".to_string(),
                body: Some("Feature description".to_string()),
                head: "feature".to_string(),
                base: "main".to_string(),
                draft: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.pull_request.number, 3);
        assert_eq!(output.pull_request.title, "New feature");
        assert!(!output.pull_request.draft);
    }

    #[tokio::test]
    async fn test_request_review_success_returns_requested() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "number": 1,
          "title": "PR",
          "state": "open",
          "html_url": "https://github.com/owner/repo/pull/1"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/repos/owner/repo/pulls/1/requested_reviewers"))
            .and(body_string_contains("\"reviewers\":[\"alice\"]"))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = request_review(
            ctx,
            RequestReviewInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                pull_number: 1,
                reviewers: vec!["alice".to_string()],
                team_reviewers: vec![],
            },
        )
        .await
        .unwrap();

        assert!(output.requested);
    }

    #[tokio::test]
    async fn test_merge_pull_request_success_returns_merged() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "merged": true,
          "sha": "abc123def456"
        }
        "#;

        Mock::given(method("PUT"))
            .and(path("/repos/owner/repo/pulls/1/merge"))
            .and(body_string_contains("\"merge_method\":\"squash\""))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = merge_pull_request(
            ctx,
            MergePullRequestInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                pull_number: 1,
                commit_message: None,
                merge_method: Some("squash".to_string()),
            },
        )
        .await
        .unwrap();

        assert!(output.merged);
        assert_eq!(output.sha, "abc123def456");
    }

    #[tokio::test]
    async fn test_close_pull_request_success_returns_closed() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "number": 1,
          "title": "PR title",
          "state": "closed",
          "html_url": "https://github.com/owner/repo/pull/1"
        }
        "#;

        Mock::given(method("PATCH"))
            .and(path("/repos/owner/repo/pulls/1"))
            .and(body_string_contains("\"state\":\"closed\""))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = close(
            ctx,
            CloseInput {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                number: 1,
                is_pull_request: true,
            },
        )
        .await
        .unwrap();

        assert!(output.closed);
    }
}
