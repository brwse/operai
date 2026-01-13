//! project-tasks/microsoft-todo integration for Operai Toolbox.

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

mod types;
pub use types::{
    BodyType, DateTimeTimeZone, ItemBody, TaskImportance, TaskStatus, TodoTask, TodoTaskList,
};

define_user_credential! {
    MicrosoftTodoCredential("microsoft_todo") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_GRAPH_ENDPOINT: &str = "https://graph.microsoft.com/v1.0";

#[init]
async fn setup() -> Result<()> {
    info!("Microsoft To Do integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Microsoft To Do integration shutting down");
}

// ========== Tool Implementations ==========

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListListsInput {
    /// Maximum number of lists to return (1-100). Defaults to 25.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListListsOutput {
    pub lists: Vec<TodoTaskList>,
}

/// # List Microsoft To Do Task Lists
///
/// Retrieves all task lists (also known as "lists" or "projects") from the
/// user's Microsoft To Do account via the Microsoft Graph API.
///
/// Use this tool when the user wants to:
/// - Browse or explore their task lists
/// - Find a specific list ID to perform operations on tasks within that list
/// - Get an overview of all available task lists
///
/// The returned lists include metadata such as display name, ownership status,
/// and whether the list is shared with others. Microsoft To Do includes special
/// well-known lists like "Tasks" (default list), "Flagged email", etc.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - tasks
/// - todo
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The `limit` parameter is outside the valid range (1-100)
/// - No Microsoft Todo credential is configured
/// - The `access_token` in the credential is empty
/// - The configured endpoint URL is invalid
/// - The HTTP request to Microsoft Graph API fails
/// - The response body cannot be parsed as JSON
#[tool]
pub async fn list_lists(ctx: Context, input: ListListsInput) -> Result<ListListsOutput> {
    let limit = input.limit.unwrap_or(25);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let query = [
        ("$top", limit.to_string()),
        (
            "$select",
            "id,displayName,isOwner,isShared,wellknownListName".to_string(),
        ),
    ];

    let response: GraphListResponse<TodoTaskList> = client
        .get_json(
            client.url_with_segments(&["me", "todo", "lists"])?,
            &query,
            &[],
        )
        .await?;

    Ok(ListListsOutput {
        lists: response.value,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateTaskInput {
    /// ID of the task list to create the task in.
    pub list_id: String,
    /// Task title.
    pub title: String,
    /// Optional task body/notes (plain text).
    #[serde(default)]
    pub body: Option<String>,
    /// Optional due date in ISO 8601 format (e.g., "2024-12-31T23:59:59").
    #[serde(default)]
    pub due_date_time: Option<String>,
    /// Optional timezone for due date (e.g., "UTC", "Pacific Standard Time").
    /// Defaults to "UTC".
    #[serde(default)]
    pub due_date_timezone: Option<String>,
    /// Task importance level. Defaults to "normal".
    #[serde(default)]
    pub importance: Option<TaskImportance>,
    /// Categories/tags for the task.
    #[serde(default)]
    pub categories: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateTaskOutput {
    pub task: TodoTask,
}

/// # Create Microsoft To Do Task
///
/// Creates a new task in a specified Microsoft To Do list via the Microsoft
/// Graph API.
///
/// Use this tool when the user wants to:
/// - Add a new task to a specific task list
/// - Create a task with a due date
/// - Set task importance/priority level
/// - Add categories or tags to a task
///
/// This tool requires a `list_id` which can be obtained using the
/// "Microsoft To Do List Task Lists" tool. Tasks are created with a status
/// of "notStarted" by default.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - tasks
/// - todo
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The `list_id` parameter is empty
/// - The `title` parameter is empty
/// - No Microsoft Todo credential is configured
/// - The `access_token` in the credential is empty
/// - The configured endpoint URL is invalid
/// - The HTTP request to Microsoft Graph API fails
/// - The response body cannot be parsed as JSON
#[tool]
pub async fn create_task(ctx: Context, input: CreateTaskInput) -> Result<CreateTaskOutput> {
    ensure!(
        !input.list_id.trim().is_empty(),
        "list_id must not be empty"
    );
    ensure!(!input.title.trim().is_empty(), "title must not be empty");

    let client = GraphClient::from_ctx(&ctx)?;

    let mut request = GraphCreateTaskRequest {
        title: input.title,
        body: input.body.map(|content| ItemBody {
            content_type: BodyType::Text,
            content,
        }),
        importance: input.importance.unwrap_or_default(),
        status: TaskStatus::NotStarted,
        due_date_time: None,
        categories: input.categories,
    };

    if let Some(due_date) = input.due_date_time {
        let timezone = input.due_date_timezone.unwrap_or_else(|| "UTC".to_string());
        request.due_date_time = Some(DateTimeTimeZone {
            date_time: due_date,
            time_zone: timezone,
        });
    }

    let task: TodoTask = client
        .post_json(
            client.url_with_segments(&["me", "todo", "lists", &input.list_id, "tasks"])?,
            &request,
            &[],
        )
        .await?;

    Ok(CreateTaskOutput { task })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompleteTaskInput {
    /// ID of the task list containing the task.
    pub list_id: String,
    /// ID of the task to complete.
    pub task_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CompleteTaskOutput {
    pub task: TodoTask,
}

/// # Complete Microsoft To Do Task
///
/// Marks a task as completed in Microsoft To Do via the Microsoft Graph API.
///
/// Use this tool when the user wants to:
/// - Mark a task as done or finished
/// - Complete a task they've finished working on
///
/// This tool updates the task's status to "completed" and automatically sets
/// the completedDateTime timestamp. The task remains in the list but is marked
/// as completed.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - tasks
/// - todo
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The `list_id` parameter is empty
/// - The `task_id` parameter is empty
/// - No Microsoft Todo credential is configured
/// - The `access_token` in the credential is empty
/// - The configured endpoint URL is invalid
/// - The HTTP request to Microsoft Graph API fails
/// - The response body cannot be parsed as JSON
#[tool]
pub async fn complete_task(ctx: Context, input: CompleteTaskInput) -> Result<CompleteTaskOutput> {
    ensure!(
        !input.list_id.trim().is_empty(),
        "list_id must not be empty"
    );
    ensure!(
        !input.task_id.trim().is_empty(),
        "task_id must not be empty"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let request = GraphUpdateTaskRequest {
        status: Some(TaskStatus::Completed),
        due_date_time: None,
        body: None,
        importance: None,
        categories: None,
    };

    let task: TodoTask = client
        .patch_json(
            client.url_with_segments(&[
                "me",
                "todo",
                "lists",
                &input.list_id,
                "tasks",
                &input.task_id,
            ])?,
            &request,
            &[],
        )
        .await?;

    Ok(CompleteTaskOutput { task })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateDueDateInput {
    /// ID of the task list containing the task.
    pub list_id: String,
    /// ID of the task to update.
    pub task_id: String,
    /// Due date in ISO 8601 format (e.g., "2024-12-31T23:59:59"). Set to null
    /// to remove due date.
    pub due_date_time: Option<String>,
    /// Timezone for due date (e.g., "UTC", "Pacific Standard Time"). Defaults
    /// to "UTC".
    #[serde(default)]
    pub due_date_timezone: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateDueDateOutput {
    pub task: TodoTask,
}

/// # Update Microsoft To Do Due Date
///
/// Updates the due date of an existing task in Microsoft To Do via the
/// Microsoft Graph API.
///
/// Use this tool when the user wants to:
/// - Set or change a task's due date
/// - Add a deadline to a task
/// - Remove a due date (by setting `due_date_time` to null)
/// - Reschedule a task to a different date
///
/// The due date requires both a datetime in ISO 8601 format and a timezone.
/// Common timezone values include "UTC", "Pacific Standard Time", "Eastern
/// Standard Time", etc. Setting `due_date_time` to null removes the due date.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - tasks
/// - todo
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The `list_id` parameter is empty
/// - The `task_id` parameter is empty
/// - No Microsoft Todo credential is configured
/// - The `access_token` in the credential is empty
/// - The configured endpoint URL is invalid
/// - The HTTP request to Microsoft Graph API fails
/// - The response body cannot be parsed as JSON
#[tool]
pub async fn update_due_date(
    ctx: Context,
    input: UpdateDueDateInput,
) -> Result<UpdateDueDateOutput> {
    ensure!(
        !input.list_id.trim().is_empty(),
        "list_id must not be empty"
    );
    ensure!(
        !input.task_id.trim().is_empty(),
        "task_id must not be empty"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let due_date_time = input.due_date_time.map(|date_time| {
        let timezone = input.due_date_timezone.unwrap_or_else(|| "UTC".to_string());
        DateTimeTimeZone {
            date_time,
            time_zone: timezone,
        }
    });

    let request = GraphUpdateTaskRequest {
        status: None,
        due_date_time,
        body: None,
        importance: None,
        categories: None,
    };

    let task: TodoTask = client
        .patch_json(
            client.url_with_segments(&[
                "me",
                "todo",
                "lists",
                &input.list_id,
                "tasks",
                &input.task_id,
            ])?,
            &request,
            &[],
        )
        .await?;

    Ok(UpdateDueDateOutput { task })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddNotesInput {
    /// ID of the task list containing the task.
    pub list_id: String,
    /// ID of the task to update.
    pub task_id: String,
    /// Notes content to add to the task.
    pub notes: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AddNotesOutput {
    pub task: TodoTask,
}

/// # Add Microsoft To Do Notes
///
/// Adds or updates the notes/body content for a task in Microsoft To Do via
/// the Microsoft Graph API.
///
/// Use this tool when the user wants to:
/// - Add detailed notes or description to a task
/// - Provide additional context or information for a task
/// - Replace existing notes with new content
///
/// Notes are stored as plain text content. This tool will replace any existing
/// notes on the task with the new content provided. To append to existing
/// notes, first retrieve the current notes and include them in the new content.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - tasks
/// - todo
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The `list_id` parameter is empty
/// - The `task_id` parameter is empty
/// - The `notes` parameter is empty
/// - No Microsoft Todo credential is configured
/// - The `access_token` in the credential is empty
/// - The configured endpoint URL is invalid
/// - The HTTP request to Microsoft Graph API fails
/// - The response body cannot be parsed as JSON
#[tool]
pub async fn add_notes(ctx: Context, input: AddNotesInput) -> Result<AddNotesOutput> {
    ensure!(
        !input.list_id.trim().is_empty(),
        "list_id must not be empty"
    );
    ensure!(
        !input.task_id.trim().is_empty(),
        "task_id must not be empty"
    );
    ensure!(!input.notes.trim().is_empty(), "notes must not be empty");

    let client = GraphClient::from_ctx(&ctx)?;

    let request = GraphUpdateTaskRequest {
        status: None,
        due_date_time: None,
        body: Some(ItemBody {
            content_type: BodyType::Text,
            content: input.notes,
        }),
        importance: None,
        categories: None,
    };

    let task: TodoTask = client
        .patch_json(
            client.url_with_segments(&[
                "me",
                "todo",
                "lists",
                &input.list_id,
                "tasks",
                &input.task_id,
            ])?,
            &request,
            &[],
        )
        .await?;

    Ok(AddNotesOutput { task })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTasksInput {
    /// ID of the task list to list tasks from.
    pub list_id: String,
    /// Maximum number of tasks to return (1-100). Defaults to 25.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Filter by task status. If not provided, returns all tasks.
    #[serde(default)]
    pub status: Option<TaskStatus>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListTasksOutput {
    pub tasks: Vec<TodoTask>,
}

/// # List Microsoft To Do Tasks
///
/// Retrieves tasks from a specific Microsoft To Do list via the Microsoft
/// Graph API.
///
/// Use this tool when the user wants to:
/// - View all tasks in a specific list
/// - Find tasks with a particular status (e.g., only incomplete tasks)
/// - Browse tasks to identify which ones to complete or modify
///
/// This tool returns comprehensive task information including title, status,
/// importance, due dates, reminders, notes/body content, and categories.
/// Tasks can be filtered by status to show only active or completed tasks.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - tasks
/// - todo
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The `list_id` parameter is empty
/// - The `limit` parameter is outside the valid range (1-100)
/// - No Microsoft Todo credential is configured
/// - The `access_token` in the credential is empty
/// - The configured endpoint URL is invalid
/// - The HTTP request to Microsoft Graph API fails
/// - The response body cannot be parsed as JSON
/// - The `status` filter cannot be serialized
#[tool]
pub async fn list_tasks(ctx: Context, input: ListTasksInput) -> Result<ListTasksOutput> {
    ensure!(
        !input.list_id.trim().is_empty(),
        "list_id must not be empty"
    );
    let limit = input.limit.unwrap_or(25);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let mut query = vec![
        ("$top", limit.to_string()),
        (
            "$select",
            "id,title,body,status,importance,isReminderOn,reminderDateTime,dueDateTime,\
             completedDateTime,createdDateTime,lastModifiedDateTime,categories"
                .to_string(),
        ),
    ];

    if let Some(status) = input.status {
        let status_str = serde_json::to_string(&status)?;
        query.push((
            "$filter",
            format!("status eq {}", status_str.trim_matches('"')),
        ));
    }

    let response: GraphListResponse<TodoTask> = client
        .get_json(
            client.url_with_segments(&["me", "todo", "lists", &input.list_id, "tasks"])?,
            &query,
            &[],
        )
        .await?;

    Ok(ListTasksOutput {
        tasks: response.value,
    })
}

// ========== Internal Graph Client ==========

#[derive(Debug, Deserialize)]
struct GraphListResponse<T> {
    value: Vec<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphCreateTaskRequest {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<ItemBody>,
    importance: TaskImportance,
    status: TaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    due_date_time: Option<DateTimeTimeZone>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    categories: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphUpdateTaskRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<TaskStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    due_date_time: Option<DateTimeTimeZone>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<ItemBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    importance: Option<TaskImportance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    categories: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
struct GraphClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl GraphClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = MicrosoftTodoCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_GRAPH_ENDPOINT))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            access_token: cred.access_token,
        })
    }

    fn url_with_segments(&self, segments: &[&str]) -> Result<reqwest::Url> {
        let mut url = reqwest::Url::parse(&self.base_url)?;
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|()| operai::anyhow::anyhow!("base_url must be an absolute URL"))?;
            for segment in segments {
                path.push(segment);
            }
        }
        Ok(url)
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        query: &[(&str, String)],
        extra_headers: &[(&str, &str)],
    ) -> Result<T> {
        let mut request = self.http.get(url).query(query);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        let response = self.send_request(request).await?;
        Ok(response.json::<T>().await?)
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
        extra_headers: &[(&str, &str)],
    ) -> Result<TRes> {
        let mut request = self.http.post(url).json(body);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        let response = self.send_request(request).await?;
        Ok(response.json::<TRes>().await?)
    }

    async fn patch_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
        extra_headers: &[(&str, &str)],
    ) -> Result<TRes> {
        let mut request = self.http.patch(url).json(body);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        let response = self.send_request(request).await?;
        Ok(response.json::<TRes>().await?)
    }

    async fn send_request(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let response = request
            .bearer_auth(&self.access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Microsoft Graph request failed ({status}): {body}"
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

// Re-export the entrypoint for tests
pub use __operai_entrypoint::get_root_module;
