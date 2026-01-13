//! Type definitions for Microsoft To Do API (Microsoft Graph).

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// Represents a task list in Microsoft To Do.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TodoTaskList {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub is_owner: bool,
    #[serde(default)]
    pub is_shared: bool,
    #[serde(default)]
    pub wellknown_list_name: Option<String>,
}

/// Represents a task in Microsoft To Do.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TodoTask {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub body: Option<ItemBody>,
    #[serde(default)]
    pub status: TaskStatus,
    #[serde(default)]
    pub importance: TaskImportance,
    #[serde(default)]
    pub is_reminder_on: bool,
    #[serde(default)]
    pub reminder_date_time: Option<DateTimeTimeZone>,
    #[serde(default)]
    pub due_date_time: Option<DateTimeTimeZone>,
    #[serde(default)]
    pub completed_date_time: Option<DateTimeTimeZone>,
    #[serde(default)]
    pub created_date_time: Option<String>,
    #[serde(default)]
    pub last_modified_date_time: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
}

/// Task status enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    #[default]
    NotStarted,
    InProgress,
    Completed,
    WaitingOnOthers,
    Deferred,
}

/// Task importance level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub enum TaskImportance {
    Low,
    #[default]
    Normal,
    High,
}

/// Body content for a task.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemBody {
    pub content_type: BodyType,
    pub content: String,
}

/// Body content type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub enum BodyType {
    #[default]
    Text,
    Html,
}

/// Date-time with timezone information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DateTimeTimeZone {
    pub date_time: String,
    pub time_zone: String,
}
