# GitHub Actions Integration for Operai Toolbox

Interact with GitHub Actions workflows, runs, artifacts, and logs through the GitHub REST API.

## Overview

This integration enables Operai Toolbox to manage GitHub Actions CI/CD workflows:

- List and trigger workflows in GitHub repositories
- Monitor workflow run status and retrieve detailed execution information
- Download workflow run logs and build artifacts
- Filter and query workflow runs by status and workflow ID

Primary use cases include automating CI/CD pipeline management, monitoring build status, retrieving build artifacts, and integrating GitHub Actions into AI-assisted development workflows.

## Authentication

This integration uses OAuth2 Bearer token authentication with a GitHub personal access token or OAuth token. Credentials are supplied as user credentials through the `GitHubActionsCredential` definition.

### Required Credentials

- `access_token`: OAuth2 access token for GitHub API (required). The token must have appropriate scopes:
  - `repo` - For private repositories
  - `actions:read` - To list workflows and view run status
  - `actions:write` - To trigger workflows
- `endpoint`: Custom API endpoint URL (optional, defaults to `https://api.github.com`)

Create a personal access token at: https://github.com/settings/tokens

## Available Tools

### list_workflows
**Tool Name:** List GitHub Workflows
**Capabilities:** read
**Tags:** ci-cd, github, workflows
**Description:** List all workflows in a GitHub repository

**Input:**
- `owner` (string): Repository owner (username or organization)
- `repo` (string): Repository name
- `per_page` (optional integer): Maximum number of workflows to return (1-100). Defaults to 30

**Output:**
- `workflows` (array of WorkflowSummary): List of workflow summaries containing:
  - `id` (integer): Workflow ID
  - `node_id` (string): GraphQL node ID
  - `name` (string): Workflow name
  - `path` (string): Workflow file path
  - `state` (string): Workflow state (e.g., "active")
  - `created_at` (optional string): ISO 8601 creation timestamp
  - `updated_at` (optional string): ISO 8601 update timestamp

### trigger_workflow
**Tool Name:** Trigger GitHub Workflow
**Capabilities:** write
**Tags:** ci-cd, github, workflows, trigger
**Description:** Manually trigger a workflow run using workflow_dispatch event

**Input:**
- `owner` (string): Repository owner (username or organization)
- `repo` (string): Repository name
- `workflow_id` (string): Workflow ID or filename (e.g., "main.yml" or workflow ID)
- `git_ref` (string): Git reference (branch or tag name) for the workflow
- `inputs` (optional object): Optional inputs to pass to the workflow as JSON object with key-value pairs

**Output:**
- `triggered` (boolean): Whether the workflow was successfully triggered

**Note:** The workflow must have `workflow_dispatch` event configured in its YAML file.

### get_run_status
**Tool Name:** Get Workflow Run Status
**Capabilities:** read
**Tags:** ci-cd, github, workflows, status
**Description:** Get detailed status and information about a specific workflow run

**Input:**
- `owner` (string): Repository owner (username or organization)
- `repo` (string): Repository name
- `run_id` (integer): Workflow run ID (must be positive)

**Output:**
- `run` (WorkflowRunDetail): Detailed workflow run information containing:
  - `id` (integer): Run ID
  - `name` (string): Workflow name
  - `status` (enum): Current status (queued, in_progress, completed, waiting)
  - `conclusion` (optional enum): Final result (success, failure, neutral, cancelled, skipped, timed_out, action_required)
  - `workflow_id` (integer): Workflow ID
  - `head_branch` (string): Branch name
  - `head_sha` (string): Commit SHA
  - `run_number` (integer): Run number for the workflow
  - `event` (string): Event that triggered the run
  - `display_title` (string): Human-readable title
  - `created_at` (optional string): ISO 8601 creation timestamp
  - `updated_at` (optional string): ISO 8601 update timestamp
  - `run_started_at` (optional string): ISO 8601 start timestamp
  - `html_url` (string): URL to view the run in browser
  - `jobs_url` (string): API URL for jobs
  - `logs_url` (string): API URL for logs
  - `artifacts_url` (string): API URL for artifacts

### list_workflow_runs
**Tool Name:** List Workflow Runs
**Capabilities:** read
**Tags:** ci-cd, github, workflows, runs
**Description:** List workflow runs for a repository, optionally filtered by workflow and status

**Input:**
- `owner` (string): Repository owner (username or organization)
- `repo` (string): Repository name
- `workflow_id` (optional string): Optional workflow ID or filename to filter runs
- `status` (optional enum): Optional status filter (queued, in_progress, completed, waiting)
- `per_page` (optional integer): Maximum number of runs to return (1-100). Defaults to 30

**Output:**
- `runs` (array of WorkflowRunSummary): List of workflow run summaries containing:
  - `id` (integer): Run ID
  - `name` (string): Workflow name
  - `status` (enum): Current status
  - `conclusion` (optional enum): Final result
  - `workflow_id` (integer): Workflow ID
  - `head_branch` (string): Branch name
  - `head_sha` (string): Commit SHA
  - `run_number` (integer): Run number
  - `event` (string): Trigger event
  - `created_at` (optional string): ISO 8601 creation timestamp
  - `updated_at` (optional string): ISO 8601 update timestamp
  - `html_url` (string): URL to view the run

### fetch_logs
**Tool Name:** Fetch Workflow Run Logs
**Capabilities:** read
**Tags:** ci-cd, github, workflows, logs
**Description:** Download logs for a workflow run as a base64-encoded zip archive

**Input:**
- `owner` (string): Repository owner (username or organization)
- `repo` (string): Repository name
- `run_id` (integer): Workflow run ID (must be positive)

**Output:**
- `logs_base64` (string): Base64-encoded zip archive containing the logs
- `size_bytes` (integer): Size of the encoded logs in bytes

**Note:** Decode the base64 string and unzip the archive to access individual log files.

### list_artifacts
**Tool Name:** List Workflow Artifacts
**Capabilities:** read
**Tags:** ci-cd, github, workflows, artifacts
**Description:** List artifacts produced by a workflow run

**Input:**
- `owner` (string): Repository owner (username or organization)
- `repo` (string): Repository name
- `run_id` (integer): Workflow run ID (must be positive)
- `per_page` (optional integer): Maximum number of artifacts to return (1-100). Defaults to 30

**Output:**
- `artifacts` (array of Artifact): List of artifacts containing:
  - `id` (integer): Artifact ID
  - `node_id` (string): GraphQL node ID
  - `name` (string): Artifact name
  - `size_in_bytes` (integer): Artifact size
  - `url` (optional string): API URL for the artifact
  - `archive_download_url` (string): Download URL for the zip archive
  - `expired` (boolean): Whether the artifact has expired
  - `created_at` (optional string): ISO 8601 creation timestamp
  - `expires_at` (optional string): ISO 8601 expiration timestamp
  - `updated_at` (optional string): ISO 8601 update timestamp

### download_artifact
**Tool Name:** Download Workflow Artifact
**Capabilities:** read
**Tags:** ci-cd, github, workflows, artifacts
**Description:** Download a specific artifact from a workflow run as base64-encoded data

**Input:**
- `owner` (string): Repository owner (username or organization)
- `repo` (string): Repository name
- `artifact_id` (integer): Artifact ID (must be positive)

**Output:**
- `artifact_base64` (string): Base64-encoded artifact archive
- `size_bytes` (integer): Size of the encoded artifact in bytes

**Note:** Decode the base64 string to retrieve the artifact zip archive.

## API Documentation

- Base URL: `https://api.github.com` (configurable via `endpoint` credential)
- API Documentation: [GitHub Actions REST API](https://docs.github.com/en/rest/actions)
  - [Workflow Runs API](https://docs.github.com/en/rest/actions/workflow-runs)
  - [Workflows API](https://docs.github.com/en/rest/actions/workflows)
  - [Artifacts API](https://docs.github.com/en/rest/actions/artifacts)

## Testing

Run tests:
```bash
cargo test -p github-actions
```

All tests use wiremock for HTTP mocking and include:
- Input validation tests
- Serialization roundtrip tests
- Integration tests with mocked GitHub API responses
- Error handling tests

## Development

- Crate: `github-actions`
- Source: `examples/cicd/github-actions/src/`
