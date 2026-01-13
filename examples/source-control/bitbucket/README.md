# Bitbucket Integration for Operai Toolbox

Manage Bitbucket pull requests, comments, and reviews with the Operai Toolbox.

## Overview

- Search, create, and manage Bitbucket pull requests
- Add comments to pull requests for collaboration
- Approve, merge, or decline pull requests
- View pull request details and participants

Primary use cases:
- Automating pull request workflows in Bitbucket repositories
- Enabling AI agents to participate in code review processes
- Integrating Bitbucket with development pipelines and tooling

## Authentication

This integration uses **HTTP Basic Authentication** with a Bitbucket username and password (or app password). Credentials are supplied via system credentials.

### Required Credentials

- `username`: Bitbucket username or workspace ID
- `password`: Bitbucket password or app password (recommended for API access)
- `endpoint` (optional): Custom API endpoint (defaults to `https://api.bitbucket.org/2.0`)

To use app passwords (recommended):
1. Go to Bitbucket Settings → App passwords
2. Create a new app password with the following permissions:
   - **Repositories**: Read, Write
   - **Pull requests**: Read, Write
3. Use the app password as the `password` credential

## Available Tools

### search_pull_requests
**Tool Name:** Search Bitbucket Pull Requests
**Capabilities:** read
**Tags:** source-control, bitbucket, pull-request
**Description:** Search pull requests in a Bitbucket repository

**Input:**
- `workspace` (string): Workspace ID (username or team name)
- `repo_slug` (string): Repository slug
- `state` (PullRequestState, optional): Filter by state (e.g., "OPEN", "MERGED", "DECLINED")
- `limit` (number, optional): Maximum number of results (1-100). Defaults to 10

**Output:**
- `pull_requests` (array of PullRequestSummary): List of pull request summaries containing:
  - `id` (number): Pull request ID
  - `title` (string): Pull request title
  - `description` (string, optional): Pull request description
  - `state` (PullRequestState): State (OPEN, MERGED, DECLINED, SUPERSEDED)
  - `author` (User): Author display name and UUID
  - `source` (BranchInfo): Source branch and repository
  - `destination` (BranchInfo): Destination branch and repository
  - `created_on` (string): ISO 8601 timestamp
  - `updated_on` (string): ISO 8601 timestamp
  - `comment_count` (number, optional): Number of comments
  - `task_count` (number, optional): Number of tasks

### get_pull_request
**Tool Name:** Get Bitbucket Pull Request
**Capabilities:** read
**Tags:** source-control, bitbucket, pull-request
**Description:** Get details of a specific Bitbucket pull request

**Input:**
- `workspace` (string): Workspace ID (username or team name)
- `repo_slug` (string): Repository slug
- `pull_request_id` (number): Pull request ID

**Output:**
- `pull_request` (PullRequestSummary): Pull request summary (see above)
- `participants` (array of Participant): List of participants containing:
  - `user` (User): User display name and UUID
  - `role` (ParticipantRole): Role (REVIEWER or PARTICIPANT)
  - `approved` (boolean): Whether the user approved
  - `participated_on` (string, optional): ISO 8601 timestamp

### create_pull_request
**Tool Name:** Create Bitbucket Pull Request
**Capabilities:** write
**Tags:** source-control, bitbucket, pull-request
**Description:** Open a new pull request in a Bitbucket repository

**Input:**
- `workspace` (string): Workspace ID (username or team name)
- `repo_slug` (string): Repository slug
- `title` (string): Pull request title
- `description` (string, optional): Pull request description
- `source_branch` (string): Source branch name
- `destination_branch` (string): Destination branch name
- `reviewers` (array of string, optional): List of reviewer UUIDs
- `close_source_branch` (boolean, optional): Close source branch after merge. Defaults to false

**Output:**
- `pull_request_id` (number): ID of the created pull request
- `title` (string): Pull request title
- `state` (PullRequestState): Initial state (typically OPEN)

### add_comment
**Tool Name:** Add Comment to Bitbucket Pull Request
**Capabilities:** write
**Tags:** source-control, bitbucket, pull-request, comment
**Description:** Add a comment to a Bitbucket pull request

**Input:**
- `workspace` (string): Workspace ID (username or team name)
- `repo_slug` (string): Repository slug
- `pull_request_id` (number): Pull request ID
- `comment` (string): Comment text (markdown supported)

**Output:**
- `comment_id` (number): ID of the created comment
- `created_on` (string): ISO 8601 timestamp of comment creation

### approve_pull_request
**Tool Name:** Approve Bitbucket Pull Request
**Capabilities:** write
**Tags:** source-control, bitbucket, pull-request, review
**Description:** Approve a Bitbucket pull request

**Input:**
- `workspace` (string): Workspace ID (username or team name)
- `repo_slug` (string): Repository slug
- `pull_request_id` (number): Pull request ID

**Output:**
- `approved` (boolean): Confirmation that the pull request was approved

### unapprove_pull_request
**Tool Name:** Unapprove Bitbucket Pull Request
**Capabilities:** write
**Tags:** source-control, bitbucket, pull-request, review
**Description:** Remove approval from a Bitbucket pull request

**Input:**
- `workspace` (string): Workspace ID (username or team name)
- `repo_slug` (string): Repository slug
- `pull_request_id` (number): Pull request ID

**Output:**
- `unapproved` (boolean): Confirmation that approval was removed

### merge_pull_request
**Tool Name:** Merge Bitbucket Pull Request
**Capabilities:** write
**Tags:** source-control, bitbucket, pull-request, merge
**Description:** Merge a Bitbucket pull request

**Input:**
- `workspace` (string): Workspace ID (username or team name)
- `repo_slug` (string): Repository slug
- `pull_request_id` (number): Pull request ID
- `message` (string, optional): Merge commit message
- `close_source_branch` (boolean, optional): Close source branch after merge
- `merge_strategy` (string, optional): Merge strategy (e.g., "merge_commit", "squash", "fast_forward")

**Output:**
- `merged` (boolean): Confirmation that the pull request was merged
- `pull_request_id` (number): ID of the merged pull request

### decline_pull_request
**Tool Name:** Decline Bitbucket Pull Request
**Capabilities:** write
**Tags:** source-control, bitbucket, pull-request
**Description:** Decline (close without merging) a Bitbucket pull request

**Input:**
- `workspace` (string): Workspace ID (username or team name)
- `repo_slug` (string): Repository slug
- `pull_request_id` (number): Pull request ID

**Output:**
- `declined` (boolean): Confirmation that the pull request was declined
- `pull_request_id` (number): ID of the declined pull request

## Example Usage

### Search for open pull requests
```json
{
  "workspace": "my-team",
  "repo_slug": "my-repo",
  "state": "OPEN",
  "limit": 20
}
```

### Create a pull request
```json
{
  "workspace": "my-team",
  "repo_slug": "my-repo",
  "title": "Add new feature",
  "description": "This PR implements feature X",
  "source_branch": "feature/x",
  "destination_branch": "main",
  "reviewers": ["{reviewer-uuid-1}", "{reviewer-uuid-2}"],
  "close_source_branch": true
}
```

### Add a comment
```json
{
  "workspace": "my-team",
  "repo_slug": "my-repo",
  "pull_request_id": 123,
  "comment": "LGTM! Great work on this feature."
}
```

### Approve a pull request
```json
{
  "workspace": "my-team",
  "repo_slug": "my-repo",
  "pull_request_id": 123
}
```

### Merge a pull request
```json
{
  "workspace": "my-team",
  "repo_slug": "my-repo",
  "pull_request_id": 123,
  "message": "Merge feature X into main",
  "close_source_branch": true,
  "merge_strategy": "merge_commit"
}
```

## API Documentation

- Base URL: `https://api.bitbucket.org/2.0`
- API Documentation: [Bitbucket REST API v2.0 Reference](https://developer.atlassian.com/cloud/bitbucket/rest/intro/)

## Testing

Run tests:
```bash
cargo test -p brwse-bitbucket
```

All tests use `wiremock` for HTTP mocking and don't require actual Bitbucket credentials.

## Development

- Crate: `brwse-bitbucket`
- Source: `examples/source-control/bitbucket`
