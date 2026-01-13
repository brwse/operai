//! meetings-calling/aircall integration for Operai Toolbox.

mod types;

use operai::{
    Context, JsonSchema, Result, define_system_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
use types::{Call, CallSummary, Comment, Meta};

define_system_credential! {
    AircallCredential("aircall") {
        api_id: String,
        api_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_API_ENDPOINT: &str = "https://api.aircall.io/v1";

#[init]
async fn setup() -> Result<()> {
    info!("Aircall integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Aircall integration shutting down");
}

// ============================================================================
// Tool 1: List Calls
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListCallsInput {
    /// Maximum number of results per page (1-50). Defaults to 10.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Pagination: page number to retrieve. Defaults to 1.
    #[serde(default)]
    pub page: Option<u32>,
    /// Filter by start timestamp (Unix timestamp in seconds).
    #[serde(default)]
    pub from: Option<i64>,
    /// Filter by end timestamp (Unix timestamp in seconds).
    #[serde(default)]
    pub to: Option<i64>,
    /// Sort order: "asc" or "desc". Defaults to "desc".
    #[serde(default)]
    pub order: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListCallsOutput {
    pub calls: Vec<CallSummary>,
    #[serde(default)]
    pub meta: Option<Meta>,
}

/// # List Aircall Calls
///
/// Retrieves a paginated list of calls from the Aircall API with optional
/// filtering by time range and sort order. Use this tool when the user wants to
/// view, browse, or search through their Aircall call history.
/// Supports pagination for large result sets and time-based filtering to find
/// calls within specific date ranges. Returns call summaries including metadata
/// such as call duration, direction (inbound/outbound), status, and participant
/// information.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - calls
/// - aircall
/// - phone
///
/// # Errors
///
/// Returns an error if:
/// - The `limit` parameter is not between 1 and 50
/// - The `page` parameter is less than 1
/// - The `order` parameter is not "asc" or "desc"
/// - Aircall credentials are not configured or are invalid
/// - The Aircall API request fails (network error, timeout, etc.)
/// - The Aircall API returns an error response
#[tool]
pub async fn list_calls(ctx: Context, input: ListCallsInput) -> Result<ListCallsOutput> {
    let limit = input.limit.unwrap_or(10);
    ensure!((1..=50).contains(&limit), "limit must be between 1 and 50");

    let page = input.page.unwrap_or(1);
    ensure!(page >= 1, "page must be at least 1");

    if let Some(ref order) = input.order {
        ensure!(
            order == "asc" || order == "desc",
            "order must be 'asc' or 'desc'"
        );
    }

    let client = AircallClient::from_ctx(&ctx)?;

    let mut query = vec![("per_page", limit.to_string()), ("page", page.to_string())];

    if let Some(from) = input.from {
        query.push(("from", from.to_string()));
    }
    if let Some(to) = input.to {
        query.push(("to", to.to_string()));
    }
    if let Some(order) = input.order {
        query.push(("order", order));
    }

    let response: AircallListResponse<CallSummary> = client.get_json("/calls", &query).await?;

    Ok(ListCallsOutput {
        calls: response.calls,
        meta: response.meta,
    })
}

// ============================================================================
// Tool 2: Assign Call
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AssignCallInput {
    /// Aircall call ID to assign.
    pub call_id: i64,
    /// User ID to assign the call to (mutually exclusive with `team_id`).
    #[serde(default)]
    pub user_id: Option<i64>,
    /// Team ID to assign the call to (mutually exclusive with `user_id`).
    #[serde(default)]
    pub team_id: Option<i64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AssignCallOutput {
    pub assigned: bool,
    pub call_id: i64,
}

/// # Assign Aircall Call
///
/// Assigns or transfers an Aircall call to a specific user or team.
/// Use this tool when the user wants to reassign ownership of a call, transfer
/// a call to another agent, or route a call to a team. Requires either a user
/// ID or a team ID (mutually exclusive - only one can be specified).
/// Commonly used for call routing, escalation, or reassigning calls after agent
/// transfers or unavailability.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - calls
/// - aircall
/// - assignment
///
/// # Errors
///
/// Returns an error if:
/// - The `call_id` parameter is not positive
/// - Neither `user_id` nor `team_id` is provided
/// - Both `user_id` and `team_id` are provided
/// - Aircall credentials are not configured or are invalid
/// - The Aircall API request fails (network error, timeout, etc.)
/// - The Aircall API returns an error response
#[tool]
pub async fn assign_call(ctx: Context, input: AssignCallInput) -> Result<AssignCallOutput> {
    ensure!(input.call_id > 0, "call_id must be positive");

    // Ensure exactly one destination is provided
    let has_user = input.user_id.is_some();
    let has_team = input.team_id.is_some();
    ensure!(
        has_user || has_team,
        "either user_id or team_id must be provided"
    );
    ensure!(
        !(has_user && has_team),
        "cannot provide both user_id and team_id"
    );

    let client = AircallClient::from_ctx(&ctx)?;

    let request = AssignCallRequest {
        user_id: input.user_id,
        team_id: input.team_id,
    };

    client
        .post_empty(&format!("/calls/{}/transfers", input.call_id), &request)
        .await?;

    Ok(AssignCallOutput {
        assigned: true,
        call_id: input.call_id,
    })
}

// ============================================================================
// Tool 3: Create Follow-up (Comment)
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateFollowupInput {
    /// Aircall call ID to add a comment to.
    pub call_id: i64,
    /// Comment content (maximum 5 comments per call).
    pub content: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateFollowupOutput {
    pub created: bool,
    pub call_id: i64,
    pub comment: Comment,
}

/// # Create Aircall Follow-up
///
/// Adds a follow-up comment or note to an existing Aircall call.
/// Use this tool when the user wants to add notes, action items, summaries, or
/// any contextual information to a call record. Supports up to 5 comments per
/// call and includes timestamp and author information. Commonly used for
/// documenting call outcomes, customer notes, next steps, or sharing context
/// with team members.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - calls
/// - aircall
/// - comments
///
/// # Errors
///
/// Returns an error if:
/// - The `call_id` parameter is not positive
/// - The `content` parameter is empty or contains only whitespace
/// - Aircall credentials are not configured or are invalid
/// - The Aircall API request fails (network error, timeout, etc.)
/// - The Aircall API returns an error response
#[tool]
pub async fn create_followup(
    ctx: Context,
    input: CreateFollowupInput,
) -> Result<CreateFollowupOutput> {
    ensure!(input.call_id > 0, "call_id must be positive");
    ensure!(
        !input.content.trim().is_empty(),
        "content must not be empty"
    );

    let client = AircallClient::from_ctx(&ctx)?;

    let request = CreateCommentRequest {
        content: input.content,
    };

    let response: AircallCommentResponse = client
        .post_json(&format!("/calls/{}/comments", input.call_id), &request)
        .await?;

    Ok(CreateFollowupOutput {
        created: true,
        call_id: input.call_id,
        comment: response.comment,
    })
}

// ============================================================================
// Tool 4: Fetch Recording Link
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FetchRecordingLinkInput {
    /// Aircall call ID.
    pub call_id: i64,
    /// When true, return short URLs (valid 3 hours) instead of direct URLs
    /// (valid 1 hour).
    #[serde(default)]
    pub use_short_url: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FetchRecordingLinkOutput {
    pub call_id: i64,
    #[serde(default)]
    pub recording_url: Option<String>,
    #[serde(default)]
    pub voicemail_url: Option<String>,
    #[serde(default)]
    pub asset_url: Option<String>,
}

/// # Fetch Aircall Recording Link
///
/// Retrieves the recording, voicemail, and asset URLs for an Aircall call.
/// Use this tool when the user wants to access call recordings, listen to
/// voicemails, or share call audio files. Returns direct URLs (valid for 1
/// hour) by default, or short URLs (valid for 3 hours) if requested.
/// Supports multiple audio types: call recordings, voicemail messages, and
/// general call assets.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - calls
/// - aircall
/// - recordings
///
/// # Errors
///
/// Returns an error if:
/// - The `call_id` parameter is not positive
/// - Aircall credentials are not configured or are invalid
/// - The Aircall API request fails (network error, timeout, etc.)
/// - The Aircall API returns an error response (e.g., call not found)
#[tool]
pub async fn fetch_recording_link(
    ctx: Context,
    input: FetchRecordingLinkInput,
) -> Result<FetchRecordingLinkOutput> {
    ensure!(input.call_id > 0, "call_id must be positive");

    let client = AircallClient::from_ctx(&ctx)?;

    let response: AircallCallResponse = client
        .get_json(&format!("/calls/{}", input.call_id), &[])
        .await?;

    let call = response.call;

    let recording_url = if input.use_short_url {
        call.recording_short_url
    } else {
        call.recording
    };

    let voicemail_url = if input.use_short_url {
        call.voicemail_short_url
    } else {
        call.voicemail
    };

    Ok(FetchRecordingLinkOutput {
        call_id: input.call_id,
        recording_url,
        voicemail_url,
        asset_url: call.asset,
    })
}

// ============================================================================
// Internal API Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct AircallListResponse<T> {
    calls: Vec<T>,
    #[serde(default)]
    meta: Option<Meta>,
}

#[derive(Debug, Deserialize)]
struct AircallCallResponse {
    call: Call,
}

#[derive(Debug, Deserialize)]
struct AircallCommentResponse {
    comment: Comment,
}

#[derive(Debug, Serialize)]
struct AssignCallRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    user_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    team_id: Option<i64>,
}

#[derive(Debug, Serialize)]
struct CreateCommentRequest {
    content: String,
}

// ============================================================================
// HTTP Client
// ============================================================================

#[derive(Debug, Clone)]
struct AircallClient {
    http: reqwest::Client,
    base_url: String,
    api_id: String,
    api_token: String,
}

impl AircallClient {
    /// Creates a new [`AircallClient`] from the context.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Aircall credentials are not configured in the context
    /// - The `api_id` or `api_token` are empty
    /// - The endpoint URL is invalid
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = AircallCredential::get(ctx)?;
        ensure!(!cred.api_id.trim().is_empty(), "api_id must not be empty");
        ensure!(
            !cred.api_token.trim().is_empty(),
            "api_token must not be empty"
        );

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_API_ENDPOINT))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            api_id: cred.api_id,
            api_token: cred.api_token,
        })
    }

    /// Builds a full URL from the base URL and path.
    ///
    /// # Errors
    ///
    /// Returns an error if the resulting URL is invalid.
    fn build_url(&self, path: &str) -> Result<reqwest::Url> {
        let trimmed_path = path.trim_start_matches('/');
        let url_string = format!("{}/{}", self.base_url, trimmed_path);
        Ok(reqwest::Url::parse(&url_string)?)
    }

    /// Sends a GET request and parses the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The URL cannot be built
    /// - The HTTP request fails
    /// - The response body cannot be parsed as JSON
    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T> {
        let url = self.build_url(path)?;
        let request = self.http.get(url).query(query);

        let response = self.send_request(request).await?;
        Ok(response.json::<T>().await?)
    }

    /// Sends a POST request with a JSON body and parses the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The URL cannot be built
    /// - The HTTP request fails
    /// - The response body cannot be parsed as JSON
    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &TReq,
    ) -> Result<TRes> {
        let url = self.build_url(path)?;
        let request = self.http.post(url).json(body);

        let response = self.send_request(request).await?;
        Ok(response.json::<TRes>().await?)
    }

    /// Sends a POST request with a JSON body, ignoring the response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The URL cannot be built
    /// - The HTTP request fails
    async fn post_empty<TReq: Serialize>(&self, path: &str, body: &TReq) -> Result<()> {
        let url = self.build_url(path)?;
        let request = self.http.post(url).json(body);

        self.send_request(request).await?;
        Ok(())
    }

    /// Sends an HTTP request with authentication and error handling.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails (network error, timeout, etc.)
    /// - The API returns a non-success status code
    async fn send_request(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let response = request
            .basic_auth(&self.api_id, Some(&self.api_token))
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Aircall API request failed ({status}): {body}"
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

    use types::{CallDirection, CallStatus, User};
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{basic_auth, body_string_contains, method, path, query_param},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut aircall_values = HashMap::new();
        aircall_values.insert("api_id".to_string(), "test-api-id".to_string());
        aircall_values.insert("api_token".to_string(), "test-api-token".to_string());
        aircall_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_system_credential("aircall", aircall_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        format!("{}/v1", server.uri())
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_call_direction_serialization_roundtrip() {
        for variant in [CallDirection::Inbound, CallDirection::Outbound] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: CallDirection = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_call_status_serialization_roundtrip() {
        for variant in [
            CallStatus::Initial,
            CallStatus::Ringing,
            CallStatus::Answered,
            CallStatus::Done,
            CallStatus::Abandoned,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: CallStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_user_serialization_roundtrip() {
        let user = User {
            id: 123,
            name: Some("Alice".to_string()),
            email: Some("alice@example.com".to_string()),
        };
        let json = serde_json::to_string(&user).unwrap();
        let parsed: User = serde_json::from_str(&json).unwrap();
        assert_eq!(user.id, parsed.id);
        assert_eq!(user.name, parsed.name);
        assert_eq!(user.email, parsed.email);
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://api.aircall.io/v1/").unwrap();
        assert_eq!(result, "https://api.aircall.io/v1");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://api.aircall.io/v1  ").unwrap();
        assert_eq!(result, "https://api.aircall.io/v1");
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
    async fn test_list_calls_limit_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_calls(
            ctx,
            ListCallsInput {
                limit: Some(0),
                page: None,
                from: None,
                to: None,
                order: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("limit must be between 1 and 50")
        );
    }

    #[tokio::test]
    async fn test_list_calls_limit_exceeds_max_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_calls(
            ctx,
            ListCallsInput {
                limit: Some(51),
                page: None,
                from: None,
                to: None,
                order: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("limit must be between 1 and 50")
        );
    }

    #[tokio::test]
    async fn test_list_calls_invalid_order_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_calls(
            ctx,
            ListCallsInput {
                limit: None,
                page: None,
                from: None,
                to: None,
                order: Some("invalid".to_string()),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("order must be 'asc' or 'desc'")
        );
    }

    #[tokio::test]
    async fn test_list_calls_page_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_calls(
            ctx,
            ListCallsInput {
                limit: None,
                page: Some(0),
                from: None,
                to: None,
                order: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("page must be at least 1")
        );
    }

    #[tokio::test]
    async fn test_assign_call_zero_call_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = assign_call(
            ctx,
            AssignCallInput {
                call_id: 0,
                user_id: Some(123),
                team_id: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("call_id must be positive")
        );
    }

    #[tokio::test]
    async fn test_assign_call_no_destination_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = assign_call(
            ctx,
            AssignCallInput {
                call_id: 456,
                user_id: None,
                team_id: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("either user_id or team_id must be provided")
        );
    }

    #[tokio::test]
    async fn test_assign_call_both_destinations_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = assign_call(
            ctx,
            AssignCallInput {
                call_id: 456,
                user_id: Some(123),
                team_id: Some(789),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("cannot provide both user_id and team_id")
        );
    }

    #[tokio::test]
    async fn test_create_followup_empty_content_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = create_followup(
            ctx,
            CreateFollowupInput {
                call_id: 456,
                content: "   ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("content must not be empty")
        );
    }

    #[tokio::test]
    async fn test_fetch_recording_link_zero_call_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = fetch_recording_link(
            ctx,
            FetchRecordingLinkInput {
                call_id: 0,
                use_short_url: false,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("call_id must be positive")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_list_calls_success_returns_calls() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "calls": [
            {
              "id": 123456,
              "direct_link": "https://app.aircall.io/calls/123456",
              "started_at": 1609459200,
              "answered_at": 1609459205,
              "ended_at": 1609459500,
              "duration": 300,
              "direction": "inbound",
              "status": "done",
              "raw_digits": "+15551234567",
              "user": { "id": 1, "name": "Alice", "email": "alice@example.com" },
              "teams": [],
              "comments": [],
              "tags": []
            }
          ],
          "meta": {
            "total": 1,
            "count": 1,
            "current_page": 1,
            "per_page": 10
          }
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1/calls"))
            .and(basic_auth("test-api-id", "test-api-token"))
            .and(query_param("per_page", "5"))
            .and(query_param("page", "1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_calls(
            ctx,
            ListCallsInput {
                limit: Some(5),
                page: None,
                from: None,
                to: None,
                order: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.calls.len(), 1);
        assert_eq!(output.calls[0].id, 123_456);
        assert_eq!(output.calls[0].direction, Some(CallDirection::Inbound));
        assert_eq!(output.calls[0].status, Some(CallStatus::Done));
    }

    #[tokio::test]
    async fn test_list_calls_error_response_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/v1/calls"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_raw(r#"{ "error": "Unauthorized" }"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = list_calls(
            ctx,
            ListCallsInput {
                limit: None,
                page: None,
                from: None,
                to: None,
                order: None,
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("401"));
    }

    #[tokio::test]
    async fn test_assign_call_success_returns_assigned() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("POST"))
            .and(path("/v1/calls/123456/transfers"))
            .and(basic_auth("test-api-id", "test-api-token"))
            .and(body_string_contains("\"user_id\":789"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = assign_call(
            ctx,
            AssignCallInput {
                call_id: 123_456,
                user_id: Some(789),
                team_id: None,
            },
        )
        .await
        .unwrap();

        assert!(output.assigned);
        assert_eq!(output.call_id, 123_456);
    }

    #[tokio::test]
    async fn test_assign_call_to_team_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("POST"))
            .and(path("/v1/calls/123456/transfers"))
            .and(body_string_contains("\"team_id\":999"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = assign_call(
            ctx,
            AssignCallInput {
                call_id: 123_456,
                user_id: None,
                team_id: Some(999),
            },
        )
        .await
        .unwrap();

        assert!(output.assigned);
    }

    #[tokio::test]
    async fn test_create_followup_success_returns_comment() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "comment": {
            "id": 7890,
            "content": "Follow-up needed",
            "posted_at": 1609459600,
            "posted_by": { "id": 1, "name": "Alice", "email": "alice@example.com" }
          }
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/v1/calls/123456/comments"))
            .and(basic_auth("test-api-id", "test-api-token"))
            .and(body_string_contains("\"content\":\"Follow-up needed\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = create_followup(
            ctx,
            CreateFollowupInput {
                call_id: 123_456,
                content: "Follow-up needed".to_string(),
            },
        )
        .await
        .unwrap();

        assert!(output.created);
        assert_eq!(output.call_id, 123_456);
        assert_eq!(output.comment.id, 7890);
        assert_eq!(output.comment.content, "Follow-up needed");
    }

    #[tokio::test]
    async fn test_fetch_recording_link_success_returns_urls() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "call": {
            "id": 123456,
            "direct_link": "https://app.aircall.io/calls/123456",
            "recording": "https://recordings.aircall.io/abc123.mp3",
            "recording_short_url": "https://aircall.io/r/xyz",
            "voicemail": "https://recordings.aircall.io/vm456.mp3",
            "voicemail_short_url": "https://aircall.io/v/abc",
            "asset": "https://app.aircall.io/assets/123456"
          }
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1/calls/123456"))
            .and(basic_auth("test-api-id", "test-api-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = fetch_recording_link(
            ctx,
            FetchRecordingLinkInput {
                call_id: 123_456,
                use_short_url: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.call_id, 123_456);
        assert_eq!(
            output.recording_url.as_deref(),
            Some("https://recordings.aircall.io/abc123.mp3")
        );
        assert_eq!(
            output.voicemail_url.as_deref(),
            Some("https://recordings.aircall.io/vm456.mp3")
        );
        assert_eq!(
            output.asset_url.as_deref(),
            Some("https://app.aircall.io/assets/123456")
        );
    }

    #[tokio::test]
    async fn test_fetch_recording_link_with_short_url_returns_short_urls() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "call": {
            "id": 123456,
            "direct_link": "https://app.aircall.io/calls/123456",
            "recording": "https://recordings.aircall.io/abc123.mp3",
            "recording_short_url": "https://aircall.io/r/xyz",
            "voicemail": "https://recordings.aircall.io/vm456.mp3",
            "voicemail_short_url": "https://aircall.io/v/abc",
            "asset": "https://app.aircall.io/assets/123456"
          }
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1/calls/123456"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = fetch_recording_link(
            ctx,
            FetchRecordingLinkInput {
                call_id: 123_456,
                use_short_url: true,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.call_id, 123_456);
        assert_eq!(
            output.recording_url.as_deref(),
            Some("https://aircall.io/r/xyz")
        );
        assert_eq!(
            output.voicemail_url.as_deref(),
            Some("https://aircall.io/v/abc")
        );
    }

    #[tokio::test]
    async fn test_fetch_recording_link_not_found_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/v1/calls/999999"))
            .respond_with(
                ResponseTemplate::new(404)
                    .set_body_raw(r#"{ "error": "Call not found" }"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = fetch_recording_link(
            ctx,
            FetchRecordingLinkInput {
                call_id: 999_999,
                use_short_url: false,
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("404"));
    }
}
