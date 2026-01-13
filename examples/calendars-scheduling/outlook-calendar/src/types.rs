//! Types for Outlook Calendar integration.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum BodyContentType {
    Text,
    Html,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum EventShowAs {
    Free,
    Tentative,
    Busy,
    Oof,
    WorkingElsewhere,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum EventSensitivity {
    Normal,
    Personal,
    Private,
    Confidential,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AttendeeType {
    Required,
    Optional,
    Resource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ResponseStatus {
    None,
    Organizer,
    TentativelyAccepted,
    Accepted,
    Declined,
    NotResponded,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EmailAddress {
    pub address: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Recipient {
    pub email_address: EmailAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Attendee {
    pub email_address: EmailAddress,
    #[serde(rename = "type")]
    pub attendee_type: AttendeeType,
    #[serde(default)]
    pub status: Option<AttendeeResponseStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttendeeResponseStatus {
    pub response: ResponseStatus,
    #[serde(default)]
    pub time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Location {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub location_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DateTimeTimeZone {
    pub date_time: String,
    pub time_zone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ItemBody {
    pub content_type: BodyContentType,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub body: Option<ItemBody>,
    #[serde(default)]
    pub start: Option<DateTimeTimeZone>,
    #[serde(default)]
    pub end: Option<DateTimeTimeZone>,
    #[serde(default)]
    pub location: Option<Location>,
    #[serde(default)]
    pub attendees: Vec<Attendee>,
    #[serde(default)]
    pub organizer: Option<Recipient>,
    #[serde(default)]
    pub is_all_day: Option<bool>,
    #[serde(default)]
    pub show_as: Option<EventShowAs>,
    #[serde(default)]
    pub sensitivity: Option<EventSensitivity>,
    #[serde(default)]
    pub is_online_meeting: Option<bool>,
    #[serde(default)]
    pub online_meeting_url: Option<String>,
    #[serde(default)]
    pub web_link: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleInformation {
    pub schedule_id: String,
    #[serde(default)]
    pub availability_view: Option<String>,
    #[serde(default)]
    pub schedule_items: Vec<ScheduleItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleItem {
    pub status: String,
    pub start: DateTimeTimeZone,
    pub end: DateTimeTimeZone,
}
