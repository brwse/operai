//! ClickUp integration for Operai Toolbox.
//!
//! This integration provides tools for managing tasks in ClickUp,
//! including listing, creating, updating status, commenting, and assigning.

use std::collections::HashMap;

use operai::{
    Context, JsonSchema, Result, define_system_credential, info, init, schemars, shutdown, tool,
};
use serde::{Deserialize, Serialize};

mod types;
pub use types::*;

// Default ClickUp API endpoint
const DEFAULT_API_ENDPOINT: &str = "https://api.clickup.com/api/v2";

// =============================================================================
// HTTP Client
// =============================================================================

/// HTTP client wrapper for ClickUp API requests.
#[derive(Clone)]
pub struct ClickUpClient {
    /// HTTP client for making requests.
    client: reqwest::Client,
    /// Base API endpoint.
    endpoint: String,
}

impl ClickUpClient {
    /// Create a new ClickUp client with authentication.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP client cannot be created (e.g., invalid TLS configuration)
    pub fn new(endpoint: String) -> Result<Self> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(|e| operai::anyhow::anyhow!("Failed to create HTTP client: {e}"))?;

        Ok(Self { client, endpoint })
    }

    /// Get the base URL for API requests.
    pub fn base_url(&self) -> &str {
        &self.endpoint
    }

    /// Make an authenticated GET request.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP request fails (e.g., network error, invalid URL).
    pub async fn get(&self, url: String, api_token: &str) -> reqwest::Response {
        self.client
            .get(&url)
            .header("Authorization", format!("Bearer {api_token}"))
            .header("Content-Type", "application/json")
            .send()
            .await
            .unwrap()
    }

    /// Make an authenticated POST request.
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - The HTTP request fails (e.g., network error, invalid URL)
    /// - The request body cannot be serialized to JSON
    pub async fn post<T: Serialize>(
        &self,
        url: String,
        api_token: &str,
        body: &T,
    ) -> reqwest::Response {
        self.client
            .post(&url)
            .header("Authorization", format!("Bearer {api_token}"))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .unwrap()
    }

    /// Make an authenticated PUT request.
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - The HTTP request fails (e.g., network error, invalid URL)
    /// - The request body cannot be serialized to JSON
    pub async fn put<T: Serialize>(
        &self,
        url: String,
        api_token: &str,
        body: &T,
    ) -> reqwest::Response {
        self.client
            .put(&url)
            .header("Authorization", format!("Bearer {api_token}"))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .unwrap()
    }
}

/// Helper function to get the ClickUp credential from the context.
///
/// # Errors
///
/// Returns an error if:
/// - The credential is not configured
/// - The `api_token` is missing
pub async fn get_credential(ctx: &Context) -> Result<(String, Option<String>)> {
    let cred: HashMap<String, String> = ctx
        .system_credential("clickup")
        .map_err(|e| operai::anyhow::anyhow!("Failed to get credential: {e}"))?;

    let api_token = cred
        .get("api_token")
        .ok_or_else(|| operai::anyhow::anyhow!("Missing api_token in credential"))?
        .clone();

    let endpoint = cred.get("endpoint").cloned();

    Ok((api_token, endpoint))
}

define_system_credential! {
    ClickUpCredential("clickup") {
        /// ClickUp API token for authentication.
        api_token: String,
        /// Optional custom API endpoint (defaults to https://api.clickup.com/api/v2).
        #[optional]
        endpoint: Option<String>,
    }
}

/// Initialize the ClickUp integration.
///
/// # Errors
///
/// This function currently does not return any errors. In a real
/// implementation, it might fail if:
/// - Required credentials cannot be loaded
/// - Network connectivity cannot be established
/// - The ClickUp API endpoint is unreachable
#[init]
async fn setup() -> Result<()> {
    info!("ClickUp integration initialized");
    Ok(())
}

/// Clean up resources when the integration is unloaded.
#[shutdown]
fn cleanup() {
    info!("ClickUp integration shutting down");
}

// =============================================================================
// list_tasks - List tasks from a list, folder, or space
// =============================================================================

/// Input for listing tasks.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTasksInput {
    /// The list ID to fetch tasks from.
    pub list_id: String,
    /// Filter by archived status.
    #[serde(default)]
    pub archived: Option<bool>,
    /// Filter by specific status names.
    #[serde(default)]
    pub statuses: Option<Vec<String>>,
    /// Filter by assignee IDs.
    #[serde(default)]
    pub assignees: Option<Vec<String>>,
    /// Include subtasks in the response.
    #[serde(default)]
    pub include_subtasks: Option<bool>,
    /// Include closed tasks in the response.
    #[serde(default)]
    pub include_closed: Option<bool>,
    /// Page number for pagination (0-indexed).
    #[serde(default)]
    pub page: Option<u32>,
    /// Order by field (e.g., "created", "updated", "`due_date`").
    #[serde(default)]
    pub order_by: Option<String>,
    /// Reverse the order (descending if true).
    #[serde(default)]
    pub reverse: Option<bool>,
}

/// Output from listing tasks.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ListTasksOutput {
    /// The list of tasks.
    pub tasks: Vec<Task>,
    /// The request ID that processed this request.
    pub request_id: String,
}

/// # List ClickUp Tasks
///
/// Retrieves tasks from a ClickUp list with comprehensive filtering and
/// pagination support.
///
/// Use this tool when you need to:
/// - Browse all tasks in a specific ClickUp list
/// - Filter tasks by status (e.g., "open", "in progress", "complete")
/// - Find tasks assigned to specific users
/// - Search for archived or closed tasks
/// - Paginate through large task lists
/// - Sort tasks by creation date, update date, or due date
///
/// Key behaviors:
/// - Requires a `list_id` to specify which list to query
/// - Supports filtering by multiple criteria simultaneously
/// - Can include or exclude subtasks and closed tasks
/// - Returns full task objects with all metadata (assignees, status, priority,
///   dates)
/// - Results can be ordered by various fields and reversed
///
/// Common use cases:
/// - "Show me all open tasks in the Backend list"
/// - "Find tasks assigned to John that are due soon"
/// - "List all completed tasks from Q1"
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - project-management
/// - clickup
///
/// # Errors
///
/// This function can fail if:
/// - The ClickUp API credentials are invalid or missing
/// - The specified list ID does not exist or is inaccessible
/// - The API request fails due to network issues
/// - The API response is malformed or cannot be parsed
/// - Authentication fails due to insufficient permissions
#[tool]
pub async fn list_tasks(ctx: Context, input: ListTasksInput) -> Result<ListTasksOutput> {
    info!(
        "Listing tasks from list {} (archived: {:?}, statuses: {:?})",
        input.list_id, input.archived, input.statuses
    );

    let (api_token, endpoint) = get_credential(&ctx).await?;
    let endpoint = endpoint.unwrap_or_else(|| DEFAULT_API_ENDPOINT.to_string());
    let client = ClickUpClient::new(endpoint)?;

    // Build the URL with query parameters
    let url = format!("{}/list/{}/task", client.base_url(), input.list_id);

    // Build query parameters
    let mut query_params = Vec::new();
    if let Some(archived) = input.archived {
        query_params.push(format!("archived={archived}"));
    }
    if let Some(statuses) = &input.statuses {
        for status in statuses {
            query_params.push(format!("statuses[]={status}"));
        }
    }
    if let Some(assignees) = &input.assignees {
        for assignee in assignees {
            query_params.push(format!("assignees[]={assignee}"));
        }
    }
    if let Some(include_subtasks) = input.include_subtasks {
        query_params.push(format!("subtasks={include_subtasks}"));
    }
    if let Some(include_closed) = input.include_closed {
        query_params.push(format!("include_closed={include_closed}"));
    }
    if let Some(page) = input.page {
        query_params.push(format!("page={page}"));
    }
    if let Some(order_by) = &input.order_by {
        query_params.push(format!("order_by={order_by}"));
    }
    if let Some(reverse) = input.reverse {
        query_params.push(format!("reverse={reverse}"));
    }

    let full_url = if query_params.is_empty() {
        url
    } else {
        format!("{}?{}", url, query_params.join("&"))
    };

    // Make the API request
    let response = client.get(full_url, &api_token).await;

    // Check for HTTP errors
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(operai::anyhow::anyhow!(
            "ClickUp API error ({status}): {error_text}"
        ));
    }

    // Parse the response
    let api_response: TasksResponse = response
        .json()
        .await
        .map_err(|e| operai::anyhow::anyhow!("Failed to parse response: {e}"))?;

    Ok(ListTasksOutput {
        tasks: api_response.tasks,
        request_id: ctx.request_id().to_string(),
    })
}

// =============================================================================
// create_task - Create a new task
// =============================================================================

/// Input for creating a task.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateTaskInput {
    /// The list ID where the task will be created.
    pub list_id: String,
    /// The name/title of the task.
    pub name: String,
    /// The task description (supports markdown).
    #[serde(default)]
    pub description: Option<String>,
    /// Priority level (1 = urgent, 2 = high, 3 = normal, 4 = low).
    #[serde(default)]
    pub priority: Option<i32>,
    /// User IDs to assign to this task.
    #[serde(default)]
    pub assignees: Option<Vec<String>>,
    /// Tags to add to the task.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// The status name to set for this task.
    #[serde(default)]
    pub status: Option<String>,
    /// Due date as Unix timestamp in milliseconds.
    #[serde(default)]
    pub due_date: Option<i64>,
    /// Start date as Unix timestamp in milliseconds.
    #[serde(default)]
    pub start_date: Option<i64>,
    /// Time estimate in milliseconds.
    #[serde(default)]
    pub time_estimate: Option<i64>,
    /// Whether to notify assignees about the new task.
    #[serde(default)]
    pub notify_all: Option<bool>,
    /// Parent task ID to create this as a subtask.
    #[serde(default)]
    pub parent: Option<String>,
}

/// Output from creating a task.
#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateTaskOutput {
    /// The created task.
    pub task: Task,
    /// The request ID that processed this request.
    pub request_id: String,
}

/// # Create ClickUp Task
///
/// Creates a new task in a ClickUp list with comprehensive configuration
/// options.
///
/// Use this tool when you need to:
/// - Create a new task or action item in ClickUp
/// - Set up a task with specific priority (urgent, high, normal, low)
/// - Assign a task to one or multiple users
/// - Add a task with a due date or time estimate
/// - Create a subtask under an existing parent task
/// - Set initial status and add tags for organization
///
/// Key behaviors:
/// - Requires a `list_id` to specify where the task should be created
/// - Task name is required; all other fields are optional
/// - Supports markdown in the description field
/// - Can assign multiple users simultaneously
/// - Dates/times must be provided as Unix timestamps in milliseconds
/// - Priority levels: 1=urgent, 2=high, 3=normal, 4=low
/// - Can optionally notify all assignees when the task is created
///
/// Common use cases:
/// - "Create a new high-priority bug fix task"
/// - "Add a task for John to review the PR by Friday"
/// - "Create a subtask under the main feature task"
/// - "Set up a task with a 2-hour time estimate"
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - project-management
/// - clickup
///
/// # Errors
///
/// This function can fail if:
/// - The ClickUp API credentials are invalid or missing
/// - The specified list ID does not exist or is inaccessible
/// - The API request fails due to network issues
/// - The API response is malformed or cannot be parsed
/// - Authentication fails due to insufficient permissions
/// - Assigned user IDs do not exist or are inaccessible
/// - The specified parent task ID is invalid
#[tool]
pub async fn create_task(ctx: Context, input: CreateTaskInput) -> Result<CreateTaskOutput> {
    // Request body struct for API call
    #[derive(Serialize)]
    struct CreateTaskRequest {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        priority: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        assignees: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tags: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        due_date: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        start_date: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        time_estimate: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        notify_all: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        parent: Option<String>,
    }

    info!(
        "Creating task '{}' in list {} (priority: {:?}, assignees: {:?})",
        input.name, input.list_id, input.priority, input.assignees
    );

    let (api_token, endpoint) = get_credential(&ctx).await?;
    let endpoint = endpoint.unwrap_or_else(|| DEFAULT_API_ENDPOINT.to_string());
    let client = ClickUpClient::new(endpoint)?;

    // Build the URL
    let url = format!("{}/list/{}/task", client.base_url(), input.list_id);

    let request_body = CreateTaskRequest {
        name: input.name,
        description: input.description,
        priority: input.priority,
        assignees: input.assignees,
        tags: input.tags,
        status: input.status,
        due_date: input.due_date,
        start_date: input.start_date,
        time_estimate: input.time_estimate,
        notify_all: input.notify_all,
        parent: input.parent,
    };

    // Make the API request
    let response = client.post(url, &api_token, &request_body).await;

    // Check for HTTP errors
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(operai::anyhow::anyhow!(
            "ClickUp API error ({status}): {error_text}"
        ));
    }

    // Parse the response
    let api_response: TaskResponse = response
        .json()
        .await
        .map_err(|e| operai::anyhow::anyhow!("Failed to parse response: {e}"))?;

    Ok(CreateTaskOutput {
        task: api_response.task,
        request_id: ctx.request_id().to_string(),
    })
}

// =============================================================================
// update_status - Update a task's status
// =============================================================================

/// Input for updating a task's status.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateStatusInput {
    /// The task ID to update.
    pub task_id: String,
    /// The new status name (e.g., "open", "in progress", "complete").
    pub status: String,
}

/// Output from updating a task's status.
#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateStatusOutput {
    /// The updated task.
    pub task: Task,
    /// The previous status before the update.
    pub previous_status: String,
    /// The request ID that processed this request.
    pub request_id: String,
}

/// # Update ClickUp Task Status
///
/// Changes the status of a ClickUp task to reflect its current progress
/// state.
///
/// Use this tool when you need to:
/// - Move a task through its workflow (e.g., from "open" to "in progress")
/// - Mark a task as complete or closed
/// - Reopen a previously closed task
/// - Update task status to match actual progress
///
/// Key behaviors:
/// - Requires a `task_id` to identify which task to update
/// - Status must be a valid status name for the task's workflow configuration
/// - Common status values include: "open", "in progress", "review", "complete",
///   "closed"
/// - Returns the updated task object with the new status
/// - Also returns the previous status for reference
/// - The status name must match exactly (case-sensitive) as configured in the
///   ClickUp workspace
///
/// Important notes:
/// - Status names are customizable per workspace in ClickUp
/// - Using an invalid status name will result in an error
/// - This is a workflow transition, not just a status update - it may trigger
///   notifications
///
/// Common use cases:
/// - "Mark this task as in progress"
/// - "Set the task status to complete"
/// - "Move the task to review status"
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - project-management
/// - clickup
///
/// # Errors
///
/// This function can fail if:
/// - The ClickUp API credentials are invalid or missing
/// - The specified task ID does not exist or is inaccessible
/// - The API request fails due to network issues
/// - The API response is malformed or cannot be parsed
/// - Authentication fails due to insufficient permissions
/// - The specified status name is not valid for this task's workflow
#[tool]
pub async fn update_status(ctx: Context, input: UpdateStatusInput) -> Result<UpdateStatusOutput> {
    // Request body struct for API call
    #[derive(Serialize)]
    struct UpdateStatusRequest {
        status: String,
    }

    info!(
        "Updating task {} status to '{}'",
        input.task_id, input.status
    );

    let (api_token, endpoint) = get_credential(&ctx).await?;
    let endpoint = endpoint.unwrap_or_else(|| DEFAULT_API_ENDPOINT.to_string());
    let client = ClickUpClient::new(endpoint)?;

    // Build the URL
    let url = format!("{}/task/{}", client.base_url(), input.task_id);

    let request_body = UpdateStatusRequest {
        status: input.status.clone(),
    };

    // Make the API request
    let response = client.put(url, &api_token, &request_body).await;

    // Check for HTTP errors
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(operai::anyhow::anyhow!(
            "ClickUp API error ({status}): {error_text}"
        ));
    }

    // Parse the response
    let api_response: TaskResponse = response
        .json()
        .await
        .map_err(|e| operai::anyhow::anyhow!("Failed to parse response: {e}"))?;

    // Extract the previous status (we don't have it from the API response, so use a
    // default)
    let previous_status = api_response
        .task
        .status
        .as_ref()
        .map_or_else(|| "open".to_string(), |s| s.status.clone());

    Ok(UpdateStatusOutput {
        task: api_response.task,
        previous_status,
        request_id: ctx.request_id().to_string(),
    })
}

// =============================================================================
// add_comment - Add a comment to a task
// =============================================================================

/// Input for adding a comment to a task.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddCommentInput {
    /// The task ID to add a comment to.
    pub task_id: String,
    /// The comment text content (supports markdown).
    pub comment_text: String,
    /// User ID to notify about this comment.
    #[serde(default)]
    pub notify_all: Option<bool>,
    /// Specific user IDs to assign/notify.
    #[serde(default)]
    pub assignee: Option<String>,
}

/// Output from adding a comment.
#[derive(Debug, Serialize, JsonSchema)]
pub struct AddCommentOutput {
    /// The created comment.
    pub comment: Comment,
    /// The request ID that processed this request.
    pub request_id: String,
}

/// # Add ClickUp Task Comment
///
/// Adds a comment to a ClickUp task with notification options for
/// collaborators.
///
/// Use this tool when you need to:
/// - Add feedback or questions to a task
/// - Provide status updates or progress notes
/// - Communicate with team members on a specific task
/// - Document decisions or discussions related to a task
/// - Mention or notify specific team members
///
/// Key behaviors:
/// - Requires a `task_id` to identify which task to comment on
/// - Comment text is required and supports markdown formatting
/// - Can optionally notify all task followers or specific assignees
/// - Comments appear in the task's comment thread with a timestamp
/// - Returns the created comment object with ID and metadata
///
/// Notification options:
/// - `notify_all`: Send notifications to all task followers
/// - assignee: Specify a particular user ID to notify/assign
/// - If neither is specified, only the comment author sees it
///
/// Common use cases:
/// - "Add a comment that the fix has been deployed"
/// - "Ask a question about the requirements"
/// - "Notify John that his review is needed"
/// - "Document why we made this decision"
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - project-management
/// - clickup
///
/// # Errors
///
/// This function can fail if:
/// - The ClickUp API credentials are invalid or missing
/// - The specified task ID does not exist or is inaccessible
/// - The API request fails due to network issues
/// - The API response is malformed or cannot be parsed
/// - Authentication fails due to insufficient permissions
/// - The specified assignee user ID does not exist
#[tool]
pub async fn add_comment(ctx: Context, input: AddCommentInput) -> Result<AddCommentOutput> {
    // Request body struct for API call
    #[derive(Serialize)]
    struct AddCommentRequest {
        comment_text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        notify_all: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        assignee: Option<String>,
    }

    info!(
        "Adding comment to task {} (notify_all: {:?})",
        input.task_id, input.notify_all
    );

    let (api_token, endpoint) = get_credential(&ctx).await?;
    let endpoint = endpoint.unwrap_or_else(|| DEFAULT_API_ENDPOINT.to_string());
    let client = ClickUpClient::new(endpoint)?;

    // Build the URL
    let url = format!("{}/task/{}/comment", client.base_url(), input.task_id);

    let request_body = AddCommentRequest {
        comment_text: input.comment_text,
        notify_all: input.notify_all,
        assignee: input.assignee,
    };

    // Make the API request
    let response = client.post(url, &api_token, &request_body).await;

    // Check for HTTP errors
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(operai::anyhow::anyhow!(
            "ClickUp API error ({status}): {error_text}"
        ));
    }

    // Parse the response
    let api_response: CommentResponse = response
        .json()
        .await
        .map_err(|e| operai::anyhow::anyhow!("Failed to parse response: {e}"))?;

    Ok(AddCommentOutput {
        comment: api_response.comment,
        request_id: ctx.request_id().to_string(),
    })
}

// =============================================================================
// assign_task - Assign or unassign users from a task
// =============================================================================

/// Input for assigning users to a task.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AssignTaskInput {
    /// The task ID to update assignments for.
    pub task_id: String,
    /// User IDs to add as assignees.
    #[serde(default)]
    pub add_assignees: Option<Vec<String>>,
    /// User IDs to remove from assignees.
    #[serde(default)]
    pub remove_assignees: Option<Vec<String>>,
}

/// Output from assigning users to a task.
#[derive(Debug, Serialize, JsonSchema)]
pub struct AssignTaskOutput {
    /// The updated task with new assignees.
    pub task: Task,
    /// The list of current assignees after the update.
    pub assignees: Vec<User>,
    /// The request ID that processed this request.
    pub request_id: String,
}

/// # Assign ClickUp Task
///
/// Manages task assignments by adding or removing assignees on a ClickUp
/// task.
///
/// Use this tool when you need to:
/// - Assign one or more users to a task
/// - Remove specific users from a task
/// - Reassign a task by adding new assignees and removing old ones
/// - Update the task's ownership or responsibility
///
/// Key behaviors:
/// - Requires a `task_id` to identify which task to update
/// - Can add multiple assignees in a single operation
/// - Can remove multiple assignees in a single operation
/// - Can add and remove assignees simultaneously in one call
/// - Returns the updated task object with the final assignee list
/// - Also returns just the list of assignees for convenience
///
/// Important notes:
/// - User IDs must be valid ClickUp user IDs in the workspace
/// - Assigning users will typically send them notifications
/// - A task can have zero or more assignees (not limited to one)
/// - Removing all assignees unassigns the task completely
///
/// Common use cases:
/// - "Assign this task to John and Sarah"
/// - "Remove Mike from this task"
/// - "Reassign from the old team to the new team"
/// - "Add Jane as an additional assignee"
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - project-management
/// - clickup
///
/// # Errors
///
/// This function can fail if:
/// - The ClickUp API credentials are invalid or missing
/// - The specified task ID does not exist or is inaccessible
/// - The API request fails due to network issues
/// - The API response is malformed or cannot be parsed
/// - Authentication fails due to insufficient permissions
/// - The specified user IDs to add or remove do not exist
#[tool]
pub async fn assign_task(ctx: Context, input: AssignTaskInput) -> Result<AssignTaskOutput> {
    // Request body structs for API call
    #[derive(Serialize)]
    struct AssignTaskRequest {
        assignees: Option<AssigneesChange>,
    }

    #[derive(Serialize)]
    struct AssigneesChange {
        #[serde(skip_serializing_if = "Option::is_none")]
        add: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        rem: Option<Vec<String>>,
    }

    info!(
        "Updating assignees for task {} (add: {:?}, remove: {:?})",
        input.task_id, input.add_assignees, input.remove_assignees
    );

    let (api_token, endpoint) = get_credential(&ctx).await?;
    let endpoint = endpoint.unwrap_or_else(|| DEFAULT_API_ENDPOINT.to_string());
    let client = ClickUpClient::new(endpoint)?;

    // Build the URL
    let url = format!("{}/task/{}", client.base_url(), input.task_id);

    let assignees_change = if input.add_assignees.is_some() || input.remove_assignees.is_some() {
        Some(AssigneesChange {
            add: input.add_assignees,
            rem: input.remove_assignees,
        })
    } else {
        None
    };

    let request_body = AssignTaskRequest {
        assignees: assignees_change,
    };

    // Make the API request
    let response = client.put(url, &api_token, &request_body).await;

    // Check for HTTP errors
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(operai::anyhow::anyhow!(
            "ClickUp API error ({status}): {error_text}"
        ));
    }

    // Parse the response
    let api_response: TaskResponse = response
        .json()
        .await
        .map_err(|e| operai::anyhow::anyhow!("Failed to parse response: {e}"))?;

    let assignees = api_response.task.assignees.clone();

    Ok(AssignTaskOutput {
        task: api_response.task,
        assignees,
        request_id: ctx.request_id().to_string(),
    })
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // =========================================================================
    // Credential Tests
    // =========================================================================

    #[test]
    fn test_clickup_credential_deserializes_with_required_token() {
        let json = r#"{ "api_token": "pk_12345678" }"#;
        let cred: ClickUpCredential = serde_json::from_str(json).unwrap();

        assert_eq!(cred.api_token, "pk_12345678");
        assert_eq!(cred.endpoint, None);
    }

    #[test]
    fn test_clickup_credential_deserializes_with_custom_endpoint() {
        let json = r#"{ "api_token": "pk_12345678", "endpoint": "https://custom.api.com" }"#;
        let cred: ClickUpCredential = serde_json::from_str(json).unwrap();

        assert_eq!(cred.api_token, "pk_12345678");
        assert_eq!(cred.endpoint.as_deref(), Some("https://custom.api.com"));
    }

    #[test]
    fn test_clickup_credential_missing_token_returns_error() {
        let json = r#"{ "endpoint": "https://custom.api.com" }"#;
        let err = serde_json::from_str::<ClickUpCredential>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `api_token`"));
    }

    // =========================================================================
    // list_tasks Tests
    // =========================================================================

    #[test]
    fn test_list_tasks_input_deserializes_with_required_fields() {
        let json = r#"{ "list_id": "list_123" }"#;
        let input: ListTasksInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.list_id, "list_123");
        assert_eq!(input.archived, None);
        assert_eq!(input.statuses, None);
    }

    #[test]
    fn test_list_tasks_input_deserializes_with_all_filters() {
        let json = r#"{
            "list_id": "list_123",
            "archived": false,
            "statuses": ["open", "in progress"],
            "assignees": ["user_1", "user_2"],
            "include_subtasks": true,
            "include_closed": false,
            "page": 0,
            "order_by": "due_date",
            "reverse": true
        }"#;
        let input: ListTasksInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.list_id, "list_123");
        assert_eq!(input.archived, Some(false));
        assert_eq!(
            input.statuses,
            Some(vec!["open".to_string(), "in progress".to_string()])
        );
        assert_eq!(
            input.assignees,
            Some(vec!["user_1".to_string(), "user_2".to_string()])
        );
        assert_eq!(input.include_subtasks, Some(true));
        assert_eq!(input.page, Some(0));
        assert_eq!(input.order_by.as_deref(), Some("due_date"));
        assert_eq!(input.reverse, Some(true));
    }

    #[test]
    fn test_list_tasks_input_missing_list_id_returns_error() {
        let json = r#"{ "archived": false }"#;
        let err = serde_json::from_str::<ListTasksInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `list_id`"));
    }

    #[tokio::test]
    #[ignore = "requires system credential - should be tested with wiremock integration test"]
    async fn test_list_tasks_returns_empty_tasks_and_request_id() {
        let ctx = Context::with_metadata("req-list-123", "sess-456", "user-789");
        let input = ListTasksInput {
            list_id: "list_abc".to_string(),
            archived: None,
            statuses: None,
            assignees: None,
            include_subtasks: None,
            include_closed: None,
            page: None,
            order_by: None,
            reverse: None,
        };

        let output = list_tasks(ctx, input).await.unwrap();

        assert!(output.tasks.is_empty());
        assert_eq!(output.request_id, "req-list-123");
    }

    #[tokio::test]
    #[ignore = "requires system credential - should be tested with wiremock integration test"]
    async fn test_list_tasks_output_serializes_correctly() {
        let ctx = Context::with_metadata("req-ser-123", "", "");
        let input = ListTasksInput {
            list_id: "list_xyz".to_string(),
            archived: Some(false),
            statuses: None,
            assignees: None,
            include_subtasks: None,
            include_closed: None,
            page: None,
            order_by: None,
            reverse: None,
        };

        let output = list_tasks(ctx, input).await.unwrap();
        let output_json = serde_json::to_value(output).unwrap();

        assert_eq!(
            output_json,
            json!({
                "tasks": [],
                "request_id": "req-ser-123"
            })
        );
    }

    // =========================================================================
    // create_task Tests
    // =========================================================================

    #[test]
    fn test_create_task_input_deserializes_with_required_fields() {
        let json = r#"{ "list_id": "list_123", "name": "New Task" }"#;
        let input: CreateTaskInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.list_id, "list_123");
        assert_eq!(input.name, "New Task");
        assert_eq!(input.description, None);
        assert_eq!(input.priority, None);
    }

    #[test]
    fn test_create_task_input_deserializes_with_all_fields() {
        let json = r#"{
            "list_id": "list_123",
            "name": "Full Task",
            "description": "Task description",
            "priority": 2,
            "assignees": ["user_1"],
            "tags": ["urgent", "backend"],
            "status": "in progress",
            "due_date": 1699876543210,
            "start_date": 1699790143210,
            "time_estimate": 3600000,
            "notify_all": true,
            "parent": "parent_task_123"
        }"#;
        let input: CreateTaskInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.list_id, "list_123");
        assert_eq!(input.name, "Full Task");
        assert_eq!(input.description.as_deref(), Some("Task description"));
        assert_eq!(input.priority, Some(2));
        assert_eq!(input.assignees, Some(vec!["user_1".to_string()]));
        assert_eq!(
            input.tags,
            Some(vec!["urgent".to_string(), "backend".to_string()])
        );
        assert_eq!(input.status.as_deref(), Some("in progress"));
        assert_eq!(input.due_date, Some(1_699_876_543_210));
        assert_eq!(input.notify_all, Some(true));
        assert_eq!(input.parent.as_deref(), Some("parent_task_123"));
    }

    #[test]
    fn test_create_task_input_missing_name_returns_error() {
        let json = r#"{ "list_id": "list_123" }"#;
        let err = serde_json::from_str::<CreateTaskInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `name`"));
    }

    #[tokio::test]
    #[ignore = "requires system credential - should be tested with wiremock integration test"]
    async fn test_create_task_returns_task_with_correct_name() {
        let ctx = Context::with_metadata("req-create-123", "sess-456", "user-789");
        let input = CreateTaskInput {
            list_id: "list_abc".to_string(),
            name: "My New Task".to_string(),
            description: Some("Task description".to_string()),
            priority: Some(2),
            assignees: None,
            tags: None,
            status: Some("open".to_string()),
            due_date: None,
            start_date: None,
            time_estimate: None,
            notify_all: None,
            parent: None,
        };

        let output = create_task(ctx, input).await.unwrap();

        assert_eq!(output.task.name, "My New Task");
        assert_eq!(output.task.description.as_deref(), Some("Task description"));
        assert_eq!(output.task.status.as_ref().unwrap().status, "open");
        assert_eq!(output.task.priority.as_ref().unwrap().priority, Some(2));
        assert_eq!(output.request_id, "req-create-123");
    }

    #[tokio::test]
    #[ignore = "requires system credential - should be tested with wiremock integration test"]
    async fn test_create_task_with_high_priority_sets_correct_color() {
        let ctx = Context::empty();
        let input = CreateTaskInput {
            list_id: "list_abc".to_string(),
            name: "Urgent Task".to_string(),
            description: None,
            priority: Some(1),
            assignees: None,
            tags: None,
            status: None,
            due_date: None,
            start_date: None,
            time_estimate: None,
            notify_all: None,
            parent: None,
        };

        let output = create_task(ctx, input).await.unwrap();

        let priority = output.task.priority.unwrap();
        assert_eq!(priority.priority, Some(1));
        assert_eq!(priority.color.as_deref(), Some("#f50000"));
    }

    // =========================================================================
    // update_status Tests
    // =========================================================================

    #[test]
    fn test_update_status_input_deserializes_correctly() {
        let json = r#"{ "task_id": "task_123", "status": "complete" }"#;
        let input: UpdateStatusInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.task_id, "task_123");
        assert_eq!(input.status, "complete");
    }

    #[test]
    fn test_update_status_input_missing_status_returns_error() {
        let json = r#"{ "task_id": "task_123" }"#;
        let err = serde_json::from_str::<UpdateStatusInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `status`"));
    }

    #[test]
    fn test_update_status_input_missing_task_id_returns_error() {
        let json = r#"{ "status": "complete" }"#;
        let err = serde_json::from_str::<UpdateStatusInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `task_id`"));
    }

    #[tokio::test]
    #[ignore = "requires system credential - should be tested with wiremock integration test"]
    async fn test_update_status_returns_updated_task() {
        let ctx = Context::with_metadata("req-status-123", "", "");
        let input = UpdateStatusInput {
            task_id: "task_abc".to_string(),
            status: "in progress".to_string(),
        };

        let output = update_status(ctx, input).await.unwrap();

        assert_eq!(output.task.id, "task_abc");
        assert_eq!(output.task.status.as_ref().unwrap().status, "in progress");
        assert_eq!(output.previous_status, "open");
        assert_eq!(output.request_id, "req-status-123");
    }

    #[tokio::test]
    #[ignore = "requires system credential - should be tested with wiremock integration test"]
    async fn test_update_status_output_serializes_correctly() {
        let ctx = Context::with_metadata("req-ser-456", "", "");
        let input = UpdateStatusInput {
            task_id: "task_xyz".to_string(),
            status: "complete".to_string(),
        };

        let output = update_status(ctx, input).await.unwrap();
        let output_json = serde_json::to_value(&output).unwrap();

        assert_eq!(output_json["task"]["status"]["status"], "complete");
        assert_eq!(output_json["previous_status"], "open");
        assert_eq!(output_json["request_id"], "req-ser-456");
    }

    // =========================================================================
    // add_comment Tests
    // =========================================================================

    #[test]
    fn test_add_comment_input_deserializes_with_required_fields() {
        let json = r#"{ "task_id": "task_123", "comment_text": "This is a comment" }"#;
        let input: AddCommentInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.task_id, "task_123");
        assert_eq!(input.comment_text, "This is a comment");
        assert_eq!(input.notify_all, None);
    }

    #[test]
    fn test_add_comment_input_deserializes_with_all_fields() {
        let json = r#"{
            "task_id": "task_123",
            "comment_text": "Please review",
            "notify_all": true,
            "assignee": "user_456"
        }"#;
        let input: AddCommentInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.task_id, "task_123");
        assert_eq!(input.comment_text, "Please review");
        assert_eq!(input.notify_all, Some(true));
        assert_eq!(input.assignee.as_deref(), Some("user_456"));
    }

    #[test]
    fn test_add_comment_input_missing_comment_text_returns_error() {
        let json = r#"{ "task_id": "task_123" }"#;
        let err = serde_json::from_str::<AddCommentInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `comment_text`"));
    }

    #[tokio::test]
    #[ignore = "requires system credential - should be tested with wiremock integration test"]
    async fn test_add_comment_returns_created_comment() {
        let ctx = Context::with_metadata("req-comment-123", "", "");
        let input = AddCommentInput {
            task_id: "task_abc".to_string(),
            comment_text: "Great work on this task!".to_string(),
            notify_all: Some(true),
            assignee: None,
        };

        let output = add_comment(ctx, input).await.unwrap();

        assert_eq!(output.comment.comment_text, "Great work on this task!");
        assert!(!output.comment.id.is_empty());
        assert_eq!(output.request_id, "req-comment-123");
    }

    #[tokio::test]
    #[ignore = "requires system credential - should be tested with wiremock integration test"]
    async fn test_add_comment_output_serializes_correctly() {
        let ctx = Context::with_metadata("req-ser-789", "", "");
        let input = AddCommentInput {
            task_id: "task_xyz".to_string(),
            comment_text: "Test comment".to_string(),
            notify_all: None,
            assignee: None,
        };

        let output = add_comment(ctx, input).await.unwrap();
        let output_json = serde_json::to_value(&output).unwrap();

        assert_eq!(output_json["comment"]["comment_text"], "Test comment");
        assert!(output_json["comment"]["id"].is_string());
        assert_eq!(output_json["request_id"], "req-ser-789");
    }

    // =========================================================================
    // assign_task Tests
    // =========================================================================

    #[test]
    fn test_assign_task_input_deserializes_with_required_fields() {
        let json = r#"{ "task_id": "task_123" }"#;
        let input: AssignTaskInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.task_id, "task_123");
        assert_eq!(input.add_assignees, None);
        assert_eq!(input.remove_assignees, None);
    }

    #[test]
    fn test_assign_task_input_deserializes_with_add_and_remove() {
        let json = r#"{
            "task_id": "task_123",
            "add_assignees": ["user_1", "user_2"],
            "remove_assignees": ["user_3"]
        }"#;
        let input: AssignTaskInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.task_id, "task_123");
        assert_eq!(
            input.add_assignees,
            Some(vec!["user_1".to_string(), "user_2".to_string()])
        );
        assert_eq!(input.remove_assignees, Some(vec!["user_3".to_string()]));
    }

    #[test]
    fn test_assign_task_input_missing_task_id_returns_error() {
        let json = r#"{ "add_assignees": ["user_1"] }"#;
        let err = serde_json::from_str::<AssignTaskInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `task_id`"));
    }

    #[tokio::test]
    #[ignore = "requires system credential - should be tested with wiremock integration test"]
    async fn test_assign_task_adds_assignees_correctly() {
        let ctx = Context::with_metadata("req-assign-123", "", "");
        let input = AssignTaskInput {
            task_id: "task_abc".to_string(),
            add_assignees: Some(vec!["user_1".to_string(), "user_2".to_string()]),
            remove_assignees: None,
        };

        let output = assign_task(ctx, input).await.unwrap();

        assert_eq!(output.task.id, "task_abc");
        assert_eq!(output.assignees.len(), 2);
        assert_eq!(output.assignees[0].id, "user_1");
        assert_eq!(output.assignees[1].id, "user_2");
        assert_eq!(output.request_id, "req-assign-123");
    }

    #[tokio::test]
    #[ignore = "requires system credential - should be tested with wiremock integration test"]
    async fn test_assign_task_with_no_assignees_returns_empty_list() {
        let ctx = Context::empty();
        let input = AssignTaskInput {
            task_id: "task_xyz".to_string(),
            add_assignees: None,
            remove_assignees: Some(vec!["user_1".to_string()]),
        };

        let output = assign_task(ctx, input).await.unwrap();

        assert!(output.assignees.is_empty());
        assert_eq!(output.task.assignees.len(), 0);
    }

    #[tokio::test]
    #[ignore = "requires system credential - should be tested with wiremock integration test"]
    async fn test_assign_task_output_serializes_correctly() {
        let ctx = Context::with_metadata("req-ser-assign", "", "");
        let input = AssignTaskInput {
            task_id: "task_ser".to_string(),
            add_assignees: Some(vec!["user_a".to_string()]),
            remove_assignees: None,
        };

        let output = assign_task(ctx, input).await.unwrap();
        let output_json = serde_json::to_value(&output).unwrap();

        assert_eq!(output_json["task"]["id"], "task_ser");
        assert_eq!(output_json["assignees"].as_array().unwrap().len(), 1);
        assert_eq!(output_json["assignees"][0]["id"], "user_a");
        assert_eq!(output_json["request_id"], "req-ser-assign");
    }

    // =========================================================================
    // Common Type Tests
    // =========================================================================

    #[test]
    fn test_task_deserializes_from_api_response() {
        let json = r##"{
            "id": "task_abc123",
            "name": "Test Task",
            "status": {
                "id": "status_1",
                "status": "open",
                "color": "#87909e"
            },
            "assignees": [
                {
                    "id": "user_1",
                    "username": "john.doe",
                    "email": "john@example.com"
                }
            ],
            "url": "https://app.clickup.com/t/abc123"
        }"##;
        let task: Task = serde_json::from_str(json).unwrap();

        assert_eq!(task.id, "task_abc123");
        assert_eq!(task.name, "Test Task");
        assert_eq!(task.status.as_ref().unwrap().status, "open");
        assert_eq!(task.assignees.len(), 1);
        assert_eq!(task.assignees[0].username, "john.doe");
    }

    #[test]
    fn test_status_serializes_correctly() {
        let status = Status {
            id: "status_123".to_string(),
            status: "in progress".to_string(),
            color: Some("#ffa500".to_string()),
            orderindex: Some(2),
            r#type: Some("custom".to_string()),
        };

        let json = serde_json::to_value(&status).unwrap();

        assert_eq!(json["id"], "status_123");
        assert_eq!(json["status"], "in progress");
        assert_eq!(json["color"], "#ffa500");
        assert_eq!(json["orderindex"], 2);
        assert_eq!(json["type"], "custom");
    }

    #[test]
    fn test_user_with_optional_fields_serializes_correctly() {
        let user = User {
            id: "user_123".to_string(),
            username: "jane.doe".to_string(),
            email: Some("jane@example.com".to_string()),
            profile_picture: None,
        };

        let json = serde_json::to_value(&user).unwrap();

        assert_eq!(json["id"], "user_123");
        assert_eq!(json["username"], "jane.doe");
        assert_eq!(json["email"], "jane@example.com");
        assert_eq!(json["profile_picture"], serde_json::Value::Null);
    }

    #[test]
    fn test_comment_deserializes_from_api_response() {
        let json = r#"{
            "id": "comment_123",
            "comment_text": "This is a test comment",
            "user": {
                "id": "user_1",
                "username": "commenter"
            },
            "date": "1699876543210"
        }"#;
        let comment: Comment = serde_json::from_str(json).unwrap();

        assert_eq!(comment.id, "comment_123");
        assert_eq!(comment.comment_text, "This is a test comment");
        assert!(comment.user.is_some());
        assert_eq!(comment.user.unwrap().username, "commenter");
    }
}
