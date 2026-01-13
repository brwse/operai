//! Type definitions for the Dialpad API.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

// --- API Request Types ---

/// Request to initiate a call for a specific user.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CallRequest {
    /// The phone number to call.
    pub phone_number: String,
    /// Optional caller ID to use for the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caller_id: Option<String>,
}

/// Request to send an SMS message.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SmsRequest {
    /// The phone number or channel to send the SMS to.
    pub target: String,
    /// The message text.
    pub text: String,
    /// Optional user ID to send the SMS on behalf of.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

// --- API Response Types ---

/// Response from call initiation endpoint.
#[derive(Debug, Deserialize)]
pub(crate) struct CallResponse {
    /// The URL of the initiated call.
    pub url: String,
}

/// Response from SMS send endpoint.
#[derive(Debug, Deserialize)]
pub(crate) struct SmsResponse {
    /// Whether the SMS was successfully queued.
    pub success: bool,
    /// The SMS message ID.
    #[serde(default)]
    pub sms_id: Option<String>,
}

/// Response from Call List endpoint.
#[derive(Debug, Deserialize)]
pub(crate) struct CallListResponse {
    /// List of call logs.
    pub items: Vec<CallLog>,
    /// Whether there are more results.
    #[serde(default)]
    pub has_more: Option<bool>,
}

/// Individual call log entry.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CallLog {
    /// Unique identifier for the call.
    pub call_id: String,
    /// Caller phone number.
    #[serde(default)]
    pub from_number: Option<String>,
    /// Called phone number.
    #[serde(default)]
    pub to_number: Option<String>,
    /// Call direction (inbound/outbound).
    #[serde(default)]
    pub direction: Option<String>,
    /// Call duration in seconds.
    #[serde(default)]
    pub duration: Option<u32>,
    /// Call start timestamp (ISO 8601).
    #[serde(default)]
    pub start_time: Option<String>,
    /// Call status.
    #[serde(default)]
    pub status: Option<String>,
}
