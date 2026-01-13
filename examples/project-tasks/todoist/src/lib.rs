//! project-tasks/todoist integration for Operai Toolbox.

mod types;

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
use types::{Comment, CreateCommentRequest, CreateTaskRequest, Project, Task, UpdateTaskRequest};

define_user_credential! {
    TodoistCredential("todoist") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_API_ENDPOINT: &str = "https://api.todoist.com/rest/v2";

#[init]
async fn setup() -> Result<()> {
    info!("Todoist integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Todoist integration shutting down");
}

// ============================================================================
// Tool Input/Output Types
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListProjectsInput {
    // No parameters needed for listing all projects
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListProjectsOutput {
    pub projects: Vec<Project>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateTaskInput {
    /// Task content (title).
    pub content: String,
    /// Optional task description.
    #[serde(default)]
    pub description: Option<String>,
    /// Optional project ID to add the task to.
    #[serde(default)]
    pub project_id: Option<String>,
    /// Optional section ID.
    #[serde(default)]
    pub section_id: Option<String>,
    /// Optional labels.
    #[serde(default)]
    pub labels: Vec<String>,
    /// Priority (1-4, where 4 is highest). Defaults to 1.
    #[serde(default)]
    pub priority: Option<i32>,
    /// Human-readable due date string (e.g., "tomorrow at 12:00", "next
    /// Monday").
    #[serde(default)]
    pub due_string: Option<String>,
    /// Due date in YYYY-MM-DD format.
    #[serde(default)]
    pub due_date: Option<String>,
    /// Due datetime in RFC3339 format.
    #[serde(default)]
    pub due_datetime: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateTaskOutput {
    pub task: Task,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompleteTaskInput {
    /// Task ID to mark as completed.
    pub task_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CompleteTaskOutput {
    pub completed: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RescheduleTaskInput {
    /// Task ID to reschedule.
    pub task_id: String,
    /// Human-readable due date string (e.g., "tomorrow at 12:00").
    #[serde(default)]
    pub due_string: Option<String>,
    /// Due date in YYYY-MM-DD format.
    #[serde(default)]
    pub due_date: Option<String>,
    /// Due datetime in RFC3339 format.
    #[serde(default)]
    pub due_datetime: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RescheduleTaskOutput {
    pub task: Task,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddNoteInput {
    /// Task ID to add the note to.
    pub task_id: String,
    /// Note content.
    pub content: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AddNoteOutput {
    pub comment: Comment,
}

// ============================================================================
// Tools
// ============================================================================

/// # List Todoist Projects
///
/// Retrieves all projects from the user's Todoist account.
///
/// Use this tool when you need to:
/// - Display all available projects to the user
/// - Find a project ID to create tasks in a specific project
/// - Show the user's project organization structure
///
/// Returns a list of projects with metadata including name, color, favorite
/// status, and view style. Each project includes an ID that can be used when
/// creating tasks.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - tasks
/// - todoist
/// - projects
///
/// # Errors
///
/// Returns an error if:
/// - No Todoist credentials are configured or the access token is missing
/// - The HTTP request to the Todoist API fails
/// - The API response cannot be parsed as valid JSON
/// - The API returns a non-success status code
#[tool]
pub async fn list_projects(ctx: Context, _input: ListProjectsInput) -> Result<ListProjectsOutput> {
    let client = TodoistClient::from_ctx(&ctx)?;
    let projects: Vec<Project> = client.get_json(client.url_with_path("/projects")?).await?;
    Ok(ListProjectsOutput { projects })
}

/// # Create Todoist Task
///
/// Creates a new task in the user's Todoist account with the specified content
/// and optional parameters.
///
/// Use this tool when:
/// - The user wants to add a new task, to-do item, or reminder
/// - The user asks to create, add, or track something they need to do
/// - The user mentions something to remember or accomplish
///
/// Task parameters:
/// - `content`: Required. The task title/name (what the user wants to do)
/// - `description`: Optional. Additional details about the task
/// - `project_id`: Optional. ID of the project to add the task to (use
///   `list_projects` to find IDs)
/// - `priority`: Optional. Priority level 1-4 (4=highest, 1=lowest, defaults to
///   1)
/// - `due_date`: Optional. Date in YYYY-MM-DD format
/// - `due_string`: Optional. Natural language date like "tomorrow", "next
///   Monday", "in 3 days"
/// - `labels`: Optional. List of label names for categorization
///
/// Returns the created task with its ID, which can be used for future
/// operations like completing or rescheduling.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - tasks
/// - todoist
///
/// # Errors
///
/// Returns an error if:
/// - The task content is empty or contains only whitespace
/// - The priority is outside the valid range (1-4)
/// - No Todoist credentials are configured or the access token is missing
/// - The HTTP request to the Todoist API fails
/// - The API response cannot be parsed as valid JSON
/// - The API returns a non-success status code (e.g., invalid `project_id`)
#[tool]
pub async fn create_task(ctx: Context, input: CreateTaskInput) -> Result<CreateTaskOutput> {
    ensure!(
        !input.content.trim().is_empty(),
        "content must not be empty"
    );

    if let Some(ref priority) = input.priority {
        ensure!(
            (1..=4).contains(priority),
            "priority must be between 1 and 4"
        );
    }

    let client = TodoistClient::from_ctx(&ctx)?;
    let request = CreateTaskRequest {
        content: input.content,
        description: input.description,
        project_id: input.project_id,
        section_id: input.section_id,
        labels: if input.labels.is_empty() {
            None
        } else {
            Some(input.labels)
        },
        priority: input.priority,
        due_string: input.due_string,
        due_date: input.due_date,
        due_datetime: input.due_datetime,
    };

    let task: Task = client
        .post_json(client.url_with_path("/tasks")?, &request)
        .await?;

    Ok(CreateTaskOutput { task })
}

/// # Complete Todoist Task
///
/// Marks a task as completed in the user's Todoist account.
///
/// Use this tool when:
/// - The user says they finished, completed, or done with a task
/// - The user wants to mark a task as done or check it off
/// - The user indicates they've accomplished something they were tracking
///
/// This tool permanently marks the task as completed and moves it to the user's
/// completed tasks history. The task will be removed from active task lists.
///
/// Requires:
/// - `task_id`: The ID of the task to mark complete (obtained from
///   `create_task` or `list_tasks`)
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - tasks
/// - todoist
///
/// # Errors
///
/// Returns an error if:
/// - The task ID is empty or contains only whitespace
/// - No Todoist credentials are configured or the access token is missing
/// - The HTTP request to the Todoist API fails
/// - The API returns a non-success status code (e.g., task not found)
#[tool]
pub async fn complete_task(ctx: Context, input: CompleteTaskInput) -> Result<CompleteTaskOutput> {
    ensure!(
        !input.task_id.trim().is_empty(),
        "task_id must not be empty"
    );

    let client = TodoistClient::from_ctx(&ctx)?;
    client
        .post_empty(client.url_with_path(&format!("/tasks/{}/close", input.task_id))?)
        .await?;

    Ok(CompleteTaskOutput { completed: true })
}

/// # Reschedule Todoist Task
///
/// Updates the due date of an existing task in the user's Todoist account.
///
/// Use this tool when:
/// - The user wants to change when a task is due
/// - The user asks to postpone, delay, or move a task to a different date
/// - The user wants to reschedule, push back, or change the deadline of a task
///
/// Date format options (one required):
/// - `due_string`: Natural language like "tomorrow", "next Monday", "in 3
///   days", "every Friday"
/// - `due_date`: Specific date in YYYY-MM-DD format (e.g., "2024-01-15")
/// - `due_datetime`: Specific datetime in RFC3339 format (e.g.,
///   "2024-01-15T14:30:00Z")
///
/// Returns the updated task with the new due date. Any existing due date will
/// be replaced.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - tasks
/// - todoist
///
/// # Errors
///
/// Returns an error if:
/// - The task ID is empty or contains only whitespace
/// - None of the due date parameters (`due_string`, `due_date`, or
///   `due_datetime`) are provided
/// - No Todoist credentials are configured or the access token is missing
/// - The HTTP request to the Todoist API fails
/// - The API response cannot be parsed as valid JSON
/// - The API returns a non-success status code (e.g., task not found)
#[tool]
pub async fn reschedule_task(
    ctx: Context,
    input: RescheduleTaskInput,
) -> Result<RescheduleTaskOutput> {
    ensure!(
        !input.task_id.trim().is_empty(),
        "task_id must not be empty"
    );
    ensure!(
        input.due_string.is_some() || input.due_date.is_some() || input.due_datetime.is_some(),
        "at least one of due_string, due_date, or due_datetime must be provided"
    );

    let client = TodoistClient::from_ctx(&ctx)?;
    let request = UpdateTaskRequest {
        content: None,
        description: None,
        labels: None,
        priority: None,
        due_string: input.due_string,
        due_date: input.due_date,
        due_datetime: input.due_datetime,
    };

    let task: Task = client
        .post_json(
            client.url_with_path(&format!("/tasks/{}", input.task_id))?,
            &request,
        )
        .await?;

    Ok(RescheduleTaskOutput { task })
}

/// # Add Todoist Note
///
/// Adds a comment or note to an existing task in the user's Todoist account.
///
/// Use this tool when:
/// - The user wants to add additional information, details, or context to a
///   task
/// - The user wants to leave a note on a task for future reference
/// - The user wants to add clarification, updates, or supplementary information
///
/// Notes are displayed as comments on the task and can include any text
/// content. This is useful for adding progress updates, relevant details, or
/// reminders without modifying the task itself.
///
/// Requires:
/// - `task_id`: The ID of the task to add the note to (obtained from
///   `create_task`)
/// - `content`: The text content of the note to add
///
/// Returns the created comment with its ID and timestamp.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - tasks
/// - todoist
/// - comments
///
/// # Errors
///
/// Returns an error if:
/// - The task ID is empty or contains only whitespace
/// - The note content is empty or contains only whitespace
/// - No Todoist credentials are configured or the access token is missing
/// - The HTTP request to the Todoist API fails
/// - The API response cannot be parsed as valid JSON
/// - The API returns a non-success status code (e.g., task not found)
#[tool]
pub async fn add_note(ctx: Context, input: AddNoteInput) -> Result<AddNoteOutput> {
    ensure!(
        !input.task_id.trim().is_empty(),
        "task_id must not be empty"
    );
    ensure!(
        !input.content.trim().is_empty(),
        "content must not be empty"
    );

    let client = TodoistClient::from_ctx(&ctx)?;
    let request = CreateCommentRequest {
        content: input.content,
        task_id: Some(input.task_id),
        project_id: None,
    };

    let comment: Comment = client
        .post_json(client.url_with_path("/comments")?, &request)
        .await?;

    Ok(AddNoteOutput { comment })
}

// ============================================================================
// HTTP Client
// ============================================================================

#[derive(Debug, Clone)]
struct TodoistClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl TodoistClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = TodoistCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_API_ENDPOINT))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            access_token: cred.access_token,
        })
    }

    fn url_with_path(&self, path: &str) -> Result<reqwest::Url> {
        let full_path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        };
        Ok(reqwest::Url::parse(&format!(
            "{}{}",
            self.base_url, full_path
        ))?)
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, url: reqwest::Url) -> Result<T> {
        let response = self.send_request(self.http.get(url)).await?;
        Ok(response.json::<T>().await?)
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
    ) -> Result<TRes> {
        let response = self.send_request(self.http.post(url).json(body)).await?;
        Ok(response.json::<TRes>().await?)
    }

    async fn post_empty(&self, url: reqwest::Url) -> Result<()> {
        self.send_request(self.http.post(url)).await?;
        Ok(())
    }

    async fn send_request(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let response = request
            .bearer_auth(&self.access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Todoist API request failed ({status}): {body}"
            ))
        }
    }
}

fn normalize_base_url(endpoint: &str) -> Result<String> {
    let trimmed = endpoint.trim();
    ensure!(!trimmed.is_empty(), "endpoint must not be empty");
    Ok(trimmed.trim_end_matches('/').to_string())
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_string_contains, header, method, path},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut todoist_values = HashMap::new();
        todoist_values.insert("access_token".to_string(), "test-token".to_string());
        todoist_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("todoist", todoist_values)
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://api.todoist.com/rest/v2/").unwrap();
        assert_eq!(result, "https://api.todoist.com/rest/v2");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://api.todoist.com/rest/v2  ").unwrap();
        assert_eq!(result, "https://api.todoist.com/rest/v2");
    }

    #[test]
    fn test_normalize_base_url_empty_returns_error() {
        let result = normalize_base_url("");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("endpoint must not be empty")
        );
    }

    #[test]
    fn test_normalize_base_url_whitespace_only_returns_error() {
        let result = normalize_base_url("   ");
        assert!(result.is_err());
    }

    // --- Input validation tests ---

    #[tokio::test]
    async fn test_create_task_empty_content_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = create_task(
            ctx,
            CreateTaskInput {
                content: "   ".to_string(),
                description: None,
                project_id: None,
                section_id: None,
                labels: vec![],
                priority: None,
                due_string: None,
                due_date: None,
                due_datetime: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("content must not be empty")
        );
    }

    #[tokio::test]
    async fn test_create_task_invalid_priority_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = create_task(
            ctx,
            CreateTaskInput {
                content: "Test Task".to_string(),
                description: None,
                project_id: None,
                section_id: None,
                labels: vec![],
                priority: Some(5),
                due_string: None,
                due_date: None,
                due_datetime: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("priority must be between 1 and 4")
        );
    }

    #[tokio::test]
    async fn test_complete_task_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = complete_task(
            ctx,
            CompleteTaskInput {
                task_id: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("task_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_reschedule_task_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = reschedule_task(
            ctx,
            RescheduleTaskInput {
                task_id: "  ".to_string(),
                due_string: Some("tomorrow".to_string()),
                due_date: None,
                due_datetime: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("task_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_reschedule_task_no_due_date_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = reschedule_task(
            ctx,
            RescheduleTaskInput {
                task_id: "task-123".to_string(),
                due_string: None,
                due_date: None,
                due_datetime: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("at least one of due_string, due_date, or due_datetime must be provided")
        );
    }

    #[tokio::test]
    async fn test_add_note_empty_task_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = add_note(
            ctx,
            AddNoteInput {
                task_id: "  ".to_string(),
                content: "Note".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("task_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_add_note_empty_content_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = add_note(
            ctx,
            AddNoteInput {
                task_id: "task-123".to_string(),
                content: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("content must not be empty")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_list_projects_success_returns_projects() {
        let server = MockServer::start().await;

        let response_body = r#"
        [
          {
            "id": "project-1",
            "name": "Work",
            "color": "red",
            "is_favorite": true,
            "view_style": "list",
            "url": "https://todoist.com/app/project/project-1"
          },
          {
            "id": "project-2",
            "name": "Personal",
            "color": "blue",
            "is_favorite": false
          }
        ]
        "#;

        Mock::given(method("GET"))
            .and(path("/projects"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = list_projects(ctx, ListProjectsInput {}).await.unwrap();

        assert_eq!(output.projects.len(), 2);
        assert_eq!(output.projects[0].id, "project-1");
        assert_eq!(output.projects[0].name, "Work");
        assert_eq!(output.projects[0].color.as_deref(), Some("red"));
        assert!(output.projects[0].is_favorite);
        assert_eq!(output.projects[1].id, "project-2");
        assert_eq!(output.projects[1].name, "Personal");
        assert!(!output.projects[1].is_favorite);
    }

    #[tokio::test]
    async fn test_create_task_success_returns_task() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "id": "task-123",
          "content": "Buy milk",
          "description": "From the store",
          "project_id": "project-1",
          "labels": ["shopping"],
          "priority": 3,
          "is_completed": false,
          "created_at": "2024-01-01T00:00:00Z",
          "url": "https://todoist.com/app/task/task-123"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/tasks"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("\"content\":\"Buy milk\""))
            .and(body_string_contains("\"priority\":3"))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = create_task(
            ctx,
            CreateTaskInput {
                content: "Buy milk".to_string(),
                description: Some("From the store".to_string()),
                project_id: Some("project-1".to_string()),
                section_id: None,
                labels: vec!["shopping".to_string()],
                priority: Some(3),
                due_string: None,
                due_date: None,
                due_datetime: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.task.id, "task-123");
        assert_eq!(output.task.content, "Buy milk");
        assert_eq!(output.task.description.as_deref(), Some("From the store"));
        assert_eq!(output.task.priority, Some(3));
        assert!(!output.task.is_completed);
    }

    #[tokio::test]
    async fn test_complete_task_success_returns_completed() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/tasks/task-123/close"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = complete_task(
            ctx,
            CompleteTaskInput {
                task_id: "task-123".to_string(),
            },
        )
        .await
        .unwrap();

        assert!(output.completed);
    }

    #[tokio::test]
    async fn test_reschedule_task_success_returns_updated_task() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "id": "task-123",
          "content": "Buy milk",
          "is_completed": false,
          "due": {
            "date": "2024-01-02",
            "string": "tomorrow",
            "is_recurring": false
          }
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/tasks/task-123"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("\"due_string\":\"tomorrow\""))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = reschedule_task(
            ctx,
            RescheduleTaskInput {
                task_id: "task-123".to_string(),
                due_string: Some("tomorrow".to_string()),
                due_date: None,
                due_datetime: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.task.id, "task-123");
        assert_eq!(output.task.content, "Buy milk");
        assert!(output.task.due.is_some());
        assert_eq!(
            output.task.due.as_ref().unwrap().date.as_deref(),
            Some("2024-01-02")
        );
    }

    #[tokio::test]
    async fn test_add_note_success_returns_comment() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "id": "comment-123",
          "content": "Don't forget organic milk",
          "task_id": "task-123",
          "posted_at": "2024-01-01T00:00:00Z"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/comments"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains(
                "\"content\":\"Don't forget organic milk\"",
            ))
            .and(body_string_contains("\"task_id\":\"task-123\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = add_note(
            ctx,
            AddNoteInput {
                task_id: "task-123".to_string(),
                content: "Don't forget organic milk".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.comment.id, "comment-123");
        assert_eq!(output.comment.content, "Don't forget organic milk");
        assert_eq!(output.comment.task_id.as_deref(), Some("task-123"));
    }

    #[tokio::test]
    async fn test_list_projects_error_response_returns_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/projects"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_raw(r#"{"error":"Invalid token"}"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let result = list_projects(ctx, ListProjectsInput {}).await;

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("401"));
    }

    #[tokio::test]
    async fn test_create_task_error_response_returns_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/tasks"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_raw(r#"{"error":"Invalid project_id"}"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let result = create_task(
            ctx,
            CreateTaskInput {
                content: "Test".to_string(),
                description: None,
                project_id: Some("invalid".to_string()),
                section_id: None,
                labels: vec![],
                priority: None,
                due_string: None,
                due_date: None,
                due_datetime: None,
            },
        )
        .await;

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("400"));
    }
}
