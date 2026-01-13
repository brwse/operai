# Asana Integration for Operai Toolbox

Provides tools for managing projects and tasks in Asana through the Operai Toolbox.

## Overview

This integration enables Operai Toolbox to interact with Asana's project management platform:

- List and query projects and tasks within Asana workspaces
- Create new tasks with assignments, due dates, and descriptions
- Update task completion status and manage task assignments
- Add comments to tasks for collaboration

**Primary Use Cases:**
- Project visibility and reporting across Asana workspaces
- Automated task creation and workflow management
- Task status tracking and updates
- Team coordination through comments and assignments

## Authentication

Asana uses Personal Access Tokens (PAT) for API authentication. Credentials are managed through the `AsanaCredential` system credential definition.

### Required Credentials

- `access_token` (required): OAuth2 personal access token for Asana API
- `workspace_gid` (optional): Default workspace GID for operations

**Credential Definition:**
```rust
define_system_credential! {
    AsanaCredential("asana") {
        access_token: String,
        #[optional]
        workspace_gid: Option<String>,
    }
}
```

To obtain a Personal Access Token:
1. Navigate to your Asana account settings
2. Go to the "Apps" section and select "Developer Pat"
3. Create a new personal access token
4. Provide the token to the credential system

## Available Tools

### list_projects
**Tool Name:** List Projects
**Description:** Lists all projects in an Asana workspace
**Capabilities:** read
**Tags:** project-tasks,asana,project

**Input:**
- `workspace_gid` (string, required): The workspace GID to list projects from
- `include_archived` (boolean, optional): Whether to include archived projects (default: false)
- `limit` (number, optional): Maximum number of projects to return (default: 100)

**Output:**
- `projects` (array of AsanaProject): The list of projects
  - `gid` (string): The globally unique identifier for the project
  - `name` (string): The name of the project
  - `archived` (boolean): Whether the project is archived
  - `color` (string, optional): The project's color
  - `notes` (string, optional): Notes/description of the project
- `count` (number): Total count of projects returned

### list_tasks
**Tool Name:** List Tasks
**Description:** Lists tasks in an Asana project with optional filters
**Capabilities:** read
**Tags:** project-tasks,asana,task

**Input:**
- `project_gid` (string, required): The project GID to list tasks from
- `completed` (boolean, optional): Filter by completion status
- `assignee_gid` (string, optional): Filter by assignee GID
- `limit` (number, optional): Maximum number of tasks to return (default: 100)

**Output:**
- `tasks` (array of AsanaTask): The list of tasks
  - `gid` (string): The globally unique identifier for the task
  - `name` (string): The name of the task
  - `completed` (boolean): Whether the task is completed
  - `due_on` (string, optional): The due date of the task (YYYY-MM-DD format)
  - `notes` (string, optional): Notes/description of the task
  - `assignee` (AsanaUser, optional): The user assigned to this task
    - `gid` (string): The globally unique identifier for the user
    - `name` (string): The user's name
    - `email` (string, optional): The user's email address
- `count` (number): Total count of tasks returned

### create_task
**Tool Name:** Create Task
**Description:** Creates a new task in an Asana project
**Capabilities:** write
**Tags:** project-tasks,asana,task

**Input:**
- `project_gid` (string, required): The project GID to create the task in
- `name` (string, required): The name of the task
- `notes` (string, optional): Notes/description for the task
- `due_on` (string, optional): Due date in YYYY-MM-DD format
- `assignee_gid` (string, optional): The GID of the user to assign the task to

**Output:**
- `task` (AsanaTask): The created task
  - `gid` (string): The globally unique identifier for the task
  - `name` (string): The name of the task
  - `completed` (boolean): Whether the task is completed
  - `due_on` (string, optional): The due date of the task (YYYY-MM-DD format)
  - `notes` (string, optional): Notes/description of the task
  - `assignee` (AsanaUser, optional): The user assigned to this task
    - `gid` (string): The globally unique identifier for the user
    - `name` (string): The user's name
    - `email` (string, optional): The user's email address
- `success` (boolean): Whether the task was successfully created

### update_status
**Tool Name:** Update Task Status
**Description:** Updates the completion status of an Asana task
**Capabilities:** write
**Tags:** project-tasks,asana,task,status

**Input:**
- `task_gid` (string, required): The task GID to update
- `status` (string, required): The new status for the task ("incomplete" or "complete")

**Output:**
- `task` (AsanaTask): The updated task
  - `gid` (string): The globally unique identifier for the task
  - `name` (string): The name of the task
  - `completed` (boolean): Whether the task is completed
  - `due_on` (string, optional): The due date of the task (YYYY-MM-DD format)
  - `notes` (string, optional): Notes/description of the task
  - `assignee` (AsanaUser, optional): The user assigned to this task
    - `gid` (string): The globally unique identifier for the user
    - `name` (string): The user's name
    - `email` (string, optional): The user's email address
- `success` (boolean): Whether the update was successful
- `previous_status` (boolean): Previous completion status

### comment
**Tool Name:** Add Comment
**Description:** Adds a comment to an Asana task
**Capabilities:** write
**Tags:** project-tasks,asana,task,comment

**Input:**
- `task_gid` (string, required): The task GID to comment on
- `text` (string, required): The comment text

**Output:**
- `comment` (AsanaComment): The created comment
  - `gid` (string): The globally unique identifier for the comment
  - `text` (string): The text content of the comment
  - `created_by` (AsanaUser, optional): The user who created the comment
    - `gid` (string): The globally unique identifier for the user
    - `name` (string): The user's name
    - `email` (string, optional): The user's email address
  - `created_at` (string, optional): When the comment was created (ISO 8601 format)
- `success` (boolean): Whether the comment was successfully added

### assign
**Tool Name:** Assign Task
**Description:** Assigns or unassigns a user to/from an Asana task
**Capabilities:** write
**Tags:** project-tasks,asana,task,assign

**Input:**
- `task_gid` (string, required): The task GID to assign
- `assignee_gid` (string, optional): The user GID to assign the task to. Set to null/none to unassign

**Output:**
- `task` (AsanaTask): The updated task
  - `gid` (string): The globally unique identifier for the task
  - `name` (string): The name of the task
  - `completed` (boolean): Whether the task is completed
  - `due_on` (string, optional): The due date of the task (YYYY-MM-DD format)
  - `notes` (string, optional): Notes/description of the task
  - `assignee` (AsanaUser, optional): The user assigned to this task
    - `gid` (string): The globally unique identifier for the user
    - `name` (string): The user's name
    - `email` (string, optional): The user's email address
- `success` (boolean): Whether the assignment was successful
- `previous_assignee` (AsanaUser, optional): The previous assignee, if any
  - `gid` (string): The globally unique identifier for the user
  - `name` (string): The user's name
  - `email` (string, optional): The user's email address

## API Documentation

- **Base URL:** `https://app.asana.com/api/1.0`
- **API Documentation:** [Official Asana API Reference](https://developers.asana.com/reference/rest-api-reference)
- **Authentication Guide:** [Asana Authentication Documentation](https://developers.asana.com/docs/authentication)

## Testing

Run tests:
```bash
cargo test -p asana
```

The test suite includes:
- Input validation tests for all tools
- Serialization/deserialization roundtrip tests
- Credential validation tests
- Tool execution tests with mock data

## Development

- **Crate:** `asana`
- **Source:** `examples/project-tasks/asana/src/`

### Implementation Notes

This integration makes actual HTTP requests to the Asana API using the reqwest HTTP client. Each tool corresponds to a specific Asana API endpoint:

- `list_projects`: `GET /workspaces/{workspace_gid}/projects`
- `list_tasks`: `GET /projects/{project_gid}/tasks`
- `create_task`: `POST /tasks` with body containing task data
- `update_status`: `PUT /tasks/{task_gid}` with updated completion status
- `comment`: `POST /tasks/{task_gid}/stories` with comment text
- `assign`: `PUT /tasks/{task_gid}` with assignee GID

The implementation includes:
- Proper error handling for network failures and API errors
- Type-safe request/response structures matching Asana's API schema
- Bearer token authentication via Personal Access Tokens
- Support for common query parameters and filters
