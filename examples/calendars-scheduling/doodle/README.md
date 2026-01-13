# Doodle Integration for Operai Toolbox

Mock integration for Doodle scheduling polls, enabling creation and management of group scheduling polls through the Operai Toolbox SDK.

## Overview

- Create scheduling polls with multiple date/time options
- List and analyze votes from participants
- Close polls and notify participants via email

> **Note:** This is a mock implementation since Doodle's public API was discontinued. The implementation follows operai patterns and can be adapted when/if Doodle provides API access for Enterprise customers.

## Authentication

This integration uses OAuth2 Bearer Token authentication. Credentials are supplied via user credentials configuration.

### Required Credentials

- `access_token`: OAuth2 access token for Doodle API (required)
- `endpoint`: Custom API endpoint (optional, defaults to `https://doodle.com/api/v2.0`)

## Available Tools

### create_poll

**Tool Name:** Create Doodle Poll
**Capabilities:** write
**Tags:** doodle, scheduling, poll
**Description:** Create a new Doodle scheduling poll with options

**Input:**
- `title` (string): Title of the poll
- `description` (string, optional): Optional description of the poll
- `location` (string, optional): Optional location for the event
- `options` (array of PollOptionInput): Poll options (e.g., dates/times or text options)
  - `text` (string): Text description of the option
  - `start_time` (string, optional): Optional start time (ISO 8601 format)
  - `end_time` (string, optional): Optional end time (ISO 8601 format)

**Output:**
- `poll_id` (string): ID of the created poll
- `poll_url` (string): URL to access the poll

### list_votes

**Tool Name:** List Doodle Poll Votes
**Capabilities:** read
**Tags:** doodle, scheduling, poll, votes
**Description:** List all votes for a Doodle poll

**Input:**
- `poll_id` (string): Doodle poll ID

**Output:**
- `votes` (array of Vote): All votes cast by participants
  - `participant_name` (string): Name of the participant
  - `option_id` (string): ID of the poll option
  - `vote_type` (VoteType): Type of vote (Yes, No, IfNeedBe)
- `poll_title` (string): Title of the poll

### close_poll

**Tool Name:** Close Doodle Poll
**Capabilities:** write
**Tags:** doodle, scheduling, poll
**Description:** Close a Doodle poll and optionally select a final option

**Input:**
- `poll_id` (string): Doodle poll ID to close
- `selected_option_id` (string, optional): Optional ID of the selected option (if finalizing a choice)

**Output:**
- `poll_id` (string): ID of the closed poll
- `closed` (boolean): Confirmation that the poll is closed
- `selected_option_id` (string, optional): ID of the selected option (if provided)

### notify_participants

**Tool Name:** Notify Doodle Participants
**Capabilities:** write
**Tags:** doodle, scheduling, poll, notification
**Description:** Send notification emails to participants of a Doodle poll

**Input:**
- `poll_id` (string): Doodle poll ID
- `emails` (array of string): Email addresses of participants to notify
- `message` (string, optional): Optional custom message to include in the notification

**Output:**
- `poll_id` (string): ID of the poll
- `notified_count` (number): Number of participants notified

## API Documentation

- **Base URL:** `https://doodle.com/api/v2.0` (default)
- **API Documentation:** Public API documentation is not currently available (Doodle's public API was discontinued)

## Testing

Run tests:

```bash
cargo test -p doodle
```

The integration includes comprehensive unit tests and integration tests using wiremock for HTTP mocking, including:
- Input validation tests
- Serialization roundtrip tests
- Wiremock-based HTTP mock tests

## Development

- **Crate:** `doodle`
- **Source:** `examples/calendars-scheduling/doodle/`
