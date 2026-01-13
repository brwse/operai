//! Type definitions for Zoom API responses and requests.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Meeting {
    /// Meeting ID
    pub id: i64,
    /// Meeting topic
    #[serde(default)]
    pub topic: Option<String>,
    /// Meeting start time (ISO 8601 format)
    #[serde(default)]
    pub start_time: Option<String>,
    /// Meeting duration in minutes
    #[serde(default)]
    pub duration: Option<i32>,
    /// Timezone
    #[serde(default)]
    pub timezone: Option<String>,
    /// Meeting agenda
    #[serde(default)]
    pub agenda: Option<String>,
    /// Meeting join URL
    #[serde(default)]
    pub join_url: Option<String>,
    /// Meeting password
    #[serde(default)]
    pub password: Option<String>,
    /// Host video enabled
    #[serde(default)]
    pub host_video: Option<bool>,
    /// Participant video enabled
    #[serde(default)]
    pub participant_video: Option<bool>,
    /// Meeting settings
    #[serde(default)]
    pub settings: Option<MeetingSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MeetingSettings {
    /// Enable waiting room
    #[serde(default)]
    pub waiting_room: Option<bool>,
    /// Enable join before host
    #[serde(default)]
    pub join_before_host: Option<bool>,
    /// Mute participants upon entry
    #[serde(default)]
    pub mute_upon_entry: Option<bool>,
    /// Auto recording
    #[serde(default)]
    pub auto_recording: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Recording {
    /// Meeting UUID
    #[serde(default)]
    pub uuid: Option<String>,
    /// Meeting ID
    pub id: i64,
    /// Host ID
    #[serde(default)]
    pub host_id: Option<String>,
    /// Meeting topic
    #[serde(default)]
    pub topic: Option<String>,
    /// Recording start time
    #[serde(default)]
    pub start_time: Option<String>,
    /// Recording duration in minutes
    #[serde(default)]
    pub duration: Option<i32>,
    /// Total file size in bytes
    #[serde(default)]
    pub total_size: Option<i64>,
    /// Recording files
    #[serde(default)]
    pub recording_files: Vec<RecordingFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RecordingFile {
    /// File ID
    #[serde(default)]
    pub id: Option<String>,
    /// Recording type (e.g., `"shared_screen_with_speaker_view"`,
    /// `"audio_transcript"`)
    #[serde(default)]
    pub recording_type: Option<String>,
    /// Recording start time
    #[serde(default)]
    pub recording_start: Option<String>,
    /// Recording end time
    #[serde(default)]
    pub recording_end: Option<String>,
    /// File size in bytes
    #[serde(default)]
    pub file_size: Option<i64>,
    /// Download URL (access token required)
    #[serde(default)]
    pub download_url: Option<String>,
    /// File type (e.g., "MP4", "M4A", "TRANSCRIPT", "VTT")
    #[serde(default)]
    pub file_type: Option<String>,
    /// File extension
    #[serde(default)]
    pub file_extension: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Transcript {
    /// Meeting ID
    pub meeting_id: String,
    /// Transcript text content
    pub content: String,
    /// File type (VTT or TRANSCRIPT)
    #[serde(default)]
    pub file_type: Option<String>,
}

// Internal API types (not exposed to users)

#[derive(Debug, Serialize)]
pub(crate) struct CreateMeetingRequest {
    pub topic: String,
    #[serde(rename = "type")]
    pub meeting_type: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agenda: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<MeetingSettingsRequest>,
}

#[derive(Debug, Serialize)]
pub(crate) struct MeetingSettingsRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_video: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participant_video: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiting_room: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub join_before_host: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mute_upon_entry: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_recording: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UpdateMeetingRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agenda: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<MeetingSettingsRequest>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ListRecordingsResponse {
    #[serde(default)]
    pub meetings: Vec<Recording>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AddRegistrantRequest {
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AddRegistrantResponse {
    pub registrant_id: String,
    pub join_url: String,
}
