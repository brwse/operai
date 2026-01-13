# GitLab CI Integration for Operai Toolbox

Brwse tool integration for GitLab CI/CD pipelines, enabling automation and monitoring of GitLab continuous integration and delivery workflows.

## Overview

This integration provides Operai Toolbox with the ability to interact with GitLab's CI/CD platform:

- **Trigger pipelines** on specific branches or tags with custom variables
- **Monitor pipeline status** and retrieve detailed execution information
- **Fetch job logs** for debugging and analysis
- **Download build artifacts** from successful pipeline runs

### Primary Use Cases

- Automating CI/CD pipeline triggers in response to external events
- Monitoring pipeline execution status and notifying on completion
- Retrieving build artifacts for deployment or further processing
- Debugging failed pipelines by fetching console logs

## Authentication

This integration uses **system credentials** configured at deployment via environment variables. Credentials are managed through the `define_system_credential!` macro and supplied to the GitLabClient via the Brwse Context.

### Required Credentials

The following credentials are configured as system credentials:

- **`access_token`** (required): GitLab personal access token or project access token with API scope. Used to authenticate API requests via the `PRIVATE-TOKEN` header.

- **`endpoint`** (optional): Custom GitLab API endpoint URL. Defaults to `https://gitlab.com` for GitLab.com. Set this for self-hosted GitLab instances (e.g., `https://gitlab.example.com`).

**Manifest Configuration:**
```toml
[[tools]]
package = "brwse-gitlab-ci"
[tools.credentials.gitlab]
access_token = "your-gitlab-access-token"
# endpoint = "https://gitlab.example.com"  # Optional
```

For self-hosted GitLab instances, set `endpoint` to your instance URL.

## Available Tools

### trigger_pipeline

**Tool Name:** Trigger GitLab Pipeline

**Capabilities:** write

**Tags:** cicd, gitlab, pipeline

**Description:** Trigger a CI/CD pipeline for a GitLab project

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `project` | `String` | GitLab project ID or path (e.g., "group/project") |
| `ref_name` | `String` | Branch or tag name to trigger the pipeline on |
| `trigger_token` | `String` | Pipeline trigger token |
| `variables` | `HashMap<String, String>` | Optional variables to pass to the pipeline (default: empty) |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `pipeline_id` | `u64` | The ID of the triggered pipeline |
| `project_id` | `u64` | The project ID |
| `status` | `PipelineStatus` | Current pipeline status (Created, Pending, Running, Success, Failed, etc.) |
| `ref_name` | `String` | The branch/tag name |
| `sha` | `String` | The commit SHA |
| `web_url` | `Option<String>` | URL to view the pipeline in GitLab |

---

### get_pipeline_status

**Tool Name:** Get GitLab Pipeline Status

**Capabilities:** read

**Tags:** cicd, gitlab, pipeline

**Description:** Get the status of a specific GitLab CI/CD pipeline

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `project` | `String` | GitLab project ID or path (e.g., "group/project") |
| `pipeline_id` | `u64` | Pipeline ID to query |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `pipeline_id` | `u64` | The pipeline ID |
| `project_id` | `u64` | The project ID |
| `status` | `PipelineStatus` | Current pipeline status |
| `ref_name` | `String` | The branch/tag name |
| `sha` | `String` | The commit SHA |
| `web_url` | `Option<String>` | URL to view the pipeline in GitLab |
| `created_at` | `Option<String>` | ISO 8601 timestamp when pipeline was created |
| `updated_at` | `Option<String>` | ISO 8601 timestamp when pipeline was last updated |
| `started_at` | `Option<String>` | ISO 8601 timestamp when pipeline started |
| `finished_at` | `Option<String>` | ISO 8601 timestamp when pipeline finished |
| `duration` | `Option<u64>` | Pipeline duration in seconds |

---

### fetch_job_logs

**Tool Name:** Fetch GitLab Job Logs

**Capabilities:** read

**Tags:** cicd, gitlab, logs

**Description:** Fetch the console logs from a specific GitLab CI/CD job

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `project` | `String` | GitLab project ID or path (e.g., "group/project") |
| `job_id` | `u64` | Job ID to fetch logs from |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `job_id` | `u64` | The job ID |
| `logs` | `String` | Raw console log output from the job |

---

### download_artifacts

**Tool Name:** Download GitLab Job Artifacts

**Capabilities:** read

**Tags:** cicd, gitlab, artifacts

**Description:** Download artifacts from a successful GitLab CI/CD job

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `project` | `String` | GitLab project ID or path (e.g., "group/project") |
| `ref_name` | `String` | Branch, tag, or commit SHA to download artifacts from |
| `job` | `String` | Job name that produced the artifacts |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `project` | `String` | The project identifier |
| `ref_name` | `String` | The branch/tag name |
| `job` | `String` | The job name |
| `artifacts_base64` | `String` | Base64-encoded ZIP archive of artifacts |
| `size_bytes` | `usize` | Size of the artifacts in bytes |

## API Documentation

- **Base URL:** `https://gitlab.com` (default, configurable via `endpoint` credential)
- **API Version:** v4
- **API Documentation:** [GitLab CI/CD API Documentation](https://docs.gitlab.com/api/pipelines/)
  - [Trigger pipelines with the API](https://docs.gitlab.com/ci/triggers/)
  - [Pipelines API](https://docs.gitlab.com/api/pipelines/)
  - [Pipeline trigger tokens API](https://docs.gitlab.com/api/pipeline_triggers/)

## Testing

Run tests with cargo:

```bash
cd examples/cicd/gitlab-ci
cargo test
```

The test suite includes:
- Input validation tests for all tools
- Serialization/deserialization roundtrip tests
- Integration tests with mock HTTP server (wiremock)
- Error handling tests for API failures

## Development

- **Crate:** `brwse-gitlab-ci`
- **Source:** `examples/cicd/gitlab-ci/`
- **Language:** Rust
- **Operai Tool SDK:** `operai`

The integration uses the Operai Tool SDK's macros for:
- `#[tool]` - Tool definition with metadata
- `define_system_credential!` - Credential management
- `#[init]` / `#[shutdown]` - Lifecycle hooks
