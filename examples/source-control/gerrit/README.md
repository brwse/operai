# Gerrit Integration for Operai Toolbox

Operai Toolbox integration for Gerrit Code Review that enables searching changes, posting reviews, submitting approved changes, and managing change lifecycle.

## Overview

This integration provides tools to interact with Gerrit Code Review:

- Search and query code review changes using Gerrit's powerful search operators
- Post reviews with votes/scores on changes (e.g., Code-Review, Verified labels)
- Add comments (both general and inline) to changes
- Submit (merge) approved changes
- Abandon changes that are no longer needed

## Authentication

Gerrit uses HTTP Basic Authentication with username and HTTP password token.

### Required Credentials

The `GerritCredential` is defined with the following fields:

- `username` (required): Your Gerrit username
- `password` (required): HTTP authentication token from Gerrit settings (not your account password)
- `endpoint` (optional): Custom Gerrit instance URL (defaults to `https://gerrit.example.com`)

### Getting Your HTTP Password

1. Log in to your Gerrit instance
2. Navigate to **Settings** → **HTTP Credentials**
3. Click **Generate New Password**
4. Copy the generated password

The password field should contain your HTTP authentication token, not your actual account password.

## Available Tools

### search_changes
**Tool Name:** Search Gerrit Changes
**Capabilities:** read
**Tags:** gerrit, code-review, source-control
**Description:** Search for Gerrit code review changes using query operators

**Input:**
- `query` (string, required): Search query string using Gerrit search operators (examples: "status:open", "project:myproject", "owner:self")
- `limit` (optional integer): Maximum number of results to return (1-100). Defaults to 25.
- `skip` (optional integer): Number of changes to skip. Useful for pagination.

**Output:**
- `changes` (array of ChangeSummary): List of changes including id, project, branch, number, subject, status, owner, updated timestamp, and mergeable flag

### review_change
**Tool Name:** Review Gerrit Change
**Capabilities:** write
**Tags:** gerrit, code-review, source-control
**Description:** Post a review with votes/scores on a Gerrit change

**Input:**
- `change_id` (string, required): Change ID in numeric ID or "project~branch~change-id" format
- `revision_id` (optional string): Revision ID (commit SHA or "current"). Defaults to "current".
- `message` (optional string): Review message/comment
- `labels` (map of string to integer): Map of label names to vote values (e.g., {"Code-Review": 1, "Verified": 1})
- `ready` (boolean): Mark the change as ready for review

**Output:**
- `labels` (map of string to integer): Map of applied label vote values
- `ready` (boolean): Whether the change is ready for review

### comment_change
**Tool Name:** Comment on Gerrit Change
**Capabilities:** write
**Tags:** gerrit, code-review, source-control
**Description:** Post a comment (general or inline) on a Gerrit change

**Input:**
- `change_id` (string, required): Change ID in numeric ID or "project~branch~change-id" format
- `revision_id` (optional string): Revision ID (commit SHA or "current"). Defaults to "current".
- `message` (string, required): Comment message text
- `file_path` (optional string): Optional file path for inline comment
- `line` (optional integer): Optional line number for inline comment

**Output:**
- `accepted` (boolean): Whether the comment was accepted

### submit_change
**Tool Name:** Submit Gerrit Change
**Capabilities:** write
**Tags:** gerrit, code-review, source-control
**Description:** Submit (merge) a Gerrit change that has been approved

**Input:**
- `change_id` (string, required): Change ID in numeric ID or "project~branch~change-id" format
- `on_behalf_of` (optional integer): Optional account ID to submit on behalf of

**Output:**
- `change_id` (string): The submitted change ID
- `status` (string): New status after submission

### abandon_change
**Tool Name:** Abandon Gerrit Change
**Capabilities:** write
**Tags:** gerrit, code-review, source-control
**Description:** Abandon a Gerrit change that is no longer needed

**Input:**
- `change_id` (string, required): Change ID in numeric ID or "project~branch~change-id" format
- `message` (optional string): Optional abandon message explaining why

**Output:**
- `change_id` (string): The abandoned change ID
- `status` (string): New status after abandoning

## API Documentation

- Base URL: `https://gerrit.example.com` (customizable via `endpoint` credential)
- API Documentation: [Gerrit REST API](https://gerrit-review.googlesource.com/Documentation/rest-api.html)
- Search Operators: [Gerrit Search Operators](https://gerrit-review.googlesource.com/Documentation/user-search.html)

## Testing

Run tests:
```bash
cargo test -p brwse-gerrit
```

## Development

- Crate: brwse-gerrit
- Source: examples/source-control/gerrit
