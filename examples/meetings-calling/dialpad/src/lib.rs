//! meetings-calling/dialpad integration for Operai Toolbox.

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

mod types;
use types::{CallListResponse, CallLog, CallRequest, CallResponse, SmsRequest, SmsResponse};

define_user_credential! {
    DialpadCredential("dialpad") {
        api_key: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_API_ENDPOINT: &str = "https://dialpad.com/api/v2";

/// Initializes the Dialpad integration.
///
/// # Errors
///
/// This function currently never returns an error, but the `Result` type is
/// required by the init macro for future extensibility.
#[init]
async fn setup() -> Result<()> {
    info!("Dialpad integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Dialpad integration shutting down");
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PlaceCallInput {
    /// The Dialpad user ID to initiate the call from.
    pub user_id: String,
    /// The phone number to call (E.164 format recommended, e.g.,
    /// "+15551234567").
    pub phone_number: String,
    /// Optional caller ID to use for the call (must be a Dialpad number you
    /// own).
    #[serde(default)]
    pub caller_id: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PlaceCallOutput {
    /// The URL of the initiated call.
    pub call_url: String,
}

/// # Place Dialpad Call
///
/// Initiates an outbound phone call using the Dialpad API. This tool causes the
/// specified user's Dialpad application (web, mobile, or desktop) to start
/// ringing and connect to the target phone number.
///
/// Use this tool when a user wants to make a phone call through their Dialpad
/// account. The call is placed from the user's Dialpad device, not directly
/// from the API. This is ideal for initiating calls that the user will then
/// answer and handle through their normal Dialpad interface.
///
/// **Key requirements:**
/// - The `user_id` must be a valid Dialpad user ID in your organization
/// - The `phone_number` should be in E.164 format (e.g., "+15551234567") for
///   best results, though other formats may be accepted
/// - The optional `caller_id` must be a phone number that your Dialpad account
///   owns and has configured for use
///
/// **Returns:** A URL to the initiated call that can be used to track or access
/// the call in the Dialpad interface.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - dialpad
/// - phone
/// - voice
///
/// # Errors
///
/// Returns an error if:
/// - `user_id` or `phone_number` is empty or contains only whitespace
/// - No Dialpad credentials are configured in the context
/// - The configured API endpoint URL is invalid
/// - The HTTP request to Dialpad API fails
/// - The Dialpad API returns a non-success status code
#[tool]
pub async fn place_call(ctx: Context, input: PlaceCallInput) -> Result<PlaceCallOutput> {
    ensure!(
        !input.user_id.trim().is_empty(),
        "user_id must not be empty"
    );
    ensure!(
        !input.phone_number.trim().is_empty(),
        "phone_number must not be empty"
    );

    let client = DialpadClient::from_ctx(&ctx)?;
    let request = CallRequest {
        phone_number: input.phone_number,
        caller_id: input.caller_id,
    };

    let response: CallResponse = client
        .post_json(
            client.url_with_path(&format!("/users/{}/initiate_call", input.user_id))?,
            &request,
            &[],
        )
        .await?;

    Ok(PlaceCallOutput {
        call_url: response.url,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SendSmsInput {
    /// The phone number or channel to send SMS to (E.164 format recommended for
    /// numbers).
    pub target: String,
    /// The message content to send.
    pub text: String,
    /// Optional user ID to send the SMS on behalf of. If not specified, uses
    /// default.
    #[serde(default)]
    pub user_id: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SendSmsOutput {
    /// Whether the SMS was successfully queued.
    pub success: bool,
    /// Unique identifier for the SMS message (if provided).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sms_id: Option<String>,
}

/// # Send Dialpad SMS
///
/// Sends an SMS/text message through the Dialpad API. This tool allows you to
/// send text messages to phone numbers or Dialpad channels on behalf of a user
/// in your organization.
///
/// Use this tool when a user wants to send a text message via their Dialpad
/// account. The message will be sent using the organization's Dialpad messaging
/// capabilities and will appear in the user's Dialpad message history.
///
/// **Key considerations:**
/// - The `target` can be a phone number (E.164 format recommended, e.g.,
///   "+15551234567") or a Dialpad channel ID
/// - The `text` field contains the message content to be sent
/// - The optional `user_id` specifies which Dialpad user sends the message; if
///   not provided, the default/authorized user will be used
/// - Message length limits and rate limiting may apply based on your Dialpad
///   plan and carrier restrictions
///
/// **Returns:** A success indicator and optional SMS message ID for tracking
/// the message delivery status.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - dialpad
/// - sms
/// - messaging
///
/// # Errors
///
/// Returns an error if:
/// - `target` or `text` is empty or contains only whitespace
/// - No Dialpad credentials are configured in the context
/// - The configured API endpoint URL is invalid
/// - The HTTP request to Dialpad API fails
/// - The Dialpad API returns a non-success status code
#[tool]
pub async fn send_sms(ctx: Context, input: SendSmsInput) -> Result<SendSmsOutput> {
    ensure!(!input.target.trim().is_empty(), "target must not be empty");
    ensure!(!input.text.trim().is_empty(), "text must not be empty");

    let client = DialpadClient::from_ctx(&ctx)?;
    let request = SmsRequest {
        target: input.target,
        text: input.text,
        user_id: input.user_id,
    };

    let response: SmsResponse = client
        .post_json(client.url_with_path("/sms")?, &request, &[])
        .await?;

    Ok(SendSmsOutput {
        success: response.success,
        sms_id: response.sms_id,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FetchCallLogsInput {
    /// Maximum number of call logs to retrieve (1-1000). Defaults to 100.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Start date for call logs (ISO 8601 format, e.g.,
    /// "2024-01-01T00:00:00Z").
    #[serde(default)]
    pub start_date: Option<String>,
    /// End date for call logs (ISO 8601 format).
    #[serde(default)]
    pub end_date: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FetchCallLogsOutput {
    pub call_logs: Vec<CallLog>,
    /// Whether there are more results available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_more: Option<bool>,
}

/// # Fetch Dialpad Call Logs
///
/// Retrieves call history and logs from the Dialpad API. This tool provides
/// access to detailed records of past phone calls that have already concluded,
/// including metadata like call duration, participants, timestamps, and status.
///
/// Use this tool when a user wants to review their call history, analyze call
/// patterns, retrieve information about past conversations, or generate reports
/// based on call activity.
///
/// **Key features:**
/// - Only returns calls that have already ended/completed; active/ongoing calls
///   are not included
/// - Supports filtering by date ranges using ISO 8601 timestamps (e.g.,
///   "2024-01-01T00:00:00Z")
/// - Configurable result limit (1-1000, default: 100) for controlling response
///   size
/// - Returns a `has_more` flag to indicate if additional results are available
///   beyond the current page
///
/// **Typical use cases:**
/// - Reviewing recent call activity
/// - Finding specific calls by date range
/// - Analyzing call duration and patterns
/// - Generating call history reports
///
/// **Returns:** An array of call log entries with details like call ID, phone
/// numbers involved, direction (inbound/outbound), duration, status, and start
/// time, plus a pagination indicator.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - dialpad
/// - phone
/// - history
///
/// # Errors
///
/// Returns an error if:
/// - `limit` is not between 1 and 1000 (inclusive)
/// - No Dialpad credentials are configured in the context
/// - The configured API endpoint URL is invalid
/// - The HTTP request to Dialpad API fails
/// - The Dialpad API returns a non-success status code
#[tool]
pub async fn fetch_call_logs(
    ctx: Context,
    input: FetchCallLogsInput,
) -> Result<FetchCallLogsOutput> {
    let limit = input.limit.unwrap_or(100);
    ensure!(
        (1..=1000).contains(&limit),
        "limit must be between 1 and 1000"
    );

    let client = DialpadClient::from_ctx(&ctx)?;

    let mut query = vec![("limit", limit.to_string())];
    if let Some(start) = input.start_date {
        query.push(("start_time", start));
    }
    if let Some(end) = input.end_date {
        query.push(("end_time", end));
    }

    let response: CallListResponse = client
        .get_json(client.url_with_path("/call")?, &query, &[])
        .await?;

    Ok(FetchCallLogsOutput {
        call_logs: response.items,
        has_more: response.has_more,
    })
}

#[derive(Debug, Clone)]
struct DialpadClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl DialpadClient {
    /// Creates a new `DialpadClient` from the provided context.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No Dialpad credentials are configured in the context
    /// - The `api_key` in credentials is empty or contains only whitespace
    /// - The `endpoint` in credentials (if provided) is empty after trimming
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = DialpadCredential::get(ctx)?;
        ensure!(!cred.api_key.trim().is_empty(), "api_key must not be empty");

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_API_ENDPOINT))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            api_key: cred.api_key,
        })
    }

    /// Constructs a full URL by combining the base URL with the given path.
    ///
    /// # Errors
    ///
    /// Returns an error if the resulting URL string is not a valid URL.
    fn url_with_path(&self, path: &str) -> Result<reqwest::Url> {
        let url_string = format!("{}{}", self.base_url, path);
        Ok(reqwest::Url::parse(&url_string)?)
    }

    /// Sends a GET request and deserializes the response body as JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails (network error, timeout, etc.)
    /// - The server returns a non-success status code
    /// - The response body cannot be deserialized into type `T`
    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        query: &[(&str, String)],
        extra_headers: &[(&str, &str)],
    ) -> Result<T> {
        let mut request = self.http.get(url).query(query);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        let response = self.send_request(request).await?;
        Ok(response.json::<T>().await?)
    }

    /// Sends a POST request with a JSON body and deserializes the response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The request body cannot be serialized to JSON
    /// - The HTTP request fails (network error, timeout, etc.)
    /// - The server returns a non-success status code
    /// - The response body cannot be deserialized into type `TRes`
    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
        extra_headers: &[(&str, &str)],
    ) -> Result<TRes> {
        let mut request = self.http.post(url).json(body);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        let response = self.send_request(request).await?;
        Ok(response.json::<TRes>().await?)
    }

    /// Sends an HTTP request with authentication and standard headers.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails (network error, timeout, etc.)
    /// - The server returns a non-success status code
    async fn send_request(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let response = request
            .bearer_auth(&self.api_key)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Dialpad API request failed ({status}): {body}"
            ))
        }
    }
}

/// Normalizes an API endpoint URL by trimming whitespace and trailing slashes.
///
/// # Errors
///
/// Returns an error if the endpoint is empty after trimming whitespace.
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

    fn test_ctx(endpoint: &str) -> Context {
        let mut dialpad_values = HashMap::new();
        dialpad_values.insert("api_key".to_string(), "test-api-key".to_string());
        dialpad_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("dialpad", dialpad_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        format!("{}/api/v2", server.uri())
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_call_request_serialization_roundtrip() {
        let request = CallRequest {
            phone_number: "+15551234567".to_string(),
            caller_id: Some("+15559876543".to_string()),
        };
        let json = serde_json::to_string(&request).unwrap();
        let parsed: CallRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request.phone_number, parsed.phone_number);
        assert_eq!(request.caller_id, parsed.caller_id);
    }

    #[test]
    fn test_sms_request_serialization_roundtrip() {
        let request = SmsRequest {
            target: "+15551234567".to_string(),
            text: "Hello".to_string(),
            user_id: Some("user-123".to_string()),
        };
        let json = serde_json::to_string(&request).unwrap();
        let parsed: SmsRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(request.target, parsed.target);
        assert_eq!(request.text, parsed.text);
        assert_eq!(request.user_id, parsed.user_id);
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://dialpad.com/api/v2/").unwrap();
        assert_eq!(result, "https://dialpad.com/api/v2");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://dialpad.com/api/v2  ").unwrap();
        assert_eq!(result, "https://dialpad.com/api/v2");
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
    async fn test_place_call_empty_user_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = place_call(
            ctx,
            PlaceCallInput {
                user_id: "   ".to_string(),
                phone_number: "+15551234567".to_string(),
                caller_id: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("user_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_place_call_empty_phone_number_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = place_call(
            ctx,
            PlaceCallInput {
                user_id: "user-123".to_string(),
                phone_number: "  ".to_string(),
                caller_id: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("phone_number must not be empty")
        );
    }

    #[tokio::test]
    async fn test_send_sms_empty_target_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = send_sms(
            ctx,
            SendSmsInput {
                target: "  ".to_string(),
                text: "Hello".to_string(),
                user_id: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("target must not be empty")
        );
    }

    #[tokio::test]
    async fn test_send_sms_empty_text_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = send_sms(
            ctx,
            SendSmsInput {
                target: "+15551234567".to_string(),
                text: "  ".to_string(),
                user_id: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("text must not be empty")
        );
    }

    #[tokio::test]
    async fn test_fetch_call_logs_limit_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = fetch_call_logs(
            ctx,
            FetchCallLogsInput {
                limit: Some(0),
                start_date: None,
                end_date: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("limit must be between 1 and 1000")
        );
    }

    #[tokio::test]
    async fn test_fetch_call_logs_limit_exceeds_max_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = fetch_call_logs(
            ctx,
            FetchCallLogsInput {
                limit: Some(1001),
                start_date: None,
                end_date: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("limit must be between 1 and 1000")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_place_call_success_returns_url() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"{
            "url": "https://dialpad.com/call/123"
        }"#;

        Mock::given(method("POST"))
            .and(path("/api/v2/users/user-123/initiate_call"))
            .and(header("authorization", "Bearer test-api-key"))
            .and(body_string_contains("\"phone_number\":\"+15551234567\""))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = place_call(
            ctx,
            PlaceCallInput {
                user_id: "user-123".to_string(),
                phone_number: "+15551234567".to_string(),
                caller_id: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.call_url, "https://dialpad.com/call/123");
    }

    #[tokio::test]
    async fn test_place_call_error_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("POST"))
            .and(path("/api/v2/users/user-123/initiate_call"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_raw(r#"{"error": "Invalid phone number"}"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = place_call(
            ctx,
            PlaceCallInput {
                user_id: "user-123".to_string(),
                phone_number: "invalid".to_string(),
                caller_id: None,
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("400"));
    }

    #[tokio::test]
    async fn test_send_sms_success_returns_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"{
            "success": true,
            "sms_id": "msg-456"
        }"#;

        Mock::given(method("POST"))
            .and(path("/api/v2/sms"))
            .and(header("authorization", "Bearer test-api-key"))
            .and(body_string_contains("\"target\":\"+15551234567\""))
            .and(body_string_contains("\"text\":\"Hello\""))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = send_sms(
            ctx,
            SendSmsInput {
                target: "+15551234567".to_string(),
                text: "Hello".to_string(),
                user_id: None,
            },
        )
        .await
        .unwrap();

        assert!(output.success);
        assert_eq!(output.sms_id.as_deref(), Some("msg-456"));
    }

    #[tokio::test]
    async fn test_fetch_call_logs_success_returns_logs() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"{
            "items": [
                {
                    "call_id": "call-1",
                    "from_number": "+15559876543",
                    "to_number": "+15551234567",
                    "direction": "outbound",
                    "duration": 120,
                    "status": "completed",
                    "start_time": "2024-01-01T10:00:00Z"
                }
            ],
            "has_more": false
        }"#;

        Mock::given(method("GET"))
            .and(path("/api/v2/call"))
            .and(query_param("limit", "100"))
            .and(header("authorization", "Bearer test-api-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = fetch_call_logs(
            ctx,
            FetchCallLogsInput {
                limit: None,
                start_date: None,
                end_date: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.call_logs.len(), 1);
        assert_eq!(output.call_logs[0].call_id, "call-1");
        assert_eq!(output.call_logs[0].duration, Some(120));
        assert_eq!(output.has_more, Some(false));
    }
}
