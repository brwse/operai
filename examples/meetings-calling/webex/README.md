# Webex Integration for Operai Toolbox

Schedule and manage Webex meetings, recordings, and transcripts.

## Overview

This integration enables AI agents to interact with the Webex Meetings API through the Operai Toolbox. It provides comprehensive meeting management capabilities including:

- **Meeting Scheduling**: Create new Webex meetings with titles, agendas, start/end times, and passwords
- **Meeting Updates**: Modify existing meeting details such as time, title, and description
- **Recording Management**: List and access meeting recordings with filtering by date and meeting
- **Transcript Access**: Fetch transcripts for meeting recordings in VTT or TXT format
- **Attendee Management**: Invite participants to meetings with co-host and presenter roles

Primary use cases include automated meeting scheduling, recording archival workflows, and meeting content analysis.

## Authentication

This integration uses OAuth2 Bearer Token authentication. Credentials are supplied via user credentials in the Operai Toolbox context.

### Required Credentials

- **access_token**: OAuth2 access token for Webex API (required)
- **endpoint**: Custom API endpoint URL (optional, defaults to `https://webexapis.com/v1`)

## Available Tools

### schedule_meeting
**Tool Name:** Schedule Webex Meeting
**Capabilities:** write
**Tags:** meetings, webex, scheduling
**Description:** Schedule a new Webex meeting

**Input:**
- `title` (string): Meeting title/subject
- `agenda` (string, optional): Meeting agenda/description
- `start` (string): Meeting start time (ISO 8601 format, e.g., "2024-01-15T10:00:00Z")
- `end` (string): Meeting end time (ISO 8601 format, e.g., "2024-01-15T11:00:00Z")
- `timezone` (string, optional): Timezone for the meeting (e.g., "America/New_York"). Defaults to UTC
- `password` (string, optional): Meeting password
- `enable_registration` (boolean, optional): Whether to enable meeting registration. Defaults to false

**Output:**
- `meeting` (Meeting): The created meeting details including ID, title, agenda, start/end times, web link, and host information

### update_meeting
**Tool Name:** Update Webex Meeting
**Capabilities:** write
**Tags:** meetings, webex, scheduling
**Description:** Update an existing Webex meeting

**Input:**
- `meeting_id` (string): Meeting ID to update
- `title` (string, optional): New meeting title/subject
- `agenda` (string, optional): New meeting agenda/description
- `start` (string, optional): New meeting start time (ISO 8601 format)
- `end` (string, optional): New meeting end time (ISO 8601 format)
- `password` (string, optional): New meeting password

**Output:**
- `meeting` (Meeting): The updated meeting details

### list_recordings
**Tool Name:** List Webex Recordings
**Capabilities:** read
**Tags:** meetings, webex, recordings
**Description:** List Webex meeting recordings

**Input:**
- `meeting_id` (string, optional): Filter recordings by meeting ID
- `from` (string, optional): Filter recordings from this date onwards (ISO 8601 format)
- `to` (string, optional): Filter recordings up to this date (ISO 8601 format)
- `limit` (number, optional): Maximum number of recordings to return (1-100). Defaults to 10

**Output:**
- `recordings` (array of Recording): List of recordings with metadata including topic, duration, size, download URLs, and status

### list_transcripts
**Tool Name:** List Webex Transcripts
**Capabilities:** read
**Tags:** meetings, webex, transcripts
**Description:** List Webex meeting transcripts

**Input:**
- `meeting_id` (string, optional): Filter transcripts by meeting ID
- `limit` (number, optional): Maximum number of transcripts to return (1-100). Defaults to 10

**Output:**
- `transcripts` (array of Transcript): List of transcripts with metadata including meeting ID, host email, meeting times, and download URLs

### get_transcript
**Tool Name:** Get Webex Transcript
**Capabilities:** read
**Tags:** meetings, webex, transcripts
**Description:** Get details for a Webex meeting transcript

**Input:**
- `transcript_id` (string): Transcript ID to fetch

**Output:**
- `transcript` (Transcript): The transcript details including meeting ID, host email, meeting times, and download URLs

### get_transcript_download_url
**Tool Name:** Get Webex Transcript Download URL
**Capabilities:** read
**Tags:** meetings, webex, transcripts
**Description:** Get download URL for a Webex meeting transcript

**Input:**
- `transcript_id` (string): Transcript ID to fetch download URL for
- `format` (string, optional): Format for the transcript ("vtt" or "txt"). Defaults to "txt"

**Output:**
- `transcript` (Transcript): The transcript details
- `download_url` (string): The download URL for the requested format

### invite_attendees
**Tool Name:** Invite Attendees to Webex Meeting
**Capabilities:** write
**Tags:** meetings, webex, invitations
**Description:** Invite attendees to a Webex meeting

**Input:**
- `meeting_id` (string): Meeting ID to invite attendees to
- `email` (string): Email address of the invitee
- `display_name` (string, optional): Display name for the invitee
- `co_host` (boolean, optional): Whether the invitee should be a co-host. Defaults to false
- `presenter` (boolean, optional): Whether the invitee should be a presenter. Defaults to false

**Output:**
- `invitee` (MeetingInvitee): The created invitee details including ID, email, display name, and role flags

## API Documentation

- **Base URL:** `https://webexapis.com/v1`
- **API Documentation:** [Webex API Documentation](https://developer.webex.com/docs/api/getting-started)

## Testing

Run tests:

```bash
cargo test -p brwse-webex
```

## Development

- **Crate:** `brwse-webex`
- **Source:** `examples/meetings-calling/webex/src/`
