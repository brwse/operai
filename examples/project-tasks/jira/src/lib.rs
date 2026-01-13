//! Jira integration for Operai Toolbox.
//!
//! This integration provides tools for interacting with Jira issues:
//! - Search issues using JQL
//! - Get issue details by key
//! - Create new issues
//! - Transition issue status
//! - Add comments to issues
use operai::{
    Context, JsonSchema, Result, define_system_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

mod types;
use types::{
    CreateIssueFields, Issue, IssueSummary, IssueTypeReference, PriorityReference,
    ProjectReference, SearchResponse, UserReference,
};

// Jira uses Basic Auth with email + API token
define_system_credential! {
    JiraCredential("jira") {
        /// Email address associated with the Jira account.
        username: String,
        /// Jira API token (from https://id.atlassian.com/manage/api-tokens).
        password: String,
        /// Jira instance base URL (e.g., "https://yourcompany.atlassian.net").
        #[optional]
        endpoint: Option<String>,
    }
}

#[init]
async fn setup() -> Result<()> {
    info!("Jira integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Jira integration shutting down");
}

// =============================================================================
// Tool 1: Search Issues
// =============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchIssuesInput {
    /// JQL query string (e.g., "project = PROJ AND status = 'In Progress'").
    pub jql: String,
    /// Maximum number of results (1-100). Defaults to 50.
    #[serde(default)]
    pub max_results: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchIssuesOutput {
    pub issues: Vec<IssueSummary>,
    pub total: u32,
}

/// # Search Jira Issues
///
/// Searches for Jira issues using JQL (Jira Query Language), a powerful query
/// syntax for filtering and finding issues in Jira. Use this tool when you need
/// to find issues matching specific criteria such as project, status, assignee,
/// priority, or any other Jira field.
///
/// Supports complex queries like "project = PROJ AND status = 'In Progress' AND
/// assignee = `currentUser()`" to find issues that match multiple conditions.
/// The results include issue summaries with key fields like status, issue type,
/// priority, assignee, and timestamps.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - jira
/// - issues
/// - search
///
/// # Errors
///
/// Returns an error if:
/// - The provided JQL query is empty or contains only whitespace
/// - The `max_results` parameter is 0
/// - Jira credentials are missing or invalid (username/password empty)
/// - The base URL is invalid
/// - The HTTP request to Jira API fails
/// - The Jira API returns a non-success status code
/// - The response JSON cannot be parsed
#[tool]
pub async fn search_issues(ctx: Context, input: SearchIssuesInput) -> Result<SearchIssuesOutput> {
    ensure!(!input.jql.trim().is_empty(), "jql must not be empty");
    let max_results = input.max_results.unwrap_or(50).min(100);
    ensure!(max_results > 0, "max_results must be greater than 0");

    let client = JiraClient::from_ctx(&ctx)?;
    let query = [
        ("jql", input.jql),
        ("maxResults", max_results.to_string()),
        (
            "fields",
            "summary,status,issuetype,priority,assignee,reporter,created,updated".to_string(),
        ),
    ];

    let response: SearchResponse = client
        .get_json(
            client.url_with_segments(&["rest", "api", "3", "search"])?,
            &query,
        )
        .await?;

    Ok(SearchIssuesOutput {
        issues: response.issues,
        total: response.total.unwrap_or(0).try_into().unwrap_or(u32::MAX),
    })
}

// =============================================================================
// Tool 2: Get Issue
// =============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetIssueInput {
    /// The issue key (e.g., "PROJ-123").
    pub issue_key: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetIssueOutput {
    pub issue: Issue,
}

/// # Get Jira Issue
///
/// Retrieves comprehensive information about a specific Jira issue by its issue
/// key (e.g., "PROJ-123"). Use this tool when you need detailed information
/// about a single issue, including its full description, comments, status
/// history, labels, and all associated metadata.
///
/// This is the tool to use when a user asks for details about a specific issue
/// they already know the key for. For finding issues based on criteria, use the
/// `search_issues` tool instead. The response includes the complete issue
/// object with description, status, issue type, priority, assignee, reporter,
/// timestamps, labels, and comments.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - jira
/// - issues
///
/// # Errors
///
/// Returns an error if:
/// - The provided issue key is empty or contains only whitespace
/// - Jira credentials are missing or invalid (username/password empty)
/// - The base URL is invalid
/// - The HTTP request to Jira API fails
/// - The Jira API returns a non-success status code
/// - The response JSON cannot be parsed
#[tool]
pub async fn get_issue(ctx: Context, input: GetIssueInput) -> Result<GetIssueOutput> {
    ensure!(
        !input.issue_key.trim().is_empty(),
        "issue_key must not be empty"
    );

    let client = JiraClient::from_ctx(&ctx)?;
    let query = [(
        "fields",
        "summary,description,status,issuetype,priority,assignee,reporter,created,updated,labels,\
         comment"
            .to_string(),
    )];

    let issue: Issue = client
        .get_json(
            client.url_with_segments(&["rest", "api", "3", "issue", input.issue_key.as_str()])?,
            &query,
        )
        .await?;

    Ok(GetIssueOutput { issue })
}

// =============================================================================
// Tool 3: Create Issue
// =============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateIssueInput {
    /// Project key (e.g., "PROJ").
    pub project_key: String,
    /// Issue summary/title.
    pub summary: String,
    /// Issue type name (e.g., "Task", "Bug", "Story").
    pub issue_type: String,
    /// Description text (plain text).
    #[serde(default)]
    pub description: Option<String>,
    /// Priority name (e.g., "High", "Medium", "Low").
    #[serde(default)]
    pub priority: Option<String>,
    /// Assignee account ID.
    #[serde(default)]
    pub assignee_account_id: Option<String>,
    /// Labels to attach.
    #[serde(default)]
    pub labels: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateIssueOutput {
    pub id: String,
    pub key: String,
}

#[derive(Debug, Serialize)]
struct CreateIssueRequest {
    fields: CreateIssueFields,
}

#[derive(Debug, Deserialize)]
struct CreateIssueResponse {
    id: String,
    key: String,
}

/// # Create Jira Issue
///
/// Creates a new issue in a Jira project with the specified project key,
/// summary, and issue type. Use this tool when a user wants to file a new bug,
/// task, story, or any other type of issue in Jira.
///
/// Requires at minimum a project key (e.g., "PROJ"), a summary/title, and an
/// issue type name (e.g., "Task", "Bug", "Story"). Optionally supports setting
/// a description, priority level, assigning to a specific user by account ID,
/// and attaching labels for categorization. The response returns the generated
/// issue ID and key for the newly created issue.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - jira
/// - issues
///
/// # Errors
///
/// Returns an error if:
/// - The provided project key, summary, or issue type is empty or contains only
///   whitespace
/// - Jira credentials are missing or invalid (username/password empty)
/// - The base URL is invalid
/// - The HTTP request to Jira API fails
/// - The Jira API returns a non-success status code (e.g., invalid project key,
///   issue type, etc.)
/// - The request or response JSON cannot be parsed
#[tool]
pub async fn create_issue(ctx: Context, input: CreateIssueInput) -> Result<CreateIssueOutput> {
    ensure!(
        !input.project_key.trim().is_empty(),
        "project_key must not be empty"
    );
    ensure!(
        !input.summary.trim().is_empty(),
        "summary must not be empty"
    );
    ensure!(
        !input.issue_type.trim().is_empty(),
        "issue_type must not be empty"
    );

    let client = JiraClient::from_ctx(&ctx)?;
    let request = CreateIssueRequest {
        fields: CreateIssueFields {
            project: ProjectReference {
                key: input.project_key,
            },
            summary: input.summary,
            issuetype: IssueTypeReference {
                name: input.issue_type,
            },
            description: input.description,
            priority: input.priority.map(|name| PriorityReference { name }),
            assignee: input
                .assignee_account_id
                .map(|account_id| UserReference { account_id }),
            labels: if input.labels.is_empty() {
                None
            } else {
                Some(input.labels)
            },
        },
    };

    let response: CreateIssueResponse = client
        .post_json(
            client.url_with_segments(&["rest", "api", "3", "issue"])?,
            &request,
        )
        .await?;

    Ok(CreateIssueOutput {
        id: response.id,
        key: response.key,
    })
}

// =============================================================================
// Tool 4: Transition Issue
// =============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TransitionIssueInput {
    /// Issue key (e.g., "PROJ-123").
    pub issue_key: String,
    /// Transition ID (use `get_transitions` to find available transitions).
    pub transition_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TransitionIssueOutput {
    pub success: bool,
}

#[derive(Debug, Serialize)]
struct TransitionRequest {
    transition: TransitionId,
}

#[derive(Debug, Serialize)]
struct TransitionId {
    id: String,
}

/// # Transition Jira Issue
///
/// Changes the status of a Jira issue by executing a workflow transition (e.g.,
/// moving an issue from "To Do" to "In Progress"). Use this tool when a user
/// wants to change the status or workflow state of an existing issue.
///
/// Requires the issue key (e.g., "PROJ-123") and a transition ID. Note that
/// transition IDs are numeric identifiers specific to each issue's current
/// state and available workflow transitions. To find valid transition IDs for
/// an issue, you would typically need to query the issue's transitions endpoint
/// first. This tool performs the actual state change and returns
/// success confirmation if the transition was valid.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - jira
/// - issues
/// - workflow
///
/// # Errors
///
/// Returns an error if:
/// - The provided issue key or transition ID is empty or contains only
///   whitespace
/// - Jira credentials are missing or invalid (username/password empty)
/// - The base URL is invalid
/// - The HTTP request to Jira API fails
/// - The Jira API returns a non-success status code (e.g., invalid transition
///   ID, issue not found)
/// - The request JSON cannot be parsed
#[tool]
pub async fn transition_issue(
    ctx: Context,
    input: TransitionIssueInput,
) -> Result<TransitionIssueOutput> {
    ensure!(
        !input.issue_key.trim().is_empty(),
        "issue_key must not be empty"
    );
    ensure!(
        !input.transition_id.trim().is_empty(),
        "transition_id must not be empty"
    );

    let client = JiraClient::from_ctx(&ctx)?;
    let request = TransitionRequest {
        transition: TransitionId {
            id: input.transition_id,
        },
    };

    client
        .post_empty(
            client.url_with_segments(&[
                "rest",
                "api",
                "3",
                "issue",
                input.issue_key.as_str(),
                "transitions",
            ])?,
            &request,
        )
        .await?;

    Ok(TransitionIssueOutput { success: true })
}

// =============================================================================
// Tool 5: Add Comment
// =============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddCommentInput {
    /// Issue key (e.g., "PROJ-123").
    pub issue_key: String,
    /// Comment body text.
    pub body: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AddCommentOutput {
    pub comment_id: String,
}

#[derive(Debug, Serialize)]
struct AddCommentRequest {
    body: String,
}

#[derive(Debug, Deserialize)]
struct AddCommentResponse {
    id: String,
}

/// # Add Jira Comment
///
/// Adds a text comment to an existing Jira issue. Use this tool when a user
/// wants to add a note, question, update, or any other textual comment to an
/// issue's comment thread.
///
/// Requires the issue key (e.g., "PROJ-123") and the comment body text. The
/// comment will be attributed to the authenticated user and will appear in the
/// issue's comment history with a timestamp. This is useful for
/// providing status updates, asking questions, documenting decisions, or
/// communicating with team members about a specific issue. The response returns
/// the ID of the newly created comment.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - jira
/// - issues
/// - comments
///
/// # Errors
///
/// Returns an error if:
/// - The provided issue key or comment body is empty or contains only
///   whitespace
/// - Jira credentials are missing or invalid (username/password empty)
/// - The base URL is invalid
/// - The HTTP request to Jira API fails
/// - The Jira API returns a non-success status code (e.g., issue not found)
/// - The request or response JSON cannot be parsed
#[tool]
pub async fn add_comment(ctx: Context, input: AddCommentInput) -> Result<AddCommentOutput> {
    ensure!(
        !input.issue_key.trim().is_empty(),
        "issue_key must not be empty"
    );
    ensure!(!input.body.trim().is_empty(), "body must not be empty");

    let client = JiraClient::from_ctx(&ctx)?;
    let request = AddCommentRequest { body: input.body };

    let response: AddCommentResponse = client
        .post_json(
            client.url_with_segments(&[
                "rest",
                "api",
                "3",
                "issue",
                input.issue_key.as_str(),
                "comment",
            ])?,
            &request,
        )
        .await?;

    Ok(AddCommentOutput {
        comment_id: response.id,
    })
}

// =============================================================================
// HTTP Client
// =============================================================================

#[derive(Debug, Clone)]
struct JiraClient {
    http: reqwest::Client,
    base_url: String,
    username: String,
    password: String,
}

impl JiraClient {
    /// Creates a new `JiraClient` from the given context, using stored Jira
    /// credentials.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Jira credentials are not found in the context
    /// - The username or password in credentials is empty or contains only
    ///   whitespace
    /// - The endpoint URL is invalid or malformed
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = JiraCredential::get(ctx)?;
        ensure!(
            !cred.username.trim().is_empty(),
            "username must not be empty"
        );
        ensure!(
            !cred.password.trim().is_empty(),
            "password must not be empty"
        );

        let base_url = normalize_base_url(
            cred.endpoint
                .as_deref()
                .unwrap_or("https://api.atlassian.com"),
        )?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            username: cred.username,
            password: cred.password,
        })
    }

    /// Constructs a URL by appending path segments to the base URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the base URL is not an absolute URL (cannot be a
    /// base).
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

    /// Sends a GET request and parses the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails
    /// - The Jira API returns a non-success status code
    /// - The response body is not valid JSON for type `T`
    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        query: &[(&str, String)],
    ) -> Result<T> {
        let response = self.send_request(self.http.get(url).query(query)).await?;
        Ok(response.json::<T>().await?)
    }

    /// Sends a POST request with a JSON body and parses the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails
    /// - The Jira API returns a non-success status code
    /// - The request body cannot be serialized to JSON
    /// - The response body is not valid JSON for type `TRes`
    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
    ) -> Result<TRes> {
        let response = self.send_request(self.http.post(url).json(body)).await?;
        Ok(response.json::<TRes>().await?)
    }

    /// Sends a POST request with a JSON body, ignoring the response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails
    /// - The Jira API returns a non-success status code
    /// - The request body cannot be serialized to JSON
    async fn post_empty<TReq: Serialize>(&self, url: reqwest::Url, body: &TReq) -> Result<()> {
        self.send_request(self.http.post(url).json(body)).await?;
        Ok(())
    }

    /// Sends an HTTP request to the Jira API with authentication and headers.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails (network errors, connection issues)
    /// - The Jira API returns a non-success status code
    /// - The response body cannot be read as text
    async fn send_request(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let response = request
            .basic_auth(&self.username, Some(&self.password))
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
                "Jira API request failed ({status}): {body}"
            ))
        }
    }
}

/// Normalizes a Jira instance base URL by trimming whitespace and trailing
/// slashes.
///
/// # Errors
///
/// Returns an error if the endpoint string is empty or contains only
/// whitespace.
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
        matchers::{basic_auth, method, path, query_param},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut jira_values = HashMap::new();
        jira_values.insert("username".to_string(), "test@example.com".to_string());
        jira_values.insert("password".to_string(), "test-token".to_string());
        jira_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_system_credential("jira", jira_values)
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://example.atlassian.net/").unwrap();
        assert_eq!(result, "https://example.atlassian.net");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://example.atlassian.net  ").unwrap();
        assert_eq!(result, "https://example.atlassian.net");
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
    async fn test_search_issues_empty_jql_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = search_issues(
            ctx,
            SearchIssuesInput {
                jql: "   ".to_string(),
                max_results: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("jql must not be empty")
        );
    }

    #[tokio::test]
    async fn test_search_issues_zero_max_results_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = search_issues(
            ctx,
            SearchIssuesInput {
                jql: "project = TEST".to_string(),
                max_results: Some(0),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("max_results must be greater than 0")
        );
    }

    #[tokio::test]
    async fn test_get_issue_empty_key_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = get_issue(
            ctx,
            GetIssueInput {
                issue_key: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("issue_key must not be empty")
        );
    }

    #[tokio::test]
    async fn test_create_issue_empty_project_key_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = create_issue(
            ctx,
            CreateIssueInput {
                project_key: "  ".to_string(),
                summary: "Test".to_string(),
                issue_type: "Task".to_string(),
                description: None,
                priority: None,
                assignee_account_id: None,
                labels: vec![],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("project_key must not be empty")
        );
    }

    #[tokio::test]
    async fn test_create_issue_empty_summary_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = create_issue(
            ctx,
            CreateIssueInput {
                project_key: "TEST".to_string(),
                summary: "  ".to_string(),
                issue_type: "Task".to_string(),
                description: None,
                priority: None,
                assignee_account_id: None,
                labels: vec![],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("summary must not be empty")
        );
    }

    #[tokio::test]
    async fn test_add_comment_empty_body_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = add_comment(
            ctx,
            AddCommentInput {
                issue_key: "TEST-123".to_string(),
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

    // --- Integration tests ---

    #[tokio::test]
    async fn test_search_issues_success() {
        let server = MockServer::start().await;

        let response_body = serde_json::json!({
            "issues": [
                {
                    "id": "10001",
                    "key": "TEST-1",
                    "fields": {
                        "summary": "Test issue",
                        "status": { "name": "To Do" },
                        "issuetype": { "name": "Task" }
                    }
                }
            ],
            "total": 1
        });

        Mock::given(method("GET"))
            .and(path("/rest/api/3/search"))
            .and(basic_auth("test@example.com", "test-token"))
            .and(query_param("jql", "project = TEST"))
            .and(query_param("maxResults", "50"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = search_issues(
            ctx,
            SearchIssuesInput {
                jql: "project = TEST".to_string(),
                max_results: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.issues.len(), 1);
        assert_eq!(output.issues[0].key, "TEST-1");
        assert_eq!(output.total, 1);
    }

    #[tokio::test]
    async fn test_get_issue_success() {
        let server = MockServer::start().await;

        let response_body = serde_json::json!({
            "id": "10001",
            "key": "TEST-123",
            "fields": {
                "summary": "Test issue",
                "description": "Description",
                "status": { "name": "In Progress" },
                "issuetype": { "name": "Bug" },
                "priority": { "name": "High" },
                "created": "2024-01-01T00:00:00.000+0000",
                "updated": "2024-01-02T00:00:00.000+0000",
                "labels": ["urgent"]
            }
        });

        Mock::given(method("GET"))
            .and(path("/rest/api/3/issue/TEST-123"))
            .and(basic_auth("test@example.com", "test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = get_issue(
            ctx,
            GetIssueInput {
                issue_key: "TEST-123".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.issue.key, "TEST-123");
    }

    #[tokio::test]
    async fn test_create_issue_success() {
        let server = MockServer::start().await;

        let response_body = serde_json::json!({
            "id": "10002",
            "key": "TEST-124"
        });

        Mock::given(method("POST"))
            .and(path("/rest/api/3/issue"))
            .and(basic_auth("test@example.com", "test-token"))
            .respond_with(ResponseTemplate::new(201).set_body_json(response_body))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = create_issue(
            ctx,
            CreateIssueInput {
                project_key: "TEST".to_string(),
                summary: "New issue".to_string(),
                issue_type: "Task".to_string(),
                description: None,
                priority: None,
                assignee_account_id: None,
                labels: vec![],
            },
        )
        .await
        .unwrap();

        assert_eq!(output.key, "TEST-124");
        assert_eq!(output.id, "10002");
    }

    #[tokio::test]
    async fn test_transition_issue_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/rest/api/3/issue/TEST-123/transitions"))
            .and(basic_auth("test@example.com", "test-token"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = transition_issue(
            ctx,
            TransitionIssueInput {
                issue_key: "TEST-123".to_string(),
                transition_id: "21".to_string(),
            },
        )
        .await
        .unwrap();

        assert!(output.success);
    }

    #[tokio::test]
    async fn test_add_comment_success() {
        let server = MockServer::start().await;

        let response_body = serde_json::json!({
            "id": "10003"
        });

        Mock::given(method("POST"))
            .and(path("/rest/api/3/issue/TEST-123/comment"))
            .and(basic_auth("test@example.com", "test-token"))
            .respond_with(ResponseTemplate::new(201).set_body_json(response_body))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = add_comment(
            ctx,
            AddCommentInput {
                issue_key: "TEST-123".to_string(),
                body: "Test comment".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.comment_id, "10003");
    }

    #[tokio::test]
    async fn test_jira_api_error_returns_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/api/3/search"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_raw(r#"{"errorMessages":["Unauthorized"]}"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let result = search_issues(
            ctx,
            SearchIssuesInput {
                jql: "project = TEST".to_string(),
                max_results: None,
            },
        )
        .await;

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("401"));
    }
}
