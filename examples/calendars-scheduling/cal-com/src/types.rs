//! Type definitions for Cal.com API v2

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum BookingStatus {
    Accepted,
    Pending,
    Cancelled,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventTypeSummary {
    pub id: i64,
    pub slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookingSummary {
    pub id: i64,
    pub uid: String,
    pub start: String,
    pub end: String,
    pub duration: i32,
    pub status: BookingStatus,
    #[serde(default)]
    pub event_type: Option<EventTypeSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    pub total_items: i64,
    pub current_page: i32,
    pub total_pages: i32,
    pub has_next_page: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleAvailability {
    pub days: Vec<String>,
    pub start_time: String,
    pub end_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    pub id: i64,
    pub name: String,
    pub time_zone: String,
    pub is_default: bool,
    #[serde(default)]
    pub availability: Vec<ScheduleAvailability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventTypeLocation {
    #[serde(rename = "type")]
    pub location_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventType {
    pub id: i64,
    pub title: String,
    pub slug: String,
    pub length_in_minutes: i32,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub locations: Vec<EventTypeLocation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApiResponse<T> {
    pub data: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApiListResponse<T> {
    pub data: Vec<T>,
    #[serde(default)]
    pub pagination: Option<Pagination>,
}
