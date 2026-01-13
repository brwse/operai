//! Type definitions for Asana API requests and responses.

use serde::{Deserialize, Serialize};

/// Asana API response wrapper.
#[derive(Debug, Deserialize)]
pub struct AsanaResponse<T> {
    pub data: T,
}

/// Asana list API response wrapper with pagination.
#[derive(Debug, Deserialize)]
pub struct AsanaListResponse<T> {
    pub data: Vec<T>,
}

/// Internal representation of an Asana user from the API.
#[derive(Debug, Clone, Deserialize)]
pub struct AsanaApiUser {
    pub gid: String,
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
}

/// Internal representation of an Asana project from the API.
#[derive(Debug, Clone, Deserialize)]
pub struct AsanaApiProject {
    pub gid: String,
    pub name: String,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Internal representation of an Asana task from the API.
#[derive(Debug, Clone, Deserialize)]
pub struct AsanaApiTask {
    pub gid: String,
    pub name: String,
    #[serde(default)]
    pub completed: bool,
    #[serde(default)]
    pub due_on: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub assignee: Option<AsanaApiUser>,
}

/// Internal representation of an Asana story (comment) from the API.
#[derive(Debug, Clone, Deserialize)]
pub struct AsanaApiStory {
    pub gid: String,
    pub text: String,
    #[serde(default)]
    pub created_by: Option<AsanaApiUser>,
    #[serde(default)]
    pub created_at: Option<String>,
}

/// Request body for creating a task.
#[derive(Debug, Serialize)]
pub struct CreateTaskRequest {
    pub data: CreateTaskData,
}

#[derive(Debug, Serialize)]
pub struct CreateTaskData {
    pub name: String,
    pub projects: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_on: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
}

/// Request body for updating a task.
#[derive(Debug, Serialize)]
pub struct UpdateTaskRequest {
    pub data: UpdateTaskData,
}

#[derive(Debug, Serialize)]
pub struct UpdateTaskData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
}

/// Request body for creating a story (comment).
#[derive(Debug, Serialize)]
pub struct CreateStoryRequest {
    pub data: CreateStoryData,
}

#[derive(Debug, Serialize)]
pub struct CreateStoryData {
    pub text: String,
}

/// Asana API error response.
#[derive(Debug, Deserialize)]
pub struct AsanaErrorResponse {
    pub errors: Vec<AsanaError>,
}

#[derive(Debug, Deserialize)]
pub struct AsanaError {
    /// Human-readable error message.
    pub message: String,
}
