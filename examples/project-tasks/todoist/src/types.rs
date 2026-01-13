//! Type definitions for Todoist REST API v2.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};

/// A Todoist project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Project {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub comment_count: Option<i32>,
    #[serde(default)]
    pub is_favorite: bool,
    #[serde(default)]
    pub is_inbox_project: Option<bool>,
    #[serde(default)]
    pub is_shared: Option<bool>,
    #[serde(default)]
    pub is_team_inbox: Option<bool>,
    #[serde(default)]
    pub order: Option<i32>,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub view_style: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

/// A Todoist task.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Task {
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub section_id: Option<String>,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub priority: Option<i32>,
    #[serde(default)]
    pub due: Option<TaskDue>,
    #[serde(default)]
    pub deadline: Option<TaskDeadline>,
    #[serde(default)]
    pub duration: Option<TaskDuration>,
    #[serde(default)]
    pub is_completed: bool,
    #[serde(default)]
    pub comment_count: Option<i32>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub creator_id: Option<String>,
    #[serde(default)]
    pub assignee_id: Option<String>,
    #[serde(default)]
    pub assigner_id: Option<String>,
    #[serde(default)]
    pub order: Option<i32>,
    #[serde(default)]
    pub url: Option<String>,
}

/// Due date information for a task.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaskDue {
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub datetime: Option<String>,
    #[serde(default)]
    pub string: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub is_recurring: bool,
    #[serde(default)]
    pub lang: Option<String>,
}

/// Deadline date information for a task.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaskDeadline {
    #[serde(default)]
    pub date: Option<String>,
}

/// Duration information for a task.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaskDuration {
    #[serde(default)]
    pub amount: Option<i32>,
    #[serde(default)]
    pub unit: Option<String>,
}

/// A Todoist comment.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Comment {
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub posted_at: Option<String>,
}

/// Request to create a new task.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct CreateTaskRequest {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_string: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_datetime: Option<String>,
}

/// Request to update a task.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct UpdateTaskRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_string: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_datetime: Option<String>,
}

/// Request to create a comment.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct CreateCommentRequest {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}
