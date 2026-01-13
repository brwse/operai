# ClickUp Integration for Operai Toolbox

Manage tasks, comments, and assignments in ClickUp through Operai Toolbox.

## Overview

This integration provides tools for interacting with ClickUp, a project management and productivity platform:
- **List tasks** from any ClickUp list with filtering by status, assignee, dates, and more
- **Create tasks** with descriptions, priorities, assignees, due dates, and time estimates
- **Update task status** to move tasks through workflows (e.g., "open" → "in progress" → "complete")
- **Add comments** to tasks for collaboration and feedback
- **Assign or unassign users** from tasks to manage ownership

## Authentication

This integration uses system credentials with API token authentication. Credentials are supplied via environment variables or configuration.

### Required Credentials

- `api_token`: ClickUp API token for authentication (starts with `pk_`)
- `endpoint` (optional): Custom API endpoint (defaults to `https://api.clickup.com/api/v2`)

#### Manifest Configuration

Add the following to your `operai.toml` manifest:

```toml
[[tools]]
package = "brwse-clickup"
[tools.credentials.clickup]
api_token = "pk_your_api_token_here"
# endpoint = "https://api.clickup.com/api/v2"  # Optional
```

#### Getting Your API Token

1. Log in to ClickUp
2. Click your avatar in the upper-right corner
3. Select **Settings**
4. Click **Apps** in the sidebar
5. Under **API Token**, click **Generate** or **Regenerate**
6. Copy the token (starts with `pk_`)

## Available Tools

### list_tasks
**Tool Name:** List Tasks
**Capabilities:** read
**Tags:** tasks, list, filter
**Description:** Lists tasks from a ClickUp list with optional filtering by status, assignees, and more

**Input:**
- `list_id` (string, required): The list ID to fetch tasks from
- `archived` (boolean, optional): Filter by archived status
- `statuses` (array of string, optional): Filter by specific status names
- `assignees` (array of string, optional): Filter by assignee IDs
- `include_subtasks` (boolean, optional): Include subtasks in the response
- `include_closed` (boolean, optional): Include closed tasks in the response
- `page` (number, optional): Page number for pagination (0-indexed)
- `order_by` (string, optional): Order by field (e.g., "created", "updated", "due_date")
- `reverse` (boolean, optional): Reverse the order (descending if true)

**Output:**
- `tasks` (array of Task): The list of tasks
- `request_id` (string): The request ID that processed this request

### create_task
**Tool Name:** Create Task
**Capabilities:** write
**Tags:** tasks, create
**Description:** Creates a new task in a ClickUp list with optional description, priority, assignees, and due date

**Input:**
- `list_id` (string, required): The list ID where the task will be created
- `name` (string, required): The name/title of the task
- `description` (string, optional): The task description (supports markdown)
- `priority` (number, optional): Priority level (1 = urgent, 2 = high, 3 = normal, 4 = low)
- `assignees` (array of string, optional): User IDs to assign to this task
- `tags` (array of string, optional): Tags to add to the task
- `status` (string, optional): The status name to set for this task
- `due_date` (number, optional): Due date as Unix timestamp in milliseconds
- `start_date` (number, optional): Start date as Unix timestamp in milliseconds
- `time_estimate` (number, optional): Time estimate in milliseconds
- `notify_all` (boolean, optional): Whether to notify assignees about the new task
- `parent` (string, optional): Parent task ID to create this as a subtask

**Output:**
- `task` (Task): The created task
- `request_id` (string): The request ID that processed this request

### update_status
**Tool Name:** Update Task Status
**Capabilities:** write
**Tags:** tasks, status, update
**Description:** Updates the status of a ClickUp task (e.g., 'open' → 'in progress' → 'complete')

**Input:**
- `task_id` (string, required): The task ID to update
- `status` (string, required): The new status name (e.g., "open", "in progress", "complete")

**Output:**
- `task` (Task): The updated task
- `previous_status` (string): The previous status before the update
- `request_id` (string): The request ID that processed this request

### add_comment
**Tool Name:** Add Comment
**Capabilities:** write
**Tags:** comments, collaboration
**Description:** Adds a comment to a ClickUp task with optional user notifications

**Input:**
- `task_id` (string, required): The task ID to add a comment to
- `comment_text` (string, required): The comment text content (supports markdown)
- `notify_all` (boolean, optional): User ID to notify about this comment
- `assignee` (string, optional): Specific user IDs to assign/notify

**Output:**
- `comment` (Comment): The created comment
- `request_id` (string): The request ID that processed this request

### assign_task
**Tool Name:** Assign Task
**Capabilities:** write
**Tags:** tasks, assign, users
**Description:** Assigns or unassigns users from a ClickUp task

**Input:**
- `task_id` (string, required): The task ID to update assignments for
- `add_assignees` (array of string, optional): User IDs to add as assignees
- `remove_assignees` (array of string, optional): User IDs to remove from assignees

**Output:**
- `task` (Task): The updated task with new assignees
- `assignees` (array of User): The list of current assignees after the update
- `request_id` (string): The request ID that processed this request

## Data Types

### Task
- `id` (string): The task's unique identifier
- `custom_id` (string, optional): Custom task ID if set
- `name` (string): The task name/title
- `description` (string, optional): The task description in markdown or plain text
- `status` (Status): The current status of the task
- `priority` (Priority, optional): The task priority
- `assignees` (array of User): Users assigned to this task
- `creator` (User, optional): The user who created the task
- `due_date` (string, optional): Due date as Unix timestamp in milliseconds
- `start_date` (string, optional): Start date as Unix timestamp in milliseconds
- `date_created` (string, optional): Date created as Unix timestamp in milliseconds
- `date_updated` (string, optional): Date updated as Unix timestamp in milliseconds
- `list_id` (string, optional): The list ID this task belongs to
- `folder_id` (string, optional): The folder ID this task belongs to
- `space_id` (string, optional): The space ID this task belongs to
- `url` (string, optional): URL to view this task in ClickUp

### Status
- `id` (string): The status identifier
- `status` (string): The status name (e.g., "open", "in progress", "complete")
- `color` (string, optional): The status color in hex format
- `orderindex` (number, optional): The order of this status in the workflow
- `type` (string, optional): The type of status (open, custom, closed)

### Priority
- `priority` (number, optional): Priority level (1 = urgent, 2 = high, 3 = normal, 4 = low)
- `color` (string, optional): Priority color in hex format

### User
- `id` (string): The user's unique identifier
- `username` (string): The user's username
- `email` (string, optional): The user's email address
- `profile_picture` (string, optional): URL to the user's profile picture

### Comment
- `id` (string): The comment's unique identifier
- `comment_text` (string): The comment text content
- `user` (User, optional): The user who posted the comment
- `date` (string, optional): Date the comment was created as Unix timestamp

## Priority Levels

ClickUp uses numeric priority levels:

- `1` = Urgent (red: `#f50000`)
- `2` = High (yellow: `#ffcc00`)
- `3` = Normal (blue: `#6fddff`)
- `4` = Low (gray: `#d8d8d8`)

## API Documentation

- Base URL: `https://api.clickup.com/api/v2`
- API Documentation: [ClickUp API Documentation](https://developer.clickup.com/)
- Tasks API Reference: [Tasks API](https://developer.clickup.com/docs/tasks)
- Comments API Reference: [Comments API](https://developer.clickup.com/docs/comments)

## Testing

Run tests:
```bash
cargo test -p brwse-clickup
```

## Development

- Crate: `brwse-clickup`
- Source: `examples/project-tasks/clickup/src/lib.rs`
