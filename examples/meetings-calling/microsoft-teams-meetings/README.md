# Microsoft Teams Meetings Integration for Operai Toolbox

Brwse tool integration for Microsoft Teams Meetings using Microsoft Graph API.

## Overview

This integration enables managing Microsoft Teams online meetings through Operai Toolbox:

- Schedule, update, and retrieve Teams meetings with participant management
- Access meeting join links and details
- List meeting recordings and transcripts (with limitations - see below)
- Full OAuth2 bearer token authentication support

Primary use cases:
- Automated meeting scheduling workflows
- Meeting management across Teams environments
- Integration with calendar and scheduling systems

## Important API Limitations

### Recordings and Transcripts

The Microsoft Graph API has **important limitations** for recordings and transcripts:

- **The `list_recordings` and `list_transcripts` tools do NOT work with meetings created via this integration's `schedule_meeting` function.**
- These APIs only work with meetings created via the Outlook calendar event API (calendar-backed meetings)
- The standalone `POST /me/onlineMeetings` API used by `schedule_meeting` creates meetings that are not associated with calendar events
- According to [Microsoft's official documentation](https://learn.microsoft.com/en-us/graph/api/onlinemeeting-list-recordings):
  > "This API doesn't support meetings created using the create onlineMeeting API that are not associated with an event on the user's calendar."

**Workaround:** To use recordings/transcripts features, create meetings using the Outlook calendar API instead of the `schedule_meeting` function in this integration.

## Authentication

This integration uses OAuth2 bearer token authentication. Credentials are supplied via user credentials in the `teams` namespace.

### Required Credentials

- `access_token`: OAuth2 access token for Microsoft Graph API (required)
- `endpoint`: Custom API endpoint (optional, defaults to `https://graph.microsoft.com/v1.0`)

### Required OAuth2 Scopes

- `OnlineMeetings.ReadWrite` - For creating and updating meetings
- `OnlineMeetings.Read` - For reading meeting details
- `OnlineMeetingRecording.Read.All` - For accessing recordings (if using calendar-backed meetings)
- `OnlineMeetingTranscript.Read.All` - For accessing transcripts (if using calendar-backed meetings)

## Available Tools

### schedule_meeting
**Tool Name:** Schedule Teams Meeting
**Capabilities:** write
**Tags:** meetings, teams, microsoft-graph
**Description:** Schedule a new Microsoft Teams online meeting using Microsoft Graph

**Input:**
- `subject` (string): Meeting subject/title
- `start_date_time` (string): Meeting start date and time in ISO 8601 format (e.g., "2024-01-15T10:00:00Z")
- `end_date_time` (string): Meeting end date and time in ISO 8601 format (e.g., "2024-01-15T11:00:00Z")
- `participants` (array of string, optional): List of participant email addresses
- `allow_meeting_chat` (boolean, optional): Allow meeting chat (defaults to true)
- `allow_participants_to_enable_camera` (boolean, optional): Allow participants to enable camera (defaults to true)

**Output:**
- `meeting_id` (string): ID of the created meeting
- `join_web_url` (string): URL for joining the meeting
- `subject` (string): Meeting subject
- `start_date_time` (string): Meeting start time
- `end_date_time` (string): Meeting end time

**Note:** Meetings created via this API are standalone and not associated with calendar events. They will not appear on the user's calendar.

### update_meeting
**Tool Name:** Update Teams Meeting
**Capabilities:** write
**Tags:** meetings, teams, microsoft-graph
**Description:** Update an existing Microsoft Teams online meeting using Microsoft Graph

**Input:**
- `meeting_id` (string): Meeting ID to update
- `subject` (string, optional): New subject/title
- `start_date_time` (string, optional): New start date and time in ISO 8601 format
- `end_date_time` (string, optional): New end date and time in ISO 8601 format

**Important:** When updating `start_date_time` or `end_date_time`, you must provide both values.

**Output:**
- `meeting_id` (string): ID of the updated meeting
- `updated` (boolean): Confirmation that the meeting was updated

### get_join_link
**Tool Name:** Get Teams Meeting Join Link
**Capabilities:** read
**Tags:** meetings, teams, microsoft-graph
**Description:** Get the join web URL for a Microsoft Teams online meeting using Microsoft Graph

**Input:**
- `meeting_id` (string): Meeting ID to get the join link for

**Output:**
- `meeting_id` (string): ID of the meeting
- `join_web_url` (string): URL for joining the meeting
- `subject` (string): Meeting subject

### list_recordings
**Tool Name:** List Teams Meeting Recordings
**Capabilities:** read
**Tags:** meetings, teams, microsoft-graph
**Description:** List recordings for a Microsoft Teams online meeting using Microsoft Graph

**⚠️ Important Limitation:** This tool does NOT work with meetings created via `schedule_meeting`. Only works with calendar-backed meetings created via the Outlook calendar API.

**Input:**
- `meeting_id` (string): Meeting ID to list recordings for

**Output:**
- `recordings` (array of Recording): List of recordings
  - `id` (string): Recording ID
  - `meeting_id` (string): Meeting ID
  - `created_date_time` (string): Creation timestamp
  - `recording_content_url` (string): URL to recording content
  - `content_correlation_id` (string, optional): Content correlation ID

### list_transcripts
**Tool Name:** List Teams Meeting Transcripts
**Capabilities:** read
**Tags:** meetings, teams, microsoft-graph
**Description:** List transcripts for a Microsoft Teams online meeting using Microsoft Graph

**⚠️ Important Limitation:** This tool does NOT work with meetings created via `schedule_meeting`. Only works with calendar-backed meetings created via the Outlook calendar API.

**Input:**
- `meeting_id` (string): Meeting ID to list transcripts for

**Output:**
- `transcripts` (array of Transcript): List of transcripts
  - `id` (string): Transcript ID
  - `meeting_id` (string): Meeting ID
  - `created_date_time` (string): Creation timestamp
  - `transcript_content_url` (string): URL to transcript content
  - `content_correlation_id` (string, optional): Content correlation ID

## API Documentation

- **Base URL:** `https://graph.microsoft.com/v1.0`
- **API Documentation:**
  - [Microsoft Graph API - Online Meetings](https://learn.microsoft.com/en-us/graph/api/resources/onlinemeeting)
  - [Create Online Meeting](https://learn.microsoft.com/en-us/graph/api/application-post-onlinemeetings)
  - [Update Online Meeting](https://learn.microsoft.com/en-us/graph/api/onlinemeeting-update)
  - [List Call Recordings](https://learn.microsoft.com/en-us/graph/api/onlinemeeting-list-recordings) ⚠️ See limitations above
  - [List Call Transcripts](https://learn.microsoft.com/en-us/graph/api/onlinemeeting-list-transcripts) ⚠️ See limitations above

## Testing

Run tests:
```bash
cargo test -p microsoft-teams-meetings
```

Tests use `wiremock` for HTTP mocking and cover:
- Input validation (empty fields, whitespace)
- Successful API interactions
- Error handling scenarios

## Development

- **Crate:** `microsoft-teams-meetings`
- **Source:** `examples/meetings-calling/microsoft-teams-meetings/`

## Example Usage

### Schedule a Meeting
```json
{
  "subject": "Team Standup",
  "start_date_time": "2024-01-15T10:00:00Z",
  "end_date_time": "2024-01-15T10:30:00Z",
  "participants": ["alice@example.com", "bob@example.com"],
  "allow_meeting_chat": true,
  "allow_participants_to_enable_camera": true
}
```

### Update a Meeting
```json
{
  "meeting_id": "AAMkAGI1...",
  "subject": "Updated Title",
  "start_date_time": "2024-01-15T11:00:00Z",
  "end_date_time": "2024-01-15T11:30:00Z"
}
```

**Note:** When updating times, both `start_date_time` and `end_date_time` are required.

### Get Join Link
```json
{
  "meeting_id": "AAMkAGI1..."
}
```

## Implementation Details

- Uses `reqwest` for HTTP client
- Follows the Outlook Mail example pattern from `examples/email-inbox/outlook-mail`
- All types are defined in `types.rs`
- Comprehensive tests using `wiremock` for HTTP mocking
- Uses user credentials (`define_user_credential!`) for OAuth2 tokens
- Proper error handling with descriptive error messages
- Input validation for all required fields

## Sources

- [Create onlineMeeting - Microsoft Graph v1.0](https://learn.microsoft.com/en-us/graph/api/application-post-onlinemeetings?view=graph-rest-1.0)
- [Update onlineMeeting - Microsoft Graph v1.0](https://learn.microsoft.com/en-us/graph/api/onlinemeeting-update?view=graph-rest-1.0)
- [List recordings - Microsoft Graph v1.0](https://learn.microsoft.com/en-us/graph/api/onlinemeeting-list-recordings?view=graph-rest-1.0)
- [List transcripts - Microsoft Graph v1.0](https://learn.microsoft.com/en-us/graph/api/onlinemeeting-list-transcripts?view=graph-rest-1.0)
- [onlineMeeting: createOrGet - Microsoft Graph v1.0](https://learn.microsoft.com/en-us/graph/api/onlinemeeting-createorget?view=graph-rest-1.0)
