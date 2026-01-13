//! Type definitions for Webex Meetings API

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Meeting object representing a Webex meeting
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Meeting {
    /// Unique identifier for the meeting
    pub id: String,
    /// Meeting number (visible to users)
    #[serde(rename = "meetingNumber", default)]
    pub number: Option<String>,
    /// Meeting title/subject
    #[serde(default)]
    pub title: Option<String>,
    /// Meeting agenda/description
    #[serde(default)]
    pub agenda: Option<String>,
    /// Meeting password
    #[serde(default)]
    pub password: Option<String>,
    /// Meeting series ID (if part of a series)
    #[serde(rename = "meetingSeriesId", default)]
    pub series_id: Option<String>,
    /// Scheduled meeting ID
    #[serde(default)]
    pub scheduled_meeting_id: Option<String>,
    /// Meeting type
    #[serde(rename = "meetingType", default)]
    pub type_: Option<MeetingType>,
    /// Meeting state
    #[serde(default)]
    pub state: Option<MeetingState>,
    /// Meeting host user ID
    #[serde(default)]
    pub host_user_id: Option<String>,
    /// Meeting host email
    #[serde(default)]
    pub host_email: Option<String>,
    /// Meeting host display name
    #[serde(default)]
    pub host_display_name: Option<String>,
    /// Meeting start time (ISO 8601)
    #[serde(default)]
    pub start: Option<String>,
    /// Meeting end time (ISO 8601)
    #[serde(default)]
    pub end: Option<String>,
    /// Meeting timezone
    #[serde(default)]
    pub timezone: Option<String>,
    /// Meeting web link
    #[serde(default)]
    pub web_link: Option<String>,
    /// SIP address for the meeting
    #[serde(default)]
    pub sip_address: Option<String>,
    /// Meeting duration in minutes
    #[serde(default)]
    pub duration: Option<i32>,
}

/// Meeting type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum MeetingType {
    /// Scheduled meeting
    MeetingSeries,
    /// Instance of a scheduled meeting
    ScheduledMeeting,
    /// Instant meeting
    Meeting,
}

/// Meeting state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MeetingState {
    /// Meeting is currently active
    Active,
    /// Meeting is scheduled
    Scheduled,
    /// Meeting has ended
    Ended,
    /// Meeting is in progress
    InProgress,
    /// Meeting is missed
    Missed,
    /// Meeting has expired
    Expired,
}

/// Meeting invitee
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MeetingInvitee {
    /// Unique identifier for the invitee
    pub id: String,
    /// Meeting ID this invitee is associated with
    #[serde(default)]
    pub meeting_id: Option<String>,
    /// Invitee email address
    pub email: String,
    /// Invitee display name
    #[serde(default)]
    pub display_name: Option<String>,
    /// Whether the invitee is a cohost
    #[serde(default)]
    pub co_host: Option<bool>,
    /// Whether the invitee is a presenter
    #[serde(default)]
    pub presenter: Option<bool>,
    /// Whether the invitee is required
    #[serde(default)]
    pub required: Option<bool>,
}

/// Recording object
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    /// Unique identifier for the recording
    pub id: String,
    /// Meeting ID this recording is associated with
    #[serde(default)]
    pub meeting_id: Option<String>,
    /// Recording topic/title
    #[serde(default)]
    pub topic: Option<String>,
    /// Recording creation time (ISO 8601)
    #[serde(default)]
    pub create_time: Option<String>,
    /// Recording time in UTC (ISO 8601)
    #[serde(default)]
    pub time_recorded: Option<String>,
    /// Recording duration in seconds
    #[serde(default)]
    pub duration_seconds: Option<i32>,
    /// Recording size in bytes
    #[serde(default)]
    pub size_bytes: Option<i64>,
    /// Recording share URL (temporary access link)
    #[serde(default)]
    pub temporary_direct_download_links: Option<RecordingDownloadLinks>,
    /// Recording playback URL
    #[serde(default)]
    pub playback_url: Option<String>,
    /// Recording status
    #[serde(default)]
    pub status: Option<RecordingStatus>,
}

/// Recording download links
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecordingDownloadLinks {
    /// Audio download URL
    #[serde(default)]
    pub audio_download_link: Option<String>,
    /// Video download URL
    #[serde(default)]
    pub recording_download_link: Option<String>,
    /// Expiration time for download links
    #[serde(default)]
    pub expiration: Option<String>,
}

/// Recording status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RecordingStatus {
    /// Recording is available
    Available,
    /// Recording is being processed
    Processing,
    /// Recording has been deleted
    Deleted,
}

/// Meeting transcript
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    /// Unique identifier for the transcript
    pub id: String,
    /// Meeting ID this transcript is associated with
    #[serde(default)]
    pub meeting_id: Option<String>,
    /// Meeting series ID
    #[serde(rename = "meetingSeriesId", default)]
    pub meeting_series_id: Option<String>,
    /// The email address of the meeting host
    #[serde(rename = "hostEmail", default)]
    pub host_email: Option<String>,
    /// The time, in ISO 8601 format, when the associated meeting started
    #[serde(rename = "meetingStartTime", default)]
    pub meeting_start_time: Option<String>,
    /// The time, in ISO 8601 format, when the associated meeting ended
    #[serde(rename = "meetingEndTime", default)]
    pub meeting_end_time: Option<String>,
    /// The time, in ISO 8601 format, when the transcript was created
    #[serde(rename = "createTime", default)]
    pub create_time: Option<String>,
    /// Transcript download URLs
    #[serde(rename = "downloadUrls", default)]
    pub download_urls: Option<TranscriptDownloadUrls>,
}

/// Transcript download URLs
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptDownloadUrls {
    /// VTT format download URL
    #[serde(rename = "vttDownloadLink", default)]
    pub vtt_download_link: Option<String>,
    /// TXT format download URL
    #[serde(rename = "txtDownloadLink", default)]
    pub txt_download_link: Option<String>,
}
