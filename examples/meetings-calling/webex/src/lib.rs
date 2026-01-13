//! meetings-calling/webex integration for Operai Toolbox.

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

mod types;
use types::{Meeting, MeetingInvitee, Recording, Transcript};

define_user_credential! {
    WebexCredential("webex") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_WEBEX_API_ENDPOINT: &str = "https://webexapis.com/v1";

#[init]
async fn setup() -> Result<()> {
    info!("Webex Meetings integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Webex Meetings integration shutting down");
}

// ============================================================================
// Schedule Meeting Tool
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ScheduleMeetingInput {
    /// Meeting title/subject.
    pub title: String,
    /// Meeting agenda/description.
    #[serde(default)]
    pub agenda: Option<String>,
    /// Meeting start time (ISO 8601 format, e.g., "2024-01-15T10:00:00Z").
    pub start: String,
    /// Meeting end time (ISO 8601 format, e.g., "2024-01-15T11:00:00Z").
    pub end: String,
    /// Timezone for the meeting (e.g., "`America/New_York`"). Defaults to UTC.
    #[serde(default)]
    pub timezone: Option<String>,
    /// Meeting password.
    #[serde(default)]
    pub password: Option<String>,
    /// Whether to enable meeting registration.
    #[serde(default)]
    pub enable_registration: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ScheduleMeetingOutput {
    /// The created meeting details.
    pub meeting: Meeting,
}

/// # Schedule Webex Meeting
///
/// Schedules a new Webex meeting with the specified details.
///
/// Use this tool when a user wants to create a new meeting in Webex. The tool
/// configures sensible defaults for meeting settings, including enabling
/// participants to join before the host and sending email invitations to
/// attendees.
///
/// Required parameters:
/// - `title`: Meeting title/subject (must not be empty)
/// - `start`: Meeting start time in ISO 8601 format (e.g.,
///   "2024-01-15T10:00:00Z")
/// - `end`: Meeting end time in ISO 8601 format (e.g., "2024-01-15T11:00:00Z")
///
/// Optional parameters:
/// - `agenda`: Meeting description or agenda
/// - `timezone`: Timezone for the meeting (e.g., "`America/New_York`"),
///   defaults to UTC
/// - `password`: Meeting password for added security
/// - `enable_registration`: Whether to require meeting registration
///
/// Returns the created meeting details including the meeting ID, web link, and
/// host information.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - meetings
/// - webex
/// - scheduling
///
/// # Errors
///
/// Returns an error if:
/// - The title, start time, or end time fields are empty or contain only
///   whitespace
/// - Webex credentials are not configured or the access token is empty
/// - The configured endpoint URL is invalid
/// - The Webex API request fails (network error, timeout, etc.)
/// - The Webex API returns an error response (e.g., invalid parameters,
///   authentication failure)
/// - The response JSON cannot be parsed
#[tool]
pub async fn schedule_meeting(
    ctx: Context,
    input: ScheduleMeetingInput,
) -> Result<ScheduleMeetingOutput> {
    ensure!(!input.title.trim().is_empty(), "title must not be empty");
    ensure!(!input.start.trim().is_empty(), "start must not be empty");
    ensure!(!input.end.trim().is_empty(), "end must not be empty");

    let client = WebexClient::from_ctx(&ctx)?;

    let request = CreateMeetingRequest {
        title: input.title,
        agenda: input.agenda,
        start: input.start,
        end: input.end,
        timezone: input.timezone,
        password: input.password,
        enable_join_before_host: Some(true),
        enable_auto_record_meeting: Some(false),
        allow_any_user_to_be_co_host: Some(false),
        enabled_auto_share_recording: Some(false),
        send_email: Some(true),
        public_meeting: Some(false),
        enable_connect_audio_before_host: Some(true),
    };

    let meeting: Meeting = client
        .post_json(client.url_with_path("/meetings")?, &request, &[])
        .await?;

    Ok(ScheduleMeetingOutput { meeting })
}

// ============================================================================
// Update Meeting Tool
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateMeetingInput {
    /// Meeting ID to update.
    pub meeting_id: String,
    /// New meeting title/subject.
    #[serde(default)]
    pub title: Option<String>,
    /// New meeting agenda/description.
    #[serde(default)]
    pub agenda: Option<String>,
    /// New meeting start time (ISO 8601 format).
    #[serde(default)]
    pub start: Option<String>,
    /// New meeting end time (ISO 8601 format).
    #[serde(default)]
    pub end: Option<String>,
    /// New meeting password.
    #[serde(default)]
    pub password: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateMeetingOutput {
    /// The updated meeting details.
    pub meeting: Meeting,
}

/// # Update Webex Meeting
///
/// Updates details of an existing Webex meeting.
///
/// Use this tool when a user wants to modify an already scheduled meeting, such
/// as changing the title, time, agenda, or password. Only the fields that are
/// provided will be updated; unspecified fields remain unchanged.
///
/// Required parameters:
/// - `meeting_id`: The ID of the meeting to update (must not be empty)
///
/// Optional parameters:
/// - `title`: New meeting title/subject
/// - `agenda`: New meeting description or agenda
/// - `start`: New meeting start time in ISO 8601 format
/// - `end`: New meeting end time in ISO 8601 format
/// - `password`: New meeting password
///
/// Returns the updated meeting details with all current values.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - meetings
/// - webex
/// - scheduling
///
/// # Errors
///
/// Returns an error if:
/// - The `meeting_id` field is empty or contains only whitespace
/// - Webex credentials are not configured or the access token is empty
/// - The configured endpoint URL is invalid
/// - The Webex API request fails (network error, timeout, etc.)
/// - The Webex API returns an error response (e.g., meeting not found, invalid
///   parameters)
/// - The response JSON cannot be parsed
#[tool]
pub async fn update_meeting(
    ctx: Context,
    input: UpdateMeetingInput,
) -> Result<UpdateMeetingOutput> {
    ensure!(
        !input.meeting_id.trim().is_empty(),
        "meeting_id must not be empty"
    );

    let client = WebexClient::from_ctx(&ctx)?;

    let request = UpdateMeetingRequest {
        title: input.title,
        agenda: input.agenda,
        start: input.start,
        end: input.end,
        password: input.password,
    };

    let meeting: Meeting = client
        .put_json(
            client.url_with_path(&format!("/meetings/{}", input.meeting_id))?,
            &request,
            &[],
        )
        .await?;

    Ok(UpdateMeetingOutput { meeting })
}

// ============================================================================
// List Recordings Tool
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListRecordingsInput {
    /// Filter recordings by meeting ID (optional).
    #[serde(default)]
    pub meeting_id: Option<String>,
    /// Filter recordings from this date onwards (ISO 8601 format).
    #[serde(default)]
    pub from: Option<String>,
    /// Filter recordings up to this date (ISO 8601 format).
    #[serde(default)]
    pub to: Option<String>,
    /// Maximum number of recordings to return (1-100). Defaults to 10.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListRecordingsOutput {
    /// List of recordings.
    pub recordings: Vec<Recording>,
}

/// # List Webex Recordings
///
/// Lists Webex meeting recordings with optional filtering by meeting, date
/// range, or count.
///
/// Use this tool when a user wants to browse, search, or retrieve recordings of
/// past Webex meetings. Recordings can be filtered by a specific meeting ID or
/// narrowed to a date range. This is useful for finding recordings of
/// particular meetings or reviewing recent recordings.
///
/// Optional parameters:
/// - `meeting_id`: Filter recordings to a specific meeting ID
/// - `from`: Filter recordings from this date onwards (ISO 8601 format)
/// - `to`: Filter recordings up to this date (ISO 8601 format)
/// - `limit`: Maximum number of recordings to return (1-100, defaults to 10)
///
/// Returns a list of recordings with metadata including recording ID, topic,
/// duration, creation time, and status.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - meetings
/// - webex
/// - recordings
///
/// # Errors
///
/// Returns an error if:
/// - The `limit` parameter is provided but not between 1 and 100
/// - The `meeting_id`, `from`, or `to` parameters are provided but contain only
///   whitespace
/// - Webex credentials are not configured or the access token is empty
/// - The configured endpoint URL is invalid
/// - The Webex API request fails (network error, timeout, etc.)
/// - The Webex API returns an error response
/// - The response JSON cannot be parsed
#[tool]
pub async fn list_recordings(
    ctx: Context,
    input: ListRecordingsInput,
) -> Result<ListRecordingsOutput> {
    let limit = input.limit.unwrap_or(10);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let client = WebexClient::from_ctx(&ctx)?;

    let mut query_params = vec![("max", limit.to_string())];

    if let Some(ref meeting_id) = input.meeting_id {
        ensure!(
            !meeting_id.trim().is_empty(),
            "meeting_id must not be empty if provided"
        );
        query_params.push(("meetingId", meeting_id.clone()));
    }

    if let Some(ref from) = input.from {
        ensure!(
            !from.trim().is_empty(),
            "from must not be empty if provided"
        );
        query_params.push(("from", from.clone()));
    }

    if let Some(ref to) = input.to {
        ensure!(!to.trim().is_empty(), "to must not be empty if provided");
        query_params.push(("to", to.clone()));
    }

    let response: WebexListResponse<Recording> = client
        .get_json(client.url_with_path("/recordings")?, &query_params, &[])
        .await?;

    Ok(ListRecordingsOutput {
        recordings: response.items,
    })
}

// ============================================================================
// List Transcripts Tool
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTranscriptsInput {
    /// Filter transcripts by meeting ID (optional).
    #[serde(default)]
    pub meeting_id: Option<String>,
    /// Maximum number of transcripts to return (1-100). Defaults to 10.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListTranscriptsOutput {
    /// List of transcripts.
    pub transcripts: Vec<Transcript>,
}

/// # List Webex Transcripts
///
/// Lists Webex meeting transcripts with optional filtering.
///
/// Use this tool when a user wants to browse or retrieve transcripts of Webex
/// meetings. Transcripts provide text records of spoken content during
/// meetings. This tool can be used to find transcripts for a specific meeting
/// or to review available transcripts in general.
///
/// Optional parameters:
/// - `meeting_id`: Filter transcripts to a specific meeting ID
/// - `limit`: Maximum number of transcripts to return (1-100, defaults to 10)
///
/// Returns a list of transcripts with metadata including transcript ID, meeting
/// ID, host email, meeting times, and download URLs for different formats (VTT
/// and TXT).
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - meetings
/// - webex
/// - transcripts
///
/// # Errors
///
/// Returns an error if:
/// - The `limit` parameter is provided but not between 1 and 100
/// - The `meeting_id` parameter is provided but contains only whitespace
/// - Webex credentials are not configured or the access token is empty
/// - The configured endpoint URL is invalid
/// - The Webex API request fails (network error, timeout, etc.)
/// - The Webex API returns an error response
/// - The response JSON cannot be parsed
#[tool]
pub async fn list_transcripts(
    ctx: Context,
    input: ListTranscriptsInput,
) -> Result<ListTranscriptsOutput> {
    let limit = input.limit.unwrap_or(10);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let client = WebexClient::from_ctx(&ctx)?;

    let mut query_params = vec![("max", limit.to_string())];

    if let Some(ref meeting_id) = input.meeting_id {
        ensure!(
            !meeting_id.trim().is_empty(),
            "meeting_id must not be empty if provided"
        );
        query_params.push(("meetingId", meeting_id.clone()));
    }

    let response: WebexListResponse<Transcript> = client
        .get_json(
            client.url_with_path("/meetingTranscripts")?,
            &query_params,
            &[],
        )
        .await?;

    Ok(ListTranscriptsOutput {
        transcripts: response.items,
    })
}

// ============================================================================
// Get Transcript Tool
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetTranscriptInput {
    /// Transcript ID to fetch.
    pub transcript_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetTranscriptOutput {
    /// The transcript details.
    pub transcript: Transcript,
}

/// # Get Webex Transcript
///
/// Fetches details for a specific Webex meeting transcript by ID.
///
/// Use this tool when a user wants to retrieve metadata and information about
/// a specific transcript. This returns the transcript details including meeting
/// information, timestamps, and download URLs. To actually download the
/// transcript content, use the "Webex Get Transcript Download URL" tool.
///
/// Required parameters:
/// - `transcript_id`: The ID of the transcript to fetch (must not be empty)
///
/// Returns the transcript details including transcript ID, meeting ID, host
/// email, meeting start/end times, creation time, and download URLs for VTT
/// and TXT formats.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - meetings
/// - webex
/// - transcripts
///
/// # Errors
///
/// Returns an error if:
/// - The `transcript_id` field is empty or contains only whitespace
/// - Webex credentials are not configured or the access token is empty
/// - The configured endpoint URL is invalid
/// - The Webex API request fails (network error, timeout, etc.)
/// - The Webex API returns an error response (e.g., transcript not found)
/// - The response JSON cannot be parsed
#[tool]
pub async fn get_transcript(
    ctx: Context,
    input: GetTranscriptInput,
) -> Result<GetTranscriptOutput> {
    ensure!(
        !input.transcript_id.trim().is_empty(),
        "transcript_id must not be empty"
    );

    let client = WebexClient::from_ctx(&ctx)?;

    // First, get the transcript details from the list endpoint
    let query_params = vec![("transcriptId", input.transcript_id.clone())];
    let response: WebexListResponse<Transcript> = client
        .get_json(
            client.url_with_path("/meetingTranscripts")?,
            &query_params,
            &[],
        )
        .await?;

    // Get the first transcript from the results
    let transcript = response
        .items
        .into_iter()
        .next()
        .ok_or_else(|| operai::anyhow::anyhow!("Transcript not found"))?;

    Ok(GetTranscriptOutput { transcript })
}

// ============================================================================
// Get Transcript Download URL Tool
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetTranscriptDownloadUrlInput {
    /// Transcript ID to fetch download URL for.
    pub transcript_id: String,
    /// Format for the transcript ("vtt" or "txt"). Defaults to "txt".
    #[serde(default)]
    pub format: Option<TranscriptFormat>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptFormat {
    /// VTT (`WebVTT`) format
    Vtt,
    /// Plain text format
    Txt,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetTranscriptDownloadUrlOutput {
    /// The transcript details.
    pub transcript: Transcript,
    /// The download URL for the requested format.
    pub download_url: String,
}

/// # Get Webex Transcript Download URL
///
/// Fetches the download URL for a Webex meeting transcript in a specified
/// format.
///
/// Use this tool when a user wants to download or access the actual transcript
/// content from a Webex meeting. The returned URL can be used to download the
/// transcript as either a `WebVTT` (.vtt) file with captions or a plain text
/// (.txt) file.
///
/// Required parameters:
/// - `transcript_id`: The ID of the transcript to download (must not be empty)
///
/// Optional parameters:
/// - `format`: Desired transcript format - "vtt" for `WebVTT` captions or "txt"
///   for plain text (defaults to "txt")
///
/// Returns the transcript metadata along with a download URL for the requested
/// format. The URL can be used to download the transcript file directly.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - meetings
/// - webex
/// - transcripts
///
/// # Errors
///
/// Returns an error if:
/// - The `transcript_id` field is empty or contains only whitespace
/// - Webex credentials are not configured or the access token is empty
/// - The configured endpoint URL is invalid
/// - The Webex API request fails (network error, timeout, etc.)
/// - The Webex API returns an error response (e.g., transcript not found)
/// - The transcript does not have download URLs available
/// - The requested format (VTT or TXT) download link is not available
/// - The response JSON cannot be parsed
#[tool]
pub async fn get_transcript_download_url(
    ctx: Context,
    input: GetTranscriptDownloadUrlInput,
) -> Result<GetTranscriptDownloadUrlOutput> {
    ensure!(
        !input.transcript_id.trim().is_empty(),
        "transcript_id must not be empty"
    );

    let client = WebexClient::from_ctx(&ctx)?;
    let format = input.format.unwrap_or(TranscriptFormat::Txt);

    // First, get the transcript details
    let query_params = vec![("transcriptId", input.transcript_id.clone())];
    let response: WebexListResponse<Transcript> = client
        .get_json(
            client.url_with_path("/meetingTranscripts")?,
            &query_params,
            &[],
        )
        .await?;

    let transcript = response
        .items
        .into_iter()
        .next()
        .ok_or_else(|| operai::anyhow::anyhow!("Transcript not found"))?;

    // Extract the appropriate download URL based on format
    let download_url = if let Some(ref urls) = transcript.download_urls {
        match format {
            TranscriptFormat::Vtt => urls
                .vtt_download_link
                .clone()
                .ok_or_else(|| operai::anyhow::anyhow!("VTT download link not available"))?,
            TranscriptFormat::Txt => urls
                .txt_download_link
                .clone()
                .ok_or_else(|| operai::anyhow::anyhow!("TXT download link not available"))?,
        }
    } else {
        return Err(operai::anyhow::anyhow!(
            "Download URLs not available for this transcript"
        ));
    };

    Ok(GetTranscriptDownloadUrlOutput {
        transcript,
        download_url,
    })
}

// ============================================================================
// Invite Attendees Tool
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InviteAttendeesInput {
    /// Meeting ID to invite attendees to.
    pub meeting_id: String,
    /// Email address of the invitee.
    pub email: String,
    /// Display name for the invitee.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Whether the invitee should be a co-host.
    #[serde(default)]
    pub co_host: bool,
    /// Whether the invitee should be a presenter.
    #[serde(default)]
    pub presenter: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct InviteAttendeesOutput {
    /// The created invitee details.
    pub invitee: MeetingInvitee,
}

/// # Invite Webex Attendees
///
/// Invites an attendee to a Webex meeting with optional role assignments.
///
/// Use this tool when a user wants to add a participant to an existing Webex
/// meeting. The tool sends an invitation to the specified email address and can
/// optionally assign special roles such as co-host or presenter to the invitee.
///
/// Required parameters:
/// - `meeting_id`: The ID of the meeting to invite the attendee to (must not be
///   empty)
/// - `email`: Email address of the person to invite (must not be empty)
///
/// Optional parameters:
/// - `display_name`: Display name for the invitee
/// - `co_host`: Whether the invitee should be a co-host (defaults to false)
/// - `presenter`: Whether the invitee should have presenter privileges
///   (defaults to false)
///
/// Returns the created invitee details including the invitee ID, email, display
/// name, and assigned roles.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - meetings
/// - webex
/// - invitations
///
/// # Errors
///
/// Returns an error if:
/// - The `meeting_id` or `email` fields are empty or contain only whitespace
/// - Webex credentials are not configured or the access token is empty
/// - The configured endpoint URL is invalid
/// - The Webex API request fails (network error, timeout, etc.)
/// - The Webex API returns an error response (e.g., meeting not found, invalid
///   email)
/// - The response JSON cannot be parsed
#[tool]
pub async fn invite_attendees(
    ctx: Context,
    input: InviteAttendeesInput,
) -> Result<InviteAttendeesOutput> {
    ensure!(
        !input.meeting_id.trim().is_empty(),
        "meeting_id must not be empty"
    );
    ensure!(!input.email.trim().is_empty(), "email must not be empty");

    let client = WebexClient::from_ctx(&ctx)?;

    let request = CreateInviteeRequest {
        meeting_id: input.meeting_id,
        email: input.email,
        display_name: input.display_name,
        co_host: Some(input.co_host),
        presenter: Some(input.presenter),
    };

    let invitee: MeetingInvitee = client
        .post_json(client.url_with_path("/meetingInvitees")?, &request, &[])
        .await?;

    Ok(InviteAttendeesOutput { invitee })
}

// ============================================================================
// HTTP Client and Request/Response Types
// ============================================================================

#[derive(Debug, Clone)]
struct WebexClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl WebexClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = WebexCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url = normalize_base_url(
            cred.endpoint
                .as_deref()
                .unwrap_or(DEFAULT_WEBEX_API_ENDPOINT),
        )?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            access_token: cred.access_token,
        })
    }

    fn url_with_path(&self, path: &str) -> Result<reqwest::Url> {
        let full_url = format!("{}{}", self.base_url, path);
        Ok(reqwest::Url::parse(&full_url)?)
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

    async fn put_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
        extra_headers: &[(&str, &str)],
    ) -> Result<TRes> {
        let mut request = self.http.put(url).json(body);
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
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Webex API request failed ({status}): {body}"
            ))
        }
    }
}

#[derive(Debug, Deserialize)]
struct WebexListResponse<T> {
    items: Vec<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateMeetingRequest {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    agenda: Option<String>,
    start: String,
    end: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enable_join_before_host: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enable_auto_record_meeting: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allow_any_user_to_be_co_host: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enabled_auto_share_recording: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    send_email: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    public_meeting: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enable_connect_audio_before_host: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateMeetingRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agenda: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateInviteeRequest {
    meeting_id: String,
    email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    co_host: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    presenter: Option<bool>,
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
        let mut webex_values = HashMap::new();
        webex_values.insert("access_token".to_string(), "test-token".to_string());
        webex_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("webex", webex_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        format!("{}/v1", server.uri())
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_transcript_format_serialization_roundtrip() {
        for variant in [TranscriptFormat::Vtt, TranscriptFormat::Txt] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: TranscriptFormat = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://webexapis.com/v1/").unwrap();
        assert_eq!(result, "https://webexapis.com/v1");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://webexapis.com/v1  ").unwrap();
        assert_eq!(result, "https://webexapis.com/v1");
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
    async fn test_schedule_meeting_empty_title_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = schedule_meeting(
            ctx,
            ScheduleMeetingInput {
                title: "   ".to_string(),
                agenda: None,
                start: "2024-01-15T10:00:00Z".to_string(),
                end: "2024-01-15T11:00:00Z".to_string(),
                timezone: None,
                password: None,
                enable_registration: false,
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
    async fn test_schedule_meeting_empty_start_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = schedule_meeting(
            ctx,
            ScheduleMeetingInput {
                title: "Test Meeting".to_string(),
                agenda: None,
                start: "  ".to_string(),
                end: "2024-01-15T11:00:00Z".to_string(),
                timezone: None,
                password: None,
                enable_registration: false,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("start must not be empty")
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
                title: Some("Updated Title".to_string()),
                agenda: None,
                start: None,
                end: None,
                password: None,
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
    async fn test_list_recordings_limit_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_recordings(
            ctx,
            ListRecordingsInput {
                meeting_id: None,
                from: None,
                to: None,
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
    async fn test_list_recordings_limit_exceeds_max_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_recordings(
            ctx,
            ListRecordingsInput {
                meeting_id: None,
                from: None,
                to: None,
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
    async fn test_list_transcripts_limit_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_transcripts(
            ctx,
            ListTranscriptsInput {
                meeting_id: None,
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
    async fn test_get_transcript_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = get_transcript(
            ctx,
            GetTranscriptInput {
                transcript_id: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("transcript_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_get_transcript_download_url_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = get_transcript_download_url(
            ctx,
            GetTranscriptDownloadUrlInput {
                transcript_id: "  ".to_string(),
                format: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("transcript_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_invite_attendees_empty_meeting_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = invite_attendees(
            ctx,
            InviteAttendeesInput {
                meeting_id: "  ".to_string(),
                email: "test@example.com".to_string(),
                display_name: None,
                co_host: false,
                presenter: false,
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
    async fn test_invite_attendees_empty_email_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = invite_attendees(
            ctx,
            InviteAttendeesInput {
                meeting_id: "meeting-123".to_string(),
                email: "  ".to_string(),
                display_name: None,
                co_host: false,
                presenter: false,
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
    async fn test_schedule_meeting_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "meeting-123",
          "meetingNumber": "1234567890",
          "title": "Test Meeting",
          "agenda": "Test agenda",
          "start": "2024-01-15T10:00:00Z",
          "end": "2024-01-15T11:00:00Z",
          "timezone": "UTC",
          "webLink": "https://example.webex.com/meet/meeting-123",
          "hostUserId": "user-123",
          "hostEmail": "host@example.com"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/v1/meetings"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("\"title\":\"Test Meeting\""))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = schedule_meeting(
            ctx,
            ScheduleMeetingInput {
                title: "Test Meeting".to_string(),
                agenda: Some("Test agenda".to_string()),
                start: "2024-01-15T10:00:00Z".to_string(),
                end: "2024-01-15T11:00:00Z".to_string(),
                timezone: Some("UTC".to_string()),
                password: None,
                enable_registration: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.meeting.id, "meeting-123");
        assert_eq!(output.meeting.title.as_deref(), Some("Test Meeting"));
    }

    #[tokio::test]
    async fn test_update_meeting_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "meeting-123",
          "title": "Updated Meeting",
          "start": "2024-01-15T14:00:00Z",
          "end": "2024-01-15T15:00:00Z"
        }
        "#;

        Mock::given(method("PUT"))
            .and(path("/v1/meetings/meeting-123"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = update_meeting(
            ctx,
            UpdateMeetingInput {
                meeting_id: "meeting-123".to_string(),
                title: Some("Updated Meeting".to_string()),
                agenda: None,
                start: Some("2024-01-15T14:00:00Z".to_string()),
                end: Some("2024-01-15T15:00:00Z".to_string()),
                password: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.meeting.id, "meeting-123");
        assert_eq!(output.meeting.title.as_deref(), Some("Updated Meeting"));
    }

    #[tokio::test]
    async fn test_list_recordings_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "items": [
            {
              "id": "recording-1",
              "meetingId": "meeting-123",
              "topic": "Test Recording",
              "createTime": "2024-01-15T11:00:00Z",
              "durationSeconds": 3600,
              "status": "available"
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1/recordings"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("max", "10"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_recordings(
            ctx,
            ListRecordingsInput {
                meeting_id: None,
                from: None,
                to: None,
                limit: Some(10),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.recordings.len(), 1);
        assert_eq!(output.recordings[0].id, "recording-1");
        assert_eq!(
            output.recordings[0].topic.as_deref(),
            Some("Test Recording")
        );
    }

    #[tokio::test]
    async fn test_list_transcripts_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "items": [
            {
              "id": "transcript-1",
              "meetingId": "meeting-123",
              "hostEmail": "host@example.com",
              "meetingStartTime": "2024-01-15T10:00:00Z",
              "meetingEndTime": "2024-01-15T11:00:00Z",
              "createTime": "2024-01-15T11:05:00Z",
              "downloadUrls": {
                "vttDownloadLink": "https://example.com/transcript.vtt",
                "txtDownloadLink": "https://example.com/transcript.txt"
              }
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1/meetingTranscripts"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("max", "10"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_transcripts(
            ctx,
            ListTranscriptsInput {
                meeting_id: None,
                limit: Some(10),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.transcripts.len(), 1);
        assert_eq!(output.transcripts[0].id, "transcript-1");
        assert_eq!(
            output.transcripts[0].meeting_id.as_deref(),
            Some("meeting-123")
        );
    }

    #[tokio::test]
    async fn test_get_transcript_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "items": [
            {
              "id": "transcript-1",
              "meetingId": "meeting-123",
              "hostEmail": "host@example.com",
              "meetingStartTime": "2024-01-15T10:00:00Z",
              "meetingEndTime": "2024-01-15T11:00:00Z",
              "createTime": "2024-01-15T11:05:00Z",
              "downloadUrls": {
                "vttDownloadLink": "https://example.com/transcript.vtt",
                "txtDownloadLink": "https://example.com/transcript.txt"
              }
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1/meetingTranscripts"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("transcriptId", "transcript-1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = get_transcript(
            ctx,
            GetTranscriptInput {
                transcript_id: "transcript-1".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.transcript.id, "transcript-1");
        assert_eq!(output.transcript.meeting_id.as_deref(), Some("meeting-123"));
    }

    #[tokio::test]
    async fn test_get_transcript_download_url_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "items": [
            {
              "id": "transcript-1",
              "meetingId": "meeting-123",
              "downloadUrls": {
                "vttDownloadLink": "https://example.com/transcript.vtt",
                "txtDownloadLink": "https://example.com/transcript.txt"
              }
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1/meetingTranscripts"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("transcriptId", "transcript-1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = get_transcript_download_url(
            ctx,
            GetTranscriptDownloadUrlInput {
                transcript_id: "transcript-1".to_string(),
                format: Some(TranscriptFormat::Txt),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.transcript.id, "transcript-1");
        assert_eq!(output.download_url, "https://example.com/transcript.txt");
    }

    #[tokio::test]
    async fn test_get_transcript_download_url_vtt_format() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "items": [
            {
              "id": "transcript-1",
              "meetingId": "meeting-123",
              "downloadUrls": {
                "vttDownloadLink": "https://example.com/transcript.vtt",
                "txtDownloadLink": "https://example.com/transcript.txt"
              }
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1/meetingTranscripts"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("transcriptId", "transcript-1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = get_transcript_download_url(
            ctx,
            GetTranscriptDownloadUrlInput {
                transcript_id: "transcript-1".to_string(),
                format: Some(TranscriptFormat::Vtt),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.download_url, "https://example.com/transcript.vtt");
    }

    #[tokio::test]
    async fn test_get_transcript_download_url_missing_format_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "items": [
            {
              "id": "transcript-1",
              "meetingId": "meeting-123",
              "downloadUrls": {
                "txtDownloadLink": "https://example.com/transcript.txt"
              }
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1/meetingTranscripts"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("transcriptId", "transcript-1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = get_transcript_download_url(
            ctx,
            GetTranscriptDownloadUrlInput {
                transcript_id: "transcript-1".to_string(),
                format: Some(TranscriptFormat::Vtt),
            },
        )
        .await;

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("VTT download link not available"));
    }

    #[tokio::test]
    async fn test_get_transcript_not_found_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "items": []
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1/meetingTranscripts"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("transcriptId", "transcript-999"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = get_transcript(
            ctx,
            GetTranscriptInput {
                transcript_id: "transcript-999".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("Transcript not found"));
    }

    #[tokio::test]
    async fn test_invite_attendees_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "invitee-1",
          "meetingId": "meeting-123",
          "email": "attendee@example.com",
          "displayName": "Test Attendee",
          "coHost": false,
          "presenter": true
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/v1/meetingInvitees"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("\"email\":\"attendee@example.com\""))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = invite_attendees(
            ctx,
            InviteAttendeesInput {
                meeting_id: "meeting-123".to_string(),
                email: "attendee@example.com".to_string(),
                display_name: Some("Test Attendee".to_string()),
                co_host: false,
                presenter: true,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.invitee.id, "invitee-1");
        assert_eq!(output.invitee.email, "attendee@example.com");
        assert_eq!(output.invitee.presenter, Some(true));
    }

    #[tokio::test]
    async fn test_schedule_meeting_api_error_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("POST"))
            .and(path("/v1/meetings"))
            .respond_with(ResponseTemplate::new(400).set_body_raw(
                r#"{ "message": "Invalid request", "errors": [{ "description": "Invalid start time" }] }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = schedule_meeting(
            ctx,
            ScheduleMeetingInput {
                title: "Test Meeting".to_string(),
                agenda: None,
                start: "invalid".to_string(),
                end: "2024-01-15T11:00:00Z".to_string(),
                timezone: None,
                password: None,
                enable_registration: false,
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("400"));
    }
}
