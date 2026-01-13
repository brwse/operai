# GitHub Integration for Operai Toolbox

Brwse tool integration for GitHub source control, enabling comprehensive repository management, issue tracking, and pull request automation.

## Overview

This integration provides AI agents and automated workflows with direct access to GitHub repositories through a comprehensive toolkit:

- **Issue Management**: Search, create, and close issues with full support for labels, assignees, and comments
- **Pull Request Automation**: Open PRs, request reviews, merge with configurable strategies, and manage the complete PR lifecycle
- **Flexible Search**: Query issues and pull requests using GitHub's powerful search syntax with filtering by type

Primary use cases include automated triage of issues, creating pull requests from bot workflows, managing review assignments, and integrating GitHub operations into AI-powered development workflows.

## Authentication

This integration uses OAuth2 Bearer Token authentication for secure API access. Credentials are supplied through the Operai Toolbox credential system and automatically attached to all GitHub API requests.

### Required Credentials

- **access_token** (required): OAuth2 access token for GitHub API. This can be a personal access token (PAT), OAuth token, or GitHub App installation token. The token must have appropriate scopes for the operations you intend to perform (e.g., `repo` for full repository access, `public_repo` for public repositories only).
- **endpoint** (optional): Custom GitHub API endpoint (defaults to `https://api.github.com`). Use this for GitHub Enterprise Server instances by providing your API base URL (e.g., `https://github.example.com/api/v3`).

## Available Tools

### search_issues_prs
**Tool Name:** Search GitHub Issues/PRs
**Capabilities:** read
**Tags:** github, issues, pull-requests, search
**Description:** Search for issues and pull requests in a GitHub repository

**Input:**
- `owner` (string): Repository owner (e.g., "octocat")
- `repo` (string): Repository name (e.g., "Hello-World")
- `query` (string): Search query string using GitHub search syntax
- `filter` (optional, enum): Filter by type - "issue", "pr", or "all". Defaults to "all"
- `limit` (optional, integer): Maximum number of results (1-100). Defaults to 30

**Output:**
- `issues` (array): List of matching issues with number, title, body, state, html_url, user, labels, created_at, updated_at
- `pull_requests` (array): List of matching pull requests with number, title, body, state, html_url, user, head, base, draft, mergeable, created_at, updated_at

### create_issue
**Tool Name:** Create GitHub Issue
**Capabilities:** write
**Tags:** github, issues
**Description:** Create a new issue in a GitHub repository

**Input:**
- `owner` (string): Repository owner
- `repo` (string): Repository name
- `title` (string): Issue title
- `body` (optional, string): Issue body/description
- `labels` (optional, array of strings): Labels to apply to the issue
- `assignees` (optional, array of strings): Usernames to assign to the issue

**Output:**
- `issue` (object): Created issue with number, title, body, state, html_url, user, labels, created_at, updated_at

### comment
**Tool Name:** Comment on GitHub Issue/PR
**Capabilities:** write
**Tags:** github, issues, pull-requests, comments
**Description:** Add a comment to a GitHub issue or pull request

**Input:**
- `owner` (string): Repository owner
- `repo` (string): Repository name
- `issue_number` (integer): Issue or PR number
- `body` (string): Comment body text

**Output:**
- `comment` (object): Created comment with id, body, user, html_url, created_at, updated_at

### open_pull_request
**Tool Name:** Open GitHub Pull Request
**Capabilities:** write
**Tags:** github, pull-requests
**Description:** Create a new pull request in a GitHub repository

**Input:**
- `owner` (string): Repository owner
- `repo` (string): Repository name
- `title` (string): PR title
- `body` (optional, string): PR body/description
- `head` (string): Head branch (source branch to merge from)
- `base` (string): Base branch (target branch to merge into)
- `draft` (optional, boolean): Create as draft PR. Defaults to false

**Output:**
- `pull_request` (object): Created pull request with number, title, body, state, html_url, user, head, base, draft, mergeable, created_at, updated_at

### request_review
**Tool Name:** Request Review on GitHub PR
**Capabilities:** write
**Tags:** github, pull-requests, reviews
**Description:** Request reviews from users or teams on a GitHub pull request

**Input:**
- `owner` (string): Repository owner
- `repo` (string): Repository name
- `pull_number` (integer): Pull request number
- `reviewers` (array of strings): Reviewers (usernames) to request
- `team_reviewers` (optional, array of strings): Team reviewers (team slugs) to request

**Output:**
- `requested` (boolean): True if review request was successfully created

### merge_pull_request
**Tool Name:** Merge GitHub Pull Request
**Capabilities:** write
**Tags:** github, pull-requests
**Description:** Merge a GitHub pull request

**Input:**
- `owner` (string): Repository owner
- `repo` (string): Repository name
- `pull_number` (integer): Pull request number
- `commit_message` (optional, string): Custom commit message for the merge
- `merge_method` (optional, string): Merge method - "merge", "squash", or "rebase". Defaults to "merge"

**Output:**
- `merged` (boolean): True if the pull request was successfully merged
- `sha` (string): SHA of the merge commit

### close
**Tool Name:** Close GitHub Issue/PR
**Capabilities:** write
**Tags:** github, issues, pull-requests
**Description:** Close a GitHub issue or pull request

**Input:**
- `owner` (string): Repository owner
- `repo` (string): Repository name
- `number` (integer): Issue or PR number
- `is_pull_request` (optional, boolean): Whether this is a pull request. Defaults to false

**Output:**
- `closed` (boolean): True if the issue/PR was successfully closed

## API Documentation

- **Base URL:** `https://api.github.com` (configurable via `endpoint` credential)
- **API Documentation:** [GitHub REST API Documentation](https://docs.github.com/en/rest)
- **API Version:** Uses GitHub API v3 with header `X-GitHub-Api-Version: 2022-11-28`

## Testing

Run tests with:

```bash
cargo test -p github
```

The test suite includes 22 tests covering:
- Serialization roundtrips for all enum types
- URL normalization and validation
- Input validation for all tools
- HTTP mocking with wiremock for integration tests

## Development

- **Crate:** `github`
- **Source:** `examples/source-control/github/`
- **Dependencies:** Uses `reqwest` for HTTP client, custom types for GitHub API responses
- **Type:** Dynamic library (`cdylib`) for runtime loading by Operai Toolbox
