//! team-chat/microsoft-teams integration for Operai Toolbox.

mod types;

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
use types::{BodyContentType, Channel, ChatMessage, ItemBody, Team};

define_user_credential! {
    TeamsCredential("microsoft_teams") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_GRAPH_ENDPOINT: &str = "https://graph.microsoft.com/v1.0";

#[init]
async fn setup() -> Result<()> {
    info!("Microsoft Teams integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Microsoft Teams integration shutting down");
}

// ============================================================================
// Tool 1: List Teams and Channels
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTeamsChannelsInput {
    /// Optional team ID to list channels for a specific team.
    /// If not provided, lists all teams the user is a member of.
    #[serde(default)]
    pub team_id: Option<String>,
    /// Maximum number of results (1-50). Defaults to 10.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListTeamsChannelsOutput {
    #[serde(default)]
    pub teams: Vec<Team>,
    #[serde(default)]
    pub channels: Vec<Channel>,
}

/// # List Microsoft Teams
///
/// Retrieves Microsoft Teams teams and channels that the authenticated user has
/// access to. Use this tool when the user wants to explore their Teams
/// workspace structure, discover available teams and channels, or obtain
/// team/channel IDs for subsequent operations.
///
/// This tool operates in two modes:
/// - **Without `team_id`**: Lists all teams the user is a member of, returning
///   team names, descriptions, and IDs
/// - **With `team_id`**: Lists all channels within a specific team, returning
///   channel names, descriptions, web URLs, and IDs
///
/// The returned IDs are essential inputs for other Teams operations like
/// posting messages, reading messages, or replying to conversations. Results
/// are paginated with configurable limits (1-50, default 10).
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - teams
/// - microsoft-teams
/// - microsoft-graph
/// - chat
///
/// # Errors
///
/// Returns an error if:
/// - The `limit` parameter is not between 1 and 50
/// - The provided `team_id` is empty
/// - No Microsoft Teams credentials are configured in the context
/// - The configured access token is empty
/// - The configured endpoint is not a valid absolute URL
/// - The Microsoft Graph API request fails (network errors, authentication
///   failures, etc.)
/// - The API response cannot be parsed as valid JSON
#[tool]
pub async fn list_teams_channels(
    ctx: Context,
    input: ListTeamsChannelsInput,
) -> Result<ListTeamsChannelsOutput> {
    let limit = input.limit.unwrap_or(10);
    ensure!((1..=50).contains(&limit), "limit must be between 1 and 50");

    let client = GraphClient::from_ctx(&ctx)?;

    if let Some(team_id) = input.team_id {
        ensure!(!team_id.trim().is_empty(), "team_id must not be empty");

        // List channels for a specific team
        let query = [("$top", limit.to_string())];
        let response: GraphListResponse<GraphChannel> = client
            .get_json(
                client.url_with_segments(&["teams", team_id.as_str(), "channels"])?,
                &query,
                &[],
            )
            .await?;

        Ok(ListTeamsChannelsOutput {
            teams: vec![],
            channels: response.value.into_iter().map(map_channel).collect(),
        })
    } else {
        // List all teams
        let query = [("$top", limit.to_string())];
        let response: GraphListResponse<GraphTeam> = client
            .get_json(
                client.url_with_segments(&["me", "joinedTeams"])?,
                &query,
                &[],
            )
            .await?;

        Ok(ListTeamsChannelsOutput {
            teams: response.value.into_iter().map(map_team).collect(),
            channels: vec![],
        })
    }
}

// ============================================================================
// Tool 2: Post Message
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PostMessageInput {
    /// Team ID where the channel is located.
    pub team_id: String,
    /// Channel ID to post the message to.
    pub channel_id: String,
    /// Message content.
    pub content: String,
    /// Content type ("text" or "html"). Defaults to "text".
    #[serde(default)]
    pub content_type: Option<BodyContentType>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PostMessageOutput {
    pub message_id: String,
    pub created_date_time: Option<String>,
}

/// # Send Microsoft Teams Message
///
/// Sends a new message to a Microsoft Teams channel on behalf of the
/// authenticated user. Use this tool when the user wants to send a message to a
/// Teams channel, broadcast an announcement, or initiate a conversation in a
/// team channel.
///
/// The message can be formatted as plain text or HTML. Messages are posted
/// immediately and will be visible to all members of the channel. The response
/// includes the created message ID and timestamp for reference.
///
/// Required inputs:
/// - **team_id**: The unique identifier of the team (obtain from
///   list_teams_channels)
/// - **channel_id**: The unique identifier of the channel (obtain from
///   list_teams_channels)
/// - **content**: The message content to post (must not be empty)
///
/// Optional inputs:
/// - **`content_type`**: Format of the message ("text" or "html", defaults to
///   "text")
///
/// Note: This requires the authenticated user to have write permissions for the
/// target channel. Use with caution as messages cannot be deleted through this
/// API.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - teams
/// - microsoft-teams
/// - microsoft-graph
/// - chat
///
/// # Errors
///
/// Returns an error if:
/// - The `team_id` parameter is empty or contains only whitespace
/// - The `channel_id` parameter is empty or contains only whitespace
/// - The `content` parameter is empty or contains only whitespace
/// - No Microsoft Teams credentials are configured in the context
/// - The configured access token is empty
/// - The configured endpoint is not a valid absolute URL
/// - The Microsoft Graph API request fails (network errors, authentication
///   failures, insufficient permissions, etc.)
/// - The API response cannot be parsed as valid JSON
#[tool]
pub async fn post_message(ctx: Context, input: PostMessageInput) -> Result<PostMessageOutput> {
    ensure!(
        !input.team_id.trim().is_empty(),
        "team_id must not be empty"
    );
    ensure!(
        !input.channel_id.trim().is_empty(),
        "channel_id must not be empty"
    );
    ensure!(
        !input.content.trim().is_empty(),
        "content must not be empty"
    );

    let content_type = input.content_type.unwrap_or(BodyContentType::Text);
    let client = GraphClient::from_ctx(&ctx)?;

    let request = GraphPostMessageRequest {
        body: GraphItemBody {
            content_type,
            content: input.content,
        },
    };

    let response: GraphChatMessage = client
        .post_json(
            client.url_with_segments(&[
                "teams",
                input.team_id.as_str(),
                "channels",
                input.channel_id.as_str(),
                "messages",
            ])?,
            &request,
            &[],
        )
        .await?;

    Ok(PostMessageOutput {
        message_id: response.id,
        created_date_time: response.created_date_time,
    })
}

// ============================================================================
// Tool 3: Reply to Message
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplyInput {
    /// Team ID where the channel is located.
    pub team_id: String,
    /// Channel ID where the message exists.
    pub channel_id: String,
    /// Message ID to reply to.
    pub message_id: String,
    /// Reply content.
    pub content: String,
    /// Content type ("text" or "html"). Defaults to "text".
    #[serde(default)]
    pub content_type: Option<BodyContentType>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReplyOutput {
    pub reply_id: String,
    pub created_date_time: Option<String>,
}

/// # Reply to Microsoft Teams Message
///
/// Posts a reply to an existing message in a Microsoft Teams channel thread.
/// Use this tool when the user wants to respond to a specific message, continue
/// a conversation thread, or provide feedback on a previous message in the
/// channel.
///
/// Unlike posting a new top-level message, replies are associated with a parent
/// message and appear as part of the conversation thread. Replies maintain
/// context and are the recommended way to respond to ongoing discussions.
///
/// Required inputs:
/// - **team_id**: The unique identifier of the team (obtain from
///   list_teams_channels)
/// - **channel_id**: The unique identifier of the channel (obtain from
///   list_teams_channels)
/// - **message_id**: The unique identifier of the message to reply to (obtain
///   from `read_messages`)
/// - **content**: The reply content (must not be empty)
///
/// Optional inputs:
/// - **`content_type`**: Format of the reply ("text" or "html", defaults to
///   "text")
///
/// Note: This requires the authenticated user to have write permissions for the
/// target channel. The `message_id` must be valid and exist in the specified
/// channel.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - teams
/// - microsoft-teams
/// - microsoft-graph
/// - chat
///
/// # Errors
///
/// Returns an error if:
/// - The `team_id` parameter is empty or contains only whitespace
/// - The `channel_id` parameter is empty or contains only whitespace
/// - The `message_id` parameter is empty or contains only whitespace
/// - The `content` parameter is empty or contains only whitespace
/// - No Microsoft Teams credentials are configured in the context
/// - The configured access token is empty
/// - The configured endpoint is not a valid absolute URL
/// - The Microsoft Graph API request fails (network errors, authentication
///   failures, insufficient permissions, etc.)
/// - The API response cannot be parsed as valid JSON
#[tool]
pub async fn reply(ctx: Context, input: ReplyInput) -> Result<ReplyOutput> {
    ensure!(
        !input.team_id.trim().is_empty(),
        "team_id must not be empty"
    );
    ensure!(
        !input.channel_id.trim().is_empty(),
        "channel_id must not be empty"
    );
    ensure!(
        !input.message_id.trim().is_empty(),
        "message_id must not be empty"
    );
    ensure!(
        !input.content.trim().is_empty(),
        "content must not be empty"
    );

    let content_type = input.content_type.unwrap_or(BodyContentType::Text);
    let client = GraphClient::from_ctx(&ctx)?;

    let request = GraphPostMessageRequest {
        body: GraphItemBody {
            content_type,
            content: input.content,
        },
    };

    let response: GraphChatMessage = client
        .post_json(
            client.url_with_segments(&[
                "teams",
                input.team_id.as_str(),
                "channels",
                input.channel_id.as_str(),
                "messages",
                input.message_id.as_str(),
                "replies",
            ])?,
            &request,
            &[],
        )
        .await?;

    Ok(ReplyOutput {
        reply_id: response.id,
        created_date_time: response.created_date_time,
    })
}

// ============================================================================
// Tool 4: Read Messages
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadMessagesInput {
    /// Team ID where the channel is located.
    pub team_id: String,
    /// Channel ID to read messages from.
    pub channel_id: String,
    /// Maximum number of messages to retrieve (1-50). Defaults to 10.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReadMessagesOutput {
    pub messages: Vec<ChatMessage>,
}

/// # Read Microsoft Teams Messages
///
/// Retrieves messages from a Microsoft Teams channel, allowing the user to
/// review conversation history, catch up on discussions, or obtain message IDs
/// for replying. Use this tool when the user wants to read recent messages in a
/// channel, monitor channel activity, or find specific messages to respond to.
///
/// Messages are returned in reverse chronological order (newest first) and
/// include author information, timestamps, content, and metadata. Each message
/// contains a unique ID that can be used with the reply tool to respond to
/// specific messages.
///
/// Required inputs:
/// - **team_id**: The unique identifier of the team (obtain from
///   list_teams_channels)
/// - **channel_id**: The unique identifier of the channel (obtain from
///   list_teams_channels)
///
/// Optional inputs:
/// - **limit**: Maximum number of messages to retrieve, 1-50 (default 10)
///
/// Response includes:
/// - Message IDs for use with reply operations
/// - Author display names and user IDs
/// - Message content (text or HTML)
/// - Created and last modified timestamps
/// - Web URLs to messages in the Teams client
///
/// Note: This requires the authenticated user to have read permissions for the
/// target channel. For large channels, consider using pagination with the limit
/// parameter.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - teams
/// - microsoft-teams
/// - microsoft-graph
/// - chat
///
/// # Errors
///
/// Returns an error if:
/// - The `team_id` parameter is empty or contains only whitespace
/// - The `channel_id` parameter is empty or contains only whitespace
/// - The `limit` parameter is not between 1 and 50
/// - No Microsoft Teams credentials are configured in the context
/// - The configured access token is empty
/// - The configured endpoint is not a valid absolute URL
/// - The Microsoft Graph API request fails (network errors, authentication
///   failures, insufficient permissions, etc.)
/// - The API response cannot be parsed as valid JSON
#[tool]
pub async fn read_messages(ctx: Context, input: ReadMessagesInput) -> Result<ReadMessagesOutput> {
    ensure!(
        !input.team_id.trim().is_empty(),
        "team_id must not be empty"
    );
    ensure!(
        !input.channel_id.trim().is_empty(),
        "channel_id must not be empty"
    );
    let limit = input.limit.unwrap_or(10);
    ensure!((1..=50).contains(&limit), "limit must be between 1 and 50");

    let client = GraphClient::from_ctx(&ctx)?;
    let query = [("$top", limit.to_string())];

    let response: GraphListResponse<GraphChatMessage> = client
        .get_json(
            client.url_with_segments(&[
                "teams",
                input.team_id.as_str(),
                "channels",
                input.channel_id.as_str(),
                "messages",
            ])?,
            &query,
            &[],
        )
        .await?;

    Ok(ReadMessagesOutput {
        messages: response.value.into_iter().map(map_message).collect(),
    })
}

// ============================================================================
// Tool 5: Schedule Meeting
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScheduleMeetingInput {
    /// Meeting subject/title.
    pub subject: String,
    /// Meeting start date and time (ISO 8601 format).
    pub start_date_time: String,
    /// Meeting end date and time (ISO 8601 format).
    pub end_date_time: String,
    /// Time zone (e.g., "Pacific Standard Time"). Defaults to "UTC".
    #[serde(default)]
    pub time_zone: Option<String>,
    /// List of attendee email addresses.
    #[serde(default)]
    pub attendees: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ScheduleMeetingOutput {
    pub meeting_id: String,
    pub join_url: Option<String>,
}

/// # Schedule Microsoft Teams Meeting
///
/// Creates a new Microsoft Teams online meeting and generates a join URL for
/// participants. Use this tool when the user wants to schedule a Teams meeting,
/// set up a video conference, or create a virtual meeting space with a dial-in
/// link.
///
/// The meeting is created in the authenticated user's Microsoft Teams account
/// and includes a unique join URL that can be shared with attendees. The
/// meeting appears in the user's Teams calendar and can be joined through the
/// Teams client or web interface.
///
/// Required inputs:
/// - **subject**: Title or topic of the meeting (must not be empty)
/// - **start_date_time**: Meeting start time in ISO 8601 format (e.g.,
///   "2024-01-15T10:00:00Z")
/// - **end_date_time**: Meeting end time in ISO 8601 format (e.g.,
///   "2024-01-15T11:00:00Z")
///
/// Optional inputs:
/// - **`time_zone`**: Time zone for the meeting (e.g., "Pacific Standard Time",
///   defaults to "UTC")
/// - **`attendees`**: List of email addresses to invite (optional, for planning
///   purposes)
///
/// Response includes:
/// - **`meeting_id`**: Unique identifier for the meeting (can be used to modify
///   or cancel)
/// - **`join_url`**: URL that participants can use to join the Teams meeting
///
/// Note: This creates an online meeting only. To add calendar events with full
/// attendee management and notifications, use the Microsoft Graph Calendar API
/// instead.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - teams
/// - microsoft-teams
/// - microsoft-graph
/// - meeting
/// - calendar
///
/// # Errors
///
/// Returns an error if:
/// - The `subject` parameter is empty or contains only whitespace
/// - The `start_date_time` parameter is empty or contains only whitespace
/// - The `end_date_time` parameter is empty or contains only whitespace
/// - No Microsoft Teams credentials are configured in the context
/// - The configured access token is empty
/// - The configured endpoint is not a valid absolute URL
/// - The Microsoft Graph API request fails (network errors, authentication
///   failures, insufficient permissions, etc.)
/// - The API response cannot be parsed as valid JSON
#[tool]
pub async fn schedule_meeting(
    ctx: Context,
    input: ScheduleMeetingInput,
) -> Result<ScheduleMeetingOutput> {
    ensure!(
        !input.subject.trim().is_empty(),
        "subject must not be empty"
    );
    ensure!(
        !input.start_date_time.trim().is_empty(),
        "start_date_time must not be empty"
    );
    ensure!(
        !input.end_date_time.trim().is_empty(),
        "end_date_time must not be empty"
    );

    let time_zone = input.time_zone.unwrap_or_else(|| "UTC".to_string());
    let client = GraphClient::from_ctx(&ctx)?;

    let request = GraphCreateMeetingRequest {
        subject: input.subject,
        start: GraphDateTimeTimeZone {
            date_time: input.start_date_time,
            time_zone: time_zone.clone(),
        },
        end: GraphDateTimeTimeZone {
            date_time: input.end_date_time,
            time_zone,
        },
    };

    let response: GraphOnlineMeeting = client
        .post_json(
            client.url_with_segments(&["me", "onlineMeetings"])?,
            &request,
            &[],
        )
        .await?;

    Ok(ScheduleMeetingOutput {
        meeting_id: response.id,
        join_url: response.join_url,
    })
}

// ============================================================================
// Internal Graph API Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct GraphListResponse<T> {
    value: Vec<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphTeam {
    id: String,
    display_name: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphChannel {
    id: String,
    display_name: Option<String>,
    description: Option<String>,
    web_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphItemBody {
    content_type: BodyContentType,
    content: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphPostMessageRequest {
    body: GraphItemBody,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphIdentity {
    display_name: Option<String>,
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphIdentitySet {
    user: Option<GraphIdentity>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphChatMessage {
    id: String,
    created_date_time: Option<String>,
    last_modified_date_time: Option<String>,
    from: Option<GraphIdentitySet>,
    body: Option<GraphItemBody>,
    web_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphDateTimeTimeZone {
    date_time: String,
    time_zone: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphCreateMeetingRequest {
    subject: String,
    start: GraphDateTimeTimeZone,
    end: GraphDateTimeTimeZone,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphOnlineMeeting {
    id: String,
    join_url: Option<String>,
}

// ============================================================================
// Mapping Functions
// ============================================================================

fn map_team(team: GraphTeam) -> Team {
    Team {
        id: team.id,
        display_name: team.display_name,
        description: team.description,
    }
}

fn map_channel(channel: GraphChannel) -> Channel {
    Channel {
        id: channel.id,
        display_name: channel.display_name,
        description: channel.description,
        web_url: channel.web_url,
    }
}

fn map_message(msg: GraphChatMessage) -> ChatMessage {
    ChatMessage {
        id: msg.id,
        created_date_time: msg.created_date_time,
        last_modified_date_time: msg.last_modified_date_time,
        from: msg.from.map(|f| types::IdentitySet {
            user: f.user.map(|u| types::Identity {
                display_name: u.display_name,
                id: u.id,
            }),
        }),
        body: msg.body.map(|b| ItemBody {
            content_type: b.content_type,
            content: b.content,
        }),
        web_url: msg.web_url,
    }
}

// ============================================================================
// Graph Client
// ============================================================================

#[derive(Debug, Clone)]
struct GraphClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl GraphClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = TeamsCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_GRAPH_ENDPOINT))?;

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
                "Microsoft Graph request failed ({status}): {body}"
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

    fn test_ctx(endpoint: &str) -> Context {
        let mut teams_values = HashMap::new();
        teams_values.insert("access_token".to_string(), "test-token".to_string());
        teams_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("microsoft_teams", teams_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        format!("{}/v1.0", server.uri())
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_body_content_type_serialization_roundtrip() {
        for variant in [BodyContentType::Text, BodyContentType::Html] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: BodyContentType = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://graph.microsoft.com/").unwrap();
        assert_eq!(result, "https://graph.microsoft.com");
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
    async fn test_list_teams_channels_limit_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_teams_channels(
            ctx,
            ListTeamsChannelsInput {
                team_id: None,
                limit: Some(0),
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
    async fn test_post_message_empty_team_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = post_message(
            ctx,
            PostMessageInput {
                team_id: "  ".to_string(),
                channel_id: "chan-1".to_string(),
                content: "Hello".to_string(),
                content_type: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("team_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_post_message_empty_content_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = post_message(
            ctx,
            PostMessageInput {
                team_id: "team-1".to_string(),
                channel_id: "chan-1".to_string(),
                content: "  ".to_string(),
                content_type: None,
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
    async fn test_schedule_meeting_empty_subject_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = schedule_meeting(
            ctx,
            ScheduleMeetingInput {
                subject: "  ".to_string(),
                start_date_time: "2024-01-01T10:00:00Z".to_string(),
                end_date_time: "2024-01-01T11:00:00Z".to_string(),
                time_zone: None,
                attendees: vec![],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("subject must not be empty")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_list_teams_success_returns_teams() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "value": [
            {
              "id": "team-1",
              "displayName": "Engineering Team",
              "description": "Engineering discussions"
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1.0/me/joinedTeams"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("$top", "10"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_teams_channels(
            ctx,
            ListTeamsChannelsInput {
                team_id: None,
                limit: Some(10),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.teams.len(), 1);
        assert_eq!(output.teams[0].id, "team-1");
        assert_eq!(
            output.teams[0].display_name.as_deref(),
            Some("Engineering Team")
        );
    }

    #[tokio::test]
    async fn test_list_channels_success_returns_channels() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "value": [
            {
              "id": "chan-1",
              "displayName": "General",
              "description": "General discussion",
              "webUrl": "https://teams.microsoft.com/..."
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1.0/teams/team-1/channels"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_teams_channels(
            ctx,
            ListTeamsChannelsInput {
                team_id: Some("team-1".to_string()),
                limit: Some(10),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.channels.len(), 1);
        assert_eq!(output.channels[0].id, "chan-1");
        assert_eq!(output.channels[0].display_name.as_deref(), Some("General"));
    }

    #[tokio::test]
    async fn test_post_message_success_returns_message_id() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "msg-123",
          "createdDateTime": "2024-01-01T10:00:00Z"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/v1.0/teams/team-1/channels/chan-1/messages"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("\"content\":\"Hello World\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = post_message(
            ctx,
            PostMessageInput {
                team_id: "team-1".to_string(),
                channel_id: "chan-1".to_string(),
                content: "Hello World".to_string(),
                content_type: Some(BodyContentType::Text),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.message_id, "msg-123");
        assert_eq!(
            output.created_date_time.as_deref(),
            Some("2024-01-01T10:00:00Z")
        );
    }

    #[tokio::test]
    async fn test_reply_success_returns_reply_id() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "reply-456",
          "createdDateTime": "2024-01-01T10:05:00Z"
        }
        "#;

        Mock::given(method("POST"))
            .and(path(
                "/v1.0/teams/team-1/channels/chan-1/messages/msg-123/replies",
            ))
            .and(body_string_contains("\"content\":\"Thanks\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = reply(
            ctx,
            ReplyInput {
                team_id: "team-1".to_string(),
                channel_id: "chan-1".to_string(),
                message_id: "msg-123".to_string(),
                content: "Thanks".to_string(),
                content_type: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.reply_id, "reply-456");
    }

    #[tokio::test]
    async fn test_read_messages_success_returns_messages() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "value": [
            {
              "id": "msg-1",
              "createdDateTime": "2024-01-01T10:00:00Z",
              "lastModifiedDateTime": "2024-01-01T10:00:00Z",
              "from": {
                "user": {
                  "displayName": "Alice",
                  "id": "user-1"
                }
              },
              "body": {
                "contentType": "text",
                "content": "Hello"
              },
              "webUrl": "https://teams.microsoft.com/..."
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1.0/teams/team-1/channels/chan-1/messages"))
            .and(query_param("$top", "10"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = read_messages(
            ctx,
            ReadMessagesInput {
                team_id: "team-1".to_string(),
                channel_id: "chan-1".to_string(),
                limit: Some(10),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.messages.len(), 1);
        assert_eq!(output.messages[0].id, "msg-1");
        assert_eq!(
            output.messages[0].body.as_ref().map(|b| b.content.as_str()),
            Some("Hello")
        );
    }

    #[tokio::test]
    async fn test_schedule_meeting_success_returns_meeting_id() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "meeting-789",
          "joinUrl": "https://teams.microsoft.com/l/meetup-join/..."
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/v1.0/me/onlineMeetings"))
            .and(body_string_contains("\"subject\":\"Team Standup\""))
            .and(body_string_contains("\"dateTime\":\"2024-01-01T10:00:00\""))
            .and(body_string_contains("\"timeZone\":\"UTC\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = schedule_meeting(
            ctx,
            ScheduleMeetingInput {
                subject: "Team Standup".to_string(),
                start_date_time: "2024-01-01T10:00:00".to_string(),
                end_date_time: "2024-01-01T11:00:00".to_string(),
                time_zone: Some("UTC".to_string()),
                attendees: vec!["alice@example.com".to_string()],
            },
        )
        .await
        .unwrap();

        assert_eq!(output.meeting_id, "meeting-789");
        assert!(output.join_url.is_some());
    }

    #[tokio::test]
    async fn test_post_message_error_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("POST"))
            .and(path("/v1.0/teams/team-1/channels/chan-1/messages"))
            .respond_with(ResponseTemplate::new(403).set_body_raw(
                r#"{ "error": { "code": "Forbidden", "message": "Access denied" } }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = post_message(
            ctx,
            PostMessageInput {
                team_id: "team-1".to_string(),
                channel_id: "chan-1".to_string(),
                content: "Hello".to_string(),
                content_type: None,
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("403"));
    }
}
