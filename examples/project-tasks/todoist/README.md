# Todoist Integration for Operai Toolbox

Provides tools for managing tasks and projects in Todoist through the Operai Toolbox.

## Overview

This integration enables Operai Toolbox to interact with Todoist's task management platform:

- List and query projects within Todoist workspaces
- Create new tasks with descriptions, priorities, due dates, and labels
- Mark tasks as completed and manage task completion status
- Reschedule tasks by updating due dates
- Add notes and comments to tasks for collaboration

**Primary Use Cases:**
- Task management automation and workflow integration
- Project visibility and task tracking across Todoist
- Automated task creation with scheduling and prioritization
- Task status updates and rescheduling
- Team collaboration through notes and comments

## Authentication

Todoist uses OAuth2 Bearer Token authentication for API access. Credentials are managed through the `TodoistCredential` user credential definition.

### Required Credentials

- `access_token` (required): OAuth2 access token for Todoist API
- `endpoint` (optional): Custom API endpoint (defaults to `https://api.todoist.com/rest/v2`)

**Credential Definition:**
```rust
define_user_credential! {
    TodoistCredential("todoist") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}
```

To obtain an OAuth2 access token:
1. Navigate to [Todoist Developer Settings](https://todoist.com/app/settings/integrations/developer)
2. Create a new OAuth client or use a personal access token
3. Provide the token to the credential system

## Available Tools

### list_projects
**Tool Name:** List Todoist Projects
**Description:** List all Todoist projects
**Capabilities:** read
**Tags:** tasks, todoist, projects

**Input:**
- No input parameters required

**Output:**
- `projects` (array of Project): The list of projects
  - `id` (string): The unique identifier for the project
  - `name` (string): The name of the project
  - `color` (string, optional): The project's color
  - `is_favorite` (boolean): Whether the project is marked as favorite
  - `view_style` (string, optional): The project's view style
  - `url` (string, optional): The project's URL

### create_task
**Tool Name:** Create Todoist Task
**Description:** Create a new task in Todoist
**Capabilities:** write
**Tags:** tasks, todoist

**Input:**
- `content` (string, required): Task content (title)
- `description` (string, optional): Task description
- `project_id` (string, optional): Project ID to add the task to
- `section_id` (string, optional): Section ID
- `labels` (array of string, optional): Labels to assign to the task
- `priority` (number, optional): Priority level 1-4 where 4 is highest (defaults to 1)
- `due_string` (string, optional): Human-readable due date (e.g., "tomorrow at 12:00", "next Monday")
- `due_date` (string, optional): Due date in YYYY-MM-DD format
- `due_datetime` (string, optional): Due datetime in RFC3339 format

**Output:**
- `task` (Task): The created task
  - `id` (string): The unique identifier for the task
  - `content` (string): The task title/content
  - `description` (string, optional): Task description
  - `project_id` (string, optional): Project ID
  - `section_id` (string, optional): Section ID
  - `labels` (array of string): Labels assigned to the task
  - `priority` (number, optional): Priority level
  - `due` (TaskDue, optional): Due date information
    - `date` (string, optional): Due date in YYYY-MM-DD format
    - `datetime` (string, optional): Due datetime in RFC3339 format
    - `string` (string, optional): Human-readable due date
    - `timezone` (string, optional): Timezone for the due date
    - `is_recurring` (boolean): Whether the due date is recurring
  - `is_completed` (boolean): Whether the task is completed
  - `created_at` (string, optional): When the task was created (ISO 8601 format)
  - `url` (string, optional): The task's URL

### complete_task
**Tool Name:** Complete Todoist Task
**Description:** Mark a Todoist task as completed
**Capabilities:** write
**Tags:** tasks, todoist

**Input:**
- `task_id` (string, required): Task ID to mark as completed

**Output:**
- `completed` (boolean): Whether the task was successfully marked as completed

### reschedule_task
**Tool Name:** Reschedule Todoist Task
**Description:** Reschedule a Todoist task by updating its due date
**Capabilities:** write
**Tags:** tasks, todoist

**Input:**
- `task_id` (string, required): Task ID to reschedule
- `due_string` (string, optional): Human-readable due date (e.g., "tomorrow at 12:00")
- `due_date` (string, optional): Due date in YYYY-MM-DD format
- `due_datetime` (string, optional): Due datetime in RFC3339 format

At least one of `due_string`, `due_date`, or `due_datetime` must be provided.

**Output:**
- `task` (Task): The updated task (see `create_task` output for full structure)

### add_note
**Tool Name:** Add Note to Todoist Task
**Description:** Add a comment/note to a Todoist task
**Capabilities:** write
**Tags:** tasks, todoist, comments

**Input:**
- `task_id` (string, required): Task ID to add the note to
- `content` (string, required): Note content

**Output:**
- `comment` (Comment): The created comment
  - `id` (string): The unique identifier for the comment
  - `content` (string): The comment text content
  - `task_id` (string, optional): The task ID this comment is attached to
  - `project_id` (string, optional): The project ID this comment is attached to
  - `posted_at` (string, optional): When the comment was posted (ISO 8601 format)

## API Documentation

- **Base URL:** `https://api.todoist.com/rest/v2`
- **API Documentation:** [Official Todoist REST API v2 Reference](https://developer.todoist.com/rest/v2/)
- **Authentication Guide:** [Todoist OAuth2 Documentation](https://developer.todoist.com/guides/#oauth)

## Testing

Run tests:
```bash
cargo test -p todoist
```

The test suite includes:
- Input validation tests for all tools
- URL normalization tests
- HTTP request/response tests with wiremock mocking
- Error handling tests for API failures

## Development

- **Crate:** `todoist`
- **Source:** `examples/project-tasks/todoist/src/`
