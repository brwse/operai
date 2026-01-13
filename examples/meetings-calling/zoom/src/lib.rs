//! meetings-calling/zoom integration for Operai Toolbox.

mod types;

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
use types::{
    CreateMeetingRequest, ListRecordingsResponse, Meeting, MeetingSettingsRequest, Recording,
    Transcript, UpdateMeetingRequest,
};

define_user_credential! {
    ZoomCredential("zoom") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_ZOOM_API_ENDPOINT: &str = "https://api.zoom.us/v2";

#[init]
async fn setup() -> Result<()> {
    info!("Zoom integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Zoom integration shutting down");
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScheduleMeetingInput {
    /// Meeting topic
    pub topic: String,
    /// Start time in ISO 8601 format (e.g., "2024-01-15T10:00:00Z"). Required
    /// for scheduled meetings.
    #[serde(default)]
    pub start_time: Option<String>,
    /// Meeting duration in minutes
    #[serde(default)]
    pub duration: Option<i32>,
    /// Timezone (e.g., "`America/New_York`"). Defaults to `UTC` if not
    /// specified.
    #[serde(default)]
    pub timezone: Option<String>,
    /// Meeting agenda/description
    #[serde(default)]
    pub agenda: Option<String>,
    /// Meeting password
    #[serde(default)]
    pub password: Option<String>,
    /// Enable host video
    #[serde(default)]
    pub host_video: Option<bool>,
    /// Enable participant video
    #[serde(default)]
    pub participant_video: Option<bool>,
    /// Enable waiting room
    #[serde(default)]
    pub waiting_room: Option<bool>,
    /// Enable join before host
    #[serde(default)]
    pub join_before_host: Option<bool>,
    /// Mute participants upon entry
    #[serde(default)]
    pub mute_upon_entry: Option<bool>,
    /// Auto recording ("local", "cloud", "none")
    #[serde(default)]
    pub auto_recording: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ScheduleMeetingOutput {
    pub meeting: Meeting,
}

/// # Schedule Zoom Meeting
///
/// Schedules a new Zoom meeting with customizable settings and returns the
/// meeting details including join URL.
///
/// Use this tool when a user wants to create a new Zoom meeting. The meeting
/// can be scheduled for a specific time or created as an instant meeting.
/// Supports comprehensive meeting configuration including video settings,
/// waiting room, password protection, auto-recording, and more.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - meetings
/// - zoom
/// - scheduling
///
/// # Errors
///
/// Returns an error if:
/// - The topic is empty or contains only whitespace
/// - The duration is negative or zero
/// - The Zoom credential is missing or the access token is empty
/// - The endpoint URL is invalid
/// - The Zoom API request fails (network error, authentication failure, etc.)
#[tool]
pub async fn schedule_meeting(
    ctx: Context,
    input: ScheduleMeetingInput,
) -> Result<ScheduleMeetingOutput> {
    ensure!(!input.topic.trim().is_empty(), "topic must not be empty");

    if let Some(ref duration) = input.duration {
        ensure!(*duration > 0, "duration must be positive");
    }

    let client = ZoomClient::from_ctx(&ctx)?;

    let meeting_type = if input.start_time.is_some() { 2 } else { 1 };

    let settings = if input.host_video.is_some()
        || input.participant_video.is_some()
        || input.waiting_room.is_some()
        || input.join_before_host.is_some()
        || input.mute_upon_entry.is_some()
        || input.auto_recording.is_some()
    {
        Some(MeetingSettingsRequest {
            host_video: input.host_video,
            participant_video: input.participant_video,
            waiting_room: input.waiting_room,
            join_before_host: input.join_before_host,
            mute_upon_entry: input.mute_upon_entry,
            auto_recording: input.auto_recording,
        })
    } else {
        None
    };

    let request = CreateMeetingRequest {
        topic: input.topic,
        meeting_type,
        start_time: input.start_time,
        duration: input.duration,
        timezone: input.timezone,
        agenda: input.agenda,
        password: input.password,
        settings,
    };

    let meeting: Meeting = client
        .post_json(
            client.url_with_segments(&["users", "me", "meetings"])?,
            &request,
        )
        .await?;

    Ok(ScheduleMeetingOutput { meeting })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateMeetingInput {
    /// Meeting ID to update
    pub meeting_id: i64,
    /// Updated meeting topic
    #[serde(default)]
    pub topic: Option<String>,
    /// Updated start time in ISO 8601 format
    #[serde(default)]
    pub start_time: Option<String>,
    /// Updated duration in minutes
    #[serde(default)]
    pub duration: Option<i32>,
    /// Updated timezone
    #[serde(default)]
    pub timezone: Option<String>,
    /// Updated agenda
    #[serde(default)]
    pub agenda: Option<String>,
    /// Enable waiting room
    #[serde(default)]
    pub waiting_room: Option<bool>,
    /// Enable join before host
    #[serde(default)]
    pub join_before_host: Option<bool>,
    /// Mute participants upon entry
    #[serde(default)]
    pub mute_upon_entry: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateMeetingOutput {
    pub updated: bool,
    pub meeting_id: i64,
}

/// # Update Zoom Meeting
///
/// Modifies settings for an existing Zoom meeting such as topic, time,
/// duration, or security options.
///
/// Use this tool when a user wants to change details of a previously scheduled
/// meeting. All parameters are optional; only the fields you want to update
/// will be modified. The meeting ID must be provided to identify which meeting
/// to update.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - meetings
/// - zoom
/// - scheduling
///
/// # Errors
///
/// Returns an error if:
/// - The `meeting_id` is not positive
/// - The topic is empty or contains only whitespace (if provided)
/// - The duration is negative or zero (if provided)
/// - The Zoom credential is missing or the access token is empty
/// - The endpoint URL is invalid
/// - The Zoom API request fails (network error, authentication failure, etc.)
#[tool]
pub async fn update_meeting(
    ctx: Context,
    input: UpdateMeetingInput,
) -> Result<UpdateMeetingOutput> {
    ensure!(input.meeting_id > 0, "meeting_id must be positive");

    if let Some(ref topic) = input.topic {
        ensure!(!topic.trim().is_empty(), "topic must not be empty");
    }

    if let Some(ref duration) = input.duration {
        ensure!(*duration > 0, "duration must be positive");
    }

    let client = ZoomClient::from_ctx(&ctx)?;

    let settings = if input.waiting_room.is_some()
        || input.join_before_host.is_some()
        || input.mute_upon_entry.is_some()
    {
        Some(MeetingSettingsRequest {
            host_video: None,
            participant_video: None,
            waiting_room: input.waiting_room,
            join_before_host: input.join_before_host,
            mute_upon_entry: input.mute_upon_entry,
            auto_recording: None,
        })
    } else {
        None
    };

    let request = UpdateMeetingRequest {
        topic: input.topic,
        start_time: input.start_time,
        duration: input.duration,
        timezone: input.timezone,
        agenda: input.agenda,
        settings,
    };

    client
        .patch_empty(
            client.url_with_segments(&["meetings", &input.meeting_id.to_string()])?,
            &request,
        )
        .await?;

    Ok(UpdateMeetingOutput {
        updated: true,
        meeting_id: input.meeting_id,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListRecordingsInput {
    /// Start date for filtering recordings (YYYY-MM-DD). Defaults to last 30
    /// days.
    #[serde(default)]
    pub from: Option<String>,
    /// End date for filtering recordings (YYYY-MM-DD). Defaults to today.
    #[serde(default)]
    pub to: Option<String>,
    /// Maximum number of results to return (1-300). Defaults to 30.
    #[serde(default)]
    pub page_size: Option<i32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListRecordingsOutput {
    pub recordings: Vec<Recording>,
}

/// # List Zoom Recordings
///
/// Retrieves cloud recordings for the authenticated user with optional date
/// range filtering.
///
/// Use this tool when a user wants to access their Zoom meeting recordings.
/// Supports filtering by date range (from/to) and pagination control. Returns a
/// list of recordings with metadata including download URLs, file types, and
/// recording details. Default date range is the last 30 days.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - meetings
/// - zoom
/// - recordings
///
/// # Errors
///
/// Returns an error if:
/// - The `page_size` is not between 1 and 300 (if provided)
/// - The Zoom credential is missing or the access token is empty
/// - The endpoint URL is invalid
/// - The Zoom API request fails (network error, authentication failure, etc.)
#[tool]
pub async fn list_recordings(
    ctx: Context,
    input: ListRecordingsInput,
) -> Result<ListRecordingsOutput> {
    if let Some(page_size) = input.page_size {
        ensure!(
            (1..=300).contains(&page_size),
            "page_size must be between 1 and 300"
        );
    }

    let client = ZoomClient::from_ctx(&ctx)?;

    let mut query = Vec::new();
    if let Some(from) = input.from {
        query.push(("from", from));
    }
    if let Some(to) = input.to {
        query.push(("to", to));
    }
    if let Some(page_size) = input.page_size {
        query.push(("page_size", page_size.to_string()));
    }

    let response: ListRecordingsResponse = client
        .get_json(
            client.url_with_segments(&["users", "me", "recordings"])?,
            &query,
        )
        .await?;

    Ok(ListRecordingsOutput {
        recordings: response.meetings,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InviteAttendeesInput {
    /// Meeting ID to add registrants to
    pub meeting_id: i64,
    /// Attendees to invite (email addresses)
    pub attendees: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct InviteAttendeesOutput {
    pub invited_count: usize,
    pub join_urls: Vec<AttendeeJoinInfo>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AttendeeJoinInfo {
    pub email: String,
    pub join_url: String,
    pub registrant_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FetchTranscriptInput {
    /// Meeting ID to fetch transcript for
    pub meeting_id: i64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FetchTranscriptOutput {
    pub transcript: Option<Transcript>,
}

/// # Invite Zoom Meeting Attendees
///
/// Registers attendees for a Zoom meeting and returns their unique join URLs.
///
/// Use this tool when a user wants to invite specific people to a meeting. Each
/// attendee is registered individually and receives a unique join URL. This is
/// required for meetings that require registration or when you need to track
/// individual attendees. The tool returns the count of invited attendees
/// and their join URLs with registrant IDs for future reference.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - meetings
/// - zoom
/// - scheduling
/// - invitations
///
/// # Errors
///
/// Returns an error if:
/// - The `meeting_id` is not positive
/// - The attendees list is empty
/// - Any attendee email is empty or contains only whitespace
/// - The Zoom credential is missing or the access token is empty
/// - The endpoint URL is invalid
/// - The Zoom API request fails (network error, authentication failure, etc.)
#[tool]
pub async fn invite_attendees(
    ctx: Context,
    input: InviteAttendeesInput,
) -> Result<InviteAttendeesOutput> {
    ensure!(input.meeting_id > 0, "meeting_id must be positive");
    ensure!(
        !input.attendees.is_empty(),
        "attendees must contain at least one email"
    );
    ensure!(
        input.attendees.iter().all(|e| !e.trim().is_empty()),
        "attendee emails must not be empty"
    );

    let client = ZoomClient::from_ctx(&ctx)?;

    let mut join_urls = Vec::new();

    // Add each attendee as a registrant
    for email in &input.attendees {
        let request = types::AddRegistrantRequest {
            email: email.clone(),
            first_name: None,
            last_name: None,
        };

        let response: types::AddRegistrantResponse = client
            .post_json(
                client.url_with_segments(&[
                    "meetings",
                    &input.meeting_id.to_string(),
                    "registrants",
                ])?,
                &request,
            )
            .await?;

        join_urls.push(AttendeeJoinInfo {
            email: email.clone(),
            join_url: response.join_url,
            registrant_id: response.registrant_id,
        });
    }

    Ok(InviteAttendeesOutput {
        invited_count: join_urls.len(),
        join_urls,
    })
}

/// # Fetch Zoom Transcript
///
/// Retrieves the transcript for a recorded Zoom meeting, if available.
///
/// Use this tool when a user wants to access the transcript of a previously
/// recorded meeting. The tool searches for transcript files (VTT or TRANSCRIPT
/// type) in the meeting's recording files and downloads the content.
/// Returns the transcript content if found, or None if the meeting has no
/// transcript. Requires that the meeting was recorded with transcription
/// enabled.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - meetings
/// - zoom
/// - recordings
/// - transcripts
///
/// # Errors
///
/// Returns an error if:
/// - The `meeting_id` is not positive
/// - The Zoom credential is missing or the access token is empty
/// - The endpoint URL is invalid
/// - The Zoom API request fails (network error, authentication failure, etc.)
#[tool]
pub async fn fetch_transcript(
    ctx: Context,
    input: FetchTranscriptInput,
) -> Result<FetchTranscriptOutput> {
    ensure!(input.meeting_id > 0, "meeting_id must be positive");

    let client = ZoomClient::from_ctx(&ctx)?;

    // Get recording details which includes transcript files
    let recording: Recording = client
        .get_json(
            client.url_with_segments(&["meetings", &input.meeting_id.to_string(), "recordings"])?,
            &[],
        )
        .await?;

    // Find transcript file (VTT or TRANSCRIPT type)
    let transcript_file = recording
        .recording_files
        .iter()
        .find(|f| {
            f.file_type
                .as_ref()
                .is_some_and(|t| t == "VTT" || t == "TRANSCRIPT")
        })
        .or_else(|| {
            recording.recording_files.iter().find(|f| {
                f.recording_type
                    .as_ref()
                    .is_some_and(|t| t.contains("transcript"))
            })
        });

    if let Some(file) = transcript_file {
        if let Some(download_url) = &file.download_url {
            // Download transcript content
            let content = client.download_text(download_url).await?;

            Ok(FetchTranscriptOutput {
                transcript: Some(Transcript {
                    meeting_id: input.meeting_id.to_string(),
                    content,
                    file_type: file.file_type.clone(),
                }),
            })
        } else {
            Ok(FetchTranscriptOutput { transcript: None })
        }
    } else {
        Ok(FetchTranscriptOutput { transcript: None })
    }
}

#[derive(Debug, Clone)]
struct ZoomClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl ZoomClient {
    /// # Errors
    ///
    /// Returns an error if:
    /// - The Zoom credential is missing from the context
    /// - The access token is empty or contains only whitespace
    /// - The endpoint URL is invalid
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = ZoomCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url = normalize_base_url(
            cred.endpoint
                .as_deref()
                .unwrap_or(DEFAULT_ZOOM_API_ENDPOINT),
        )?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            access_token: cred.access_token,
        })
    }

    /// # Errors
    ///
    /// Returns an error if the `base_url` is not an absolute URL (cannot be a
    /// relative URL).
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

    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails (network error, connection timeout, etc.)
    /// - The Zoom API returns a non-success status code
    /// - The response body cannot be parsed as JSON
    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        query: &[(&str, String)],
    ) -> Result<T> {
        let response = self.send_request(self.http.get(url).query(query)).await?;
        Ok(response.json::<T>().await?)
    }

    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails (network error, connection timeout, etc.)
    /// - The Zoom API returns a non-success status code
    /// - The response body cannot be parsed as JSON
    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
    ) -> Result<TRes> {
        let response = self.send_request(self.http.post(url).json(body)).await?;
        Ok(response.json::<TRes>().await?)
    }

    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails (network error, connection timeout, etc.)
    /// - The Zoom API returns a non-success status code
    async fn patch_empty<TReq: Serialize>(&self, url: reqwest::Url, body: &TReq) -> Result<()> {
        self.send_request(self.http.patch(url).json(body)).await?;
        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails (network error, connection timeout, etc.)
    /// - The Zoom API returns a non-success status code
    /// - The response body cannot be read as text
    async fn download_text(&self, url: &str) -> Result<String> {
        let response = self.send_request(self.http.get(url)).await?;
        Ok(response.text().await?)
    }

    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails (network error, connection timeout, etc.)
    /// - The Zoom API returns a non-success status code
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
                "Zoom API request failed ({status}): {body}"
            ))
        }
    }
}

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

    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_string_contains, header, method, path, path_regex, query_param},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut zoom_values = HashMap::new();
        zoom_values.insert("access_token".to_string(), "test-token".to_string());
        zoom_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("zoom", zoom_values)
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://api.zoom.us/v2/").unwrap();
        assert_eq!(result, "https://api.zoom.us/v2");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://api.zoom.us/v2  ").unwrap();
        assert_eq!(result, "https://api.zoom.us/v2");
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
    async fn test_schedule_meeting_empty_topic_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = schedule_meeting(
            ctx,
            ScheduleMeetingInput {
                topic: "   ".to_string(),
                start_time: None,
                duration: None,
                timezone: None,
                agenda: None,
                password: None,
                host_video: None,
                participant_video: None,
                waiting_room: None,
                join_before_host: None,
                mute_upon_entry: None,
                auto_recording: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("topic must not be empty")
        );
    }

    #[tokio::test]
    async fn test_schedule_meeting_negative_duration_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = schedule_meeting(
            ctx,
            ScheduleMeetingInput {
                topic: "Test Meeting".to_string(),
                start_time: None,
                duration: Some(-10),
                timezone: None,
                agenda: None,
                password: None,
                host_video: None,
                participant_video: None,
                waiting_room: None,
                join_before_host: None,
                mute_upon_entry: None,
                auto_recording: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("duration must be positive")
        );
    }

    #[tokio::test]
    async fn test_update_meeting_invalid_meeting_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = update_meeting(
            ctx,
            UpdateMeetingInput {
                meeting_id: 0,
                topic: Some("Updated".to_string()),
                start_time: None,
                duration: None,
                timezone: None,
                agenda: None,
                waiting_room: None,
                join_before_host: None,
                mute_upon_entry: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("meeting_id must be positive")
        );
    }

    #[tokio::test]
    async fn test_list_recordings_invalid_page_size_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = list_recordings(
            ctx,
            ListRecordingsInput {
                from: None,
                to: None,
                page_size: Some(500),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("page_size must be between 1 and 300")
        );
    }

    #[tokio::test]
    async fn test_fetch_transcript_invalid_meeting_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = fetch_transcript(ctx, FetchTranscriptInput { meeting_id: 0 }).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("meeting_id must be positive")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_schedule_meeting_success() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "id": 123456789,
          "topic": "Team Standup",
          "start_time": "2024-01-15T10:00:00Z",
          "duration": 30,
          "timezone": "America/New_York",
          "join_url": "https://zoom.us/j/123456789",
          "password": "abc123"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/users/me/meetings"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("\"topic\":\"Team Standup\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = schedule_meeting(
            ctx,
            ScheduleMeetingInput {
                topic: "Team Standup".to_string(),
                start_time: Some("2024-01-15T10:00:00Z".to_string()),
                duration: Some(30),
                timezone: Some("America/New_York".to_string()),
                agenda: None,
                password: None,
                host_video: None,
                participant_video: None,
                waiting_room: None,
                join_before_host: None,
                mute_upon_entry: None,
                auto_recording: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.meeting.id, 123_456_789);
        assert_eq!(output.meeting.topic.as_deref(), Some("Team Standup"));
        assert_eq!(output.meeting.duration, Some(30));
    }

    #[tokio::test]
    async fn test_update_meeting_success() {
        let server = MockServer::start().await;

        Mock::given(method("PATCH"))
            .and(path_regex(r"^/meetings/\d+$"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("\"topic\":\"Updated Topic\""))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = update_meeting(
            ctx,
            UpdateMeetingInput {
                meeting_id: 123_456_789,
                topic: Some("Updated Topic".to_string()),
                start_time: None,
                duration: Some(45),
                timezone: None,
                agenda: None,
                waiting_room: None,
                join_before_host: None,
                mute_upon_entry: None,
            },
        )
        .await
        .unwrap();

        assert!(output.updated);
        assert_eq!(output.meeting_id, 123_456_789);
    }

    #[tokio::test]
    async fn test_list_recordings_success() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "meetings": [
            {
              "id": 123456789,
              "uuid": "abc-def-ghi",
              "host_id": "user123",
              "topic": "Team Meeting",
              "start_time": "2024-01-10T10:00:00Z",
              "duration": 30,
              "total_size": 1048576,
              "recording_files": [
                {
                  "id": "file1",
                  "recording_type": "shared_screen_with_speaker_view",
                  "file_type": "MP4",
                  "file_size": 1048576,
                  "download_url": "https://zoom.us/rec/download/abc123"
                }
              ]
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/users/me/recordings"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("page_size", "30"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = list_recordings(
            ctx,
            ListRecordingsInput {
                from: None,
                to: None,
                page_size: Some(30),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.recordings.len(), 1);
        assert_eq!(output.recordings[0].id, 123_456_789);
        assert_eq!(output.recordings[0].topic.as_deref(), Some("Team Meeting"));
        assert_eq!(output.recordings[0].recording_files.len(), 1);
    }

    #[tokio::test]
    async fn test_fetch_transcript_success() {
        let server = MockServer::start().await;
        let transcript_url = format!("{}/transcript/download", server.uri());

        let recording_response = format!(
            r#"{{
              "id": 123456789,
              "recording_files": [
                {{
                  "id": "transcript1",
                  "recording_type": "audio_transcript",
                  "file_type": "VTT",
                  "download_url": "{transcript_url}"
                }}
              ]
            }}"#
        );

        let transcript_content =
            "WEBVTT\n\n00:00:00.000 --> 00:00:05.000\nHello, this is a test transcript.";

        Mock::given(method("GET"))
            .and(path_regex(r"^/meetings/\d+/recordings$"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(recording_response, "application/json"),
            )
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/transcript/download"))
            .respond_with(ResponseTemplate::new(200).set_body_string(transcript_content))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = fetch_transcript(
            ctx,
            FetchTranscriptInput {
                meeting_id: 123_456_789,
            },
        )
        .await
        .unwrap();

        assert!(output.transcript.is_some());
        let transcript = output.transcript.unwrap();
        assert_eq!(transcript.meeting_id, "123456789");
        assert!(
            transcript
                .content
                .contains("Hello, this is a test transcript")
        );
    }

    #[tokio::test]
    async fn test_fetch_transcript_no_transcript_returns_none() {
        let server = MockServer::start().await;

        let recording_response = r#"
        {
          "id": 123456789,
          "recording_files": [
            {
              "id": "video1",
              "recording_type": "shared_screen",
              "file_type": "MP4"
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path_regex(r"^/meetings/\d+/recordings$"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(recording_response, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = fetch_transcript(
            ctx,
            FetchTranscriptInput {
                meeting_id: 123_456_789,
            },
        )
        .await
        .unwrap();

        assert!(output.transcript.is_none());
    }

    #[tokio::test]
    async fn test_schedule_meeting_error_response() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/users/me/meetings"))
            .respond_with(ResponseTemplate::new(401).set_body_raw(
                r#"{ "code": 124, "message": "Invalid access token" }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let result = schedule_meeting(
            ctx,
            ScheduleMeetingInput {
                topic: "Test".to_string(),
                start_time: None,
                duration: None,
                timezone: None,
                agenda: None,
                password: None,
                host_video: None,
                participant_video: None,
                waiting_room: None,
                join_before_host: None,
                mute_upon_entry: None,
                auto_recording: None,
            },
        )
        .await;

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("401"));
    }

    // --- invite_attendees tests ---

    #[tokio::test]
    async fn test_invite_attendees_invalid_meeting_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = invite_attendees(
            ctx,
            InviteAttendeesInput {
                meeting_id: 0,
                attendees: vec!["test@example.com".to_string()],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("meeting_id must be positive")
        );
    }

    #[tokio::test]
    async fn test_invite_attendees_empty_attendees_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = invite_attendees(
            ctx,
            InviteAttendeesInput {
                meeting_id: 123_456_789,
                attendees: vec![],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("attendees must contain at least one email")
        );
    }

    #[tokio::test]
    async fn test_invite_attendees_empty_email_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = invite_attendees(
            ctx,
            InviteAttendeesInput {
                meeting_id: 123_456_789,
                attendees: vec!["  ".to_string()],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("attendee emails must not be empty")
        );
    }

    #[tokio::test]
    async fn test_invite_attendees_success_single_attendee() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "id": 1,
          "registrant_id": "reg-abc123",
          "join_url": "https://zoom.us/j/123_456_789?tk=abc123",
          "topic": "Team Meeting"
        }
        "#;

        Mock::given(method("POST"))
            .and(path_regex(r"^/meetings/\d+/registrants$"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("\"email\":\"alice@example.com\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = invite_attendees(
            ctx,
            InviteAttendeesInput {
                meeting_id: 123_456_789,
                attendees: vec!["alice@example.com".to_string()],
            },
        )
        .await
        .unwrap();

        assert_eq!(output.invited_count, 1);
        assert_eq!(output.join_urls.len(), 1);
        assert_eq!(output.join_urls[0].email, "alice@example.com");
        assert_eq!(output.join_urls[0].registrant_id, "reg-abc123");
        assert_eq!(
            output.join_urls[0].join_url,
            "https://zoom.us/j/123_456_789?tk=abc123"
        );
    }

    #[tokio::test]
    async fn test_invite_attendees_success_multiple_attendees() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path_regex(r"^/meetings/\d+/registrants$"))
            .and(body_string_contains("\"email\":\"alice@example.com\""))
            .respond_with(ResponseTemplate::new(201).set_body_raw(
                r#"{ "id": 1, "registrant_id": "reg-1", "join_url": "https://zoom.us/j/123_456_789?tk=1" }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path_regex(r"^/meetings/\d+/registrants$"))
            .and(body_string_contains("\"email\":\"bob@example.com\""))
            .respond_with(ResponseTemplate::new(201).set_body_raw(
                r#"{ "id": 2, "registrant_id": "reg-2", "join_url": "https://zoom.us/j/123_456_789?tk=2" }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = invite_attendees(
            ctx,
            InviteAttendeesInput {
                meeting_id: 123_456_789,
                attendees: vec![
                    "alice@example.com".to_string(),
                    "bob@example.com".to_string(),
                ],
            },
        )
        .await
        .unwrap();

        assert_eq!(output.invited_count, 2);
        assert_eq!(output.join_urls.len(), 2);
        assert_eq!(output.join_urls[0].email, "alice@example.com");
        assert_eq!(output.join_urls[0].registrant_id, "reg-1");
        assert_eq!(output.join_urls[1].email, "bob@example.com");
        assert_eq!(output.join_urls[1].registrant_id, "reg-2");
    }

    #[tokio::test]
    async fn test_invite_attendees_error_response() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path_regex(r"^/meetings/\d+/registrants$"))
            .respond_with(ResponseTemplate::new(404).set_body_raw(
                r#"{ "code": 3001, "message": "Meeting not found" }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let result = invite_attendees(
            ctx,
            InviteAttendeesInput {
                meeting_id: 123_456_789,
                attendees: vec!["alice@example.com".to_string()],
            },
        )
        .await;

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("404"));
    }
}
