# Microsoft To Do Integration for Operai Toolbox

Manage Microsoft To Do tasks and lists through the Microsoft Graph API.

## Overview

- Provides access to Microsoft To Do functionality via Microsoft Graph API v1.0
- Supports creating, reading, updating, and completing tasks
- Manages task lists and filters tasks by status
- Handles task metadata including due dates, importance, notes, and categories

### Primary Use Cases

- Task management automation and workflows
- Integration with AI assistants for task creation and tracking
- Synchronizing tasks across different platforms
- Building custom task management interfaces

## Authentication

This integration uses **OAuth2 Bearer Token** authentication. The access token is required to authenticate requests to the Microsoft Graph API.

### Required Credentials

- **`access_token`** (String, required): OAuth2 access token for Microsoft Graph API. Obtain this through the Microsoft identity platform OAuth 2.0 authorization flow.
- **`endpoint`** (String, optional): Custom API endpoint URL. Defaults to `https://graph.microsoft.com/v1.0`.

Credentials are supplied via the `microsoft_todo` credential namespace in the Operai Toolbox configuration.

## Available Tools

### list_lists
**Tool Name:** List To Do Lists
**Capabilities:** read
**Tags:** tasks, todo, microsoft-graph
**Description:** List all task lists in Microsoft To Do using Microsoft Graph

**Input:**
- `limit` (Option<u32>): Maximum number of lists to return (1-100). Defaults to 25.

**Output:**
- `lists` (Vec<TodoTaskList>): Array of task list objects containing:
  - `id` (String): Unique identifier for the list
  - `display_name` (String): Display name of the list
  - `is_owner` (bool): Whether the current user is the owner
  - `is_shared` (bool): Whether the list is shared
  - `wellknown_list_name` (Option<String>): Well-known list name if applicable

### create_task
**Tool Name:** Create To Do Task
**Capabilities:** write
**Tags:** tasks, todo, microsoft-graph
**Description:** Create a new task in a Microsoft To Do list using Microsoft Graph

**Input:**
- `list_id` (String): ID of the task list to create the task in
- `title` (String): Task title
- `body` (Option<String>): Optional task body/notes (plain text)
- `due_date_time` (Option<String>): Optional due date in ISO 8601 format (e.g., "2024-12-31T23:59:59")
- `due_date_timezone` (Option<String>): Optional timezone for due date (e.g., "UTC", "Pacific Standard Time"). Defaults to "UTC"
- `importance` (Option<TaskImportance>): Task importance level (low, normal, high). Defaults to "normal"
- `categories` (Vec<String>): Categories/tags for the task

**Output:**
- `task` (TodoTask): The created task object with all properties including:
  - `id`, `title`, `body`, `status`, `importance`
  - `due_date_time`, `completed_date_time`, `created_date_time`
  - `is_reminder_on`, `reminder_date_time`, `categories`

### complete_task
**Tool Name:** Complete To Do Task
**Capabilities:** write
**Tags:** tasks, todo, microsoft-graph
**Description:** Mark a task as completed in Microsoft To Do using Microsoft Graph

**Input:**
- `list_id` (String): ID of the task list containing the task
- `task_id` (String): ID of the task to complete

**Output:**
- `task` (TodoTask): The updated task object with status set to "completed"

### update_due_date
**Tool Name:** Update Task Due Date
**Capabilities:** write
**Tags:** tasks, todo, microsoft-graph
**Description:** Update the due date of a task in Microsoft To Do using Microsoft Graph

**Input:**
- `list_id` (String): ID of the task list containing the task
- `task_id` (String): ID of the task to update
- `due_date_time` (Option<String>): Due date in ISO 8601 format (e.g., "2024-12-31T23:59:59"). Set to null to remove due date
- `due_date_timezone` (Option<String>): Timezone for due date (e.g., "UTC", "Pacific Standard Time"). Defaults to "UTC"

**Output:**
- `task` (TodoTask): The updated task object with new due date

### add_notes
**Tool Name:** Add Notes to Task
**Capabilities:** write
**Tags:** tasks, todo, microsoft-graph
**Description:** Add or update notes for a task in Microsoft To Do using Microsoft Graph

**Input:**
- `list_id` (String): ID of the task list containing the task
- `task_id` (String): ID of the task to update
- `notes` (String): Notes content to add to the task

**Output:**
- `task` (TodoTask): The updated task object with notes added to the body

### list_tasks
**Tool Name:** List Tasks
**Capabilities:** read
**Tags:** tasks, todo, microsoft-graph
**Description:** List tasks from a Microsoft To Do list using Microsoft Graph

**Input:**
- `list_id` (String): ID of the task list to list tasks from
- `limit` (Option<u32>): Maximum number of tasks to return (1-100). Defaults to 25
- `status` (Option<TaskStatus>): Filter by task status. If not provided, returns all tasks. Values: "notStarted", "inProgress", "completed", "waitingOnOthers", "deferred"

**Output:**
- `tasks` (Vec<TodoTask>): Array of task objects with full task details

## API Documentation

- **Base URL:** `https://graph.microsoft.com/v1.0`
- **API Documentation:** [Microsoft Graph To Do API](https://learn.microsoft.com/en-us/graph/api/resources/todo-overview)

## Testing

Run tests with:
```bash
cargo test -p microsoft-to-do
```

Or from the repository root:
```bash
cargo test --package microsoft-to-do
```

## Development

- **Crate:** `microsoft-to-do`
- **Source:** `examples/project-tasks/microsoft-to-do/`
- **Implementation:** Uses `reqwest` HTTP client with OAuth2 bearer token authentication
- **Error Handling:** Validates input parameters and provides descriptive error messages for API failures
