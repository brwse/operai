# Aircall Integration for Operai Toolbox

Integrate with Aircall's phone system API to manage calls, recordings, and follow-ups.

## Overview

This integration enables Operai Toolbox to interact with Aircall's cloud-based phone system:

- List and retrieve calls with filtering and pagination support
- Assign or transfer calls to users or teams
- Add follow-up comments and notes to calls
- Fetch call recording and voicemail URLs

Primary use cases include call logging, call assignment workflows, and retrieving call recordings for review or analysis.

## Authentication

This integration uses **Basic Authentication** with API ID and API Token credentials. Credentials are supplied as system credentials through the `define_system_credential!` macro.

### Required Credentials

- `api_id`: Aircall API ID for authentication (required)
- `api_token`: Aircall API Token for authentication (required)
- `endpoint`: Custom API endpoint URL (optional, defaults to `https://api.aircall.io/v1`)

### Getting Credentials

1. Log in to your Aircall dashboard
2. Navigate to Company Settings > API Keys
3. Create a new API key or use an existing one
4. Copy the API ID and API Token

## Available Tools

### list_calls
**Tool Name:** List Aircall Calls
**Capabilities:** read
**Tags:** calls, aircall, phone
**Description:** List calls from Aircall with optional filters

**Input:**
- `limit` (optional `u32`): Maximum number of results per page (1-50). Defaults to 10
- `page` (optional `u32`): Pagination: page number to retrieve. Defaults to 1
- `from` (optional `i64`): Filter by start timestamp (Unix timestamp in seconds)
- `to` (optional `i64`): Filter by end timestamp (Unix timestamp in seconds)
- `order` (optional `String`): Sort order: "asc" or "desc". Defaults to "desc"

**Output:**
- `calls` (`Vec<CallSummary>`): List of call summaries with basic metadata
- `meta` (optional `Meta`): Pagination metadata including total count, current page, and per-page values

### assign_call
**Tool Name:** Assign Aircall Call
**Capabilities:** write
**Tags:** calls, aircall, assignment
**Description:** Assign or transfer a call to a user or team in Aircall

**Input:**
- `call_id` (`i64`): Aircall call ID to assign
- `user_id` (optional `i64`): User ID to assign the call to (mutually exclusive with `team_id`)
- `team_id` (optional `i64`): Team ID to assign the call to (mutually exclusive with `user_id`)

**Output:**
- `assigned` (`bool`): Confirmation that the call was assigned
- `call_id` (`i64`): The ID of the call that was assigned

### create_followup
**Tool Name:** Create Aircall Follow-up
**Capabilities:** write
**Tags:** calls, aircall, comments
**Description:** Add a follow-up comment/note to an Aircall call

**Input:**
- `call_id` (`i64`): Aircall call ID to add a comment to
- `content` (`String`): Comment content (maximum 5 comments per call)

**Output:**
- `created` (`bool`): Confirmation that the comment was created
- `call_id` (`i64`): The ID of the call the comment was added to
- `comment` (`Comment`): The created comment object including ID, content, and posting timestamp

### fetch_recording_link
**Tool Name:** Fetch Aircall Recording Link
**Capabilities:** read
**Tags:** calls, aircall, recordings
**Description:** Get the recording URL for an Aircall call

**Input:**
- `call_id` (`i64`): Aircall call ID
- `use_short_url` (`bool`): When true, return short URLs (valid 3 hours) instead of direct URLs (valid 1 hour)

**Output:**
- `call_id` (`i64`): The ID of the call
- `recording_url` (optional `String`): URL to the call recording
- `voicemail_url` (optional `String`): URL to the voicemail recording
- `asset_url` (optional `String`): URL to the secured webpage view of the call

## API Documentation

- Base URL: `https://api.aircall.io/v1`
- API Documentation: https://developer.aircall.io/
- Rate Limit: 60 requests per minute per company

## Testing

Run tests:
```bash
cargo test -p aircall
```

The integration includes comprehensive tests:
- Input validation tests for all tools
- Serialization roundtrip tests for enums and structs
- HTTP mock tests using Wiremock

## Development

- Crate: `aircall`
- Source: `examples/meetings-calling/aircall/src/lib.rs`

## Notes

- Recording URLs expire after 1 hour (direct) or 3 hours (short URLs)
- Maximum 5 comments per call (Aircall API limit)
- Transfers create a new call assignment, not a live transfer
- Call directions: `inbound` (incoming) or `outbound` (outgoing)
- Call statuses: `initial`, `ringing`, `answered`, `done`, `abandoned`
