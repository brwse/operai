//! project-tasks/jira-service-management integration for Operai Toolbox.

mod types;

use operai::{
    Context, JsonSchema, Result, define_system_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
use types::{
    AddCommentRequest, Comment, PagedResponse, PerformTransitionRequest, RequestDetail,
    RequestSummary,
};

define_system_credential! {
    JiraCredential("jira_service_management") {
        username: String,
        password: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_ENDPOINT: &str = "https://your-domain.atlassian.net";

#[init]
async fn setup() -> Result<()> {
    info!("Jira Service Management integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Jira Service Management integration shutting down");
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchRequestsInput {
    /// Service desk ID to search within.
    pub service_desk_id: String,
    /// JQL query to filter requests (optional).
    #[serde(default)]
    pub query: Option<String>,
    /// Maximum number of results (1-100). Defaults to 50.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchRequestsOutput {
    pub requests: Vec<RequestSummary>,
}

/// # Search Jira Service Management Requests
///
/// Searches for customer service requests within a specific Jira Service
/// Management service desk.
///
/// Use this tool when the user wants to find, list, or filter service
/// requests/tickets in Jira Service Management. This is ideal for browsing
/// requests, searching for specific tickets by text, or retrieving a paginated
/// list of requests for further processing.
///
/// ## Inputs
/// - `service_desk_id`: The ID of the service desk to search within (required)
/// - `query`: Optional text search term to filter requests (searches ticket
///   summaries, descriptions, and comments)
/// - `limit`: Number of results to return (1-100, defaults to 50)
///
/// ## Outputs
/// Returns a list of request summaries including issue key, status, request
/// type, reporter, and creation date.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - jira
/// - service-management
/// - tickets
///
/// # Errors
///
/// Returns an error if:
/// - The `service_desk_id` is empty or contains only whitespace
/// - The `limit` is not between 1 and 100 (inclusive)
/// - Required credentials (username, password) are missing or empty
/// - The configured endpoint is not a valid absolute URL
/// - The HTTP request fails due to network issues or authentication errors
/// - The API returns a non-success status code
/// - The response body cannot be parsed as JSON
#[tool]
pub async fn search_requests(
    ctx: Context,
    input: SearchRequestsInput,
) -> Result<SearchRequestsOutput> {
    ensure!(
        !input.service_desk_id.trim().is_empty(),
        "service_desk_id must not be empty"
    );
    let limit = input.limit.unwrap_or(50);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let client = JiraClient::from_ctx(&ctx)?;

    let mut query_params = vec![
        ("serviceDeskId", input.service_desk_id.clone()),
        ("limit", limit.to_string()),
    ];
    if let Some(query) = input.query.as_ref()
        && !query.trim().is_empty()
    {
        query_params.push(("searchTerm", query.clone()));
    }

    let response: PagedResponse<RequestSummary> = client
        .get_json(
            client.url_with_segments(&["rest", "servicedeskapi", "request"])?,
            &query_params,
        )
        .await?;

    Ok(SearchRequestsOutput {
        requests: response.values,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetTicketInput {
    /// Issue key (e.g., "SD-123") or issue ID.
    pub issue_id_or_key: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetTicketOutput {
    pub request: RequestDetail,
}

/// # Get Jira Service Management Ticket
///
/// Retrieves detailed information for a single service request from Jira
/// Service Management.
///
/// Use this tool when the user wants to view full details of a specific ticket,
/// including all fields, custom field values, comments, attachments, and
/// complete request metadata. This provides more comprehensive information than
/// the summary returned by `search_requests`.
///
/// ## Inputs
/// - `issue_id_or_key`: The issue identifier (e.g., "SD-123" format) or numeric
///   issue ID
///
/// ## Outputs
/// Returns complete request details including:
/// - Issue key and ID
/// - Request type and current status
/// - Reporter and assignee information
/// - Creation and update dates
/// - Custom field values
/// - Comments and attachments
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - jira
/// - service-management
/// - tickets
///
/// # Errors
///
/// Returns an error if:
/// - The `issue_id_or_key` is empty or contains only whitespace
/// - Required credentials (username, password) are missing or empty
/// - The configured endpoint is not a valid absolute URL
/// - The HTTP request fails due to network issues or authentication errors
/// - The API returns a non-success status code
/// - The response body cannot be parsed as JSON
#[tool]
pub async fn get_ticket(ctx: Context, input: GetTicketInput) -> Result<GetTicketOutput> {
    ensure!(
        !input.issue_id_or_key.trim().is_empty(),
        "issue_id_or_key must not be empty"
    );

    let client = JiraClient::from_ctx(&ctx)?;

    let request: RequestDetail = client
        .get_json(
            client.url_with_segments(&[
                "rest",
                "servicedeskapi",
                "request",
                input.issue_id_or_key.as_str(),
            ])?,
            &[],
        )
        .await?;

    Ok(GetTicketOutput { request })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CommentInput {
    /// Issue key (e.g., "SD-123") or issue ID.
    pub issue_id_or_key: String,
    /// Comment body text.
    pub body: String,
    /// Whether the comment is public (visible to customers). Defaults to true.
    #[serde(default)]
    pub public: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CommentOutput {
    pub comment: Comment,
}

/// # Add Jira Service Management Comment
///
/// Adds a comment to an existing service request in Jira Service Management.
///
/// Use this tool when the user wants to add a comment or note to a ticket.
/// Comments can be used to provide updates, ask questions, or communicate with
/// customers and team members. The visibility of comments can be controlled to
/// determine whether customers (portal users) can see them.
///
/// ## Inputs
/// - `issue_id_or_key`: The issue identifier (e.g., "SD-123") or numeric issue
///   ID
/// - `body`: The comment text content (required, must not be empty)
/// - `public`: Whether the comment is visible to customers via the portal
///   (defaults to true)
///   - Set to `true` for customer-facing communications
///   - Set to `false` for internal agent-only notes
///
/// ## Outputs
/// Returns the created comment including ID, body, author, creation timestamp,
/// and visibility setting.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - jira
/// - service-management
/// - tickets
///
/// # Errors
///
/// Returns an error if:
/// - The `issue_id_or_key` is empty or contains only whitespace
/// - The `body` is empty or contains only whitespace
/// - Required credentials (username, password) are missing or empty
/// - The configured endpoint is not a valid absolute URL
/// - The HTTP request fails due to network issues or authentication errors
/// - The API returns a non-success status code
/// - The response body cannot be parsed as JSON
#[tool]
pub async fn comment(ctx: Context, input: CommentInput) -> Result<CommentOutput> {
    ensure!(
        !input.issue_id_or_key.trim().is_empty(),
        "issue_id_or_key must not be empty"
    );
    ensure!(!input.body.trim().is_empty(), "body must not be empty");

    let client = JiraClient::from_ctx(&ctx)?;

    let request = AddCommentRequest {
        body: input.body,
        public: input.public,
    };

    let comment: Comment = client
        .post_json(
            client.url_with_segments(&[
                "rest",
                "servicedeskapi",
                "request",
                input.issue_id_or_key.as_str(),
                "comment",
            ])?,
            &request,
        )
        .await?;

    Ok(CommentOutput { comment })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TransitionInput {
    /// Issue key (e.g., "SD-123") or issue ID.
    pub issue_id_or_key: String,
    /// Transition ID to perform.
    pub transition_id: String,
    /// Optional comment to add with the transition.
    #[serde(default)]
    pub comment: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TransitionOutput {
    pub success: bool,
}

/// # Transition Jira Service Management Ticket
///
/// Transitions (moves) a service request to a different workflow status in Jira
/// Service Management.
///
/// Use this tool when the user wants to change the status of a ticket, such as
/// moving it from "Open" to "In Progress", or "In Progress" to "Resolved".
/// Workflows are configured per service desk, and the available transitions
/// depend on the current status and workflow configuration.
///
/// ## Inputs
/// - `issue_id_or_key`: The issue identifier (e.g., "SD-123") or numeric issue
///   ID
/// - `transition_id`: The ID of the transition to perform (required)
///   - This is NOT the status ID, but the transition ID from the workflow
///   - Transition IDs can be obtained from Jira's workflow API or UI
/// - `comment`: Optional comment to add along with the transition
///
/// ## Outputs
/// Returns a success indicator confirming the transition was executed.
///
/// ## Important Notes
/// - The `transition_id` is workflow-specific and differs from status names/IDs
/// - Invalid transitions will fail (e.g., trying to skip required workflow
///   steps)
/// - Consider adding a comment to explain the reason for the status change
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - jira
/// - service-management
/// - tickets
///
/// # Errors
///
/// Returns an error if:
/// - The `issue_id_or_key` is empty or contains only whitespace
/// - The `transition_id` is empty or contains only whitespace
/// - Required credentials (username, password) are missing or empty
/// - The configured endpoint is not a valid absolute URL
/// - The HTTP request fails due to network issues or authentication errors
/// - The API returns a non-success status code
#[tool]
pub async fn transition(ctx: Context, input: TransitionInput) -> Result<TransitionOutput> {
    ensure!(
        !input.issue_id_or_key.trim().is_empty(),
        "issue_id_or_key must not be empty"
    );
    ensure!(
        !input.transition_id.trim().is_empty(),
        "transition_id must not be empty"
    );

    let client = JiraClient::from_ctx(&ctx)?;

    let additional_comment =
        input
            .comment
            .filter(|c| !c.trim().is_empty())
            .map(|body| AddCommentRequest {
                body,
                public: Some(true),
            });

    let request = PerformTransitionRequest {
        id: input.transition_id,
        additional_comment,
    };

    client
        .post_empty(
            client.url_with_segments(&[
                "rest",
                "servicedeskapi",
                "request",
                input.issue_id_or_key.as_str(),
                "transition",
            ])?,
            &request,
        )
        .await?;

    Ok(TransitionOutput { success: true })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AssignInput {
    /// Issue key (e.g., "SD-123") or issue ID.
    pub issue_id_or_key: String,
    /// Account ID of the user to assign.
    pub account_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AssignOutput {
    pub success: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AssignRequest {
    account_id: String,
}

/// # Assign Jira Service Management Ticket
///
/// Assigns a service request to a specific user in Jira Service Management.
///
/// Use this tool when the user wants to assign or reassign a ticket to an agent
/// or team member. Assignment helps track ownership and responsibility for
/// resolving customer requests.
///
/// ## Inputs
/// - `issue_id_or_key`: The issue identifier (e.g., "SD-123") or numeric issue
///   ID
/// - `account_id`: The Atlassian account ID of the user to assign the ticket to
///   (required)
///   - This is the user's unique account ID, not their username or display name
///   - Account IDs can be found in Jira's user directory or via the user API
///
/// ## Outputs
/// Returns a success indicator confirming the assignment was completed.
///
/// ## Important Notes
/// - Use the user's Atlassian account ID (format:
///   "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx")
/// - The user must have appropriate permissions to be assigned tickets in the
///   service desk
/// - To unassign a ticket, use the special account ID for unassignment (if
///   supported by your Jira configuration)
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - jira
/// - service-management
/// - tickets
///
/// # Errors
///
/// Returns an error if:
/// - The `issue_id_or_key` is empty or contains only whitespace
/// - The `account_id` is empty or contains only whitespace
/// - Required credentials (username, password) are missing or empty
/// - The configured endpoint is not a valid absolute URL
/// - The HTTP request fails due to network issues or authentication errors
/// - The API returns a non-success status code
#[tool]
pub async fn assign(ctx: Context, input: AssignInput) -> Result<AssignOutput> {
    ensure!(
        !input.issue_id_or_key.trim().is_empty(),
        "issue_id_or_key must not be empty"
    );
    ensure!(
        !input.account_id.trim().is_empty(),
        "account_id must not be empty"
    );

    let client = JiraClient::from_ctx(&ctx)?;

    let request = AssignRequest {
        account_id: input.account_id,
    };

    // Use standard Jira Platform API for assignment
    client
        .put_empty(
            client.url_with_segments(&[
                "rest",
                "api",
                "3",
                "issue",
                input.issue_id_or_key.as_str(),
                "assignee",
            ])?,
            &request,
        )
        .await?;

    Ok(AssignOutput { success: true })
}

#[derive(Debug, Clone)]
struct JiraClient {
    http: reqwest::Client,
    base_url: String,
    username: String,
    password: String,
}

impl JiraClient {
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

        let base_url = normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_ENDPOINT))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            username: cred.username,
            password: cred.password,
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
                "Jira Service Management request failed ({status}): {body}"
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
    use crate::types::User;

    fn test_ctx(endpoint: &str) -> Context {
        let mut jira_values = HashMap::new();
        jira_values.insert("username".to_string(), "test-user".to_string());
        jira_values.insert("password".to_string(), "test-token".to_string());
        jira_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_system_credential("jira_service_management", jira_values)
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_user_serialization_roundtrip() {
        let user = User {
            account_id: Some("123".to_string()),
            display_name: Some("John Doe".to_string()),
            email_address: Some("john@example.com".to_string()),
        };
        let json = serde_json::to_string(&user).unwrap();
        let parsed: User = serde_json::from_str(&json).unwrap();
        assert_eq!(user.account_id, parsed.account_id);
        assert_eq!(user.display_name, parsed.display_name);
    }

    #[test]
    fn test_request_summary_serialization_roundtrip() {
        let request = RequestSummary {
            issue_id: "10001".to_string(),
            issue_key: "SD-123".to_string(),
            request_type: None,
            current_status: None,
            reporter: None,
            created_date: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        let parsed: RequestSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(request.issue_id, parsed.issue_id);
        assert_eq!(request.issue_key, parsed.issue_key);
    }

    #[test]
    fn test_comment_serialization_roundtrip() {
        let comment = Comment {
            id: "100".to_string(),
            body: "Test comment".to_string(),
            author: None,
            created: None,
            public: Some(true),
        };
        let json = serde_json::to_string(&comment).unwrap();
        let parsed: Comment = serde_json::from_str(&json).unwrap();
        assert_eq!(comment.id, parsed.id);
        assert_eq!(comment.body, parsed.body);
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
    async fn test_search_requests_empty_service_desk_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = search_requests(
            ctx,
            SearchRequestsInput {
                service_desk_id: "  ".to_string(),
                query: None,
                limit: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("service_desk_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_search_requests_limit_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = search_requests(
            ctx,
            SearchRequestsInput {
                service_desk_id: "1".to_string(),
                query: None,
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
    async fn test_search_requests_limit_exceeds_max_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = search_requests(
            ctx,
            SearchRequestsInput {
                service_desk_id: "1".to_string(),
                query: None,
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
    async fn test_get_ticket_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = get_ticket(
            ctx,
            GetTicketInput {
                issue_id_or_key: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("issue_id_or_key must not be empty")
        );
    }

    #[tokio::test]
    async fn test_comment_empty_issue_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = comment(
            ctx,
            CommentInput {
                issue_id_or_key: "  ".to_string(),
                body: "Test".to_string(),
                public: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("issue_id_or_key must not be empty")
        );
    }

    #[tokio::test]
    async fn test_comment_empty_body_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = comment(
            ctx,
            CommentInput {
                issue_id_or_key: "SD-123".to_string(),
                body: "  ".to_string(),
                public: None,
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
    async fn test_transition_empty_issue_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = transition(
            ctx,
            TransitionInput {
                issue_id_or_key: "  ".to_string(),
                transition_id: "11".to_string(),
                comment: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("issue_id_or_key must not be empty")
        );
    }

    #[tokio::test]
    async fn test_transition_empty_transition_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = transition(
            ctx,
            TransitionInput {
                issue_id_or_key: "SD-123".to_string(),
                transition_id: "  ".to_string(),
                comment: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("transition_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_assign_empty_issue_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = assign(
            ctx,
            AssignInput {
                issue_id_or_key: "  ".to_string(),
                account_id: "123".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("issue_id_or_key must not be empty")
        );
    }

    #[tokio::test]
    async fn test_assign_empty_account_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = assign(
            ctx,
            AssignInput {
                issue_id_or_key: "SD-123".to_string(),
                account_id: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("account_id must not be empty")
        );
    }

    // --- Integration tests with mock server ---

    #[tokio::test]
    async fn test_search_requests_success_returns_requests() {
        let server = MockServer::start().await;
        let endpoint = server.uri();

        let response_body = r#"
        {
          "values": [
            {
              "issueId": "10001",
              "issueKey": "SD-123",
              "requestType": {
                "id": "1",
                "name": "Incident"
              },
              "currentStatus": {
                "status": "Open",
                "statusCategory": "new"
              },
              "reporter": {
                "accountId": "user-1",
                "displayName": "John Doe"
              },
              "createdDate": {
                "epochMillis": 1704067200000,
                "friendly": "January 1, 2024",
                "iso8601": "2024-01-01T00:00:00.000Z",
                "jira": "2024-01-01T00:00:00.000Z"
              }
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/rest/servicedeskapi/request"))
            .and(header(
                "authorization",
                "Basic dGVzdC11c2VyOnRlc3QtdG9rZW4=",
            ))
            .and(query_param("serviceDeskId", "1"))
            .and(query_param("limit", "10"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = search_requests(
            ctx,
            SearchRequestsInput {
                service_desk_id: "1".to_string(),
                query: None,
                limit: Some(10),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.requests.len(), 1);
        assert_eq!(output.requests[0].issue_key, "SD-123");
        assert_eq!(output.requests[0].issue_id, "10001");
    }

    #[tokio::test]
    async fn test_get_ticket_success_returns_request() {
        let server = MockServer::start().await;
        let endpoint = server.uri();

        let response_body = r#"
        {
          "issueId": "10001",
          "issueKey": "SD-123",
          "requestType": {
            "id": "1",
            "name": "Incident"
          },
          "currentStatus": {
            "status": "Open"
          },
          "createdDate": {
            "epochMillis": 1704067200000,
            "friendly": "January 1, 2024",
            "iso8601": "2024-01-01T00:00:00.000Z"
          },
          "requestFieldValues": []
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/rest/servicedeskapi/request/SD-123"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = get_ticket(
            ctx,
            GetTicketInput {
                issue_id_or_key: "SD-123".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.request.issue_key, "SD-123");
    }

    #[tokio::test]
    async fn test_comment_success_adds_comment() {
        let server = MockServer::start().await;
        let endpoint = server.uri();

        let response_body = r#"
        {
          "id": "100",
          "body": "Test comment",
          "public": true,
          "created": {
            "epochMillis": 1704067200000,
            "friendly": "January 1, 2024",
            "iso8601": "2024-01-01T00:00:00.000Z"
          }
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/rest/servicedeskapi/request/SD-123/comment"))
            .and(body_string_contains("Test comment"))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = comment(
            ctx,
            CommentInput {
                issue_id_or_key: "SD-123".to_string(),
                body: "Test comment".to_string(),
                public: Some(true),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.comment.body, "Test comment");
    }

    #[tokio::test]
    async fn test_transition_success_performs_transition() {
        let server = MockServer::start().await;
        let endpoint = server.uri();

        Mock::given(method("POST"))
            .and(path("/rest/servicedeskapi/request/SD-123/transition"))
            .and(body_string_contains("\"id\":\"11\""))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = transition(
            ctx,
            TransitionInput {
                issue_id_or_key: "SD-123".to_string(),
                transition_id: "11".to_string(),
                comment: None,
            },
        )
        .await
        .unwrap();

        assert!(output.success);
    }

    #[tokio::test]
    async fn test_assign_success_assigns_user() {
        let server = MockServer::start().await;
        let endpoint = server.uri();

        Mock::given(method("PUT"))
            .and(path("/rest/api/3/issue/SD-123/assignee"))
            .and(body_string_contains("\"accountId\":\"user-1\""))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = assign(
            ctx,
            AssignInput {
                issue_id_or_key: "SD-123".to_string(),
                account_id: "user-1".to_string(),
            },
        )
        .await
        .unwrap();

        assert!(output.success);
    }

    #[tokio::test]
    async fn test_search_requests_error_response_returns_error() {
        let server = MockServer::start().await;
        let endpoint = server.uri();

        Mock::given(method("GET"))
            .and(path("/rest/servicedeskapi/request"))
            .and(query_param("serviceDeskId", "1"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_raw(r#"{"errorMessages":["Unauthorized"]}"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = search_requests(
            ctx,
            SearchRequestsInput {
                service_desk_id: "1".to_string(),
                query: None,
                limit: None,
            },
        )
        .await;

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("401"));
    }
}
