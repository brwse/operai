# Gitea Integration for Operai Toolbox

A painless self-hosted Git service integration for managing repositories and pull requests on Gitea instances.

## Overview

- **Repository Management**: List repositories for owners and organizations on Gitea Cloud or self-hosted instances
- **Pull Request Workflows**: Create, comment, approve, merge, and close pull requests
- **Self-Hosted Support**: Compatible with both Gitea Cloud and custom Gitea installations

## Authentication

This integration uses **Bearer Token** authentication. Credentials are supplied via user credentials with the `gitea` key.

### Required Credentials

- `access_token`: Personal access token or OAuth2 access token for the Gitea API
- `endpoint` (optional): Custom Gitea API endpoint for self-hosted instances (defaults to `https://gitea.com`)

### Obtaining a Personal Access Token

1. Log in to your Gitea instance
2. Navigate to **Settings** → **Applications** → **Generate New Token**
3. Select the required permissions:
   - `repo` - Full control of repositories
   - `write:org` - Read and write org and team membership
4. Copy the generated token and configure it as the `access_token` credential

For self-hosted instances, provide the `endpoint` credential (e.g., `https://git.example.com`).

## Available Tools

### list_repos
**Tool Name:** List Gitea Repositories
**Capabilities:** read
**Tags:** git, gitea, repository
**Description:** List repositories for an owner/organization on Gitea

**Input:**
- `owner` (string, required): Owner/organization name
- `limit` (number, optional): Maximum number of results (1-100). Defaults to 30

**Output:**
- `repositories` (array of RepositorySummary): List of repository summaries
  - `id` (number): Repository ID
  - `name` (string): Repository name
  - `full_name` (string): Full repository name (e.g., "owner/repo")
  - `description` (string, optional): Repository description
  - `private` (boolean): Whether the repository is private
  - `html_url` (string, optional): URL to the repository in the browser

### create_pr
**Tool Name:** Create Gitea Pull Request
**Capabilities:** write
**Tags:** git, gitea, pull-request
**Description:** Create a new pull request on Gitea

**Input:**
- `owner` (string, required): Owner/organization name
- `repo` (string, required): Repository name
- `title` (string, required): Pull request title
- `body` (string, optional): Pull request body/description
- `head` (string, required): Head branch (source branch)
- `base` (string, required): Base branch (target branch)

**Output:**
- `pull_request` (PullRequestSummary): Created pull request summary
  - `id` (number): Pull request ID
  - `number` (number): Pull request number
  - `title` (string, optional): Pull request title
  - `state` (string, optional): Pull request state (e.g., "open", "closed")
  - `html_url` (string, optional): URL to the pull request in the browser

### comment
**Tool Name:** Comment on Gitea Pull Request
**Capabilities:** write
**Tags:** git, gitea, pull-request, comment
**Description:** Add a comment to a Gitea pull request

**Input:**
- `owner` (string, required): Owner/organization name
- `repo` (string, required): Repository name
- `pr_number` (number, required): Pull request number
- `body` (string, required): Comment text

**Output:**
- `comment_id` (number): ID of the created comment
- `created` (boolean): Whether the comment was successfully created

### approve
**Tool Name:** Approve Gitea Pull Request
**Capabilities:** write
**Tags:** git, gitea, pull-request, review
**Description:** Approve a Gitea pull request

**Input:**
- `owner` (string, required): Owner/organization name
- `repo` (string, required): Repository name
- `pr_number` (number, required): Pull request number
- `body` (string, optional): Optional review comment

**Output:**
- `review_id` (number): ID of the created review
- `approved` (boolean): Whether the pull request was successfully approved

### merge
**Tool Name:** Merge Gitea Pull Request
**Capabilities:** write
**Tags:** git, gitea, pull-request, merge
**Description:** Merge a Gitea pull request

**Input:**
- `owner` (string, required): Owner/organization name
- `repo` (string, required): Repository name
- `pr_number` (number, required): Pull request number
- `merge_method` (string, optional): Merge method: "merge", "rebase", "rebase-merge", or "squash". Defaults to "merge"

**Output:**
- `merged` (boolean): Whether the pull request was successfully merged

### close
**Tool Name:** Close Gitea Pull Request
**Capabilities:** write
**Tags:** git, gitea, pull-request
**Description:** Close a Gitea pull request without merging

**Input:**
- `owner` (string, required): Owner/organization name
- `repo` (string, required): Repository name
- `pr_number` (number, required): Pull request number

**Output:**
- `closed` (boolean): Whether the pull request was successfully closed

## API Documentation

- **Base URL:** `https://gitea.com/api/v1` (or custom endpoint via `endpoint` credential)
- **API Documentation:** [Gitea API Documentation](https://docs.gitea.com/development/api-usage)

## Testing

Run tests with:

```bash
cargo test -p gitea
```

The test suite includes:
- Input validation tests for all tools
- Serialization roundtrip tests
- HTTP integration tests with mocked API responses (using wiremock)
- Error handling tests for API failures

## Development

- **Crate:** `gitea`
- **Source:** `examples/source-control/gitea/src/`
- **Dependencies:** `reqwest` for HTTP requests, `serde` for serialization
