# GitLab Integration for Operai Toolbox

Operai Toolbox integration for GitLab source control and code review workflows.

## Overview

This integration enables AI agents and tools to interact with GitLab for source control operations, including:

- **Merge Request Management**: Search, create, approve, merge, and close merge requests
- **Issue Tracking**: Search, create, and close issues in GitLab projects
- **Code Review Collaboration**: Add comments to issues and merge requests
- **Project Operations**: Support for both gitlab.com and self-hosted GitLab instances

## Authentication

This integration uses **GitLab Personal Access Tokens** for authentication. Credentials are supplied via user credentials in the toolbox runtime.

### Required Credentials

- `access_token`: GitLab Personal Access Token (required). Generate a token at [User Settings → Access Tokens](https://gitlab.com/-/user_settings/personal_access_tokens) with `api` scope for full access or `read_api` for read-only operations.
- `endpoint`: Custom API endpoint URL (optional, defaults to `https://gitlab.com/api/v4`). Use this for self-hosted GitLab instances.

**Example credential configuration:**
```json
{
  "access_token": "glpat-xxxxxxxxxxxxxxxxxxxx",
  "endpoint": "https://gitlab.example.com/api/v4"
}
```

## Available Tools

### search_merge_requests
**Tool Name:** Search GitLab Merge Requests
**Capabilities:** read
**Tags:** source-control, gitlab, merge-request
**Description:** Search merge requests in a GitLab project

**Input:**
- `project` (string): Project ID or namespace/project-name (e.g., "myorg/myproject")
- `search` (string, optional): Search query string (searches in title and description)
- `state` (string, optional): Filter by state: "opened", "closed", "locked", or "merged"
- `limit` (number, optional): Maximum number of results (1-100, defaults to 20)

**Output:**
- `merge_requests` (array): List of merge request summaries including id, iid, title, state, author, branches, and timestamps

### search_issues
**Tool Name:** Search GitLab Issues
**Capabilities:** read
**Tags:** source-control, gitlab, issue
**Description:** Search issues in a GitLab project

**Input:**
- `project` (string): Project ID or namespace/project-name
- `search` (string, optional): Search query string (searches in title and description)
- `state` (string, optional): Filter by state: "opened" or "closed"
- `limit` (number, optional): Maximum number of results (1-100, defaults to 20)

**Output:**
- `issues` (array): List of issue summaries including id, iid, title, state, author, assignees, labels, and timestamps

### create_issue
**Tool Name:** Create GitLab Issue
**Capabilities:** write
**Tags:** source-control, gitlab, issue
**Description:** Create a new issue in a GitLab project

**Input:**
- `project` (string): Project ID or namespace/project-name
- `title` (string): Issue title
- `description` (string, optional): Issue description/body
- `assignee_ids` (array of numbers, optional): Assignee user IDs
- `labels` (string, optional): Comma-separated label names (e.g., "bug,priority::high")

**Output:**
- `issue` (object): Created issue summary with all issue details

### comment
**Tool Name:** Comment on GitLab Issue or MR
**Capabilities:** write
**Tags:** source-control, gitlab, comment
**Description:** Add a comment to a GitLab issue or merge request

**Input:**
- `project` (string): Project ID or namespace/project-name
- `iid` (number): Issue or MR IID (internal ID, not the global ID)
- `resource_type` (string): Type of resource: "issue" or "merge_request"
- `body` (string): Comment body/text

**Output:**
- `note` (object): Created comment note with id, body, author, and timestamps

### open_merge_request
**Tool Name:** Open GitLab Merge Request
**Capabilities:** write
**Tags:** source-control, gitlab, merge-request
**Description:** Create a new merge request in a GitLab project

**Input:**
- `project` (string): Project ID or namespace/project-name
- `source_branch` (string): Source branch name
- `target_branch` (string): Target branch name
- `title` (string): MR title
- `description` (string, optional): MR description/body
- `assignee_ids` (array of numbers, optional): Assignee user IDs

**Output:**
- `merge_request` (object): Created merge request summary with all MR details

### approve_merge_request
**Tool Name:** Approve GitLab Merge Request
**Capabilities:** write
**Tags:** source-control, gitlab, merge-request
**Description:** Approve a merge request in GitLab

**Input:**
- `project` (string): Project ID or namespace/project-name
- `iid` (number): MR IID (internal ID)

**Output:**
- `approved` (boolean): True if the merge request was approved successfully

### merge_request
**Tool Name:** Merge GitLab Merge Request
**Capabilities:** write
**Tags:** source-control, gitlab, merge-request
**Description:** Merge a merge request in GitLab

**Input:**
- `project` (string): Project ID or namespace/project-name
- `iid` (number): MR IID (internal ID)

**Output:**
- `merged` (boolean): True if the merge request was merged successfully

### close_merge_request
**Tool Name:** Close GitLab Merge Request
**Capabilities:** write
**Tags:** source-control, gitlab, merge-request
**Description:** Close a merge request in GitLab without merging

**Input:**
- `project` (string): Project ID or namespace/project-name
- `iid` (number): MR IID (internal ID)

**Output:**
- `closed` (boolean): True if the merge request was closed successfully

### close_issue
**Tool Name:** Close GitLab Issue
**Capabilities:** write
**Tags:** source-control, gitlab, issue
**Description:** Close an issue in GitLab

**Input:**
- `project` (string): Project ID or namespace/project-name
- `iid` (number): Issue IID (internal ID)

**Output:**
- `closed` (boolean): True if the issue was closed successfully

## API Documentation

- **Base URL:** `https://gitlab.com/api/v4` (default)
- **API Documentation:** [GitLab REST API v4](https://docs.gitlab.com/api/rest/)

## Testing

Run tests:
```bash
cargo test -p gitlab
```

The test suite uses wiremock to mock GitLab API responses and verifies:
- Input validation for all tools
- API request formatting and authentication
- Response parsing and serialization
- Error handling for various failure scenarios

## Development

- **Crate:** `gitlab`
- **Source:** `examples/source-control/gitlab/`
