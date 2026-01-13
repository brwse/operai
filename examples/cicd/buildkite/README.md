# Buildkite Integration for Operai Toolbox

This integration provides tools for interacting with Buildkite CI/CD pipelines through the REST API, enabling automated build management and monitoring.

## Overview

The Buildkite integration enables Operai Toolbox to:

- **Trigger builds** programmatically in Buildkite pipelines with custom environment variables, metadata, and author information
- **Monitor build status** including state, commit details, and job information
- **Fetch job logs** for debugging and analysis of build failures
- **Annotate builds** with contextual information, warnings, or errors directly on build pages

**Primary use cases:**
- Automated CI/CD workflows and deployment pipelines
- Build monitoring and notification systems
- Integration with AI-powered development tools
- Custom dashboards and reporting

## Authentication

This integration uses **system credentials** with a Bearer API token for authentication. Credentials are supplied via environment variables or runtime configuration.

### Required Credentials

- `api_token`: Buildkite API access token (required)
- `endpoint`: Custom API endpoint URL (optional, defaults to `https://api.buildkite.com/v2`)

### Creating an API Token

1. Go to your [Buildkite API Access Tokens](https://buildkite.com/user/api-access-tokens) page
2. Click "New API Access Token"
3. Give it a description (e.g., "Operai Toolbox")
4. Select the required scopes:
   - `read_builds` - Required for `get_build_status` and `fetch_job_logs`
   - `write_builds` - Required for `trigger_build` and `annotate_build`
5. Click "Create Token" and copy the token

### Manifest Configuration

Add the following to your `operai.toml` manifest:

```toml
[[tools]]
package = "brwse-buildkite"
[tools.credentials.buildkite]
api_token = "..."
# endpoint = "..."  # Optional
```

## Available Tools

### trigger_build

**Tool Name:** Trigger Buildkite Build
**Capabilities:** write
**Tags:** ci, buildkite, build
**Description:** Trigger a new build in Buildkite

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `organization` | string | Organization slug |
| `pipeline` | string | Pipeline slug |
| `commit` | string | Git commit SHA, reference, or tag to build |
| `branch` | string | Branch containing the commit |
| `message` | string (optional) | Build message/description |
| `author` | Author (optional) | Author information with `name` and `email` fields |
| `env` | map<string, string> (optional) | Environment variables for the build |
| `meta_data` | map<string, string> (optional) | Metadata for the build |
| `clean_checkout` | boolean (optional) | Force a clean checkout |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `build` | Build | Build object including ID, number, state, commit, branch, jobs, and URLs |

### get_build_status

**Tool Name:** Get Buildkite Build Status
**Capabilities:** read
**Tags:** ci, buildkite, build
**Description:** Get the status of a Buildkite build

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `organization` | string | Organization slug |
| `pipeline` | string | Pipeline slug |
| `build_number` | integer | Build number (not ID) |
| `include_retried_jobs` | boolean | Include all retried jobs (default: false) |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `build` | Build | Build object with state, jobs, timestamps, and environment variables |

### fetch_job_logs

**Tool Name:** Fetch Buildkite Job Logs
**Capabilities:** read
**Tags:** ci, buildkite, logs
**Description:** Fetch the log output for a specific job in a Buildkite build

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `organization` | string | Organization slug |
| `pipeline` | string | Pipeline slug |
| `build_number` | integer | Build number (not ID) |
| `job_id` | string | Job ID |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `log` | JobLog | Job log object with URL, content, size, and header times |

### annotate_build

**Tool Name:** Annotate Buildkite Build
**Capabilities:** write
**Tags:** ci, buildkite, annotation
**Description:** Create an annotation on a Buildkite build

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `organization` | string | Organization slug |
| `pipeline` | string | Pipeline slug |
| `build_number` | integer | Build number (not ID) |
| `body` | string | Annotation body (Markdown or HTML) |
| `style` | AnnotationStyle (optional) | Visual style: `success`, `info`, `warning`, or `error` |
| `context` | string (optional) | Context identifier for grouping/updating annotations |
| `append` | boolean | Whether to append to existing annotation (default: false) |
| `priority` | integer (optional) | Display priority 1-10, default 3 |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `annotation` | Annotation | Annotation object with ID, context, style, and HTML body |

## API Documentation

- **Base URL:** `https://api.buildkite.com/v2`
- **Buildkite REST API:** https://buildkite.com/docs/apis/rest-api
- **Builds API:** https://buildkite.com/docs/apis/rest-api/builds
- **Annotations API:** https://buildkite.com/docs/apis/rest-api/annotations

## Testing

Run tests with:

```bash
cargo test -p brwse-buildkite
```

The test suite includes:
- Serialization roundtrip tests for enums and structs
- Input validation tests for all tools
- Integration tests with mock HTTP server (wiremock)
- Error handling tests for various failure scenarios

## Development

- **Crate:** `brwse-buildkite`
- **Source:** `examples/cicd/buildkite/`
