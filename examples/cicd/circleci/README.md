# CircleCI Integration for Operai Toolbox

Interact with CircleCI API v2 to manage CI/CD pipelines, workflows, and jobs.

## Overview

This integration enables Operai Toolbox to:

- **Trigger pipelines** - Start new CircleCI pipelines for specific projects and branches
- **Monitor pipeline status** - Query pipeline and workflow status in real-time
- **Access job logs** - Retrieve job details and log URLs for debugging
- **Rerun jobs** - Retry failed jobs or workflows from specific points

**Primary use cases:**
- Automating CI/CD workflows from AI agents
- Monitoring build and deployment status
- Debugging failed builds by accessing logs
- Implementing custom deployment pipelines

## Authentication

This integration uses a **system credential** with an API token for CircleCI API v2 authentication.

### Required Credentials

The following credentials are configured in the `operai.toml` manifest:
- `api_key` (required) - CircleCI personal API token
- `endpoint` (optional) - Custom API endpoint (defaults to `https://circleci.com/api/v2`)

**Manifest Configuration:**

```toml
[[tools]]
package = "circleci"
[tools.credentials.circleci]
api_key = "..."
# endpoint = "..."
```

### Getting Your API Token

1. Log in to [CircleCI](https://app.circleci.com/)
2. Go to **User Settings** → **Personal API Tokens**
3. Click **Create New Token**
4. Give it a descriptive name and click **Add API Token**
5. Copy the token and add it to your manifest

## Available Tools

### trigger_pipeline

**Tool Name:** Trigger CircleCI Pipeline

**Capabilities:** write

**Tags:** cicd, circleci, pipeline

**Description:** Trigger a new pipeline for a CircleCI project

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `project_slug` | string | Project slug in the format: vcs-slug/org-name/repo-name (e.g., "gh/myorg/myrepo") |
| `branch` | string, optional | Branch to build (defaults to project's default branch) |
| `tag` | string, optional | Tag to build |
| `parameters` | object, optional | Pipeline parameters as JSON key-value pairs |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `pipeline_id` | string | Unique identifier for the triggered pipeline |
| `pipeline_number` | number | Sequential pipeline number |
| `state` | string | Current state of the pipeline (created, errored, setup-pending, setup, pending) |
| `created_at` | string | ISO 8601 timestamp of pipeline creation |

---

### get_pipeline_status

**Tool Name:** Get CircleCI Pipeline Status

**Capabilities:** read

**Tags:** cicd, circleci, pipeline, status

**Description:** Get the status of a CircleCI pipeline and its workflows

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `pipeline_id` | string | Pipeline ID (UUID) |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `pipeline` | object | Full pipeline details including id, project_slug, number, state, created_at, vcs, trigger, errors |
| `workflows` | array | List of workflows with id, name, status (success, running, failed, etc.), created_at, stopped_at |

---

### get_job_logs

**Tool Name:** Get CircleCI Job Logs

**Capabilities:** read

**Tags:** cicd, circleci, job, logs

**Description:** Get details and log URL for a CircleCI job

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `project_slug` | string | Project slug in the format: vcs-slug/org-name/repo-name |
| `job_number` | number | Job number |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `job` | object | Job details including id, name, status, started_at, stopped_at, type, web_url, organization, pipeline |
| `log_url` | string | Web URL to view job logs in CircleCI dashboard |

**Note:** CircleCI API v2 doesn't provide direct log content. The returned `web_url` can be used to view logs in the CircleCI dashboard.

---

### rerun_job

**Tool Name:** Rerun CircleCI Job

**Capabilities:** write

**Tags:** cicd, circleci, job, rerun

**Description:** Rerun a CircleCI job or workflow

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `project_slug` | string | Project slug in the format: vcs-slug/org-name/repo-name |
| `job_number` | number | Job number to rerun |
| `from_failed` | boolean, optional | Whether to rerun from failed jobs only (default: false) |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `success` | boolean | Whether the rerun request succeeded |
| `message` | string | Result message from the API |

## API Documentation

- **Base URL:** `https://circleci.com/api/v2`
- **API Documentation:** [CircleCI API v2 Reference](https://circleci.com/docs/api/v2/)

## Project Slug Format

The project slug identifies a specific project in CircleCI and follows this format:

```
<vcs-slug>/<org-name>/<repo-name>
```

Examples:
- GitHub: `gh/myorg/myrepo`
- Bitbucket: `bb/myorg/myrepo`
- GitLab: `gitlab/myorg/myrepo`

You can find your project slug in the CircleCI dashboard URL when viewing a project.

## Error Handling

All tools validate inputs and return descriptive errors:

- Empty or invalid project slugs
- Missing pipeline IDs
- Invalid job numbers (must be > 0)
- API authentication failures (401)
- Resource not found (404)
- API rate limiting (429)

## Testing

Comprehensive tests are included covering:
- Input validation for all tools
- Serialization roundtrip tests for enums
- URL normalization
- Integration tests with wiremock for HTTP mocking

Run tests:
```bash
cargo test -p circleci
```

## Development

- **Crate:** `circleci`
- **Source:** `examples/cicd/circleci/`

## Resources

- [CircleCI Documentation](https://circleci.com/docs/)
- [CircleCI API v2 Reference](https://circleci.com/docs/api/v2/)
- [Managing API Tokens](https://circleci.com/docs/managing-api-tokens/)
