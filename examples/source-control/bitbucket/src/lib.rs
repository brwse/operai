//! source-control/bitbucket integration for Operai Toolbox.

mod types;

use operai::{
    Context, JsonSchema, Result, define_system_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
use types::{
    BitbucketComment, BitbucketPullRequest, BranchName, BranchRef, CommentContentInput,
    CreateCommentRequest, CreatePullRequestRequest, DeclinePullRequestRequest,
    MergePullRequestRequest, PaginatedResponse, Participant, PullRequestState, PullRequestSummary,
    ReviewerRef,
};

define_system_credential! {
    BitbucketCredential("bitbucket") {
        username: String,
        password: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_API_ENDPOINT: &str = "https://api.bitbucket.org/2.0";

#[init]
async fn setup() -> Result<()> {
    info!("Bitbucket integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Bitbucket integration shutting down");
}

// ============================================================================
// Tool: search_pull_requests
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchPullRequestsInput {
    /// Workspace ID (username or team name).
    pub workspace: String,
    /// Repository slug.
    pub repo_slug: String,
    /// Filter by state (e.g., "OPEN", "MERGED", "DECLINED"). Optional.
    #[serde(default)]
    pub state: Option<PullRequestState>,
    /// Maximum number of results (1-100). Defaults to 10.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchPullRequestsOutput {
    pub pull_requests: Vec<PullRequestSummary>,
}

/// # Search Bitbucket Pull Requests
///
/// Searches for pull requests in a Bitbucket repository with optional filtering
/// by state. Use this tool when you need to find, list, or browse pull requests
/// in a Bitbucket repository. This is the primary tool for discovering pull
/// requests, whether you're looking for all open PRs, recently merged PRs, or
/// PRs in a specific state.
///
/// Key inputs:
/// - `workspace`: The Bitbucket workspace (username or team name) that owns the
///   repository
/// - `repo_slug`: The repository identifier (slug, not the full name)
/// - `state`: Optional filter to limit results to specific states (OPEN,
///   MERGED, DECLINED, SUPERSEDED)
/// - `limit`: Maximum number of results to return (1-100, defaults to 10)
///
/// Returns a list of pull request summaries including ID, title, state, author,
/// branches, and timestamps.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - source-control
/// - bitbucket
/// - pull-request
///
/// # Errors
///
/// Returns an error if:
/// - The `workspace` or `repo_slug` is empty after trimming whitespace
/// - The `limit` parameter is outside the valid range (1-100)
/// - No Bitbucket credentials are configured or they are invalid
/// - The Bitbucket API request fails (network error, authentication failure,
///   etc.)
/// - The API response cannot be parsed as JSON
#[tool]
pub async fn search_pull_requests(
    ctx: Context,
    input: SearchPullRequestsInput,
) -> Result<SearchPullRequestsOutput> {
    ensure!(
        !input.workspace.trim().is_empty(),
        "workspace must not be empty"
    );
    ensure!(
        !input.repo_slug.trim().is_empty(),
        "repo_slug must not be empty"
    );

    let limit = input.limit.unwrap_or(10);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let client = BitbucketClient::from_ctx(&ctx)?;
    let mut url = client.url_with_segments(&[
        "repositories",
        &input.workspace,
        &input.repo_slug,
        "pullrequests",
    ])?;

    if let Some(state) = input.state {
        let state_str = match state {
            PullRequestState::Open => "OPEN",
            PullRequestState::Merged => "MERGED",
            PullRequestState::Declined => "DECLINED",
            PullRequestState::Superseded => "SUPERSEDED",
        };
        url.query_pairs_mut().append_pair("state", state_str);
    }

    url.query_pairs_mut()
        .append_pair("pagelen", &limit.to_string());

    let response: PaginatedResponse<BitbucketPullRequest> = client.get_json(url).await?;

    Ok(SearchPullRequestsOutput {
        pull_requests: response.values.into_iter().map(map_pull_request).collect(),
    })
}

// ============================================================================
// Tool: get_pull_request
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetPullRequestInput {
    /// Workspace ID (username or team name).
    pub workspace: String,
    /// Repository slug.
    pub repo_slug: String,
    /// Pull request ID.
    pub pull_request_id: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetPullRequestOutput {
    pub pull_request: PullRequestSummary,
    pub participants: Vec<Participant>,
}

/// # Get Bitbucket Pull Request
///
/// Retrieves detailed information about a specific pull request from a
/// Bitbucket repository. Use this tool when you need comprehensive details
/// about a particular PR, including its description, participants, reviewers,
/// and approval status. This provides more detailed information than the search
/// results, including the full list of participants with their approval status
/// and roles.
///
/// Key inputs:
/// - `workspace`: The Bitbucket workspace (username or team name) that owns the
///   repository
/// - `repo_slug`: The repository identifier (slug, not the full name)
/// - `pull_request_id`: The numeric ID of the pull request (obtained from
///   search or create operations)
///
/// Returns:
/// - Full pull request details (title, description, state, author, branches,
///   timestamps)
/// - List of participants with their roles (REVIEWER or PARTICIPANT), approval
///   status, and participation dates
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - source-control
/// - bitbucket
/// - pull-request
///
/// # Errors
///
/// Returns an error if:
/// - The `workspace` or `repo_slug` is empty after trimming whitespace
/// - No Bitbucket credentials are configured or they are invalid
/// - The Bitbucket API request fails (network error, authentication failure, PR
///   not found, etc.)
/// - The API response cannot be parsed as JSON
#[tool]
pub async fn get_pull_request(
    ctx: Context,
    input: GetPullRequestInput,
) -> Result<GetPullRequestOutput> {
    ensure!(
        !input.workspace.trim().is_empty(),
        "workspace must not be empty"
    );
    ensure!(
        !input.repo_slug.trim().is_empty(),
        "repo_slug must not be empty"
    );

    let client = BitbucketClient::from_ctx(&ctx)?;
    let url = client.url_with_segments(&[
        "repositories",
        &input.workspace,
        &input.repo_slug,
        "pullrequests",
        &input.pull_request_id.to_string(),
    ])?;

    let pr: BitbucketPullRequest = client.get_json(url).await?;
    let participants = pr.participants.clone();

    Ok(GetPullRequestOutput {
        pull_request: map_pull_request(pr),
        participants,
    })
}

// ============================================================================
// Tool: create_pull_request
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreatePullRequestInput {
    /// Workspace ID (username or team name).
    pub workspace: String,
    /// Repository slug.
    pub repo_slug: String,
    /// Pull request title.
    pub title: String,
    /// Pull request description. Optional.
    #[serde(default)]
    pub description: Option<String>,
    /// Source branch name.
    pub source_branch: String,
    /// Destination branch name.
    pub destination_branch: String,
    /// List of reviewer UUIDs. Optional.
    #[serde(default)]
    pub reviewers: Vec<String>,
    /// Close source branch after merge. Defaults to false.
    #[serde(default)]
    pub close_source_branch: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreatePullRequestOutput {
    pub pull_request_id: u64,
    pub title: String,
    pub state: PullRequestState,
}

/// # Create Bitbucket Pull Request
///
/// Creates a new pull request in a Bitbucket repository to merge changes from a
/// source branch into a destination branch. Use this tool when a user wants to
/// propose code changes for review and merging. This is the primary tool for
/// initiating the code review process in Bitbucket.
///
/// Key inputs:
/// - `workspace`: The Bitbucket workspace (username or team name) that owns the
///   repository
/// - `repo_slug`: The repository identifier (slug, not the full name)
/// - `title`: The pull request title (should be concise and descriptive)
/// - `description`: Optional detailed description of the changes (markdown
///   supported)
/// - `source_branch`: The branch containing the changes to be merged
/// - `destination_branch`: The branch to merge changes into (typically "main"
///   or "develop")
/// - `reviewers`: Optional list of reviewer UUIDs to request reviews from
/// - `close_source_branch`: Optional flag to automatically close the source
///   branch after merging
///
/// Returns the created pull request ID, title, and initial state (typically
/// OPEN).
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - source-control
/// - bitbucket
/// - pull-request
///
/// # Errors
///
/// Returns an error if:
/// - The `workspace`, `repo_slug`, `title`, `source_branch`, or
///   `destination_branch` is empty after trimming whitespace
/// - No Bitbucket credentials are configured or they are invalid
/// - The Bitbucket API request fails (network error, authentication failure,
///   invalid branch, etc.)
/// - The API response cannot be parsed as JSON
#[tool]
pub async fn create_pull_request(
    ctx: Context,
    input: CreatePullRequestInput,
) -> Result<CreatePullRequestOutput> {
    ensure!(
        !input.workspace.trim().is_empty(),
        "workspace must not be empty"
    );
    ensure!(
        !input.repo_slug.trim().is_empty(),
        "repo_slug must not be empty"
    );
    ensure!(!input.title.trim().is_empty(), "title must not be empty");
    ensure!(
        !input.source_branch.trim().is_empty(),
        "source_branch must not be empty"
    );
    ensure!(
        !input.destination_branch.trim().is_empty(),
        "destination_branch must not be empty"
    );

    let client = BitbucketClient::from_ctx(&ctx)?;
    let url = client.url_with_segments(&[
        "repositories",
        &input.workspace,
        &input.repo_slug,
        "pullrequests",
    ])?;

    let request = CreatePullRequestRequest {
        title: input.title,
        description: input.description,
        source: BranchRef {
            branch: BranchName {
                name: input.source_branch,
            },
        },
        destination: BranchRef {
            branch: BranchName {
                name: input.destination_branch,
            },
        },
        reviewers: input
            .reviewers
            .into_iter()
            .map(|uuid| ReviewerRef { uuid })
            .collect(),
        close_source_branch: input.close_source_branch,
    };

    let pr: BitbucketPullRequest = client.post_json(url, &request).await?;

    Ok(CreatePullRequestOutput {
        pull_request_id: pr.id,
        title: pr.title,
        state: pr.state,
    })
}

// ============================================================================
// Tool: add_comment
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddCommentInput {
    /// Workspace ID (username or team name).
    pub workspace: String,
    /// Repository slug.
    pub repo_slug: String,
    /// Pull request ID.
    pub pull_request_id: u64,
    /// Comment text.
    pub comment: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AddCommentOutput {
    pub comment_id: u64,
    pub created_on: String,
}

/// # Add Comment to Bitbucket Pull Request
///
/// Adds a comment to a Bitbucket pull request discussion. Use this tool when a
/// user wants to provide feedback, ask questions, or participate in the pull
/// request review conversation. Comments are posted as the authenticated user
/// and support markdown formatting.
///
/// Key inputs:
/// - `workspace`: The Bitbucket workspace (username or team name) that owns the
///   repository
/// - `repo_slug`: The repository identifier (slug, not the full name)
/// - `pull_request_id`: The numeric ID of the pull request to comment on
/// - `comment`: The comment text (supports markdown formatting, must not be
///   empty)
///
/// Returns the created comment ID and timestamp. The comment will be visible to
/// all participants in the pull request and will trigger notifications for
/// reviewers and author.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - source-control
/// - bitbucket
/// - pull-request
/// - comment
///
/// # Errors
///
/// Returns an error if:
/// - The `workspace`, `repo_slug`, or comment text is empty after trimming
///   whitespace
/// - No Bitbucket credentials are configured or they are invalid
/// - The Bitbucket API request fails (network error, authentication failure, PR
///   not found, etc.)
/// - The API response cannot be parsed as JSON
#[tool]
pub async fn add_comment(ctx: Context, input: AddCommentInput) -> Result<AddCommentOutput> {
    ensure!(
        !input.workspace.trim().is_empty(),
        "workspace must not be empty"
    );
    ensure!(
        !input.repo_slug.trim().is_empty(),
        "repo_slug must not be empty"
    );
    ensure!(
        !input.comment.trim().is_empty(),
        "comment must not be empty"
    );

    let client = BitbucketClient::from_ctx(&ctx)?;
    let url = client.url_with_segments(&[
        "repositories",
        &input.workspace,
        &input.repo_slug,
        "pullrequests",
        &input.pull_request_id.to_string(),
        "comments",
    ])?;

    let request = CreateCommentRequest {
        content: CommentContentInput { raw: input.comment },
    };

    let comment: BitbucketComment = client.post_json(url, &request).await?;

    Ok(AddCommentOutput {
        comment_id: comment.id,
        created_on: comment.created_on,
    })
}

// ============================================================================
// Tool: approve_pull_request
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ApprovePullRequestInput {
    /// Workspace ID (username or team name).
    pub workspace: String,
    /// Repository slug.
    pub repo_slug: String,
    /// Pull request ID.
    pub pull_request_id: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ApprovePullRequestOutput {
    pub approved: bool,
}

/// # Approve Bitbucket Pull Request
///
/// Approves a Bitbucket pull request on behalf of the authenticated user. Use
/// this tool when a user wants to indicate their approval and signal that the
/// pull request is ready to merge. Approval is a key part of the code review
/// workflow and typically required before merging.
///
/// Key inputs:
/// - `workspace`: The Bitbucket workspace (username or team name) that owns the
///   repository
/// - `repo_slug`: The repository identifier (slug, not the full name)
/// - `pull_request_id`: The numeric ID of the pull request to approve
///
/// Returns confirmation of approval. The approval will be visible to other
/// participants and may satisfy merge requirements depending on repository
/// settings. Note: If the user has already approved, this will update their
/// existing approval.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - source-control
/// - bitbucket
/// - pull-request
/// - review
///
/// # Errors
///
/// Returns an error if:
/// - The `workspace` or `repo_slug` is empty after trimming whitespace
/// - No Bitbucket credentials are configured or they are invalid
/// - The Bitbucket API request fails (network error, authentication failure, PR
///   not found, etc.)
#[tool]
pub async fn approve_pull_request(
    ctx: Context,
    input: ApprovePullRequestInput,
) -> Result<ApprovePullRequestOutput> {
    ensure!(
        !input.workspace.trim().is_empty(),
        "workspace must not be empty"
    );
    ensure!(
        !input.repo_slug.trim().is_empty(),
        "repo_slug must not be empty"
    );

    let client = BitbucketClient::from_ctx(&ctx)?;
    let url = client.url_with_segments(&[
        "repositories",
        &input.workspace,
        &input.repo_slug,
        "pullrequests",
        &input.pull_request_id.to_string(),
        "approve",
    ])?;

    client.post_empty(url).await?;

    Ok(ApprovePullRequestOutput { approved: true })
}

// ============================================================================
// Tool: unapprove_pull_request
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UnapprovePullRequestInput {
    /// Workspace ID (username or team name).
    pub workspace: String,
    /// Repository slug.
    pub repo_slug: String,
    /// Pull request ID.
    pub pull_request_id: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UnapprovePullRequestOutput {
    pub unapproved: bool,
}

/// # Unapprove Bitbucket Pull Request
///
/// Removes the authenticated user's approval from a Bitbucket pull request. Use
/// this tool when a user wants to revoke their previous approval, typically
/// because new changes were added after they reviewed it or they identified
/// issues that need to be addressed.
///
/// Key inputs:
/// - `workspace`: The Bitbucket workspace (username or team name) that owns the
///   repository
/// - `repo_slug`: The repository identifier (slug, not the full name)
/// - `pull_request_id`: The numeric ID of the pull request to unapprove
///
/// Returns confirmation of approval removal. The unapproval will be visible to
/// other participants and may block merging if approval requirements are no
/// longer met. This is the inverse of the approve operation.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - source-control
/// - bitbucket
/// - pull-request
/// - review
///
/// # Errors
///
/// Returns an error if:
/// - The `workspace` or `repo_slug` is empty after trimming whitespace
/// - No Bitbucket credentials are configured or they are invalid
/// - The Bitbucket API request fails (network error, authentication failure, PR
///   not found, etc.)
#[tool]
pub async fn unapprove_pull_request(
    ctx: Context,
    input: UnapprovePullRequestInput,
) -> Result<UnapprovePullRequestOutput> {
    ensure!(
        !input.workspace.trim().is_empty(),
        "workspace must not be empty"
    );
    ensure!(
        !input.repo_slug.trim().is_empty(),
        "repo_slug must not be empty"
    );

    let client = BitbucketClient::from_ctx(&ctx)?;
    let url = client.url_with_segments(&[
        "repositories",
        &input.workspace,
        &input.repo_slug,
        "pullrequests",
        &input.pull_request_id.to_string(),
        "approve",
    ])?;

    client.delete(url).await?;

    Ok(UnapprovePullRequestOutput { unapproved: true })
}

// ============================================================================
// Tool: merge_pull_request
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MergePullRequestInput {
    /// Workspace ID (username or team name).
    pub workspace: String,
    /// Repository slug.
    pub repo_slug: String,
    /// Pull request ID.
    pub pull_request_id: u64,
    /// Merge commit message. Optional.
    #[serde(default)]
    pub message: Option<String>,
    /// Close source branch after merge. Optional.
    #[serde(default)]
    pub close_source_branch: Option<bool>,
    /// Merge strategy (e.g., "`merge_commit`", "squash", "`fast_forward`").
    /// Optional.
    #[serde(default)]
    pub merge_strategy: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MergePullRequestOutput {
    pub merged: bool,
    pub pull_request_id: u64,
}

/// # Merge Bitbucket Pull Request
///
/// Merges a Bitbucket pull request into its destination branch. Use this tool
/// when a user wants to complete the code review process and integrate the
/// changes into the target branch. This operation will fail if the pull request
/// does not meet merge requirements (e.g., insufficient approvals, merge
/// conflicts, failing build checks).
///
/// Key inputs:
/// - `workspace`: The Bitbucket workspace (username or team name) that owns the
///   repository
/// - `repo_slug`: The repository identifier (slug, not the full name)
/// - `pull_request_id`: The numeric ID of the pull request to merge
/// - `message`: Optional custom merge commit message
/// - `close_source_branch`: Optional flag to close the source branch after
///   merging
/// - `merge_strategy`: Optional merge strategy ("`merge_commit`", "squash", or
///   "`fast_forward`")
///
/// Returns confirmation of merge. The pull request state will change to MERGED
/// upon success. Merge failures may occur due to conflicts, insufficient
/// approvals, or policy violations.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - source-control
/// - bitbucket
/// - pull-request
/// - merge
///
/// # Errors
///
/// Returns an error if:
/// - The `workspace` or `repo_slug` is empty after trimming whitespace
/// - No Bitbucket credentials are configured or they are invalid
/// - The Bitbucket API request fails (network error, authentication failure,
///   merge conflicts, etc.)
/// - The API response cannot be parsed as JSON
#[tool]
pub async fn merge_pull_request(
    ctx: Context,
    input: MergePullRequestInput,
) -> Result<MergePullRequestOutput> {
    ensure!(
        !input.workspace.trim().is_empty(),
        "workspace must not be empty"
    );
    ensure!(
        !input.repo_slug.trim().is_empty(),
        "repo_slug must not be empty"
    );

    let client = BitbucketClient::from_ctx(&ctx)?;
    let url = client.url_with_segments(&[
        "repositories",
        &input.workspace,
        &input.repo_slug,
        "pullrequests",
        &input.pull_request_id.to_string(),
        "merge",
    ])?;

    let request = MergePullRequestRequest {
        message: input.message,
        close_source_branch: input.close_source_branch,
        merge_strategy: input.merge_strategy,
    };

    let _: BitbucketPullRequest = client.post_json(url, &request).await?;

    Ok(MergePullRequestOutput {
        merged: true,
        pull_request_id: input.pull_request_id,
    })
}

// ============================================================================
// Tool: decline_pull_request
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeclinePullRequestInput {
    /// Workspace ID (username or team name).
    pub workspace: String,
    /// Repository slug.
    pub repo_slug: String,
    /// Pull request ID.
    pub pull_request_id: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DeclinePullRequestOutput {
    pub declined: bool,
    pub pull_request_id: u64,
}

/// # Decline Bitbucket Pull Request
///
/// Declines (closes without merging) a Bitbucket pull request. Use this tool
/// when a user wants to reject a pull request and prevent it from being merged.
/// This is typically done when the changes are no longer needed, the approach
/// is not acceptable, or the pull request should be abandoned. Declined pull
/// requests cannot be merged (they must be reopened with a new PR).
///
/// Key inputs:
/// - `workspace`: The Bitbucket workspace (username or team name) that owns the
///   repository
/// - `repo_slug`: The repository identifier (slug, not the full name)
/// - `pull_request_id`: The numeric ID of the pull request to decline
///
/// Returns confirmation of decline. The pull request state will change to
/// DECLINED upon success. This action is irreversible; a declined pull request
/// cannot be reopened and a new PR must be created if the changes are still
/// needed.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - source-control
/// - bitbucket
/// - pull-request
///
/// # Errors
///
/// Returns an error if:
/// - The `workspace` or `repo_slug` is empty after trimming whitespace
/// - No Bitbucket credentials are configured or they are invalid
/// - The Bitbucket API request fails (network error, authentication failure, PR
///   not found, etc.)
/// - The API response cannot be parsed as JSON
#[tool]
pub async fn decline_pull_request(
    ctx: Context,
    input: DeclinePullRequestInput,
) -> Result<DeclinePullRequestOutput> {
    ensure!(
        !input.workspace.trim().is_empty(),
        "workspace must not be empty"
    );
    ensure!(
        !input.repo_slug.trim().is_empty(),
        "repo_slug must not be empty"
    );

    let client = BitbucketClient::from_ctx(&ctx)?;
    let url = client.url_with_segments(&[
        "repositories",
        &input.workspace,
        &input.repo_slug,
        "pullrequests",
        &input.pull_request_id.to_string(),
        "decline",
    ])?;

    let request = DeclinePullRequestRequest {};
    let _: BitbucketPullRequest = client.post_json(url, &request).await?;

    Ok(DeclinePullRequestOutput {
        declined: true,
        pull_request_id: input.pull_request_id,
    })
}

// ============================================================================
// Helper: BitbucketClient
// ============================================================================

#[derive(Debug, Clone)]
struct BitbucketClient {
    http: reqwest::Client,
    base_url: String,
    username: String,
    password: String,
}

impl BitbucketClient {
    /// Creates a new `BitbucketClient` from the Brwse context.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No Bitbucket credentials are configured
    /// - The username or password is empty after trimming whitespace
    /// - The endpoint URL is invalid (if custom endpoint is configured)
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = BitbucketCredential::get(ctx)?;
        ensure!(
            !cred.username.trim().is_empty(),
            "username must not be empty"
        );
        ensure!(
            !cred.password.trim().is_empty(),
            "password must not be empty"
        );

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_API_ENDPOINT))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            username: cred.username,
            password: cred.password,
        })
    }

    /// Builds a URL by appending path segments to the base URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the `base_url` is not a valid absolute URL that can
    /// have path segments.
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

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, url: reqwest::Url) -> Result<T> {
        let response = self.send_request(self.http.get(url)).await?;
        Ok(response.json::<T>().await?)
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
    ) -> Result<TRes> {
        let response = self.send_request(self.http.post(url).json(body)).await?;
        Ok(response.json::<TRes>().await?)
    }

    async fn post_empty(&self, url: reqwest::Url) -> Result<()> {
        self.send_request(self.http.post(url)).await?;
        Ok(())
    }

    async fn delete(&self, url: reqwest::Url) -> Result<()> {
        self.send_request(self.http.delete(url)).await?;
        Ok(())
    }

    async fn send_request(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let response = request
            .basic_auth(&self.username, Some(&self.password))
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Bitbucket API request failed ({status}): {body}"
            ))
        }
    }
}

/// Normalizes a Bitbucket API endpoint URL by trimming whitespace and trailing
/// slashes.
///
/// # Errors
///
/// Returns an error if the endpoint is empty after trimming whitespace.
fn normalize_base_url(endpoint: &str) -> Result<String> {
    let trimmed = endpoint.trim();
    ensure!(!trimmed.is_empty(), "endpoint must not be empty");
    Ok(trimmed.trim_end_matches('/').to_string())
}

fn map_pull_request(pr: BitbucketPullRequest) -> PullRequestSummary {
    PullRequestSummary {
        id: pr.id,
        title: pr.title,
        description: pr.description,
        state: pr.state,
        author: pr.author,
        source: pr.source,
        destination: pr.destination,
        created_on: pr.created_on,
        updated_on: pr.updated_on,
        comment_count: pr.comment_count,
        task_count: pr.task_count,
    }
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{basic_auth, method, path, query_param},
    };

    use super::*;
    use crate::types::ParticipantRole;

    fn test_ctx(endpoint: &str) -> Context {
        let mut bitbucket_values = HashMap::new();
        bitbucket_values.insert("username".to_string(), "test-user".to_string());
        bitbucket_values.insert("password".to_string(), "test-password".to_string());
        bitbucket_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_system_credential("bitbucket", bitbucket_values)
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_pull_request_state_serialization_roundtrip() {
        for variant in [
            PullRequestState::Open,
            PullRequestState::Merged,
            PullRequestState::Declined,
            PullRequestState::Superseded,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: PullRequestState = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_participant_role_serialization_roundtrip() {
        for variant in [ParticipantRole::Reviewer, ParticipantRole::Participant] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: ParticipantRole = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://api.bitbucket.org/2.0/").unwrap();
        assert_eq!(result, "https://api.bitbucket.org/2.0");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://api.bitbucket.org/2.0  ").unwrap();
        assert_eq!(result, "https://api.bitbucket.org/2.0");
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
    async fn test_search_pull_requests_empty_workspace_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = search_pull_requests(
            ctx,
            SearchPullRequestsInput {
                workspace: "  ".to_string(),
                repo_slug: "my-repo".to_string(),
                state: None,
                limit: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("workspace must not be empty")
        );
    }

    #[tokio::test]
    async fn test_search_pull_requests_empty_repo_slug_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = search_pull_requests(
            ctx,
            SearchPullRequestsInput {
                workspace: "my-workspace".to_string(),
                repo_slug: "  ".to_string(),
                state: None,
                limit: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("repo_slug must not be empty")
        );
    }

    #[tokio::test]
    async fn test_search_pull_requests_limit_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = search_pull_requests(
            ctx,
            SearchPullRequestsInput {
                workspace: "my-workspace".to_string(),
                repo_slug: "my-repo".to_string(),
                state: None,
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
    async fn test_create_pull_request_empty_title_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = create_pull_request(
            ctx,
            CreatePullRequestInput {
                workspace: "my-workspace".to_string(),
                repo_slug: "my-repo".to_string(),
                title: "  ".to_string(),
                description: None,
                source_branch: "feature".to_string(),
                destination_branch: "main".to_string(),
                reviewers: vec![],
                close_source_branch: None,
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
    async fn test_create_pull_request_empty_source_branch_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = create_pull_request(
            ctx,
            CreatePullRequestInput {
                workspace: "my-workspace".to_string(),
                repo_slug: "my-repo".to_string(),
                title: "My PR".to_string(),
                description: None,
                source_branch: "  ".to_string(),
                destination_branch: "main".to_string(),
                reviewers: vec![],
                close_source_branch: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("source_branch must not be empty")
        );
    }

    #[tokio::test]
    async fn test_add_comment_empty_comment_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = add_comment(
            ctx,
            AddCommentInput {
                workspace: "my-workspace".to_string(),
                repo_slug: "my-repo".to_string(),
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

    // --- Integration tests ---

    #[tokio::test]
    async fn test_search_pull_requests_success_returns_results() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "values": [
            {
              "id": 1,
              "title": "Add feature X",
              "description": "This PR adds feature X",
              "state": "OPEN",
              "author": {
                "display_name": "Alice",
                "uuid": "{alice-uuid}",
                "nickname": "alice"
              },
              "source": {
                "branch": { "name": "feature-x" },
                "repository": {
                  "uuid": "{repo-uuid}",
                  "name": "my-repo",
                  "full_name": "workspace/my-repo"
                }
              },
              "destination": {
                "branch": { "name": "main" },
                "repository": {
                  "uuid": "{repo-uuid}",
                  "name": "my-repo",
                  "full_name": "workspace/my-repo"
                }
              },
              "created_on": "2024-01-01T00:00:00Z",
              "updated_on": "2024-01-02T00:00:00Z",
              "comment_count": 5,
              "task_count": 2
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/repositories/my-workspace/my-repo/pullrequests"))
            .and(basic_auth("test-user", "test-password"))
            .and(query_param("pagelen", "10"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = search_pull_requests(
            ctx,
            SearchPullRequestsInput {
                workspace: "my-workspace".to_string(),
                repo_slug: "my-repo".to_string(),
                state: None,
                limit: Some(10),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.pull_requests.len(), 1);
        assert_eq!(output.pull_requests[0].id, 1);
        assert_eq!(output.pull_requests[0].title, "Add feature X");
        assert_eq!(output.pull_requests[0].state, PullRequestState::Open);
    }

    #[tokio::test]
    async fn test_search_pull_requests_with_state_filter() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repositories/my-workspace/my-repo/pullrequests"))
            .and(query_param("state", "MERGED"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(r#"{"values": []}"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = search_pull_requests(
            ctx,
            SearchPullRequestsInput {
                workspace: "my-workspace".to_string(),
                repo_slug: "my-repo".to_string(),
                state: Some(PullRequestState::Merged),
                limit: Some(10),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.pull_requests.len(), 0);
    }

    #[tokio::test]
    async fn test_get_pull_request_success() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "id": 42,
          "title": "Fix critical bug",
          "description": "This PR fixes a critical bug",
          "state": "OPEN",
          "author": {
            "display_name": "Bob",
            "uuid": "{bob-uuid}"
          },
          "source": {
            "branch": { "name": "bugfix" },
            "repository": {
              "uuid": "{repo-uuid}",
              "name": "my-repo",
              "full_name": "workspace/my-repo"
            }
          },
          "destination": {
            "branch": { "name": "main" },
            "repository": {
              "uuid": "{repo-uuid}",
              "name": "my-repo",
              "full_name": "workspace/my-repo"
            }
          },
          "created_on": "2024-01-01T00:00:00Z",
          "updated_on": "2024-01-02T00:00:00Z",
          "comment_count": 3,
          "task_count": 1,
          "participants": [
            {
              "user": {
                "display_name": "Alice",
                "uuid": "{alice-uuid}"
              },
              "role": "REVIEWER",
              "approved": true,
              "participated_on": "2024-01-02T00:00:00Z"
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/repositories/my-workspace/my-repo/pullrequests/42"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = get_pull_request(
            ctx,
            GetPullRequestInput {
                workspace: "my-workspace".to_string(),
                repo_slug: "my-repo".to_string(),
                pull_request_id: 42,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.pull_request.id, 42);
        assert_eq!(output.pull_request.title, "Fix critical bug");
        assert_eq!(output.pull_request.state, PullRequestState::Open);
        assert_eq!(output.participants.len(), 1);
        assert_eq!(output.participants[0].user.display_name, "Alice");
        assert!(output.participants[0].approved);
    }

    #[tokio::test]
    async fn test_create_pull_request_success() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "id": 123,
          "title": "My New PR",
          "state": "OPEN",
          "author": {
            "display_name": "Bob",
            "uuid": "{bob-uuid}"
          },
          "source": {
            "branch": { "name": "feature" },
            "repository": {
              "uuid": "{repo-uuid}",
              "name": "my-repo",
              "full_name": "workspace/my-repo"
            }
          },
          "destination": {
            "branch": { "name": "main" },
            "repository": {
              "uuid": "{repo-uuid}",
              "name": "my-repo",
              "full_name": "workspace/my-repo"
            }
          },
          "created_on": "2024-01-01T00:00:00Z",
          "updated_on": "2024-01-01T00:00:00Z"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/repositories/my-workspace/my-repo/pullrequests"))
            .and(basic_auth("test-user", "test-password"))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = create_pull_request(
            ctx,
            CreatePullRequestInput {
                workspace: "my-workspace".to_string(),
                repo_slug: "my-repo".to_string(),
                title: "My New PR".to_string(),
                description: Some("Description".to_string()),
                source_branch: "feature".to_string(),
                destination_branch: "main".to_string(),
                reviewers: vec![],
                close_source_branch: Some(false),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.pull_request_id, 123);
        assert_eq!(output.title, "My New PR");
        assert_eq!(output.state, PullRequestState::Open);
    }

    #[tokio::test]
    async fn test_add_comment_success() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "id": 456,
          "content": {
            "raw": "Great work!",
            "markup": "markdown"
          },
          "user": {
            "display_name": "Bob",
            "uuid": "{bob-uuid}"
          },
          "created_on": "2024-01-01T12:00:00Z",
          "updated_on": "2024-01-01T12:00:00Z"
        }
        "#;

        Mock::given(method("POST"))
            .and(path(
                "/repositories/my-workspace/my-repo/pullrequests/1/comments",
            ))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = add_comment(
            ctx,
            AddCommentInput {
                workspace: "my-workspace".to_string(),
                repo_slug: "my-repo".to_string(),
                pull_request_id: 1,
                comment: "Great work!".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.comment_id, 456);
        assert_eq!(output.created_on, "2024-01-01T12:00:00Z");
    }

    #[tokio::test]
    async fn test_approve_pull_request_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path(
                "/repositories/my-workspace/my-repo/pullrequests/1/approve",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_raw(r"{}", "application/json"))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = approve_pull_request(
            ctx,
            ApprovePullRequestInput {
                workspace: "my-workspace".to_string(),
                repo_slug: "my-repo".to_string(),
                pull_request_id: 1,
            },
        )
        .await
        .unwrap();

        assert!(output.approved);
    }

    #[tokio::test]
    async fn test_unapprove_pull_request_success() {
        let server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path(
                "/repositories/my-workspace/my-repo/pullrequests/1/approve",
            ))
            .respond_with(ResponseTemplate::new(204).set_body_raw("", "application/json"))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = unapprove_pull_request(
            ctx,
            UnapprovePullRequestInput {
                workspace: "my-workspace".to_string(),
                repo_slug: "my-repo".to_string(),
                pull_request_id: 1,
            },
        )
        .await
        .unwrap();

        assert!(output.unapproved);
    }

    #[tokio::test]
    async fn test_merge_pull_request_success() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "id": 1,
          "title": "Merged PR",
          "state": "MERGED",
          "author": {
            "display_name": "Alice",
            "uuid": "{alice-uuid}"
          },
          "source": {
            "branch": { "name": "feature" },
            "repository": {
              "uuid": "{repo-uuid}",
              "name": "my-repo",
              "full_name": "workspace/my-repo"
            }
          },
          "destination": {
            "branch": { "name": "main" },
            "repository": {
              "uuid": "{repo-uuid}",
              "name": "my-repo",
              "full_name": "workspace/my-repo"
            }
          },
          "created_on": "2024-01-01T00:00:00Z",
          "updated_on": "2024-01-02T00:00:00Z"
        }
        "#;

        Mock::given(method("POST"))
            .and(path(
                "/repositories/my-workspace/my-repo/pullrequests/1/merge",
            ))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = merge_pull_request(
            ctx,
            MergePullRequestInput {
                workspace: "my-workspace".to_string(),
                repo_slug: "my-repo".to_string(),
                pull_request_id: 1,
                message: Some("Merge commit".to_string()),
                close_source_branch: Some(true),
                merge_strategy: None,
            },
        )
        .await
        .unwrap();

        assert!(output.merged);
        assert_eq!(output.pull_request_id, 1);
    }

    #[tokio::test]
    async fn test_decline_pull_request_success() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "id": 1,
          "title": "Declined PR",
          "state": "DECLINED",
          "author": {
            "display_name": "Alice",
            "uuid": "{alice-uuid}"
          },
          "source": {
            "branch": { "name": "feature" },
            "repository": {
              "uuid": "{repo-uuid}",
              "name": "my-repo",
              "full_name": "workspace/my-repo"
            }
          },
          "destination": {
            "branch": { "name": "main" },
            "repository": {
              "uuid": "{repo-uuid}",
              "name": "my-repo",
              "full_name": "workspace/my-repo"
            }
          },
          "created_on": "2024-01-01T00:00:00Z",
          "updated_on": "2024-01-02T00:00:00Z"
        }
        "#;

        Mock::given(method("POST"))
            .and(path(
                "/repositories/my-workspace/my-repo/pullrequests/1/decline",
            ))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = decline_pull_request(
            ctx,
            DeclinePullRequestInput {
                workspace: "my-workspace".to_string(),
                repo_slug: "my-repo".to_string(),
                pull_request_id: 1,
            },
        )
        .await
        .unwrap();

        assert!(output.declined);
        assert_eq!(output.pull_request_id, 1);
    }

    #[tokio::test]
    async fn test_bitbucket_api_error_returns_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repositories/my-workspace/my-repo/pullrequests"))
            .respond_with(ResponseTemplate::new(401).set_body_raw(
                r#"{"error": {"message": "Unauthorized"}}"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let result = search_pull_requests(
            ctx,
            SearchPullRequestsInput {
                workspace: "my-workspace".to_string(),
                repo_slug: "my-repo".to_string(),
                state: None,
                limit: Some(10),
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("401"));
    }
}
