# Azure Repos Integration for Operai Toolbox

Interact with Azure DevOps Repositories to manage Git repositories, pull requests, reviews, and comments.

## Overview

- List repositories in Azure DevOps projects
- Create and manage pull requests in Azure Repos
- Add comments to pull requests for code review collaboration
- Approve or reject pull requests with configurable voting
- Merge pull requests with custom merge options

### Primary Use Cases

- Automating repository discovery and inventory across Azure DevOps projects
- Creating pull requests from feature branches for code review
- Streamlining code review workflows with automated commenting and approval
- Managing pull request lifecycle from creation to merge

## Authentication

This integration uses **OAuth2 Bearer Token** authentication via Azure DevOps Personal Access Tokens (PATs). Credentials are supplied through the Operai Toolbox credential system and used in HTTP Basic Auth format (empty username, PAT as password).

### Required Credentials

- `access_token`: OAuth2 access token (Personal Access Token) for Azure DevOps API
- `organization`: Azure DevOps organization name (e.g., `"myorg"` for `dev.azure.com/myorg`)
- `endpoint` (optional): Custom API endpoint for on-premise Azure DevOps Server (defaults to `"https://dev.azure.com/{organization}"`)

### Getting a Personal Access Token

1. Sign in to your Azure DevOps organization (`https://dev.azure.com/{organization}`)
2. From your home page, open user settings and select **Personal access tokens**
3. Select **+ New Token**
4. Name your token, select the organization where you want to use the token
5. Set an expiration date for your token
6. Select the scopes for this token:
   - **Code** > **Read** (for list_repos)
   - **Code** > **Read & write** (for create_pr, comment, approve, merge)
7. Select **Create**
8. Copy the token - you won't be able to see it again

## Available Tools

### list_repos

**Tool Name:** List Azure Repos
**Capabilities:** read
**Tags:** source-control, azure-repos, azure-devops
**Description:** List all repositories in an Azure DevOps project

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `project` | `String` | Project name or ID |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `repositories` | `Vec<Repository>` | List of repositories with metadata (id, name, default_branch, urls) |

---

### create_pr

**Tool Name:** Create Azure Repos Pull Request
**Capabilities:** write
**Tags:** source-control, azure-repos, azure-devops
**Description:** Create a new pull request in an Azure Repos repository

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `project` | `String` | Project name or ID |
| `repository_id` | `String` | Repository ID or name |
| `title` | `String` | Title of the pull request |
| `description` | `Option<String>` | Optional description of the pull request |
| `source_ref_name` | `String` | Source branch ref (e.g., `"refs/heads/feature-branch"`) |
| `target_ref_name` | `String` | Target branch ref (e.g., `"refs/heads/main"`) |
| `reviewers` | `Vec<String>` | Optional reviewers to add (user IDs) |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `pull_request` | `PullRequest` | Created pull request with ID, status, and metadata |

---

### comment

**Tool Name:** Comment on Azure Repos Pull Request
**Capabilities:** write
**Tags:** source-control, azure-repos, azure-devops
**Description:** Add a comment to an Azure Repos pull request

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `project` | `String` | Project name or ID |
| `repository_id` | `String` | Repository ID or name |
| `pull_request_id` | `i32` | Pull request ID |
| `comment` | `String` | Comment text |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `thread` | `CommentThread` | Created comment thread with ID and comments |

---

### approve

**Tool Name:** Approve Azure Repos Pull Request
**Capabilities:** write
**Tags:** source-control, azure-repos, azure-devops
**Description:** Approve or vote on an Azure Repos pull request

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `project` | `String` | Project name or ID |
| `repository_id` | `String` | Repository ID or name |
| `pull_request_id` | `i32` | Pull request ID |
| `vote` | `i32` | Vote: 10 = approved, 5 = approved with suggestions, 0 = no vote, -5 = waiting for author, -10 = rejected |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `reviewer` | `Reviewer` | Reviewer info with updated vote status |

---

### merge

**Tool Name:** Merge Azure Repos Pull Request
**Capabilities:** write
**Tags:** source-control, azure-repos, azure-devops
**Description:** Complete and merge an Azure Repos pull request

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `project` | `String` | Project name or ID |
| `repository_id` | `String` | Repository ID or name |
| `pull_request_id` | `i32` | Pull request ID |
| `commit_message` | `Option<String>` | Merge commit message (optional) |
| `delete_source_branch` | `bool` | Whether to delete source branch after merge |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `pull_request` | `PullRequest` | Merged pull request with completed status |

## API Documentation

- **Base URL:** `https://dev.azure.com/{organization}` (default for cloud)
- **API Version:** 7.1
- **Documentation:** [Azure DevOps REST API Reference](https://learn.microsoft.com/en-us/rest/api/azure/devops/git/?view=azure-devops-rest-7.1)

For on-premise Azure DevOps Server, provide a custom `endpoint` credential with the server URL.

## Testing

Run tests:

```bash
cargo test -p azure-repos
```

The test suite includes:
- Input validation tests for all tools
- Serialization roundtrip tests for all enums and structs
- Wiremock-based HTTP mock tests for all API endpoints
- Authentication and error handling tests

## Development

- **Crate:** `azure-repos`
- **Source:** `examples/source-control/azure-repos/`
- **Types:** See `src/types.rs` for complete data structure definitions
