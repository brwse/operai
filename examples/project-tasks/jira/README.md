# Jira Integration for Operai Toolbox

This integration provides tools for interacting with Jira Cloud through the Operai Toolbox.

## Overview

The Jira integration enables Operai Toolbox to interact with Jira Cloud instances using the Jira REST API v3. It provides tools for:

- **Search and retrieve issues** using JQL (Jira Query Language) or by issue key
- **Create new issues** in Jira projects with full configuration support
- **Manage issue workflows** by transitioning issues between statuses
- **Add comments** to existing issues for team collaboration

**Primary use cases:**
- Automated issue creation and management
- Integration with AI assistants for issue tracking
- Custom workflows and automation scripts
- Issue search and reporting

## Authentication

Jira uses **Basic Authentication** with an email address and API token. Credentials are supplied via system credentials configured in the Operai Toolbox runtime.

### Required Credentials

The following credentials are required for authentication:

- **`username`**: Email address associated with the Jira account (required)
- **`password`**: Jira API token generated from [https://id.atlassian.com/manage/api-tokens](https://id.atlassian.com/manage/api-tokens) (required)
- **`endpoint`**: Jira instance base URL (optional, defaults to `https://api.atlassian.com`)

**Example endpoint format:** `https://yourcompany.atlassian.net`

**Note:** For Jira Cloud instances, use your Atlassian account email. Generate an API token at [https://id.atlassian.com/manage/api-tokens](https://id.atlassian.com/manage/api-tokens).

## Available Tools

### search_issues
**Tool Name:** Search Jira Issues
**Capabilities:** read
**Tags:** jira, issues, search
**Description:** Search for Jira issues using JQL (Jira Query Language)

**Input:**
- `jql` (string): JQL query string (e.g., "project = PROJ AND status = 'In Progress'")
- `max_results` (optional u32): Maximum number of results (1-100). Defaults to 50

**Output:**
- `issues` (array of IssueSummary): List of matching issues with summary fields
- `total` (u32): Total number of issues matching the query

### get_issue
**Tool Name:** Get Jira Issue
**Capabilities:** read
**Tags:** jira, issues
**Description:** Get detailed information about a specific Jira issue

**Input:**
- `issue_key` (string): The issue key (e.g., "PROJ-123")

**Output:**
- `issue` (Issue): Full issue details including description, comments, labels, and metadata

### create_issue
**Tool Name:** Create Jira Issue
**Capabilities:** write
**Tags:** jira, issues
**Description:** Create a new issue in a Jira project

**Input:**
- `project_key` (string): Project key (e.g., "PROJ")
- `summary` (string): Issue summary/title
- `issue_type` (string): Issue type name (e.g., "Task", "Bug", "Story")
- `description` (optional string): Description text (plain text)
- `priority` (optional string): Priority name (e.g., "High", "Medium", "Low")
- `assignee_account_id` (optional string): Assignee account ID
- `labels` (array of string): Labels to attach to the issue

**Output:**
- `id` (string): The ID of the created issue
- `key` (string): The key of the created issue (e.g., "PROJ-124")

### transition_issue
**Tool Name:** Transition Jira Issue
**Capabilities:** write
**Tags:** jira, issues, workflow
**Description:** Change the status of a Jira issue by executing a workflow transition

**Input:**
- `issue_key` (string): Issue key (e.g., "PROJ-123")
- `transition_id` (string): Transition ID (use get_transitions to find available transitions)

**Output:**
- `success` (boolean): Indicates whether the transition was successful

### add_comment
**Tool Name:** Add Comment to Jira Issue
**Capabilities:** write
**Tags:** jira, issues, comments
**Description:** Add a comment to an existing Jira issue

**Input:**
- `issue_key` (string): Issue key (e.g., "PROJ-123")
- `body` (string): Comment body text

**Output:**
- `comment_id` (string): The ID of the created comment

## API Documentation

- **Base URL:** `https://api.atlassian.com` (configurable via `endpoint` credential)
- **API Version:** REST API v3
- **API Documentation:** [Jira Cloud REST API](https://developer.atlassian.com/cloud/jira/platform/rest/v3/)

## Testing

Run tests with:

```bash
cargo test -p jira
```

The test suite includes:
- Unit tests for URL normalization
- Input validation tests for all tools
- Integration tests using wiremock for HTTP mocking

## Development

- **Crate:** `jira`
- **Source:** `examples/project-tasks/jira/`
- **Type Library:** Dynamic library (`cdylib`)
