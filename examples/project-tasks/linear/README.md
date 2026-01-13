# Linear Integration for Operai Toolbox

A project and issue management integration for Linear through Operai Toolbox.

## Overview

- Provides tools for searching, creating, and managing Linear issues
- Supports team cycles (sprints), comments, and state transitions
- Enables full project workflow integration with Linear's GraphQL API

Primary use cases include:
- Managing software development projects and issues
- Tracking work across teams and cycles
- Automating issue creation and status updates

## Authentication

Linear uses **OAuth2 Bearer Token** authentication. Credentials are supplied via user credentials in the Operai Toolbox configuration.

### Required Credentials

- `access_token` (required): OAuth2 access token for Linear API
- `endpoint` (optional): Custom API endpoint URL (defaults to `https://api.linear.app/graphql`)

### Obtaining Credentials

1. Go to Linear Settings → API
2. Create a new Personal API Key or set up OAuth2
3. Copy the access token
4. Configure the token in your Operai Toolbox credentials

## Available Tools

### search_issues
**Tool Name:** Search Linear Issues
**Capabilities:** read
**Tags:** project-management, linear, issues
**Description:** Search for issues in Linear by query, state, assignee, priority, or team

**Input:**
- `query: String` - Search query string to match against issue titles (required)
- `team_id: Option<String>` - Filter by team ID
- `state: Option<String>` - Filter by state name (e.g., "In Progress", "Done")
- `assignee_id: Option<String>` - Filter by assignee user ID
- `priority: Option<u8>` - Filter by priority level (0-4)
- `limit: Option<u32>` - Maximum number of results to return (default: 50, max: 100)

**Output:**
- `issues: Vec<Issue>` - List of matching issues
- `total_count: u32` - Total number of issues returned
- `has_more: bool` - Whether more results are available

**Example:**
```json
{
  "query": "authentication bug",
  "team_id": "team-eng-123",
  "state": "In Progress",
  "priority": 1,
  "limit": 25
}
```

### create_issue
**Tool Name:** Create Linear Issue
**Capabilities:** write
**Tags:** project-management, linear, issues
**Description:** Create a new issue in Linear with title, description, priority, and other properties

**Input:**
- `title: String` - Issue title (required)
- `team_id: String` - Team ID to create the issue in (required)
- `description: Option<String>` - Issue description
- `priority: Option<u8>` - Priority level (0-4)
- `assignee_id: Option<String>` - User ID to assign the issue to
- `state_id: Option<String>` - Initial state ID
- `label_ids: Option<Vec<String>>` - List of label IDs to apply
- `cycle_id: Option<String>` - Cycle ID to add the issue to
- `estimate: Option<f32>` - Issue estimate value

**Output:**
- `issue: Issue` - The created issue
- `success: bool` - Whether the creation was successful

**Example:**
```json
{
  "title": "Implement OAuth 2.0",
  "team_id": "team-eng-123",
  "description": "Add OAuth 2.0 authentication support",
  "priority": 2,
  "assignee_id": "user-456",
  "estimate": 5.0
}
```

### update_state
**Tool Name:** Update Issue State
**Capabilities:** write
**Tags:** project-management, linear, issues
**Description:** Update the state/status of an existing Linear issue

**Input:**
- `issue_id: String` - ID of the issue to update (required)
- `state_id: String` - New state ID to apply (required)

**Output:**
- `issue: Issue` - The updated issue
- `success: bool` - Whether the update was successful

**Example:**
```json
{
  "issue_id": "issue-abc-123",
  "state_id": "state-done-456"
}
```

### add_comment
**Tool Name:** Add Comment to Issue
**Capabilities:** write
**Tags:** project-management, linear, issues
**Description:** Add a comment to an existing Linear issue

**Input:**
- `issue_id: String` - ID of the issue to comment on (required)
- `body: String` - Comment text (required)

**Output:**
- `comment: Comment` - The created comment
- `issue_id: String` - ID of the issue
- `success: bool` - Whether the comment was created successfully

**Example:**
```json
{
  "issue_id": "issue-abc-123",
  "body": "Working on this now. Should be done by EOD."
}
```

### list_cycles
**Tool Name:** List Team Cycles
**Capabilities:** read
**Tags:** project-management, linear, cycles, sprints
**Description:** List cycles (sprints) for a Linear team

**Input:**
- `team_id: String` - Team ID to list cycles for (required)
- `limit: Option<u32>` - Maximum number of cycles to return (default: 10, max: 50)

**Output:**
- `cycles: Vec<Cycle>` - List of cycles
- `team: Team` - Team information
- `total_count: u32` - Total number of cycles returned

**Example:**
```json
{
  "team_id": "team-eng-123",
  "limit": 10
}
```

## Data Structures

**Issue:**
- `id: String` - Unique issue identifier
- `identifier: String` - Human-readable issue identifier (e.g., "ENG-123")
- `title: String` - Issue title
- `description: Option<String>` - Issue description
- `state: IssueState` - Current state information
- `priority: u8` - Priority level
- `assignee: Option<User>` - Assigned user
- `team: Team` - Team information
- `labels: Vec<Label>` - Associated labels
- `created_at: String` - Creation timestamp
- `updated_at: String` - Last update timestamp

**IssueState:**
- `id: String` - State identifier
- `name: String` - State name
- `state_type: String` - State type (e.g., "backlog", "started", "completed")
- `color: String` - Display color

**User:**
- `id: String` - User identifier
- `name: String` - User name
- `email: String` - User email

**Team:**
- `id: String` - Team identifier
- `name: String` - Team name
- `key: String` - Team key (used in identifiers)

**Label:**
- `id: String` - Label identifier
- `name: String` - Label name
- `color: String` - Display color

**Comment:**
- `id: String` - Comment identifier
- `body: String` - Comment text
- `user: User` - Author information
- `created_at: String` - Creation timestamp
- `updated_at: String` - Last update timestamp
- `resolves_parent: bool` - Whether comment resolves the issue

**Cycle:**
- `id: String` - Cycle identifier
- `number: u32` - Cycle number
- `name: Option<String>` - Cycle name
- `description: Option<String>` - Cycle description
- `starts_at: String` - Start timestamp
- `ends_at: String` - End timestamp
- `issue_count: u32` - Total issues in cycle
- `completed_issue_count: u32` - Completed issues
- `scope: f32` - Total scope (estimate)
- `completed_scope: f32` - Completed scope
- `progress: f32` - Progress percentage (0-100)

## API Documentation

- **Base URL:** `https://api.linear.app/graphql`
- **API Documentation:** [Linear API Reference](https://developers.linear.app/docs/graphql/working-with-the-graphql-api)
- **Authentication:** [Linear Authentication Guide](https://developers.linear.app/docs/graphql/authentication)
- **GraphQL Schema:** [Linear GraphQL Schema](https://studio.apollographql.com/public/Linear-API/schema/reference)

## Testing

Run tests:
```bash
cargo test -p brwse-linear
```

The tests use wiremock to mock HTTP responses from the Linear API.

## Development

- **Crate:** `brwse-linear`
- **Source:** `examples/project-tasks/linear/`
