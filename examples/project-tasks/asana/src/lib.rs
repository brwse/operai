//! Asana integration for Operai Toolbox.
//!
//! Provides tools for managing projects and tasks in Asana:
//! - List projects and tasks
//! - Create and update tasks
//! - Add comments and assign tasks

mod client;
mod types;

use operai::{
    Context, JsonSchema, Result, define_system_credential, info, init, schemars, shutdown, tool,
};
use serde::{Deserialize, Serialize};
use types::{AsanaApiProject, AsanaApiStory, AsanaApiTask, AsanaApiUser};

// Asana API credential with personal access token
define_system_credential! {
    AsanaCredential("asana") {
        access_token: String,
        #[optional]
        workspace_gid: Option<String>,
    }
}

/// Initializes the Asana integration.
///
/// # Errors
///
/// This function returns an error if the integration fails to initialize.
/// In a real implementation, this could fail due to:
/// - Invalid credential configuration
/// - Network connectivity issues when validating credentials
/// - Authentication failures when connecting to the Asana API
#[init]
async fn setup() -> Result<()> {
    info!("Asana integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Asana integration shutting down");
}

// ============================================================================
// Common Types
// ============================================================================

/// Represents an Asana user.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AsanaUser {
    /// The globally unique identifier for the user.
    pub gid: String,
    /// The user's name.
    pub name: String,
    /// The user's email address.
    #[serde(default)]
    pub email: Option<String>,
}

impl From<AsanaApiUser> for AsanaUser {
    fn from(api: AsanaApiUser) -> Self {
        Self {
            gid: api.gid,
            name: api.name,
            email: api.email,
        }
    }
}

/// Represents an Asana project.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AsanaProject {
    /// The globally unique identifier for the project.
    pub gid: String,
    /// The name of the project.
    pub name: String,
    /// Whether the project is archived.
    #[serde(default)]
    pub archived: bool,
    /// The project's color.
    #[serde(default)]
    pub color: Option<String>,
    /// Notes/description of the project.
    #[serde(default)]
    pub notes: Option<String>,
}

impl From<AsanaApiProject> for AsanaProject {
    fn from(api: AsanaApiProject) -> Self {
        Self {
            gid: api.gid,
            name: api.name,
            archived: api.archived,
            color: api.color,
            notes: api.notes,
        }
    }
}

/// Represents an Asana task.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AsanaTask {
    /// The globally unique identifier for the task.
    pub gid: String,
    /// The name of the task.
    pub name: String,
    /// Whether the task is completed.
    #[serde(default)]
    pub completed: bool,
    /// The due date of the task (YYYY-MM-DD format).
    #[serde(default)]
    pub due_on: Option<String>,
    /// Notes/description of the task.
    #[serde(default)]
    pub notes: Option<String>,
    /// The user assigned to this task.
    #[serde(default)]
    pub assignee: Option<AsanaUser>,
}

impl From<AsanaApiTask> for AsanaTask {
    fn from(api: AsanaApiTask) -> Self {
        Self {
            gid: api.gid,
            name: api.name,
            completed: api.completed,
            due_on: api.due_on,
            notes: api.notes,
            assignee: api.assignee.map(Into::into),
        }
    }
}

/// Represents an Asana comment (story).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AsanaComment {
    /// The globally unique identifier for the comment.
    pub gid: String,
    /// The text content of the comment.
    pub text: String,
    /// The user who created the comment.
    #[serde(default)]
    pub created_by: Option<AsanaUser>,
    /// When the comment was created (ISO 8601 format).
    #[serde(default)]
    pub created_at: Option<String>,
}

impl From<AsanaApiStory> for AsanaComment {
    fn from(api: AsanaApiStory) -> Self {
        Self {
            gid: api.gid,
            text: api.text,
            created_by: api.created_by.map(Into::into),
            created_at: api.created_at,
        }
    }
}

// ============================================================================
// List Projects Tool
// ============================================================================

/// Input for listing Asana projects.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListProjectsInput {
    /// The workspace GID to list projects from.
    pub workspace_gid: String,
    /// Whether to include archived projects.
    #[serde(default)]
    pub include_archived: bool,
    /// Maximum number of projects to return (default: 100).
    #[serde(default)]
    pub limit: Option<u32>,
}

/// Output from listing Asana projects.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ListProjectsOutput {
    /// The list of projects.
    pub projects: Vec<AsanaProject>,
    /// Total count of projects returned.
    pub count: usize,
}

/// # List Asana Projects
///
/// Lists all projects in an Asana workspace with optional filtering.
///
/// Use this tool when the user wants to view, browse, or retrieve projects
/// from their Asana workspace. This is useful for getting an overview of
/// available projects before performing operations on specific projects.
///
/// ## Key Inputs
/// - `workspace_gid`: Required - The workspace to list projects from
/// - `include_archived`: Optional - Set to true to include archived projects
/// - `limit`: Optional - Maximum number of projects to return (default: 100)
///
/// ## Returns
/// A list of projects with their GID, name, archived status, color, and notes.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - project-management
/// - asana
///
/// # Errors
///
/// This function returns an error if:
/// - The workspace GID is invalid or doesn't exist
/// - Authentication credentials are missing or invalid
/// - Network connectivity fails when calling the Asana API
/// - The API returns an error response
/// - Response deserialization fails
#[tool]
pub async fn list_projects(ctx: Context, input: ListProjectsInput) -> Result<ListProjectsOutput> {
    let (access_token, _) = client::get_credential(&ctx).await?;
    let client = client::AsanaClient::new(&access_token)?;

    let projects = client
        .list_projects(&input.workspace_gid, input.include_archived, input.limit)
        .await?;

    let projects: Vec<AsanaProject> = projects.into_iter().map(Into::into).collect();
    let count = projects.len();

    Ok(ListProjectsOutput { projects, count })
}

// ============================================================================
// List Tasks Tool
// ============================================================================

/// Input for listing Asana tasks.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTasksInput {
    /// The project GID to list tasks from.
    pub project_gid: String,
    /// Filter by completion status.
    #[serde(default)]
    pub completed: Option<bool>,
    /// Filter by assignee GID.
    #[serde(default)]
    pub assignee_gid: Option<String>,
    /// Maximum number of tasks to return (default: 100).
    #[serde(default)]
    pub limit: Option<u32>,
}

/// Output from listing Asana tasks.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ListTasksOutput {
    /// The list of tasks.
    pub tasks: Vec<AsanaTask>,
    /// Total count of tasks returned.
    pub count: usize,
}

/// # List Asana Tasks
///
/// Lists tasks in an Asana project with powerful filtering capabilities.
///
/// Use this tool when the user wants to view, browse, or retrieve tasks from
/// a specific Asana project. Supports filtering by completion status and
/// assignee, making it easy to find specific subsets of tasks.
///
/// ## Key Inputs
/// - `project_gid`: Required - The project to list tasks from
/// - `completed`: Optional - Filter by completion status (true/false)
/// - `assignee_gid`: Optional - Filter by specific assignee
/// - `limit`: Optional - Maximum number of tasks to return (default: 100)
///
/// ## Returns
/// A list of tasks with their GID, name, completion status, due date, notes,
/// and assignee information.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - project-management
/// - asana
///
/// # Errors
///
/// This function returns an error if:
/// - The project GID is invalid or doesn't exist
/// - Authentication credentials are missing or invalid
/// - Network connectivity fails when calling the Asana API
/// - The API returns an error response
/// - Response deserialization fails
#[tool]
pub async fn list_tasks(ctx: Context, input: ListTasksInput) -> Result<ListTasksOutput> {
    let (access_token, _) = client::get_credential(&ctx).await?;
    let client = client::AsanaClient::new(&access_token)?;

    let tasks = client
        .list_tasks(
            &input.project_gid,
            input.completed,
            input.assignee_gid.as_deref(),
            input.limit,
        )
        .await?;

    let tasks: Vec<AsanaTask> = tasks.into_iter().map(Into::into).collect();
    let count = tasks.len();

    Ok(ListTasksOutput { tasks, count })
}

// ============================================================================
// Create Task Tool
// ============================================================================

/// Input for creating an Asana task.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateTaskInput {
    /// The project GID to create the task in.
    pub project_gid: String,
    /// The name of the task.
    pub name: String,
    /// Notes/description for the task.
    #[serde(default)]
    pub notes: Option<String>,
    /// Due date in YYYY-MM-DD format.
    #[serde(default)]
    pub due_on: Option<String>,
    /// The GID of the user to assign the task to.
    #[serde(default)]
    pub assignee_gid: Option<String>,
}

/// Output from creating an Asana task.
#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateTaskOutput {
    /// The created task.
    pub task: AsanaTask,
    /// Whether the task was successfully created.
    pub success: bool,
}

/// # Create Asana Task
///
/// Creates a new task in an Asana project with optional details.
///
/// Use this tool when the user wants to add a new task to a project. The task
/// can include a description, due date, and assignee. This is the primary way
/// to create new work items in Asana.
///
/// ## Key Inputs
/// - `project_gid`: Required - The project to create the task in
/// - `name`: Required - The task name/title
/// - `notes`: Optional - Task description or additional details
/// - `due_on`: Optional - Due date in YYYY-MM-DD format
/// - `assignee_gid`: Optional - User GID to assign the task to
///
/// ## Returns
/// The created task with its generated GID, name, and all provided details.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - project-management
/// - asana
///
/// # Errors
///
/// This function returns an error if:
/// - The project GID is invalid or doesn't exist
/// - Authentication credentials are missing or invalid
/// - Network connectivity fails when calling the Asana API
/// - The API returns an error response (e.g., validation failed, insufficient
///   permissions)
/// - Response deserialization fails
#[tool]
pub async fn create_task(ctx: Context, input: CreateTaskInput) -> Result<CreateTaskOutput> {
    let (access_token, _) = client::get_credential(&ctx).await?;
    let client = client::AsanaClient::new(&access_token)?;

    let request = types::CreateTaskRequest {
        data: types::CreateTaskData {
            name: input.name.clone(),
            projects: vec![input.project_gid],
            notes: input.notes.clone(),
            due_on: input.due_on.clone(),
            assignee: input.assignee_gid.clone(),
        },
    };

    let api_task = client.create_task(request).await?;
    let task: AsanaTask = api_task.into();

    Ok(CreateTaskOutput {
        task,
        success: true,
    })
}

// ============================================================================
// Update Status Tool
// ============================================================================

/// Task status options.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Mark task as incomplete.
    Incomplete,
    /// Mark task as complete.
    Complete,
}

/// Input for updating an Asana task's status.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateStatusInput {
    /// The task GID to update.
    pub task_gid: String,
    /// The new status for the task.
    pub status: TaskStatus,
}

/// Output from updating an Asana task's status.
#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateStatusOutput {
    /// The updated task.
    pub task: AsanaTask,
    /// Whether the update was successful.
    pub success: bool,
    /// Previous completion status.
    pub previous_status: bool,
}

/// # Update Asana Task Status
///
/// Updates the completion status of an Asana task (mark as complete or
/// incomplete).
///
/// Use this tool when the user wants to mark a task as done or reopen a
/// completed task. This is the primary way to track task completion progress in
/// Asana.
///
/// ## Key Inputs
/// - `task_gid`: Required - The task to update
/// - `status`: Required - Either "complete" or "incomplete"
///
/// ## Returns
/// The updated task with its new completion status, plus the previous status
/// for reference.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - project-management
/// - asana
///
/// # Errors
///
/// This function returns an error if:
/// - The task GID is invalid or doesn't exist
/// - Authentication credentials are missing or invalid
/// - Network connectivity fails when calling the Asana API
/// - The API returns an error response (e.g., insufficient permissions)
/// - Response deserialization fails
#[tool]
pub async fn update_status(ctx: Context, input: UpdateStatusInput) -> Result<UpdateStatusOutput> {
    let (access_token, _) = client::get_credential(&ctx).await?;
    let client = client::AsanaClient::new(&access_token)?;

    let completed = input.status == TaskStatus::Complete;
    let api_task = client
        .update_task_completed(&input.task_gid, completed)
        .await?;

    let task: AsanaTask = api_task.into();

    Ok(UpdateStatusOutput {
        task,
        success: true,
        previous_status: !completed,
    })
}

// ============================================================================
// Comment Tool
// ============================================================================

/// Input for adding a comment to an Asana task.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CommentInput {
    /// The task GID to comment on.
    pub task_gid: String,
    /// The comment text.
    pub text: String,
}

/// Output from adding a comment.
#[derive(Debug, Serialize, JsonSchema)]
pub struct CommentOutput {
    /// The created comment.
    pub comment: AsanaComment,
    /// Whether the comment was successfully added.
    pub success: bool,
}

/// # Add Asana Task Comment
///
/// Adds a comment to an Asana task for collaboration and communication.
///
/// Use this tool when the user wants to add a note, feedback, question, or any
/// other comment to a task. Comments are visible to all task collaborators and
/// are essential for team communication.
///
/// ## Key Inputs
/// - `task_gid`: Required - The task to comment on
/// - `text`: Required - The comment text content
///
/// ## Returns
/// The created comment with its GID, text, author, and creation timestamp.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - project-management
/// - asana
///
/// # Errors
///
/// This function returns an error if:
/// - The task GID is invalid or doesn't exist
/// - Authentication credentials are missing or invalid
/// - Network connectivity fails when calling the Asana API
/// - The API returns an error response (e.g., insufficient permissions, invalid
///   text)
/// - Response deserialization fails
#[tool]
pub async fn comment(ctx: Context, input: CommentInput) -> Result<CommentOutput> {
    let (access_token, _) = client::get_credential(&ctx).await?;
    let client = client::AsanaClient::new(&access_token)?;

    let request = types::CreateStoryRequest {
        data: types::CreateStoryData {
            text: input.text.clone(),
        },
    };

    let api_story = client.create_story(&input.task_gid, request).await?;
    let comment: AsanaComment = api_story.into();

    Ok(CommentOutput {
        comment,
        success: true,
    })
}

// ============================================================================
// Assign Tool
// ============================================================================

/// Input for assigning an Asana task.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AssignInput {
    /// The task GID to assign.
    pub task_gid: String,
    /// The user GID to assign the task to. Set to null/none to unassign.
    #[serde(default)]
    pub assignee_gid: Option<String>,
}

/// Output from assigning a task.
#[derive(Debug, Serialize, JsonSchema)]
pub struct AssignOutput {
    /// The updated task.
    pub task: AsanaTask,
    /// Whether the assignment was successful.
    pub success: bool,
    /// The previous assignee, if any.
    pub previous_assignee: Option<AsanaUser>,
}

/// # Assign Asana Task
///
/// Assigns or unassigns a user to/from an Asana task.
///
/// Use this tool when the user wants to change task ownership or
/// responsibility. Tasks can be assigned to a specific user or unassigned (set
/// to no one). This is essential for task management and workload distribution.
///
/// ## Key Inputs
/// - `task_gid`: Required - The task to assign
/// - `assignee_gid`: Optional - User GID to assign the task to. Omit or set to
///   null/none to unassign the task (remove all assignees)
///
/// ## Returns
/// The updated task with its new assignee, plus the previous assignee for
/// reference.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - project-management
/// - asana
///
/// # Errors
///
/// This function returns an error if:
/// - The task GID is invalid or doesn't exist
/// - The assignee GID (if provided) is invalid or doesn't exist
/// - Authentication credentials are missing or invalid
/// - Network connectivity fails when calling the Asana API
/// - The API returns an error response (e.g., insufficient permissions)
/// - Response deserialization fails
#[tool]
pub async fn assign(ctx: Context, input: AssignInput) -> Result<AssignOutput> {
    let (access_token, _) = client::get_credential(&ctx).await?;
    let client = client::AsanaClient::new(&access_token)?;

    // First get the current task to capture the previous assignee
    // Note: This requires an additional API call, but is necessary to return
    // the previous_assignee in the output as specified by the tool contract.
    let current_task_response = client
        .client
        .get(format!(
            "{}/tasks/{}",
            client::ASANA_API_BASE,
            input.task_gid
        ))
        .send()
        .await;

    let previous_assignee = if let Ok(response) = current_task_response {
        if response.status().is_success() {
            if let Ok(api_response) = response.json::<types::AsanaResponse<AsanaApiTask>>().await {
                api_response.data.assignee.map(AsanaUser::from)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    let api_task = client
        .update_task_assignee(&input.task_gid, input.assignee_gid.as_deref())
        .await?;

    let task: AsanaTask = api_task.into();

    Ok(AssignOutput {
        task,
        success: true,
        previous_assignee,
    })
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Credential Tests
    // ========================================================================

    #[test]
    fn test_asana_credential_deserializes_with_access_token_only() {
        let json = r#"{ "access_token": "1/1234567890:abcdef" }"#;
        let cred: AsanaCredential = serde_json::from_str(json).unwrap();

        assert_eq!(cred.access_token, "1/1234567890:abcdef");
        assert_eq!(cred.workspace_gid, None);
    }

    #[test]
    fn test_asana_credential_deserializes_with_workspace_gid() {
        let json = r#"{ "access_token": "1/1234567890:abcdef", "workspace_gid": "ws123" }"#;
        let cred: AsanaCredential = serde_json::from_str(json).unwrap();

        assert_eq!(cred.access_token, "1/1234567890:abcdef");
        assert_eq!(cred.workspace_gid.as_deref(), Some("ws123"));
    }

    #[test]
    fn test_asana_credential_missing_access_token_fails() {
        let json = r#"{ "workspace_gid": "ws123" }"#;
        let err = serde_json::from_str::<AsanaCredential>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `access_token`"));
    }

    // ========================================================================
    // List Projects Tests
    // ========================================================================

    #[test]
    fn test_list_projects_input_deserializes_minimal() {
        let json = r#"{ "workspace_gid": "ws123" }"#;
        let input: ListProjectsInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.workspace_gid, "ws123");
        assert!(!input.include_archived);
        assert_eq!(input.limit, None);
    }

    #[test]
    fn test_list_projects_input_deserializes_with_all_fields() {
        let json = r#"{ "workspace_gid": "ws123", "include_archived": true, "limit": 50 }"#;
        let input: ListProjectsInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.workspace_gid, "ws123");
        assert!(input.include_archived);
        assert_eq!(input.limit, Some(50));
    }

    // ========================================================================
    // List Tasks Tests
    // ========================================================================

    #[test]
    fn test_list_tasks_input_deserializes_minimal() {
        let json = r#"{ "project_gid": "proj123" }"#;
        let input: ListTasksInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.project_gid, "proj123");
        assert_eq!(input.completed, None);
        assert_eq!(input.assignee_gid, None);
    }

    #[test]
    fn test_list_tasks_input_deserializes_with_filters() {
        let json = r#"{
            "project_gid": "proj123",
            "completed": false,
            "assignee_gid": "user123",
            "limit": 25
        }"#;
        let input: ListTasksInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.project_gid, "proj123");
        assert_eq!(input.completed, Some(false));
        assert_eq!(input.assignee_gid.as_deref(), Some("user123"));
        assert_eq!(input.limit, Some(25));
    }

    // ========================================================================
    // Create Task Tests
    // ========================================================================

    #[test]
    fn test_create_task_input_deserializes_minimal() {
        let json = r#"{ "project_gid": "proj123", "name": "New Task" }"#;
        let input: CreateTaskInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.project_gid, "proj123");
        assert_eq!(input.name, "New Task");
        assert_eq!(input.notes, None);
        assert_eq!(input.due_on, None);
        assert_eq!(input.assignee_gid, None);
    }

    #[test]
    fn test_create_task_input_deserializes_with_all_fields() {
        let json = r#"{
            "project_gid": "proj123",
            "name": "New Task",
            "notes": "Task description",
            "due_on": "2024-03-20",
            "assignee_gid": "user123"
        }"#;
        let input: CreateTaskInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.project_gid, "proj123");
        assert_eq!(input.name, "New Task");
        assert_eq!(input.notes.as_deref(), Some("Task description"));
        assert_eq!(input.due_on.as_deref(), Some("2024-03-20"));
        assert_eq!(input.assignee_gid.as_deref(), Some("user123"));
    }

    // ========================================================================
    // Update Status Tests
    // ========================================================================

    #[test]
    fn test_update_status_input_deserializes_complete() {
        let json = r#"{ "task_gid": "task123", "status": "complete" }"#;
        let input: UpdateStatusInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.task_gid, "task123");
        assert_eq!(input.status, TaskStatus::Complete);
    }

    #[test]
    fn test_update_status_input_deserializes_incomplete() {
        let json = r#"{ "task_gid": "task123", "status": "incomplete" }"#;
        let input: UpdateStatusInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.task_gid, "task123");
        assert_eq!(input.status, TaskStatus::Incomplete);
    }

    // ========================================================================
    // Comment Tests
    // ========================================================================

    #[test]
    fn test_comment_input_deserializes() {
        let json = r#"{ "task_gid": "task123", "text": "This is a comment" }"#;
        let input: CommentInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.task_gid, "task123");
        assert_eq!(input.text, "This is a comment");
    }

    #[test]
    fn test_comment_input_missing_text_fails() {
        let json = r#"{ "task_gid": "task123" }"#;
        let err = serde_json::from_str::<CommentInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `text`"));
    }

    // ========================================================================
    // Assign Tests
    // ========================================================================

    #[test]
    fn test_assign_input_deserializes_with_assignee() {
        let json = r#"{ "task_gid": "task123", "assignee_gid": "user456" }"#;
        let input: AssignInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.task_gid, "task123");
        assert_eq!(input.assignee_gid.as_deref(), Some("user456"));
    }

    #[test]
    fn test_assign_input_deserializes_without_assignee() {
        let json = r#"{ "task_gid": "task123" }"#;
        let input: AssignInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.task_gid, "task123");
        assert_eq!(input.assignee_gid, None);
    }

    // ========================================================================
    // Common Type Tests
    // ========================================================================

    #[test]
    fn test_asana_project_serializes_correctly() {
        let project = AsanaProject {
            gid: "123".to_string(),
            name: "Test Project".to_string(),
            archived: false,
            color: Some("blue".to_string()),
            notes: Some("Description".to_string()),
        };

        let json = serde_json::to_value(&project).unwrap();

        assert_eq!(json["gid"], "123");
        assert_eq!(json["name"], "Test Project");
        assert_eq!(json["archived"], false);
        assert_eq!(json["color"], "blue");
        assert_eq!(json["notes"], "Description");
    }

    #[test]
    fn test_asana_task_serializes_with_optional_fields() {
        let task = AsanaTask {
            gid: "456".to_string(),
            name: "Test Task".to_string(),
            completed: true,
            due_on: None,
            notes: None,
            assignee: None,
        };

        let json = serde_json::to_value(&task).unwrap();

        assert_eq!(json["gid"], "456");
        assert_eq!(json["name"], "Test Task");
        assert_eq!(json["completed"], true);
        assert!(json["due_on"].is_null());
        assert!(json["notes"].is_null());
        assert!(json["assignee"].is_null());
    }

    #[test]
    fn test_asana_user_deserializes_with_email() {
        let json = r#"{ "gid": "user1", "name": "Test User", "email": "test@example.com" }"#;
        let user: AsanaUser = serde_json::from_str(json).unwrap();

        assert_eq!(user.gid, "user1");
        assert_eq!(user.name, "Test User");
        assert_eq!(user.email.as_deref(), Some("test@example.com"));
    }

    #[test]
    fn test_asana_user_deserializes_without_email() {
        let json = r#"{ "gid": "user1", "name": "Test User" }"#;
        let user: AsanaUser = serde_json::from_str(json).unwrap();

        assert_eq!(user.gid, "user1");
        assert_eq!(user.name, "Test User");
        assert_eq!(user.email, None);
    }

    // ========================================================================
    // Conversion Tests
    // ========================================================================

    #[test]
    fn test_api_user_converts_to_asana_user() {
        let api_user = types::AsanaApiUser {
            gid: "123".to_string(),
            name: "Test User".to_string(),
            email: Some("test@example.com".to_string()),
        };

        let user: AsanaUser = api_user.into();

        assert_eq!(user.gid, "123");
        assert_eq!(user.name, "Test User");
        assert_eq!(user.email.as_deref(), Some("test@example.com"));
    }

    #[test]
    fn test_api_project_converts_to_asana_project() {
        let api_project = types::AsanaApiProject {
            gid: "456".to_string(),
            name: "Test Project".to_string(),
            archived: false,
            color: Some("blue".to_string()),
            notes: Some("Notes".to_string()),
        };

        let project: AsanaProject = api_project.into();

        assert_eq!(project.gid, "456");
        assert_eq!(project.name, "Test Project");
        assert!(!project.archived);
        assert_eq!(project.color.as_deref(), Some("blue"));
        assert_eq!(project.notes.as_deref(), Some("Notes"));
    }

    #[test]
    fn test_api_task_converts_to_asana_task() {
        let api_user = types::AsanaApiUser {
            gid: "789".to_string(),
            name: "Assignee".to_string(),
            email: None,
        };

        let api_task = types::AsanaApiTask {
            gid: "101".to_string(),
            name: "Test Task".to_string(),
            completed: true,
            due_on: Some("2024-03-20".to_string()),
            notes: Some("Task notes".to_string()),
            assignee: Some(api_user),
        };

        let task: AsanaTask = api_task.into();

        assert_eq!(task.gid, "101");
        assert_eq!(task.name, "Test Task");
        assert!(task.completed);
        assert_eq!(task.due_on.as_deref(), Some("2024-03-20"));
        assert_eq!(task.notes.as_deref(), Some("Task notes"));
        assert!(task.assignee.is_some());
        assert_eq!(task.assignee.unwrap().gid, "789");
    }

    #[test]
    fn test_api_story_converts_to_asana_comment() {
        let api_user = types::AsanaApiUser {
            gid: "202".to_string(),
            name: "Commenter".to_string(),
            email: None,
        };

        let api_story = types::AsanaApiStory {
            gid: "303".to_string(),
            text: "Great progress!".to_string(),
            created_by: Some(api_user),
            created_at: Some("2024-03-15T10:30:00Z".to_string()),
        };

        let comment: AsanaComment = api_story.into();

        assert_eq!(comment.gid, "303");
        assert_eq!(comment.text, "Great progress!");
        assert!(comment.created_by.is_some());
        assert_eq!(comment.created_by.unwrap().gid, "202");
        assert_eq!(comment.created_at.as_deref(), Some("2024-03-15T10:30:00Z"));
    }
}
