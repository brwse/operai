//! calendars-scheduling/doodle integration for Operai Toolbox.
//!
//! This is a mock implementation since Doodle's public API was discontinued.
//! The implementation follows the operai patterns and can be adapted
//! when/if Doodle provides API access for Enterprise customers.

mod types;

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
pub use types::{Participant, Poll, PollOption, PollSummary, Vote, VoteType};

define_user_credential! {
    DoodleCredential("doodle") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_DOODLE_ENDPOINT: &str = "https://doodle.com/api/v2.0";

#[init]
async fn setup() -> Result<()> {
    info!("Doodle integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Doodle integration shutting down");
}

// ============================================================================
// Tool: create_poll
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreatePollInput {
    /// Title of the poll.
    pub title: String,
    /// Optional description of the poll.
    #[serde(default)]
    pub description: Option<String>,
    /// Optional location for the event.
    #[serde(default)]
    pub location: Option<String>,
    /// Poll options (e.g., dates/times or text options).
    pub options: Vec<PollOptionInput>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PollOptionInput {
    /// Text description of the option.
    pub text: String,
    /// Optional start time (ISO 8601 format).
    #[serde(default)]
    pub start_time: Option<String>,
    /// Optional end time (ISO 8601 format).
    #[serde(default)]
    pub end_time: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreatePollOutput {
    pub poll_id: String,
    pub poll_url: String,
}

/// # Create Doodle Poll
///
/// Creates a new scheduling poll using the Doodle API.
///
/// Use this tool when a user wants to:
/// - Schedule a meeting or event and let participants vote on preferred time
///   slots
/// - Create a poll with multiple date/time options for attendees to choose from
/// - Set up a group scheduling decision with optional location information
///
/// The tool returns a poll URL that can be shared with participants to collect
/// their availability preferences.
///
/// ## Key Requirements
/// - **Title**: Must be non-empty and describes the event/meeting
/// - **Options**: Must contain at least one option (each option must have
///   non-empty text)
/// - **Optional**: Description (additional context), Location (physical or
///   virtual venue)
/// - **Option metadata**: Can include start/end times in ISO 8601 format for
///   time-aware options
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - doodle
/// - scheduling
/// - poll
///
/// # Errors
///
/// This function will return an error if:
/// - The poll title is empty or contains only whitespace
/// - The options list is empty
/// - Any option text is empty or contains only whitespace
/// - The Doodle credentials (`access_token`) are missing or invalid
/// - The Doodle API request fails (network errors, rate limiting, etc.)
/// - The Doodle API returns a non-success status code
#[tool]
pub async fn create_poll(ctx: Context, input: CreatePollInput) -> Result<CreatePollOutput> {
    ensure!(!input.title.trim().is_empty(), "title must not be empty");
    ensure!(
        !input.options.is_empty(),
        "options must contain at least one option"
    );
    for (idx, option) in input.options.iter().enumerate() {
        ensure!(
            !option.text.trim().is_empty(),
            "option {idx} text must not be empty"
        );
    }

    let client = DoodleClient::from_ctx(&ctx)?;

    let request = DoodleCreatePollRequest {
        title: input.title,
        description: input.description,
        location: input.location,
        options: input
            .options
            .into_iter()
            .map(|opt| DoodlePollOption {
                text: opt.text,
                start_time: opt.start_time,
                end_time: opt.end_time,
            })
            .collect(),
    };

    let response: DoodleCreatePollResponse = client
        .post_json(client.url_with_segments(&["polls"])?, &request, &[])
        .await?;

    Ok(CreatePollOutput {
        poll_id: response.id.clone(),
        poll_url: format!("https://doodle.com/poll/{}", response.id),
    })
}

// ============================================================================
// Tool: list_votes
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListVotesInput {
    /// Doodle poll ID.
    pub poll_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListVotesOutput {
    pub votes: Vec<Vote>,
    pub poll_title: String,
}

/// # List Doodle Poll Votes
///
/// Retrieves all votes and participant preferences from a Doodle scheduling
/// poll.
///
/// Use this tool when a user wants to:
/// - Check the current voting status of a poll they created
/// - See which participants have voted and their preferences
///   (yes/no/if-need-be)
/// - Analyze the results before making a final decision on meeting time
/// - Get a summary of all votes to determine the most popular option
///
/// This tool extracts and returns vote information from all participants who
/// have responded to the poll, showing their preferences for each option.
///
/// ## Key Requirements
/// - **poll_id**: The Doodle poll identifier (must be non-empty)
///
/// ## Output
/// - Returns the poll title along with all votes
/// - Each vote includes: participant name, option ID, and vote type
///   (Yes/No/IfNeedBe)
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - doodle
/// - scheduling
/// - poll
/// - votes
///
/// # Errors
///
/// This function will return an error if:
/// - The `poll_id` is empty or contains only whitespace
/// - The Doodle credentials (`access_token`) are missing or invalid
/// - The Doodle API request fails (network errors, rate limiting, etc.)
/// - The Doodle API returns a non-success status code (e.g., 404 if poll
///   doesn't exist)
/// - The API response cannot be parsed as expected JSON structure
#[tool]
pub async fn list_votes(ctx: Context, input: ListVotesInput) -> Result<ListVotesOutput> {
    ensure!(
        !input.poll_id.trim().is_empty(),
        "poll_id must not be empty"
    );

    let client = DoodleClient::from_ctx(&ctx)?;

    let poll: DoodlePoll = client
        .get_json(
            client.url_with_segments(&["polls", input.poll_id.as_str()])?,
            &[],
            &[],
        )
        .await?;

    Ok(ListVotesOutput {
        votes: poll
            .participants
            .into_iter()
            .flat_map(|p| {
                p.preferences.into_iter().map(move |pref| Vote {
                    participant_name: p.name.clone(),
                    option_id: pref.option_id,
                    vote_type: pref.vote_type,
                })
            })
            .collect(),
        poll_title: poll.title,
    })
}

// ============================================================================
// Tool: close_poll
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClosePollInput {
    /// Doodle poll ID to close.
    pub poll_id: String,
    /// Optional: ID of the selected option (if finalizing a choice).
    #[serde(default)]
    pub selected_option_id: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ClosePollOutput {
    pub poll_id: String,
    pub closed: bool,
    #[serde(default)]
    pub selected_option_id: Option<String>,
}

/// # Close Doodle Poll
///
/// Closes a Doodle scheduling poll to prevent further votes and optionally
/// finalizes the chosen option.
///
/// Use this tool when a user wants to:
/// - End the voting period for a poll after enough participants have responded
/// - Lock in the final meeting time by selecting the winning option
/// - Prevent additional changes to the poll results
/// - Close a poll without necessarily choosing an option (just stop voting)
///
/// Once closed, participants can no longer submit or modify their votes.
///
/// ## Key Requirements
/// - **poll_id**: The Doodle poll identifier to close (must be non-empty)
/// - **`selected_option_id`** (optional): The ID of the chosen option to
///   finalize
///   - If provided, indicates the final decision (e.g., "Monday 3pm")
///   - If omitted, the poll closes without selecting a winner
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - doodle
/// - scheduling
/// - poll
///
/// # Errors
///
/// This function will return an error if:
/// - The `poll_id` is empty or contains only whitespace
/// - The Doodle credentials (`access_token`) are missing or invalid
/// - The Doodle API request fails (network errors, rate limiting, etc.)
/// - The Doodle API returns a non-success status code (e.g., 404 if poll
///   doesn't exist)
#[tool]
pub async fn close_poll(ctx: Context, input: ClosePollInput) -> Result<ClosePollOutput> {
    ensure!(
        !input.poll_id.trim().is_empty(),
        "poll_id must not be empty"
    );

    let client = DoodleClient::from_ctx(&ctx)?;

    let request = DoodleClosePollRequest {
        closed: true,
        selected_option_id: input.selected_option_id.clone(),
    };

    client
        .patch_empty(
            client.url_with_segments(&["polls", input.poll_id.as_str()])?,
            &request,
            &[],
        )
        .await?;

    Ok(ClosePollOutput {
        poll_id: input.poll_id,
        closed: true,
        selected_option_id: input.selected_option_id,
    })
}

// ============================================================================
// Tool: notify_participants
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NotifyParticipantsInput {
    /// Doodle poll ID.
    pub poll_id: String,
    /// Email addresses of participants to notify.
    pub emails: Vec<String>,
    /// Optional custom message to include in the notification.
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct NotifyParticipantsOutput {
    pub poll_id: String,
    pub notified_count: usize,
}

/// # Notify Doodle Participants
///
/// Sends email notifications to specific participants about a Doodle scheduling
/// poll.
///
/// Use this tool when a user wants to:
/// - Remind participants to vote if they haven't responded yet
/// - Notify participants that the poll status has changed (e.g., poll closed)
/// - Send a follow-up message to specific attendees about the scheduling poll
/// - Alert participants that a decision has been made and the poll is closing
///
/// This triggers email notifications through Doodle's system to the specified
/// recipients.
///
/// ## Key Requirements
/// - **poll_id**: The Doodle poll identifier (must be non-empty)
/// - **emails**: List of recipient email addresses (must contain at least one)
/// - **message** (optional): Custom message to include in the notification body
///
/// ## Notes
/// - Emails are validated to be non-empty (basic format validation is the
///   caller's responsibility)
/// - The notification is sent immediately to all specified recipients
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - doodle
/// - scheduling
/// - poll
/// - notification
///
/// # Errors
///
/// This function will return an error if:
/// - The `poll_id` is empty or contains only whitespace
/// - The emails list is empty
/// - Any email address is empty or contains only whitespace
/// - The Doodle credentials (`access_token`) are missing or invalid
/// - The Doodle API request fails (network errors, rate limiting, etc.)
/// - The Doodle API returns a non-success status code (e.g., 404 if poll
///   doesn't exist)
#[tool]
pub async fn notify_participants(
    ctx: Context,
    input: NotifyParticipantsInput,
) -> Result<NotifyParticipantsOutput> {
    ensure!(
        !input.poll_id.trim().is_empty(),
        "poll_id must not be empty"
    );
    ensure!(
        !input.emails.is_empty(),
        "emails must contain at least one recipient"
    );
    for email in &input.emails {
        ensure!(!email.trim().is_empty(), "email must not be empty");
    }

    let client = DoodleClient::from_ctx(&ctx)?;

    let request = DoodleNotifyRequest {
        emails: input.emails.clone(),
        message: input.message,
    };

    client
        .post_empty(
            client.url_with_segments(&["polls", input.poll_id.as_str(), "notify"])?,
            &request,
            &[],
        )
        .await?;

    Ok(NotifyParticipantsOutput {
        poll_id: input.poll_id,
        notified_count: input.emails.len(),
    })
}

// ============================================================================
// Internal API types and client
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoodleCreatePollRequest {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<String>,
    options: Vec<DoodlePollOption>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DoodlePollOption {
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_time: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DoodleCreatePollResponse {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DoodlePoll {
    title: String,
    #[serde(default)]
    participants: Vec<DoodleParticipant>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DoodleParticipant {
    name: String,
    #[serde(default)]
    preferences: Vec<DoodlePreference>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DoodlePreference {
    option_id: String,
    #[serde(rename = "type")]
    vote_type: VoteType,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoodleClosePollRequest {
    closed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    selected_option_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoodleNotifyRequest {
    emails: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Debug, Clone)]
struct DoodleClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl DoodleClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = DoodleCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_DOODLE_ENDPOINT))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
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
        extra_headers: &[(&str, &str)],
    ) -> Result<T> {
        let mut request = self.http.get(url).query(query);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        let response = self.send_request(request).await?;
        Ok(response.json::<T>().await?)
    }

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

    async fn post_empty<TReq: Serialize>(
        &self,
        url: reqwest::Url,
        body: &TReq,
        extra_headers: &[(&str, &str)],
    ) -> Result<()> {
        let mut request = self.http.post(url).json(body);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        self.send_request(request).await?;
        Ok(())
    }

    async fn patch_empty<TReq: Serialize>(
        &self,
        url: reqwest::Url,
        body: &TReq,
        extra_headers: &[(&str, &str)],
    ) -> Result<()> {
        let mut request = self.http.patch(url).json(body);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        self.send_request(request).await?;
        Ok(())
    }

    async fn send_request(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let response = request
            .bearer_auth(&self.access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Doodle API request failed ({status}): {body}"
            ))
        }
    }
}

fn normalize_base_url(endpoint: &str) -> Result<String> {
    let trimmed = endpoint.trim();
    ensure!(!trimmed.is_empty(), "endpoint must not be empty");
    Ok(trimmed.trim_end_matches('/').to_string())
}

operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_string_contains, header, method, path},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut doodle_values = HashMap::new();
        doodle_values.insert("access_token".to_string(), "test-token".to_string());
        doodle_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("doodle", doodle_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        format!("{}/v2.0", server.uri())
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_vote_type_serialization_roundtrip() {
        for variant in [VoteType::Yes, VoteType::No, VoteType::IfNeedBe] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: VoteType = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_poll_option_serialization_roundtrip() {
        let option = PollOption {
            option_id: "opt-1".to_string(),
            text: "Monday 3pm".to_string(),
            start_time: Some("2024-01-01T15:00:00Z".to_string()),
            end_time: Some("2024-01-01T16:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&option).unwrap();
        let parsed: PollOption = serde_json::from_str(&json).unwrap();
        assert_eq!(option.option_id, parsed.option_id);
        assert_eq!(option.text, parsed.text);
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://doodle.com/api/").unwrap();
        assert_eq!(result, "https://doodle.com/api");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://doodle.com/api  ").unwrap();
        assert_eq!(result, "https://doodle.com/api");
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

    // --- DoodleClient::url_with_segments tests ---

    #[test]
    fn test_url_with_segments_single_segment() {
        let client = DoodleClient {
            http: reqwest::Client::new(),
            base_url: "https://doodle.com/api/v2.0".to_string(),
            access_token: "test-token".to_string(),
        };

        let url = client.url_with_segments(&["polls"]).unwrap();
        assert_eq!(url.as_str(), "https://doodle.com/api/v2.0/polls");
    }

    #[test]
    fn test_url_with_segments_multiple_segments() {
        let client = DoodleClient {
            http: reqwest::Client::new(),
            base_url: "https://doodle.com/api/v2.0".to_string(),
            access_token: "test-token".to_string(),
        };

        let url = client
            .url_with_segments(&["polls", "abc123", "notify"])
            .unwrap();
        assert_eq!(
            url.as_str(),
            "https://doodle.com/api/v2.0/polls/abc123/notify"
        );
    }

    #[test]
    fn test_url_with_segments_preserves_query_params() {
        let client = DoodleClient {
            http: reqwest::Client::new(),
            base_url: "https://doodle.com/api/v2.0?existing=param".to_string(),
            access_token: "test-token".to_string(),
        };

        let url = client.url_with_segments(&["polls"]).unwrap();
        // reqwest::Url properly reorders query params to the end
        assert_eq!(
            url.as_str(),
            "https://doodle.com/api/v2.0/polls?existing=param"
        );
    }

    // --- Input validation tests ---

    #[tokio::test]
    async fn test_create_poll_empty_access_token_returns_error() {
        let server = MockServer::start().await;

        let mut doodle_values = HashMap::new();
        doodle_values.insert("access_token".to_string(), "  ".to_string()); // Empty/whitespace token
        doodle_values.insert("endpoint".to_string(), endpoint_for(&server));

        let ctx = Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("doodle", doodle_values);

        let result = create_poll(
            ctx,
            CreatePollInput {
                title: "Meeting".to_string(),
                description: None,
                location: None,
                options: vec![PollOptionInput {
                    text: "Option 1".to_string(),
                    start_time: None,
                    end_time: None,
                }],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("access_token must not be empty")
        );
    }

    #[tokio::test]
    async fn test_create_poll_empty_title_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = create_poll(
            ctx,
            CreatePollInput {
                title: "   ".to_string(),
                description: None,
                location: None,
                options: vec![PollOptionInput {
                    text: "Option 1".to_string(),
                    start_time: None,
                    end_time: None,
                }],
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
    async fn test_create_poll_empty_options_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = create_poll(
            ctx,
            CreatePollInput {
                title: "Meeting".to_string(),
                description: None,
                location: None,
                options: vec![],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("options must contain at least one option")
        );
    }

    #[tokio::test]
    async fn test_create_poll_empty_option_text_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = create_poll(
            ctx,
            CreatePollInput {
                title: "Meeting".to_string(),
                description: None,
                location: None,
                options: vec![PollOptionInput {
                    text: "  ".to_string(),
                    start_time: None,
                    end_time: None,
                }],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("option 0 text must not be empty")
        );
    }

    #[tokio::test]
    async fn test_list_votes_empty_poll_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_votes(
            ctx,
            ListVotesInput {
                poll_id: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("poll_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_close_poll_empty_poll_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = close_poll(
            ctx,
            ClosePollInput {
                poll_id: "  ".to_string(),
                selected_option_id: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("poll_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_notify_participants_empty_poll_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = notify_participants(
            ctx,
            NotifyParticipantsInput {
                poll_id: "  ".to_string(),
                emails: vec!["test@example.com".to_string()],
                message: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("poll_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_notify_participants_empty_emails_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = notify_participants(
            ctx,
            NotifyParticipantsInput {
                poll_id: "poll-1".to_string(),
                emails: vec![],
                message: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("emails must contain at least one recipient")
        );
    }

    #[tokio::test]
    async fn test_notify_participants_empty_email_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = notify_participants(
            ctx,
            NotifyParticipantsInput {
                poll_id: "poll-1".to_string(),
                emails: vec!["  ".to_string()],
                message: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("email must not be empty")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_create_poll_success_returns_poll_id() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"{ "id": "abc123" }"#;

        Mock::given(method("POST"))
            .and(path("/v2.0/polls"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("\"title\":\"Team Meeting\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = create_poll(
            ctx,
            CreatePollInput {
                title: "Team Meeting".to_string(),
                description: Some("Weekly sync".to_string()),
                location: Some("Conference Room A".to_string()),
                options: vec![
                    PollOptionInput {
                        text: "Monday 3pm".to_string(),
                        start_time: Some("2024-01-01T15:00:00Z".to_string()),
                        end_time: Some("2024-01-01T16:00:00Z".to_string()),
                    },
                    PollOptionInput {
                        text: "Tuesday 2pm".to_string(),
                        start_time: Some("2024-01-02T14:00:00Z".to_string()),
                        end_time: Some("2024-01-02T15:00:00Z".to_string()),
                    },
                ],
            },
        )
        .await
        .unwrap();

        assert_eq!(output.poll_id, "abc123");
        assert_eq!(output.poll_url, "https://doodle.com/poll/abc123");
    }

    #[tokio::test]
    async fn test_create_poll_api_error_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("POST"))
            .and(path("/v2.0/polls"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_raw(r#"{ "error": "Invalid request" }"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = create_poll(
            ctx,
            CreatePollInput {
                title: "Meeting".to_string(),
                description: None,
                location: None,
                options: vec![PollOptionInput {
                    text: "Option 1".to_string(),
                    start_time: None,
                    end_time: None,
                }],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("400"));
    }

    #[tokio::test]
    async fn test_list_votes_success_returns_votes() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "poll-123",
          "title": "Team Meeting",
          "options": [
            { "id": "opt-1", "text": "Monday" },
            { "id": "opt-2", "text": "Tuesday" }
          ],
          "participants": [
            {
              "name": "Alice",
              "preferences": [
                { "optionId": "opt-1", "type": "yes" },
                { "optionId": "opt-2", "type": "ifneedbe" }
              ]
            },
            {
              "name": "Bob",
              "preferences": [
                { "optionId": "opt-1", "type": "no" },
                { "optionId": "opt-2", "type": "yes" }
              ]
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v2.0/polls/poll-123"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_votes(
            ctx,
            ListVotesInput {
                poll_id: "poll-123".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.poll_title, "Team Meeting");
        assert_eq!(output.votes.len(), 4);
        assert_eq!(output.votes[0].participant_name, "Alice");
        assert_eq!(output.votes[0].option_id, "opt-1");
        assert_eq!(output.votes[0].vote_type, VoteType::Yes);
    }

    #[tokio::test]
    async fn test_list_votes_not_found_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/v2.0/polls/missing"))
            .respond_with(
                ResponseTemplate::new(404)
                    .set_body_raw(r#"{ "error": "Poll not found" }"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = list_votes(
            ctx,
            ListVotesInput {
                poll_id: "missing".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("404"));
    }

    #[tokio::test]
    async fn test_close_poll_success_returns_closed() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("PATCH"))
            .and(path("/v2.0/polls/poll-123"))
            .and(body_string_contains("\"closed\":true"))
            .and(body_string_contains("\"selectedOptionId\":\"opt-1\""))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = close_poll(
            ctx,
            ClosePollInput {
                poll_id: "poll-123".to_string(),
                selected_option_id: Some("opt-1".to_string()),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.poll_id, "poll-123");
        assert!(output.closed);
        assert_eq!(output.selected_option_id, Some("opt-1".to_string()));
    }

    #[tokio::test]
    async fn test_close_poll_without_selection_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("PATCH"))
            .and(path("/v2.0/polls/poll-123"))
            .and(body_string_contains("\"closed\":true"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = close_poll(
            ctx,
            ClosePollInput {
                poll_id: "poll-123".to_string(),
                selected_option_id: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.poll_id, "poll-123");
        assert!(output.closed);
        assert!(output.selected_option_id.is_none());
    }

    #[tokio::test]
    async fn test_notify_participants_success_returns_count() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("POST"))
            .and(path("/v2.0/polls/poll-123/notify"))
            .and(body_string_contains("\"emails\":["))
            .and(body_string_contains("alice@example.com"))
            .and(body_string_contains("bob@example.com"))
            .respond_with(ResponseTemplate::new(202))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = notify_participants(
            ctx,
            NotifyParticipantsInput {
                poll_id: "poll-123".to_string(),
                emails: vec![
                    "alice@example.com".to_string(),
                    "bob@example.com".to_string(),
                ],
                message: Some("Please vote!".to_string()),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.poll_id, "poll-123");
        assert_eq!(output.notified_count, 2);
    }

    #[tokio::test]
    async fn test_notify_participants_api_error_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("POST"))
            .and(path("/v2.0/polls/poll-123/notify"))
            .respond_with(ResponseTemplate::new(500).set_body_raw(
                r#"{ "error": "Internal server error" }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = notify_participants(
            ctx,
            NotifyParticipantsInput {
                poll_id: "poll-123".to_string(),
                emails: vec!["test@example.com".to_string()],
                message: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("500"));
    }
}
