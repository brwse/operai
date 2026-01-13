# Zoom Integration for Operai Toolbox

Manage Zoom meetings, recordings, and transcripts through the Operai Toolbox.

## Overview

- Schedule and update Zoom meetings with configurable settings (video, waiting room, recordings)
- List and retrieve cloud recordings from Zoom
- Fetch meeting transcripts for recorded meetings
- Invite attendees to meetings and retrieve unique join URLs

**Primary use cases:** Automating meeting scheduling, managing recurring meetings, retrieving meeting transcripts for documentation, and integrating Zoom with workflow automation tools.

## Authentication

This integration uses OAuth2 Bearer token authentication. Credentials are supplied via the `define_user_credential!` macro under the `zoom` namespace.

### Required Credentials

- **`access_token`** (required): OAuth2 access token for the Zoom API. Obtain this through the [Zoom OAuth app flow](https://developers.zoom.us/docs/internal/apps/).
- **`endpoint`** (optional): Custom API endpoint URL. Defaults to `https://api.zoom.us/v2`.

**Note:** The authenticated user must be on a Pro, Business, or Enterprise tier plan for recording and transcript features to work.

## Available Tools

### schedule_meeting
**Tool Name:** Schedule Zoom Meeting
**Capabilities:** write
**Tags:** meetings, zoom, scheduling
**Description:** Schedule a new Zoom meeting with specified settings

**Input:**
- `topic` (string): Meeting topic
- `start_time` (string, optional): Start time in ISO 8601 format (e.g., "2024-01-15T10:00:00Z"). Required for scheduled meetings
- `duration` (integer, optional): Meeting duration in minutes
- `timezone` (string, optional): Timezone (e.g., "America/New_York"). Defaults to UTC if not specified
- `agenda` (string, optional): Meeting agenda/description
- `password` (string, optional): Meeting password
- `host_video` (boolean, optional): Enable host video
- `participant_video` (boolean, optional): Enable participant video
- `waiting_room` (boolean, optional): Enable waiting room
- `join_before_host` (boolean, optional): Enable join before host
- `mute_upon_entry` (boolean, optional): Mute participants upon entry
- `auto_recording` (string, optional): Auto recording setting ("local", "cloud", "none")

**Output:**
- `meeting` (object): Scheduled meeting details including ID, join URL, and settings

### update_meeting
**Tool Name:** Update Zoom Meeting
**Capabilities:** write
**Tags:** meetings, zoom, scheduling
**Description:** Update an existing Zoom meeting with new settings

**Input:**
- `meeting_id` (integer): Meeting ID to update
- `topic` (string, optional): Updated meeting topic
- `start_time` (string, optional): Updated start time in ISO 8601 format
- `duration` (integer, optional): Updated duration in minutes
- `timezone` (string, optional): Updated timezone
- `agenda` (string, optional): Updated agenda
- `waiting_room` (boolean, optional): Enable waiting room
- `join_before_host` (boolean, optional): Enable join before host
- `mute_upon_entry` (boolean, optional): Mute participants upon entry

**Output:**
- `updated` (boolean): Confirmation that the meeting was updated
- `meeting_id` (integer): The ID of the updated meeting

### list_recordings
**Tool Name:** List Zoom Recordings
**Capabilities:** read
**Tags:** meetings, zoom, recordings
**Description:** List cloud recordings for the authenticated user

**Input:**
- `from` (string, optional): Start date for filtering recordings (YYYY-MM-DD). Defaults to last 30 days
- `to` (string, optional): End date for filtering recordings (YYYY-MM-DD). Defaults to today
- `page_size` (integer, optional): Maximum number of results to return (1-300). Defaults to 30

**Output:**
- `recordings` (array): List of recording objects with metadata, file sizes, and download URLs

### invite_attendees
**Tool Name:** Invite Zoom Meeting Attendees
**Capabilities:** write
**Tags:** meetings, zoom, scheduling, invitations
**Description:** Add meeting registrants (attendees) to a Zoom meeting and get their unique join URLs

**Input:**
- `meeting_id` (integer): Meeting ID to add registrants to
- `attendees` (array of strings): Attendees to invite (email addresses)

**Output:**
- `invited_count` (integer): Number of attendees invited
- `join_urls` (array): List of attendee join information with email, join URL, and registrant ID

### fetch_transcript
**Tool Name:** Fetch Zoom Transcript
**Capabilities:** read
**Tags:** meetings, zoom, recordings, transcripts
**Description:** Fetch the transcript for a recorded Zoom meeting

**Input:**
- `meeting_id` (integer): Meeting ID to fetch transcript for

**Output:**
- `transcript` (object, optional): Transcript object with meeting ID, content, and file type. Returns null if no transcript is available

## API Documentation

- **Base URL:** `https://api.zoom.us/v2` (configurable via `endpoint` credential)
- **API Documentation:** [Zoom REST API Reference](https://developers.zoom.us/docs/api/rest/reference/zoom-api/methods/)
- **Meeting APIs:** [Zoom Meeting APIs](https://developers.zoom.us/docs/api/meetings/)

## Testing

Run tests:
```bash
cargo test -p zoom
```

The integration includes comprehensive tests:
- Input validation for all tool parameters
- Wiremock-based HTTP mock tests for all endpoints
- Error handling tests for various failure scenarios
- Serialization roundtrip tests

## Development

- **Crate:** `zoom`
- **Source:** `examples/meetings-calling/zoom/src/`
- **Type definitions:** `src/types.rs` contains all API request/response types
- **Tool implementations:** `src/lib.rs` contains all tool functions with `#[tool]` attributes
