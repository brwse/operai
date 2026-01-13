//! Type definitions for ClickUp API.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

// =============================================================================
// API Response Wrapper Types
// =============================================================================

/// ClickUp API error response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ClickUpErrorResponse {
    /// Error code.
    #[serde(default)]
    pub err: Option<String>,
    /// Error message.
    #[serde(default)]
    pub err_message: Option<String>,
}

/// ClickUp API wrapper for list tasks response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TasksResponse {
    /// List of tasks returned from the API.
    #[serde(default)]
    pub tasks: Vec<Task>,
    /// The last task ID in the list (for pagination).
    #[serde(default, rename = "last_task_id")]
    pub last_id: Option<String>,
}

/// ClickUp API wrapper for single task response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaskResponse {
    /// The task returned from the API.
    pub task: Task,
}

/// ClickUp API wrapper for comment response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CommentResponse {
    /// The comment returned from the API.
    pub comment: Comment,
}

// =============================================================================
// Common Public Types
// =============================================================================

/// A ClickUp user reference.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct User {
    /// The user's unique identifier.
    pub id: String,
    /// The user's username.
    pub username: String,
    /// The user's email address.
    #[serde(default)]
    pub email: Option<String>,
    /// URL to the user's profile picture.
    #[serde(default)]
    pub profile_picture: Option<String>,
}

/// A ClickUp task status.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Status {
    /// The status identifier.
    pub id: String,
    /// The status name (e.g., "open", "in progress", "complete").
    pub status: String,
    /// The status color in hex format.
    #[serde(default)]
    pub color: Option<String>,
    /// The order of this status in the workflow.
    #[serde(default)]
    pub orderindex: Option<i32>,
    /// The type of status (open, custom, closed).
    #[serde(default)]
    pub r#type: Option<String>,
}

/// A ClickUp task priority.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Priority {
    /// Priority level (1 = urgent, 2 = high, 3 = normal, 4 = low).
    pub priority: Option<i32>,
    /// Priority color in hex format.
    #[serde(default)]
    pub color: Option<String>,
}

/// A ClickUp task.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Task {
    /// The task's unique identifier.
    pub id: String,
    /// Custom task ID if set.
    #[serde(default)]
    pub custom_id: Option<String>,
    /// The task name/title.
    pub name: String,
    /// The task description in markdown or plain text.
    #[serde(default)]
    pub description: Option<String>,
    /// The current status of the task.
    #[serde(default)]
    pub status: Option<Status>,
    /// The task priority.
    #[serde(default)]
    pub priority: Option<Priority>,
    /// Users assigned to this task.
    #[serde(default)]
    pub assignees: Vec<User>,
    /// The user who created the task.
    #[serde(default)]
    pub creator: Option<User>,
    /// Due date as Unix timestamp in milliseconds.
    #[serde(default)]
    pub due_date: Option<i64>,
    /// Start date as Unix timestamp in milliseconds.
    #[serde(default)]
    pub start_date: Option<i64>,
    /// Date created as Unix timestamp in milliseconds.
    #[serde(default)]
    pub date_created: Option<String>,
    /// Date updated as Unix timestamp in milliseconds.
    #[serde(default)]
    pub date_updated: Option<String>,
    /// The list ID this task belongs to.
    #[serde(default)]
    pub list_id: Option<String>,
    /// The folder ID this task belongs to.
    #[serde(default)]
    pub folder_id: Option<String>,
    /// The space ID this task belongs to.
    #[serde(default)]
    pub space_id: Option<String>,
    /// URL to view this task in ClickUp.
    #[serde(default)]
    pub url: Option<String>,
}

/// A ClickUp comment.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Comment {
    /// The comment's unique identifier.
    pub id: String,
    /// The comment text content.
    pub comment_text: String,
    /// The user who posted the comment.
    #[serde(default)]
    pub user: Option<User>,
    /// Date the comment was created as Unix timestamp.
    #[serde(default)]
    pub date: Option<String>,
}
