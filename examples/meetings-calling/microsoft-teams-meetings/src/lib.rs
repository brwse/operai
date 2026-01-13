//! meetings-calling/microsoft-teams-meetings integration for Operai Toolbox.

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

mod types;
pub use types::*;

define_user_credential! {
    TeamsCredential("teams") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_GRAPH_ENDPOINT: &str = "https://graph.microsoft.com/v1.0";

#[init]
async fn setup() -> Result<()> {
    info!("Microsoft Teams Meetings integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Microsoft Teams Meetings integration shutting down");
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScheduleMeetingInput {
    /// Meeting subject/title.
    pub subject: String,
    /// Meeting start date and time in ISO 8601 format (e.g.,
    /// "2024-01-15T10:00:00Z").
    pub start_date_time: String,
    /// Meeting end date and time in ISO 8601 format (e.g.,
    /// "2024-01-15T11:00:00Z").
    pub end_date_time: String,
    /// Optional list of participant email addresses.
    #[serde(default)]
    pub participants: Vec<String>,
    /// Allow meeting chat. Defaults to true.
    #[serde(default)]
    pub allow_meeting_chat: Option<bool>,
    /// Allow participants to enable camera. Defaults to true.
    #[serde(default)]
    pub allow_participants_to_enable_camera: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ScheduleMeetingOutput {
    pub meeting_id: String,
    pub join_web_url: String,
    pub subject: String,
    pub start_date_time: String,
    pub end_date_time: String,
}

/// # Schedule Microsoft Teams Meeting
///
/// Schedules a new Microsoft Teams online meeting using the Microsoft Graph
/// API. Use this tool when a user wants to create a new Teams meeting and
/// generate a join URL for participants.
///
/// ## What it does
/// Creates a new Teams meeting via Microsoft Graph's `/me/onlineMeetings`
/// endpoint with the specified subject, start/end times, and participant list.
/// Returns the meeting ID, join web URL, and meeting details.
///
/// ## When to use it
/// - User wants to schedule a new Teams meeting
/// - User needs to create a meeting link for others to join
/// - User wants to set up a recurring or one-time online meeting
///
/// ## Key inputs
/// - `subject`: Meeting title (required, non-empty)
/// - `start_date_time`: ISO 8601 timestamp (e.g., "2024-01-15T10:00:00Z")
/// - `end_date_time`: ISO 8601 timestamp (e.g., "2024-01-15T11:00:00Z")
/// - `participants`: Optional list of attendee email addresses
/// - `allow_meeting_chat`: Optional, defaults to true
/// - `allow_participants_to_enable_camera`: Optional, defaults to true
///
/// ## Output
/// Returns `meeting_id`, `join_web_url`, `subject`, `start_date_time`, and
/// `end_date_time`.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - meetings
/// - teams
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The `subject`, `start_date_time`, or `end_date_time` fields are empty or
///   contain only whitespace
/// - The Teams credential is not configured or contains an invalid access token
/// - The endpoint URL is invalid or cannot be parsed
/// - The Microsoft Graph API request fails due to network issues or
///   authentication problems
/// - The API response cannot be parsed or does not contain expected fields
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

    let client = GraphClient::from_ctx(&ctx)?;

    let request = GraphCreateOnlineMeetingRequest {
        subject: input.subject.clone(),
        start_date_time: input.start_date_time.clone(),
        end_date_time: input.end_date_time.clone(),
        participants: GraphMeetingParticipants {
            attendees: input
                .participants
                .into_iter()
                .map(|email| GraphMeetingParticipantInfo {
                    identity: GraphIdentitySet {
                        user: GraphIdentity {
                            id: None,
                            display_name: None,
                            user_principal_name: Some(email),
                        },
                    },
                    upn: None,
                })
                .collect(),
        },
        allow_meeting_chat: input.allow_meeting_chat.unwrap_or(true),
        allow_participants_to_enable_camera: input
            .allow_participants_to_enable_camera
            .unwrap_or(true),
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
        join_web_url: response.join_web_url,
        subject: response.subject,
        start_date_time: response.start_date_time,
        end_date_time: response.end_date_time,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateMeetingInput {
    /// Meeting ID to update.
    pub meeting_id: String,
    /// Optional new subject/title.
    #[serde(default)]
    pub subject: Option<String>,
    /// Optional new start date and time in ISO 8601 format.
    #[serde(default)]
    pub start_date_time: Option<String>,
    /// Optional new end date and time in ISO 8601 format.
    #[serde(default)]
    pub end_date_time: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateMeetingOutput {
    pub meeting_id: String,
    pub updated: bool,
}

/// # Update Microsoft Teams Meeting
///
/// Updates an existing Microsoft Teams online meeting using the Microsoft Graph
/// API. Use this tool when a user wants to modify the details of a previously
/// scheduled Teams meeting.
///
/// ## What it does
/// Sends a PATCH request to Microsoft Graph's `/me/onlineMeetings/{meeting_id}`
/// endpoint to update specific fields of an existing meeting. Only the fields
/// provided in the request are modified; unspecified fields remain unchanged.
///
/// ## When to use it
/// - User wants to change the time of an existing meeting
/// - User needs to update the meeting title/subject
/// - User wants to reschedule a meeting while keeping the same meeting ID
///
/// ## Key inputs
/// - `meeting_id`: The ID of the meeting to update (required, non-empty)
/// - `subject`: New meeting title (optional)
/// - `start_date_time`: New ISO 8601 start timestamp (optional)
/// - `end_date_time`: New ISO 8601 end timestamp (optional)
///
/// At least one of `subject`, `start_date_time`, or `end_date_time` must be
/// provided.
///
/// ## Output
/// Returns `meeting_id` and `updated` boolean flag.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - meetings
/// - teams
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The `meeting_id` field is empty or contains only whitespace
/// - None of the optional fields (`subject`, `start_date_time`,
///   `end_date_time`) are provided
/// - The Teams credential is not configured or contains an invalid access token
/// - The endpoint URL is invalid or cannot be parsed
/// - The Microsoft Graph API request fails due to network issues or
///   authentication problems
#[tool]
pub async fn update_meeting(
    ctx: Context,
    input: UpdateMeetingInput,
) -> Result<UpdateMeetingOutput> {
    ensure!(
        !input.meeting_id.trim().is_empty(),
        "meeting_id must not be empty"
    );

    ensure!(
        input.subject.is_some() || input.start_date_time.is_some() || input.end_date_time.is_some(),
        "at least one field (subject, start_date_time, or end_date_time) must be provided"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let request = GraphUpdateOnlineMeetingRequest {
        subject: input.subject,
        start_date_time: input.start_date_time,
        end_date_time: input.end_date_time,
    };

    client
        .patch_empty(
            client.url_with_segments(&["me", "onlineMeetings", input.meeting_id.as_str()])?,
            &request,
            &[],
        )
        .await?;

    Ok(UpdateMeetingOutput {
        meeting_id: input.meeting_id,
        updated: true,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetJoinLinkInput {
    /// Meeting ID to get the join link for.
    pub meeting_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetJoinLinkOutput {
    pub meeting_id: String,
    pub join_web_url: String,
    pub subject: String,
}

/// # Get Microsoft Teams Meeting Join Link
///
/// Retrieves the join web URL and details for an existing Microsoft Teams
/// online meeting. Use this tool when a user needs to get the meeting link or
/// verify meeting details for a previously scheduled Teams meeting.
///
/// ## What it does
/// Queries Microsoft Graph's `/me/onlineMeetings/{meeting_id}` endpoint to
/// retrieve the meeting's join URL, subject, and time details. Useful for
/// retrieving lost meeting links or displaying meeting information to
/// participants.
///
/// ## When to use it
/// - User lost their meeting link and needs to recover it
/// - User wants to share the join URL with participants
/// - User needs to verify meeting details (subject, time) for an existing
///   meeting
///
/// ## Key inputs
/// - `meeting_id`: The ID of the meeting to query (required, non-empty)
///
/// ## Output
/// Returns `meeting_id`, `join_web_url`, and `subject` of the meeting.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - meetings
/// - teams
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The `meeting_id` field is empty or contains only whitespace
/// - The Teams credential is not configured or contains an invalid access token
/// - The endpoint URL is invalid or cannot be parsed
/// - The Microsoft Graph API request fails due to network issues or
///   authentication problems
/// - The meeting does not exist or the user does not have permission to access
///   it
/// - The API response cannot be parsed or does not contain expected fields
#[tool]
pub async fn get_join_link(ctx: Context, input: GetJoinLinkInput) -> Result<GetJoinLinkOutput> {
    ensure!(
        !input.meeting_id.trim().is_empty(),
        "meeting_id must not be empty"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let response: GraphOnlineMeeting = client
        .get_json(
            client.url_with_segments(&["me", "onlineMeetings", input.meeting_id.as_str()])?,
            &[],
            &[],
        )
        .await?;

    Ok(GetJoinLinkOutput {
        meeting_id: response.id,
        join_web_url: response.join_web_url,
        subject: response.subject,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListRecordingsInput {
    /// Meeting ID to list recordings for.
    pub meeting_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListRecordingsOutput {
    pub recordings: Vec<Recording>,
}

/// # List Microsoft Teams Meeting Recordings
///
/// Lists all available recordings for a Microsoft Teams online meeting.
/// Use this tool when a user wants to access or review recordings from a
/// previously held Teams meeting.
///
/// ## What it does
/// Queries Microsoft Graph's `/me/onlineMeetings/{meeting_id}/recordings`
/// endpoint to retrieve a list of all recording objects associated with the
/// meeting, including their content URLs and metadata.
///
/// ## When to use it
/// - User wants to access recordings of a past meeting
/// - User needs to download or share meeting recordings
/// - User is looking for specific content discussed in a recorded meeting
///
/// ## Important constraint
/// **This API only works with meetings created via the Outlook calendar event
/// API, NOT with standalone meetings created via the `schedule_meeting`
/// function.** Ensure the meeting was created through Outlook/calendar
/// integration before using.
///
/// ## Key inputs
/// - `meeting_id`: The ID of the meeting to list recordings for (required,
///   non-empty)
///
/// ## Output
/// Returns an array of `Recording` objects, each containing `id`, `meeting_id`,
/// `created_date_time`, `recording_content_url`, and optional
/// `content_correlation_id`.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - meetings
/// - teams
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The `meeting_id` field is empty or contains only whitespace
/// - The Teams credential is not configured or contains an invalid access token
/// - The endpoint URL is invalid or cannot be parsed
/// - The Microsoft Graph API request fails due to network issues or
///   authentication problems
/// - The meeting does not exist or the user does not have permission to access
///   it
/// - The API response cannot be parsed or does not contain expected fields
#[tool]
pub async fn list_recordings(
    ctx: Context,
    input: ListRecordingsInput,
) -> Result<ListRecordingsOutput> {
    ensure!(
        !input.meeting_id.trim().is_empty(),
        "meeting_id must not be empty"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let response: GraphListResponse<GraphCallRecording> = client
        .get_json(
            client.url_with_segments(&[
                "me",
                "onlineMeetings",
                input.meeting_id.as_str(),
                "recordings",
            ])?,
            &[],
            &[],
        )
        .await?;

    Ok(ListRecordingsOutput {
        recordings: response.value.into_iter().map(map_recording).collect(),
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTranscriptsInput {
    /// Meeting ID to list transcripts for.
    pub meeting_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListTranscriptsOutput {
    pub transcripts: Vec<Transcript>,
}

/// # List Microsoft Teams Meeting Transcripts
///
/// Lists all available transcripts for a Microsoft Teams online meeting.
/// Use this tool when a user wants to access or review text transcripts from a
/// previously held Teams meeting.
///
/// ## What it does
/// Queries Microsoft Graph's `/me/onlineMeetings/{meeting_id}/transcripts`
/// endpoint to retrieve a list of all transcript objects associated with the
/// meeting, including their content URLs and metadata. Transcripts provide
/// searchable text versions of meeting conversations.
///
/// ## When to use it
/// - User wants to read or search through meeting transcripts
/// - User needs to review what was discussed in a past meeting
/// - User wants to extract quotes or action items from meeting conversations
/// - User prefers reading text over watching recordings
///
/// ## Important constraint
/// **This API only works with meetings created via the Outlook calendar event
/// API, NOT with standalone meetings created via the `schedule_meeting`
/// function.** Ensure the meeting was created through Outlook/calendar
/// integration before using.
///
/// ## Key inputs
/// - `meeting_id`: The ID of the meeting to list transcripts for (required,
///   non-empty)
///
/// ## Output
/// Returns an array of `Transcript` objects, each containing `id`,
/// `meeting_id`, `created_date_time`, `transcript_content_url`, and optional
/// `content_correlation_id`.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - meetings
/// - teams
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The `meeting_id` field is empty or contains only whitespace
/// - The Teams credential is not configured or contains an invalid access token
/// - The endpoint URL is invalid or cannot be parsed
/// - The Microsoft Graph API request fails due to network issues or
///   authentication problems
/// - The meeting does not exist or the user does not have permission to access
///   it
/// - The API response cannot be parsed or does not contain expected fields
#[tool]
pub async fn list_transcripts(
    ctx: Context,
    input: ListTranscriptsInput,
) -> Result<ListTranscriptsOutput> {
    ensure!(
        !input.meeting_id.trim().is_empty(),
        "meeting_id must not be empty"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let response: GraphListResponse<GraphCallTranscript> = client
        .get_json(
            client.url_with_segments(&[
                "me",
                "onlineMeetings",
                input.meeting_id.as_str(),
                "transcripts",
            ])?,
            &[],
            &[],
        )
        .await?;

    Ok(ListTranscriptsOutput {
        transcripts: response.value.into_iter().map(map_transcript).collect(),
    })
}

#[derive(Debug, Deserialize)]
struct GraphListResponse<T> {
    value: Vec<T>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphOnlineMeeting {
    id: String,
    subject: String,
    start_date_time: String,
    end_date_time: String,
    join_web_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphCreateOnlineMeetingRequest {
    subject: String,
    start_date_time: String,
    end_date_time: String,
    participants: GraphMeetingParticipants,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    allow_meeting_chat: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    allow_participants_to_enable_camera: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphUpdateOnlineMeetingRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_date_time: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphMeetingParticipants {
    attendees: Vec<GraphMeetingParticipantInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphMeetingParticipantInfo {
    identity: GraphIdentitySet,
    #[serde(skip_serializing_if = "Option::is_none")]
    upn: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphIdentitySet {
    user: GraphIdentity,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphIdentity {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_principal_name: Option<String>,
}

// Recording and transcript types are defined in types.rs with proper field
// names

#[derive(Debug, Clone)]
struct GraphClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl GraphClient {
    /// Create a new `GraphClient` from the provided context.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The Teams credential is not configured in the context
    /// - The access token in the credential is empty or contains only
    ///   whitespace
    /// - The endpoint URL (if provided) cannot be normalized or is invalid
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

    /// Build a URL by appending path segments to the base URL.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The base URL is not a valid absolute URL that can have path segments
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

    /// Send a GET request and parse the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails due to network issues or authentication
    ///   problems
    /// - The response status indicates an error (4xx or 5xx)
    /// - The response body cannot be parsed as JSON
    /// - The JSON cannot be deserialized into type T
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

    /// Send a POST request with a JSON body and parse the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails due to network issues or authentication
    ///   problems
    /// - The response status indicates an error (4xx or 5xx)
    /// - The response body cannot be parsed as JSON
    /// - The JSON cannot be deserialized into type `TRes`
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

    /// Send a PATCH request with a JSON body, ignoring the response body.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails due to network issues or authentication
    ///   problems
    /// - The response status indicates an error (4xx or 5xx)
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

    /// Send an HTTP request with authentication and handle errors.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails due to network issues or authentication
    ///   problems
    /// - The response status indicates an error (4xx or 5xx)
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

/// Normalize a base URL by trimming whitespace and removing trailing slashes.
///
/// # Errors
///
/// Returns an error if:
/// - The endpoint is empty or contains only whitespace after trimming
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
        let mut teams_values = HashMap::new();
        teams_values.insert("access_token".to_string(), "test-token".to_string());
        teams_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("teams", teams_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        format!("{}/v1.0", server.uri())
    }

    #[tokio::test]
    async fn test_schedule_meeting_empty_subject_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = schedule_meeting(
            ctx,
            ScheduleMeetingInput {
                subject: "  ".to_string(),
                start_date_time: "2024-01-15T10:00:00Z".to_string(),
                end_date_time: "2024-01-15T11:00:00Z".to_string(),
                participants: vec![],
                allow_meeting_chat: None,
                allow_participants_to_enable_camera: None,
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

    #[tokio::test]
    async fn test_schedule_meeting_empty_start_time_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = schedule_meeting(
            ctx,
            ScheduleMeetingInput {
                subject: "Test Meeting".to_string(),
                start_date_time: "  ".to_string(),
                end_date_time: "2024-01-15T11:00:00Z".to_string(),
                participants: vec![],
                allow_meeting_chat: None,
                allow_participants_to_enable_camera: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("start_date_time must not be empty")
        );
    }

    #[tokio::test]
    async fn test_update_meeting_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = update_meeting(
            ctx,
            UpdateMeetingInput {
                meeting_id: "  ".to_string(),
                subject: Some("Updated".to_string()),
                start_date_time: None,
                end_date_time: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("meeting_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_update_meeting_no_fields_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = update_meeting(
            ctx,
            UpdateMeetingInput {
                meeting_id: "meeting-1".to_string(),
                subject: None,
                start_date_time: None,
                end_date_time: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("at least one field")
        );
    }

    #[tokio::test]
    async fn test_schedule_meeting_success_returns_meeting_details() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "meeting-123",
          "subject": "Team Sync",
          "startDateTime": "2024-01-15T10:00:00Z",
          "endDateTime": "2024-01-15T11:00:00Z",
          "joinWebUrl": "https://teams.microsoft.com/l/meetup-join/123"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/v1.0/me/onlineMeetings"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("\"subject\":\"Team Sync\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = schedule_meeting(
            ctx,
            ScheduleMeetingInput {
                subject: "Team Sync".to_string(),
                start_date_time: "2024-01-15T10:00:00Z".to_string(),
                end_date_time: "2024-01-15T11:00:00Z".to_string(),
                participants: vec!["user@example.com".to_string()],
                allow_meeting_chat: Some(true),
                allow_participants_to_enable_camera: Some(true),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.meeting_id, "meeting-123");
        assert_eq!(output.subject, "Team Sync");
        assert!(output.join_web_url.contains("teams.microsoft.com"));
    }

    #[tokio::test]
    async fn test_update_meeting_success_returns_updated() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("PATCH"))
            .and(path("/v1.0/me/onlineMeetings/meeting-123"))
            .and(body_string_contains("\"subject\":\"Updated Title\""))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = update_meeting(
            ctx,
            UpdateMeetingInput {
                meeting_id: "meeting-123".to_string(),
                subject: Some("Updated Title".to_string()),
                start_date_time: None,
                end_date_time: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.meeting_id, "meeting-123");
        assert!(output.updated);
    }

    #[tokio::test]
    async fn test_get_join_link_success_returns_url() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "meeting-123",
          "subject": "Team Sync",
          "startDateTime": "2024-01-15T10:00:00Z",
          "endDateTime": "2024-01-15T11:00:00Z",
          "joinWebUrl": "https://teams.microsoft.com/l/meetup-join/123"
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1.0/me/onlineMeetings/meeting-123"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = get_join_link(
            ctx,
            GetJoinLinkInput {
                meeting_id: "meeting-123".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.meeting_id, "meeting-123");
        assert!(output.join_web_url.contains("teams.microsoft.com"));
    }

    #[tokio::test]
    async fn test_list_recordings_success_returns_recordings() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "value": [
            {
              "id": "rec-1",
              "meetingId": "meeting-123",
              "createdDateTime": "2024-01-15T11:00:00Z",
              "recordingContentUrl": "https://example.com/recording1.mp4",
              "contentCorrelationId": "corr-1"
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1.0/me/onlineMeetings/meeting-123/recordings"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_recordings(
            ctx,
            ListRecordingsInput {
                meeting_id: "meeting-123".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.recordings.len(), 1);
        assert_eq!(output.recordings[0].id, "rec-1");
        assert_eq!(output.recordings[0].meeting_id, "meeting-123");
        assert_eq!(
            output.recordings[0].recording_content_url,
            "https://example.com/recording1.mp4"
        );
        assert_eq!(
            output.recordings[0].content_correlation_id,
            Some("corr-1".to_string())
        );
    }

    #[tokio::test]
    async fn test_list_transcripts_success_returns_transcripts() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "value": [
            {
              "id": "trans-1",
              "meetingId": "meeting-123",
              "createdDateTime": "2024-01-15T11:05:00Z",
              "transcriptContentUrl": "https://example.com/transcript1.vtt",
              "contentCorrelationId": "corr-2"
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1.0/me/onlineMeetings/meeting-123/transcripts"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_transcripts(
            ctx,
            ListTranscriptsInput {
                meeting_id: "meeting-123".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.transcripts.len(), 1);
        assert_eq!(output.transcripts[0].id, "trans-1");
        assert_eq!(output.transcripts[0].meeting_id, "meeting-123");
        assert_eq!(
            output.transcripts[0].transcript_content_url,
            "https://example.com/transcript1.vtt"
        );
        assert_eq!(
            output.transcripts[0].content_correlation_id,
            Some("corr-2".to_string())
        );
    }

    #[tokio::test]
    async fn test_list_recordings_empty_response() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "value": []
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1.0/me/onlineMeetings/meeting-123/recordings"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_recordings(
            ctx,
            ListRecordingsInput {
                meeting_id: "meeting-123".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.recordings.len(), 0);
    }

    #[tokio::test]
    async fn test_list_transcripts_empty_response() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "value": []
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1.0/me/onlineMeetings/meeting-123/transcripts"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_transcripts(
            ctx,
            ListTranscriptsInput {
                meeting_id: "meeting-123".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.transcripts.len(), 0);
    }

    #[tokio::test]
    async fn test_get_join_link_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = get_join_link(
            ctx,
            GetJoinLinkInput {
                meeting_id: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("meeting_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_list_recordings_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_recordings(
            ctx,
            ListRecordingsInput {
                meeting_id: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("meeting_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_list_transcripts_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_transcripts(
            ctx,
            ListTranscriptsInput {
                meeting_id: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("meeting_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_schedule_meeting_with_participants() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "meeting-456",
          "subject": "Team Standup",
          "startDateTime": "2024-01-15T10:00:00Z",
          "endDateTime": "2024-01-15T10:30:00Z",
          "joinWebUrl": "https://teams.microsoft.com/l/meetup-join/456"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/v1.0/me/onlineMeetings"))
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
                start_date_time: "2024-01-15T10:00:00Z".to_string(),
                end_date_time: "2024-01-15T10:30:00Z".to_string(),
                participants: vec![
                    "alice@example.com".to_string(),
                    "bob@example.com".to_string(),
                ],
                allow_meeting_chat: Some(false),
                allow_participants_to_enable_camera: Some(false),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.meeting_id, "meeting-456");
        assert_eq!(output.subject, "Team Standup");
    }
}
