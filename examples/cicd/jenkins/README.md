# Jenkins Integration for Operai Toolbox

Provides tools for interacting with Jenkins CI/CD server, including triggering builds, checking build status, fetching console logs, and downloading artifacts.

## Overview

This integration enables Operai Toolbox to interact with Jenkins CI/CD servers through the REST API.

- **Trigger job builds** with optional parameters, supporting nested job paths
- **Monitor build status** and retrieve detailed build information
- **Fetch console output** for debugging and log analysis
- **Download build artifacts** as base64-encoded content

### Primary Use Cases

- Automating CI/CD workflows from AI assistants
- Monitoring build status and retrieving build logs
- Fetching build artifacts for deployment or analysis
- Integrating Jenkins operations into larger automation pipelines

## Authentication

Uses **Basic Authentication** with system credentials. Credentials are supplied via environment variables or system configuration.

### Required Credentials

- `username`: Jenkins username for authentication (required)
- `password`: Jenkins API token (required) - Use an API token, not your account password
- `endpoint` (optional): Custom Jenkins server URL (defaults to `http://localhost:8080`)

### Obtaining an API Token

1. Log in to Jenkins
2. Click your name in the top right corner
3. Click "Configure" in the left sidebar
4. Under "API Token", click "Add new Token"
5. Give it a name and click "Generate"
6. Copy the generated token

## Available Tools

### trigger_job
**Tool Name:** Trigger Jenkins Job
**Capabilities:** write
**Tags:** jenkins, cicd, build
**Description:** Trigger a Jenkins job build with optional parameters

**Input:**
- `job_name` (string): Job name or path (e.g., "my-job" or "folder/my-job")
- `parameters` (array<JobParameter>, optional): Job parameters as key-value pairs
  - `name` (string): Parameter name
  - `value` (string): Parameter value

**Output:**
- `triggered` (boolean): Whether the job was successfully triggered
- `queue_id` (number, optional): Queue item ID if available from Jenkins

---

### get_build_status
**Tool Name:** Get Jenkins Build Status
**Capabilities:** read
**Tags:** jenkins, cicd, build, status
**Description:** Get the status of a specific Jenkins build

**Input:**
- `job_name` (string): Job name or path
- `build_number` (string): Build number. Use "lastBuild" for the most recent build

**Output:**
- `status` (BuildStatus): Build status information
  - `number` (number): Build number
  - `display_name` (string, optional): Build display name
  - `url` (string): Build URL
  - `building` (boolean): Whether build is currently in progress
  - `result` (string, optional): Build result (SUCCESS, FAILURE, UNSTABLE, ABORTED, NOT_BUILT)
  - `duration` (number): Build duration in milliseconds
  - `estimated_duration` (number, optional): Estimated duration in milliseconds for running builds
  - `timestamp` (number): Build timestamp (Unix epoch milliseconds)

---

### fetch_console_log
**Tool Name:** Fetch Jenkins Console Log
**Capabilities:** read
**Tags:** jenkins, cicd, build, logs
**Description:** Fetch the console output of a Jenkins build

**Input:**
- `job_name` (string): Job name or path
- `build_number` (string): Build number

**Output:**
- `console_log` (string): Console log output as plain text

---

### download_artifact
**Tool Name:** Download Jenkins Artifact
**Capabilities:** read
**Tags:** jenkins, cicd, build, artifacts
**Description:** Download a build artifact from Jenkins

**Input:**
- `job_name` (string): Job name or path
- `build_number` (string): Build number
- `artifact_path` (string): Artifact relative path from build's artifacts list

**Output:**
- `content` (string): Artifact content as base64-encoded bytes
- `file_name` (string): Artifact file name

## API Documentation

- **Base URL:** `http://localhost:8080` (configurable via `endpoint` credential)
- **API Documentation:** [Jenkins Remote Access API](https://www.jenkins.io/doc/book/using/remote-access-api/)

## Implementation Notes

- Uses Jenkins REST API with Basic Authentication
- Supports nested job paths (e.g., "folder/subfolder/job")
- Build numbers can use special Jenkins identifiers like "lastBuild", "lastSuccessfulBuild", "lastStableBuild", "lastFailedBuild", "lastCompletedBuild", "lastUnstableBuild", "lastUnsuccessfulBuild"
- Artifacts are returned as base64-encoded strings for binary safe transport
- Queue IDs are extracted from the HTTP Location header when triggering builds

## Examples

### Trigger a build without parameters
```json
{
  "job_name": "my-project",
  "parameters": []
}
```

### Trigger a build with parameters
```json
{
  "job_name": "my-project/main",
  "parameters": [
    {"name": "BRANCH", "value": "develop"},
    {"name": "ENVIRONMENT", "value": "staging"}
  ]
}
```

### Get build status
```json
{
  "job_name": "my-project",
  "build_number": "42"
}
```

### Get status of last build
```json
{
  "job_name": "my-project",
  "build_number": "lastBuild"
}
```

### Fetch console log
```json
{
  "job_name": "my-project",
  "build_number": "42"
}
```

### Download artifact
```json
{
  "job_name": "my-project",
  "build_number": "lastSuccessfulBuild",
  "artifact_path": "target/my-app.jar"
}
```

## Testing

The integration includes comprehensive tests:
- Serialization roundtrip tests for all types
- Input validation tests for all tools
- Integration tests with wiremock for HTTP mocking
- Error handling tests (401, 404 responses)

Run tests:
```bash
cargo test -p jenkins
```

## Development

- **Crate:** `jenkins`
- **Source:** `examples/cicd/jenkins/`
