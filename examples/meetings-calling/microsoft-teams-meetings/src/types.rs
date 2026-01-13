//! Type definitions for Microsoft Teams Meetings integration.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Recording information returned from API.
///
/// **Note:** The Microsoft Graph API for recordings
/// (`/me/onlineMeetings/{meetingId}/recordings`) does NOT support meetings
/// created via the `POST /me/onlineMeetings` API (standalone meetings
/// not associated with calendar events). Recordings are only available for
/// meetings created via the Outlook calendar event API. See the [official documentation](https://learn.microsoft.com/en-us/graph/api/onlinemeeting-list-recordings)
/// for details.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Recording {
    pub id: String,
    pub meeting_id: String,
    pub created_date_time: String,
    /// Content URL for downloading the recording.
    #[serde(rename = "recordingContentUrl")]
    pub recording_content_url: String,
    #[serde(default)]
    pub content_correlation_id: Option<String>,
}

/// Transcript information returned from API.
///
/// **Note:** The Microsoft Graph API for transcripts
/// (`/me/onlineMeetings/{meetingId}/transcripts`) does NOT support meetings
/// created via the `POST /me/onlineMeetings` API (standalone meetings
/// not associated with calendar events). Transcripts are only available for
/// meetings created via the Outlook calendar event API. See the [official documentation](https://learn.microsoft.com/en-us/graph/api/onlinemeeting-list-transcripts)
/// for details.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Transcript {
    pub id: String,
    pub meeting_id: String,
    pub created_date_time: String,
    /// Content URL for downloading the transcript.
    #[serde(rename = "transcriptContentUrl")]
    pub transcript_content_url: String,
    #[serde(default)]
    pub content_correlation_id: Option<String>,
}

/// Internal representation for deserialization from Graph API
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphCallRecording {
    id: String,
    meeting_id: String,
    created_date_time: String,
    recording_content_url: String,
    #[serde(default)]
    content_correlation_id: Option<String>,
}

/// Internal representation for deserialization from Graph API
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphCallTranscript {
    id: String,
    meeting_id: String,
    created_date_time: String,
    transcript_content_url: String,
    #[serde(default)]
    content_correlation_id: Option<String>,
}

/// Convert from Graph API format to public output format
pub fn map_recording(recording: GraphCallRecording) -> Recording {
    Recording {
        id: recording.id,
        meeting_id: recording.meeting_id,
        created_date_time: recording.created_date_time,
        recording_content_url: recording.recording_content_url,
        content_correlation_id: recording.content_correlation_id,
    }
}

/// Convert from Graph API format to public output format
pub fn map_transcript(transcript: GraphCallTranscript) -> Transcript {
    Transcript {
        id: transcript.id,
        meeting_id: transcript.meeting_id,
        created_date_time: transcript.created_date_time,
        transcript_content_url: transcript.transcript_content_url,
        content_correlation_id: transcript.content_correlation_id,
    }
}
