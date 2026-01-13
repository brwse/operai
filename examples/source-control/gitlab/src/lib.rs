//! source-control/gitlab integration for Operai Toolbox.

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

mod types;
use types::{
    CreateIssueRequest, CreateMergeRequestRequest, CreateNoteRequest, IssueSummary,
    MergeRequestSummary, Note, UpdateIssueRequest, UpdateMergeRequestRequest,
};

define_user_credential! {
    GitLabCredential("gitlab") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_GITLAB_ENDPOINT: &str = "https://gitlab.com/api/v4";

#[init]
async fn setup() -> Result<()> {
    info!("GitLab integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("GitLab integration shutting down");
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchMergeRequestsInput {
    /// Project ID or namespace/project-name
    pub project: String,
    /// Search query string (optional, searches in title and description)
    #[serde(default)]
    pub search: Option<String>,
    /// Filter by state: opened, closed, locked, merged
    #[serde(default)]
    pub state: Option<String>,
    /// Maximum number of results (1-100). Defaults to 20.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchMergeRequestsOutput {
    pub merge_requests: Vec<MergeRequestSummary>,
}

/// # Search GitLab Merge Requests
///
/// Searches for merge requests in a GitLab project using the GitLab API.
///
/// Use this tool when you need to find, list, or query merge requests within a
/// specific GitLab project. Supports filtering by state (opened, closed,
/// locked, merged) and full-text search across merge request titles and
/// descriptions.
///
/// ## When to use
/// - Finding merge requests by state (e.g., all open MRs, recently merged MRs)
/// - Searching for merge requests containing specific keywords in
///   title/description
/// - Browsing merge requests with pagination control
/// - Retrieving merge request metadata (author, branches, timestamps, URLs)
///
/// ## Key inputs
/// - `project`: Project identifier (ID or namespace/project-name like
///   "mygroup/myproject")
/// - `state`: Optional filter - one of "opened", "closed", "locked", "merged"
/// - `search`: Optional search string for full-text search in title and
///   description
/// - `limit`: Result count (1-100, defaults to 20)
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - gitlab
/// - source-control
/// - merge-request
///
/// # Errors
///
/// Returns an error if:
/// - The project string is empty or contains only whitespace
/// - The limit is not between 1 and 100
/// - No GitLab credentials are configured or the access token is empty
/// - The GitLab API request fails (network error, authentication failure, etc.)
/// - The response body cannot be parsed as merge request data
#[tool]
pub async fn search_merge_requests(
    ctx: Context,
    input: SearchMergeRequestsInput,
) -> Result<SearchMergeRequestsOutput> {
    ensure!(
        !input.project.trim().is_empty(),
        "project must not be empty"
    );
    let limit = input.limit.unwrap_or(20);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let client = GitLabClient::from_ctx(&ctx)?;
    let project_encoded = urlencoding::encode(&input.project);

    let mut query = vec![("per_page", limit.to_string())];
    if let Some(state) = &input.state {
        let valid_states = ["opened", "closed", "locked", "merged"];
        ensure!(
            valid_states.contains(&state.as_str()),
            "state must be one of: opened, closed, locked, merged"
        );
        query.push(("state", state.clone()));
    }
    if let Some(search) = input.search {
        query.push(("search", search));
    }

    let merge_requests: Vec<MergeRequestSummary> = client
        .get_json(
            client.url_with_path(&format!("projects/{project_encoded}/merge_requests"))?,
            &query,
        )
        .await?;

    Ok(SearchMergeRequestsOutput { merge_requests })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchIssuesInput {
    /// Project ID or namespace/project-name
    pub project: String,
    /// Search query string (optional, searches in title and description)
    #[serde(default)]
    pub search: Option<String>,
    /// Filter by state: opened, closed
    #[serde(default)]
    pub state: Option<String>,
    /// Maximum number of results (1-100). Defaults to 20.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchIssuesOutput {
    pub issues: Vec<IssueSummary>,
}

/// # Search GitLab Issues
///
/// Searches for issues in a GitLab project using the GitLab API.
///
/// Use this tool when you need to find, list, or query issues within a specific
/// GitLab project. Supports filtering by state (opened, closed) and full-text
/// search across issue titles and descriptions.
///
/// ## When to use
/// - Finding issues by state (e.g., all open issues, recently closed issues)
/// - Searching for issues containing specific keywords in title/description
/// - Browsing issues with pagination control
/// - Retrieving issue metadata (author, assignees, labels, timestamps, URLs)
///
/// ## Key inputs
/// - `project`: Project identifier (ID or namespace/project-name like
///   "mygroup/myproject")
/// - `state`: Optional filter - one of "opened", "closed"
/// - `search`: Optional search string for full-text search in title and
///   description
/// - `limit`: Result count (1-100, defaults to 20)
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - gitlab
/// - source-control
/// - issue
///
/// # Errors
///
/// Returns an error if:
/// - The project string is empty or contains only whitespace
/// - The limit is not between 1 and 100
/// - No GitLab credentials are configured or the access token is empty
/// - The GitLab API request fails (network error, authentication failure, etc.)
/// - The response body cannot be parsed as issue data
#[tool]
pub async fn search_issues(ctx: Context, input: SearchIssuesInput) -> Result<SearchIssuesOutput> {
    ensure!(
        !input.project.trim().is_empty(),
        "project must not be empty"
    );
    let limit = input.limit.unwrap_or(20);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let client = GitLabClient::from_ctx(&ctx)?;
    let project_encoded = urlencoding::encode(&input.project);

    let mut query = vec![("per_page", limit.to_string())];
    if let Some(state) = &input.state {
        let valid_states = ["opened", "closed"];
        ensure!(
            valid_states.contains(&state.as_str()),
            "state must be one of: opened, closed"
        );
        query.push(("state", state.clone()));
    }
    if let Some(search) = input.search {
        query.push(("search", search));
    }

    let issues: Vec<IssueSummary> = client
        .get_json(
            client.url_with_path(&format!("projects/{project_encoded}/issues"))?,
            &query,
        )
        .await?;

    Ok(SearchIssuesOutput { issues })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateIssueInput {
    /// Project ID or namespace/project-name
    pub project: String,
    /// Issue title
    pub title: String,
    /// Issue description/body
    #[serde(default)]
    pub description: Option<String>,
    /// Assignee user IDs
    #[serde(default)]
    pub assignee_ids: Vec<u64>,
    /// Comma-separated label names
    #[serde(default)]
    pub labels: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateIssueOutput {
    pub issue: IssueSummary,
}

/// # Create GitLab Issue
///
/// Creates a new issue in a GitLab project using the GitLab API.
///
/// Use this tool when a user wants to create a new issue, bug report, feature
/// request, or task in a GitLab project. The issue will be created in the
/// "opened" state.
///
/// ## When to use
/// - Creating bug reports or feature requests
/// - Adding new tasks to a project's issue tracker
/// - Reporting problems or requesting improvements
/// - Any scenario where a new issue needs to be filed
///
/// ## Key inputs
/// - `project`: Project identifier (ID or namespace/project-name like
///   "mygroup/myproject")
/// - `title`: Issue title (required, must not be empty)
/// - `description`: Issue body/description (optional)
/// - `assignee_ids`: Array of user IDs to assign (optional)
/// - `labels`: Comma-separated label names (optional)
///
/// ## Returns
/// The created issue with all GitLab-generated fields including:
/// - Internal ID (iid) for referencing the issue
/// - Issue URL for direct access
/// - Creation timestamp
/// - Author information
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - gitlab
/// - source-control
/// - issue
///
/// # Errors
///
/// Returns an error if:
/// - The project string is empty or contains only whitespace
/// - The title string is empty or contains only whitespace
/// - No GitLab credentials are configured or the access token is empty
/// - The GitLab API request fails (network error, authentication failure, etc.)
/// - The response body cannot be parsed as issue data
#[tool]
pub async fn create_issue(ctx: Context, input: CreateIssueInput) -> Result<CreateIssueOutput> {
    ensure!(
        !input.project.trim().is_empty(),
        "project must not be empty"
    );
    ensure!(!input.title.trim().is_empty(), "title must not be empty");

    let client = GitLabClient::from_ctx(&ctx)?;
    let project_encoded = urlencoding::encode(&input.project);

    let request = CreateIssueRequest {
        title: input.title,
        description: input.description,
        assignee_ids: if input.assignee_ids.is_empty() {
            None
        } else {
            Some(input.assignee_ids)
        },
        labels: input.labels,
    };

    let issue: IssueSummary = client
        .post_json(
            client.url_with_path(&format!("projects/{project_encoded}/issues"))?,
            &request,
        )
        .await?;

    Ok(CreateIssueOutput { issue })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CommentInput {
    /// Project ID or namespace/project-name
    pub project: String,
    /// Issue or MR IID (internal ID)
    pub iid: u64,
    /// Type of resource: "issue" or "`merge_request`"
    pub resource_type: String,
    /// Comment body/text
    pub body: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CommentOutput {
    pub note: Note,
}

/// # Add GitLab Comment
///
/// Adds a comment to a GitLab issue or merge request using the GitLab API.
///
/// Use this tool when a user wants to add a comment, note, or reply to an
/// existing issue or merge request. Comments appear in the discussion thread
/// and can include any text content (questions, feedback, code snippets, etc.).
///
/// ## When to use
/// - Adding comments to issues for discussion or clarification
/// - Providing feedback on merge requests
/// - Asking questions about issues or MRs
/// - Adding notes to track progress or decisions
/// - Any scenario requiring commentary on existing resources
///
/// ## Key inputs
/// - `project`: Project identifier (ID or namespace/project-name like
///   "mygroup/myproject")
/// - `iid`: Internal ID of the issue or merge request (the number shown in
///   URLs)
/// - `resource_type`: Type of resource - must be "issue" or "`merge_request`"
/// - `body`: Comment text/body (required, must not be empty)
///
/// ## Returns
/// The created note with:
/// - Note ID and timestamp
/// - Author information
/// - Comment body text
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - gitlab
/// - source-control
/// - comment
///
/// # Errors
///
/// Returns an error if:
/// - The project string is empty or contains only whitespace
/// - The body string is empty or contains only whitespace
/// - The `resource_type` is not `"issue"` or `"merge_request"`
/// - No GitLab credentials are configured or the access token is empty
/// - The GitLab API request fails (network error, authentication failure, etc.)
/// - The response body cannot be parsed as note data
#[tool]
pub async fn comment(ctx: Context, input: CommentInput) -> Result<CommentOutput> {
    ensure!(
        !input.project.trim().is_empty(),
        "project must not be empty"
    );
    ensure!(!input.body.trim().is_empty(), "body must not be empty");
    ensure!(
        input.resource_type == "issue" || input.resource_type == "merge_request",
        "resource_type must be 'issue' or 'merge_request'"
    );

    let client = GitLabClient::from_ctx(&ctx)?;
    let project_encoded = urlencoding::encode(&input.project);
    let resource_path = if input.resource_type == "issue" {
        format!("projects/{}/issues/{}/notes", project_encoded, input.iid)
    } else {
        format!(
            "projects/{}/merge_requests/{}/notes",
            project_encoded, input.iid
        )
    };

    let request = CreateNoteRequest { body: input.body };

    let note: Note = client
        .post_json(client.url_with_path(&resource_path)?, &request)
        .await?;

    Ok(CommentOutput { note })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OpenMergeRequestInput {
    /// Project ID or namespace/project-name
    pub project: String,
    /// Source branch name
    pub source_branch: String,
    /// Target branch name
    pub target_branch: String,
    /// MR title
    pub title: String,
    /// MR description/body
    #[serde(default)]
    pub description: Option<String>,
    /// Assignee user IDs
    #[serde(default)]
    pub assignee_ids: Vec<u64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct OpenMergeRequestOutput {
    pub merge_request: MergeRequestSummary,
}

/// # Open GitLab Merge Request
///
/// Creates a new merge request in a GitLab project using the GitLab API.
///
/// Use this tool when a user wants to create a merge request (MR) to propose
/// changes from a source branch to a target branch. Merge requests are the
/// primary mechanism for code review and collaboration in GitLab.
///
/// ## When to use
/// - Creating a merge request for code review
/// - Proposing feature branches for merging
/// - Submitting bug fixes or changes for review
/// - Initiating the code review process
///
/// ## Key inputs
/// - `project`: Project identifier (ID or namespace/project-name like
///   "mygroup/myproject")
/// - `source_branch`: Name of the branch containing changes (required)
/// - `target_branch`: Name of the branch to merge into (e.g., "main", "master")
///   (required)
/// - `title`: Merge request title (required, must not be empty)
/// - `description`: MR description/body (optional, can include markdown,
///   images, etc.)
/// - `assignee_ids`: Array of user IDs to assign as reviewers (optional)
///
/// ## Returns
/// The created merge request with:
/// - Internal ID (iid) and web URL
/// - Source and target branch names
/// - Author and assignee information
/// - Creation and update timestamps
/// - Merge status (e.g., "`can_be_merged`", "`cannot_be_merged`")
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - gitlab
/// - source-control
/// - merge-request
///
/// # Errors
///
/// Returns an error if:
/// - The project string is empty or contains only whitespace
/// - The `source_branch` string is empty or contains only whitespace
/// - The `target_branch` string is empty or contains only whitespace
/// - The title string is empty or contains only whitespace
/// - No GitLab credentials are configured or the access token is empty
/// - The GitLab API request fails (network error, authentication failure, etc.)
/// - The response body cannot be parsed as merge request data
#[tool]
pub async fn open_merge_request(
    ctx: Context,
    input: OpenMergeRequestInput,
) -> Result<OpenMergeRequestOutput> {
    ensure!(
        !input.project.trim().is_empty(),
        "project must not be empty"
    );
    ensure!(
        !input.source_branch.trim().is_empty(),
        "source_branch must not be empty"
    );
    ensure!(
        !input.target_branch.trim().is_empty(),
        "target_branch must not be empty"
    );
    ensure!(!input.title.trim().is_empty(), "title must not be empty");

    let client = GitLabClient::from_ctx(&ctx)?;
    let project_encoded = urlencoding::encode(&input.project);

    let request = CreateMergeRequestRequest {
        source_branch: input.source_branch,
        target_branch: input.target_branch,
        title: input.title,
        description: input.description,
        assignee_ids: if input.assignee_ids.is_empty() {
            None
        } else {
            Some(input.assignee_ids)
        },
    };

    let merge_request: MergeRequestSummary = client
        .post_json(
            client.url_with_path(&format!("projects/{project_encoded}/merge_requests"))?,
            &request,
        )
        .await?;

    Ok(OpenMergeRequestOutput { merge_request })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ApproveMergeRequestInput {
    /// Project ID or namespace/project-name
    pub project: String,
    /// MR IID (internal ID)
    pub iid: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ApproveMergeRequestOutput {
    pub approved: bool,
}

/// # Approve GitLab Merge Request
///
/// Approves a merge request in GitLab using the GitLab API.
///
/// Use this tool when a user wants to approve a merge request as part of the
/// code review process. Approval is typically required before a merge request
/// can be merged, depending on project settings.
///
/// ## When to use
/// - Approving a merge request after successful code review
/// - Signaling approval for proposed changes
/// - Moving a merge request closer to being mergeable
/// - Completing the review process with an approval
///
/// ## Key inputs
/// - `project`: Project identifier (ID or namespace/project-name like
///   "mygroup/myproject")
/// - `iid`: Internal ID of the merge request (the number shown in URLs)
///
/// ## Returns
/// Confirmation that the merge request was approved successfully.
///
/// ## Notes
/// - The user must have appropriate permissions to approve the MR
/// - Some projects may require approvals from multiple users
/// - Approval rules vary by project configuration
/// - An MR can be approved multiple times by different reviewers
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - gitlab
/// - source-control
/// - merge-request
///
/// # Errors
///
/// Returns an error if:
/// - The project string is empty or contains only whitespace
/// - No GitLab credentials are configured or the access token is empty
/// - The GitLab API request fails (network error, authentication failure,
///   insufficient permissions, etc.)
#[tool]
pub async fn approve_merge_request(
    ctx: Context,
    input: ApproveMergeRequestInput,
) -> Result<ApproveMergeRequestOutput> {
    ensure!(
        !input.project.trim().is_empty(),
        "project must not be empty"
    );

    let client = GitLabClient::from_ctx(&ctx)?;
    let project_encoded = urlencoding::encode(&input.project);

    client
        .post_empty(
            client.url_with_path(&format!(
                "projects/{}/merge_requests/{}/approve",
                project_encoded, input.iid
            ))?,
            &serde_json::json!({}),
        )
        .await?;

    Ok(ApproveMergeRequestOutput { approved: true })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MergeRequestInput {
    /// Project ID or namespace/project-name
    pub project: String,
    /// MR IID (internal ID)
    pub iid: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MergeRequestOutput {
    pub merged: bool,
}

/// # Merge GitLab Merge Request
///
/// Merges a merge request in GitLab using the GitLab API.
///
/// Use this tool when a user wants to merge an approved merge request,
/// integrating the source branch into the target branch. This is the final step
/// in the merge request workflow.
///
/// ## When to use
/// - Merging an approved merge request after successful review
/// - Integrating feature branches into the main branch
/// - Completing the merge request lifecycle
/// - Deploying changes via merge
///
/// ## Key inputs
/// - `project`: Project identifier (ID or namespace/project-name like
///   "mygroup/myproject")
/// - `iid`: Internal ID of the merge request (the number shown in URLs)
///
/// ## Returns
/// Confirmation that the merge request was merged successfully.
///
/// ## Prerequisites
/// - The merge request must typically be approved first (depending on project
///   settings)
/// - The user must have appropriate permissions to merge
/// - There should be no merge conflicts
/// - All CI/CD pipelines should typically be passing
///
/// ## Notes
/// - This action cannot be undone
/// - The source branch may be deleted after merge (depending on project
///   settings)
/// - GitLab will perform the actual merge using the project's configured
///   strategy
/// - The MR will be moved to the "merged" state
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - gitlab
/// - source-control
/// - merge-request
///
/// # Errors
///
/// Returns an error if:
/// - The project string is empty or contains only whitespace
/// - No GitLab credentials are configured or the access token is empty
/// - The GitLab API request fails (network error, authentication failure,
///   insufficient permissions, merge conflicts, etc.)
#[tool]
pub async fn merge_request(ctx: Context, input: MergeRequestInput) -> Result<MergeRequestOutput> {
    ensure!(
        !input.project.trim().is_empty(),
        "project must not be empty"
    );

    let client = GitLabClient::from_ctx(&ctx)?;
    let project_encoded = urlencoding::encode(&input.project);

    client
        .put_empty(
            client.url_with_path(&format!(
                "projects/{}/merge_requests/{}/merge",
                project_encoded, input.iid
            ))?,
            &serde_json::json!({}),
        )
        .await?;

    Ok(MergeRequestOutput { merged: true })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CloseMergeRequestInput {
    /// Project ID or namespace/project-name
    pub project: String,
    /// MR IID (internal ID)
    pub iid: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CloseMergeRequestOutput {
    pub closed: bool,
}

/// # Close GitLab Merge Request
///
/// Closes a merge request in GitLab without merging it using the GitLab API.
///
/// Use this tool when a user wants to close/abandon a merge request without
/// merging it. This is appropriate when the changes are no longer needed, the
/// approach has changed, or the merge request should be discarded for any
/// reason.
///
/// ## When to use
/// - Abandoning a merge request that is no longer needed
/// - Closing an MR that was created in error
/// - Discarding proposed changes that won't be merged
/// - Rejecting a merge request without merging
///
/// ## Key inputs
/// - `project`: Project identifier (ID or namespace/project-name like
///   "mygroup/myproject")
/// - `iid`: Internal ID of the merge request (the number shown in URLs)
///
/// ## Returns
/// Confirmation that the merge request was closed successfully.
///
/// ## Notes
/// - This action changes the MR state to "closed"
/// - Unlike merging, this does not integrate any code
/// - The MR remains visible in the project history
/// - A closed MR can typically be reopened if needed
/// - The source branch is not automatically deleted
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - gitlab
/// - source-control
/// - merge-request
///
/// # Errors
///
/// Returns an error if:
/// - The project string is empty or contains only whitespace
/// - No GitLab credentials are configured or the access token is empty
/// - The GitLab API request fails (network error, authentication failure,
///   insufficient permissions, etc.)
#[tool]
pub async fn close_merge_request(
    ctx: Context,
    input: CloseMergeRequestInput,
) -> Result<CloseMergeRequestOutput> {
    ensure!(
        !input.project.trim().is_empty(),
        "project must not be empty"
    );

    let client = GitLabClient::from_ctx(&ctx)?;
    let project_encoded = urlencoding::encode(&input.project);

    let request = UpdateMergeRequestRequest {
        state_event: "close".to_string(),
    };

    client
        .put_empty(
            client.url_with_path(&format!(
                "projects/{}/merge_requests/{}",
                project_encoded, input.iid
            ))?,
            &request,
        )
        .await?;

    Ok(CloseMergeRequestOutput { closed: true })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CloseIssueInput {
    /// Project ID or namespace/project-name
    pub project: String,
    /// Issue IID (internal ID)
    pub iid: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CloseIssueOutput {
    pub closed: bool,
}

/// # Close GitLab Issue
///
/// Closes an issue in GitLab using the GitLab API.
///
/// Use this tool when a user wants to close an issue that has been resolved,
/// fixed, or is no longer relevant. Closing an issue marks it as complete and
/// removes it from active issue queues.
///
/// ## When to use
/// - Closing an issue after the fix has been implemented
/// - Marking a task or feature request as completed
/// - Closing issues that are no longer relevant or were created in error
/// - Finalizing the issue workflow after resolution
///
/// ## Key inputs
/// - `project`: Project identifier (ID or namespace/project-name like
///   "mygroup/myproject")
/// - `iid`: Internal ID of the issue (the number shown in URLs)
///
/// ## Returns
/// Confirmation that the issue was closed successfully.
///
/// ## Notes
/// - This action changes the issue state to "closed"
/// - The issue remains visible in the project history
/// - A closed issue can typically be reopened if needed
/// - Consider adding a comment explaining the resolution before closing
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - gitlab
/// - source-control
/// - issue
///
/// # Errors
///
/// Returns an error if:
/// - The project string is empty or contains only whitespace
/// - No GitLab credentials are configured or the access token is empty
/// - The GitLab API request fails (network error, authentication failure,
///   insufficient permissions, etc.)
#[tool]
pub async fn close_issue(ctx: Context, input: CloseIssueInput) -> Result<CloseIssueOutput> {
    ensure!(
        !input.project.trim().is_empty(),
        "project must not be empty"
    );

    let client = GitLabClient::from_ctx(&ctx)?;
    let project_encoded = urlencoding::encode(&input.project);

    let request = UpdateIssueRequest {
        state_event: "close".to_string(),
    };

    client
        .put_empty(
            client.url_with_path(&format!(
                "projects/{}/issues/{}",
                project_encoded, input.iid
            ))?,
            &request,
        )
        .await?;

    Ok(CloseIssueOutput { closed: true })
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

    async fn post_empty<TReq: Serialize>(&self, url: reqwest::Url, body: &TReq) -> Result<()> {
        let request = self.http.post(url).json(body);
        self.send_request(request).await?;
        Ok(())
    }

    async fn put_empty<TReq: Serialize>(&self, url: reqwest::Url, body: &TReq) -> Result<()> {
        let request = self.http.put(url).json(body);
        self.send_request(request).await?;
        Ok(())
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

            // Try to parse GitLab error message for better error reporting
            let error_msg = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                // GitLab often returns { "message": "error description" } or { "error":
                // "description" }
                let msg = json
                    .get("message")
                    .or(json.get("error"))
                    .and_then(|v| v.as_str());
                if let Some(m) = msg {
                    m.to_string()
                } else {
                    body.clone()
                }
            } else {
                body.clone()
            };

            // Provide user-friendly error messages for common status codes
            let user_msg = match status.as_u16() {
                401 => "Authentication failed. Check your GitLab access token.",
                403 => {
                    "Permission denied. Your access token may not have the required permissions."
                }
                404 => "Resource not found. Check the project path and IDs.",
                422 => "Validation failed. The request parameters may be invalid.",
                _ => "",
            };

            Err(operai::anyhow::anyhow!(
                "GitLab API request failed ({status}): {}{}",
                error_msg,
                if user_msg.is_empty() {
                    String::new()
                } else {
                    format!(" ({user_msg})")
                }
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
        matchers::{body_string_contains, header, method, path, query_param},
    };

    use super::*;
    use crate::types::{IssueState, MergeRequestState};

    fn test_ctx(endpoint: &str) -> Context {
        let mut gitlab_values = HashMap::new();
        gitlab_values.insert("access_token".to_string(), "test-token".to_string());
        gitlab_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("gitlab", gitlab_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        format!("{}/api/v4", server.uri())
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_merge_request_state_serialization_roundtrip() {
        for variant in [
            MergeRequestState::Opened,
            MergeRequestState::Closed,
            MergeRequestState::Locked,
            MergeRequestState::Merged,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: MergeRequestState = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{variant:?}"), format!("{parsed:?}"));
        }
    }

    #[test]
    fn test_issue_state_serialization_roundtrip() {
        for variant in [IssueState::Opened, IssueState::Closed] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: IssueState = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{variant:?}"), format!("{parsed:?}"));
        }
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://gitlab.com/api/v4/").unwrap();
        assert_eq!(result, "https://gitlab.com/api/v4");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://gitlab.com/api/v4  ").unwrap();
        assert_eq!(result, "https://gitlab.com/api/v4");
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
    async fn test_search_merge_requests_empty_project_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = search_merge_requests(
            ctx,
            SearchMergeRequestsInput {
                project: "   ".to_string(),
                search: None,
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
                .contains("project must not be empty")
        );
    }

    #[tokio::test]
    async fn test_search_merge_requests_limit_exceeds_max_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = search_merge_requests(
            ctx,
            SearchMergeRequestsInput {
                project: "myorg/myproject".to_string(),
                search: None,
                state: None,
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
    async fn test_create_issue_empty_title_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = create_issue(
            ctx,
            CreateIssueInput {
                project: "myorg/myproject".to_string(),
                title: "  ".to_string(),
                description: None,
                assignee_ids: vec![],
                labels: None,
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
    async fn test_comment_invalid_resource_type_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = comment(
            ctx,
            CommentInput {
                project: "myorg/myproject".to_string(),
                iid: 1,
                resource_type: "invalid".to_string(),
                body: "comment".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("resource_type must be")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_search_merge_requests_success_returns_mrs() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        [
          {
            "id": 100,
            "iid": 1,
            "project_id": 10,
            "title": "Add feature X",
            "description": "This adds feature X",
            "state": "opened",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-02T00:00:00Z",
            "merged_at": null,
            "closed_at": null,
            "author": {
              "id": 1,
              "username": "alice",
              "name": "Alice",
              "avatar_url": null
            },
            "source_branch": "feature-x",
            "target_branch": "main",
            "web_url": "https://gitlab.com/myorg/myproject/-/merge_requests/1"
          }
        ]
        "#;

        Mock::given(method("GET"))
            .and(path("/api/v4/projects/myorg%2Fmyproject/merge_requests"))
            .and(header("PRIVATE-TOKEN", "test-token"))
            .and(query_param("per_page", "20"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = search_merge_requests(
            ctx,
            SearchMergeRequestsInput {
                project: "myorg/myproject".to_string(),
                search: None,
                state: None,
                limit: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.merge_requests.len(), 1);
        assert_eq!(output.merge_requests[0].title, "Add feature X");
    }

    #[tokio::test]
    async fn test_create_issue_success_returns_issue() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": 200,
          "iid": 1,
          "project_id": 10,
          "title": "Bug report",
          "description": "There is a bug",
          "state": "opened",
          "created_at": "2024-01-01T00:00:00Z",
          "updated_at": "2024-01-01T00:00:00Z",
          "closed_at": null,
          "author": {
            "id": 1,
            "username": "alice",
            "name": "Alice",
            "avatar_url": null
          },
          "assignees": [],
          "labels": [],
          "web_url": "https://gitlab.com/myorg/myproject/-/issues/1"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/api/v4/projects/myorg%2Fmyproject/issues"))
            .and(header("PRIVATE-TOKEN", "test-token"))
            .and(body_string_contains("\"title\":\"Bug report\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = create_issue(
            ctx,
            CreateIssueInput {
                project: "myorg/myproject".to_string(),
                title: "Bug report".to_string(),
                description: Some("There is a bug".to_string()),
                assignee_ids: vec![],
                labels: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.issue.title, "Bug report");
        assert_eq!(output.issue.iid, 1);
    }

    #[tokio::test]
    async fn test_comment_on_issue_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": 300,
          "body": "This is a comment",
          "author": {
            "id": 1,
            "username": "alice",
            "name": "Alice",
            "avatar_url": null
          },
          "created_at": "2024-01-01T00:00:00Z",
          "updated_at": "2024-01-01T00:00:00Z"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/api/v4/projects/myorg%2Fmyproject/issues/1/notes"))
            .and(body_string_contains("\"body\":\"This is a comment\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = comment(
            ctx,
            CommentInput {
                project: "myorg/myproject".to_string(),
                iid: 1,
                resource_type: "issue".to_string(),
                body: "This is a comment".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.note.body, "This is a comment");
    }

    #[tokio::test]
    async fn test_approve_merge_request_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("POST"))
            .and(path(
                "/api/v4/projects/myorg%2Fmyproject/merge_requests/1/approve",
            ))
            .respond_with(ResponseTemplate::new(201).set_body_raw("{}", "application/json"))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = approve_merge_request(
            ctx,
            ApproveMergeRequestInput {
                project: "myorg/myproject".to_string(),
                iid: 1,
            },
        )
        .await
        .unwrap();

        assert!(output.approved);
    }

    #[tokio::test]
    async fn test_merge_request_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("PUT"))
            .and(path(
                "/api/v4/projects/myorg%2Fmyproject/merge_requests/1/merge",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_raw("{}", "application/json"))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = merge_request(
            ctx,
            MergeRequestInput {
                project: "myorg/myproject".to_string(),
                iid: 1,
            },
        )
        .await
        .unwrap();

        assert!(output.merged);
    }

    #[tokio::test]
    async fn test_close_issue_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("PUT"))
            .and(path("/api/v4/projects/myorg%2Fmyproject/issues/1"))
            .and(body_string_contains("\"state_event\":\"close\""))
            .respond_with(ResponseTemplate::new(200).set_body_raw("{}", "application/json"))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = close_issue(
            ctx,
            CloseIssueInput {
                project: "myorg/myproject".to_string(),
                iid: 1,
            },
        )
        .await
        .unwrap();

        assert!(output.closed);
    }

    // --- Additional error scenario tests ---

    #[tokio::test]
    async fn test_search_merge_requests_invalid_state_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = search_merge_requests(
            ctx,
            SearchMergeRequestsInput {
                project: "myorg/myproject".to_string(),
                search: None,
                state: Some("invalid".to_string()),
                limit: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("state must be one of")
        );
    }

    #[tokio::test]
    async fn test_search_issues_invalid_state_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = search_issues(
            ctx,
            SearchIssuesInput {
                project: "myorg/myproject".to_string(),
                search: None,
                state: Some("merged".to_string()),
                limit: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("state must be one of")
        );
    }

    #[tokio::test]
    async fn test_search_merge_requests_with_valid_states_succeeds() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        [
          {
            "id": 100,
            "iid": 1,
            "project_id": 10,
            "title": "MR title",
            "description": null,
            "state": "opened",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z",
            "merged_at": null,
            "closed_at": null,
            "author": {
              "id": 1,
              "username": "alice",
              "name": "Alice",
              "avatar_url": null
            },
            "source_branch": "feature",
            "target_branch": "main",
            "web_url": "https://gitlab.com/myorg/myproject/-/merge_requests/1",
            "merge_status": "can_be_merged",
            "draft": false,
            "has_conflicts": false
          }
        ]
        "#;

        for state in ["opened", "closed", "locked", "merged"] {
            Mock::given(method("GET"))
                .and(path("/api/v4/projects/myorg%2Fmyproject/merge_requests"))
                .and(query_param("state", state))
                .respond_with(
                    ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
                )
                .mount(&server)
                .await;

            let ctx = test_ctx(&endpoint);
            let result = search_merge_requests(
                ctx,
                SearchMergeRequestsInput {
                    project: "myorg/myproject".to_string(),
                    search: None,
                    state: Some(state.to_string()),
                    limit: None,
                },
            )
            .await;

            assert!(result.is_ok(), "state {state} should be valid");
        }
    }

    #[tokio::test]
    async fn test_api_401_error_returns_friendly_message() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/api/v4/projects/myorg%2Fmyproject/merge_requests"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_raw(r#"{"message": "401 Unauthorized"}"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = search_merge_requests(
            ctx,
            SearchMergeRequestsInput {
                project: "myorg/myproject".to_string(),
                search: None,
                state: None,
                limit: None,
            },
        )
        .await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("401"));
        assert!(error_msg.contains("Authentication failed"));
    }

    #[tokio::test]
    async fn test_api_404_error_returns_friendly_message() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/api/v4/projects/myorg%2Fmyproject/merge_requests"))
            .respond_with(ResponseTemplate::new(404).set_body_raw(
                r#"{"message": "404 Project Not Found"}"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = search_merge_requests(
            ctx,
            SearchMergeRequestsInput {
                project: "myorg/myproject".to_string(),
                search: None,
                state: None,
                limit: None,
            },
        )
        .await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("404"));
        assert!(error_msg.contains("Resource not found"));
    }

    #[tokio::test]
    async fn test_search_merge_requests_empty_results() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/api/v4/projects/myorg%2Fmyproject/merge_requests"))
            .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = search_merge_requests(
            ctx,
            SearchMergeRequestsInput {
                project: "myorg/myproject".to_string(),
                search: Some("nonexistent".to_string()),
                state: None,
                limit: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.merge_requests.len(), 0);
    }

    #[tokio::test]
    async fn test_create_issue_with_new_fields() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": 200,
          "iid": 1,
          "project_id": 10,
          "title": "Bug report",
          "description": "There is a bug",
          "state": "opened",
          "created_at": "2024-01-01T00:00:00Z",
          "updated_at": "2024-01-01T00:00:00Z",
          "closed_at": null,
          "author": {
            "id": 1,
            "username": "alice",
            "name": "Alice",
            "avatar_url": null
          },
          "assignees": [],
          "labels": [],
          "web_url": "https://gitlab.com/myorg/myproject/-/issues/1",
          "confidential": false,
          "issue_type": "issue"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/api/v4/projects/myorg%2Fmyproject/issues"))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = create_issue(
            ctx,
            CreateIssueInput {
                project: "myorg/myproject".to_string(),
                title: "Bug report".to_string(),
                description: Some("There is a bug".to_string()),
                assignee_ids: vec![],
                labels: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.issue.title, "Bug report");
        assert_eq!(output.issue.confidential, Some(false));
        assert_eq!(output.issue.issue_type, Some("issue".to_string()));
    }

    #[tokio::test]
    async fn test_search_merge_requests_with_new_fields() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        [
          {
            "id": 100,
            "iid": 1,
            "project_id": 10,
            "title": "Add feature X",
            "description": "This adds feature X",
            "state": "opened",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-02T00:00:00Z",
            "merged_at": null,
            "closed_at": null,
            "author": {
              "id": 1,
              "username": "alice",
              "name": "Alice",
              "avatar_url": null
            },
            "source_branch": "feature-x",
            "target_branch": "main",
            "web_url": "https://gitlab.com/myorg/myproject/-/merge_requests/1",
            "merge_status": "can_be_merged",
            "draft": false,
            "has_conflicts": false
          }
        ]
        "#;

        Mock::given(method("GET"))
            .and(path("/api/v4/projects/myorg%2Fmyproject/merge_requests"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = search_merge_requests(
            ctx,
            SearchMergeRequestsInput {
                project: "myorg/myproject".to_string(),
                search: None,
                state: None,
                limit: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.merge_requests.len(), 1);
        assert_eq!(
            output.merge_requests[0].merge_status,
            Some("can_be_merged".to_string())
        );
        assert_eq!(output.merge_requests[0].draft, Some(false));
        assert_eq!(output.merge_requests[0].has_conflicts, Some(false));
    }
}
