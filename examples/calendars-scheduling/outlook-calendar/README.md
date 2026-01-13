# Outlook Calendar Integration for Operai Toolbox

Manage Outlook Calendar events and query free/busy schedule information using Microsoft Graph API.

## Overview

- **List calendar events** from Outlook Calendar with optional date/time filtering
- **Create, update, and cancel** calendar events in Outlook
- **Query free/busy schedule information** for multiple users
- Support for event features like online meetings, attendees, locations, and more

Primary use cases include:
- Building calendar assistants and scheduling bots
- Integrating Outlook Calendar into AI-powered workflow automation
- Creating meeting coordination tools that work with Outlook/Office 365

## Authentication

This integration uses **OAuth2 Bearer Token** authentication via Microsoft Graph API. Credentials are supplied through user credentials in the toolbox context.

### Required Credentials

The integration uses the `OutlookCalendarCredential` definition with the following fields:

- `access_token` (required): OAuth2 access token for Microsoft Graph API
- `endpoint` (optional): Custom API endpoint (defaults to `https://graph.microsoft.com/v1.0`)

## Available Tools

### list_events
**Tool Name:** List Outlook Calendar Events
**Capabilities:** read
**Tags:** calendar, outlook, microsoft-graph
**Description:** List calendar events from Outlook Calendar using Microsoft Graph

**Input:**
- `start` (optional `string`): Start date-time filter (ISO 8601)
- `end` (optional `string`): End date-time filter (ISO 8601)
- `limit` (optional `u32`): Maximum number of results (1-1000). Defaults to 50

**Output:**
- `events` (`Vec<Event>`): Array of calendar events with fields including:
  - `id` (`string`): Event identifier
  - `subject` (optional `string`): Event title
  - `body` (optional `ItemBody`): Event body content
  - `start` (optional `DateTimeTimeZone`): Start time with timezone
  - `end` (optional `DateTimeTimeZone`): End time with timezone
  - `location` (optional `Location`): Event location
  - `attendees` (`Vec<Attendee>`): Event attendees
  - `organizer` (optional `Recipient`): Event organizer
  - `is_all_day` (optional `bool`): Whether this is an all-day event
  - `show_as` (optional `EventShowAs`): Free/busy status (free, tentative, busy, oof, workingElsewhere, unknown)
  - `sensitivity` (optional `EventSensitivity`): Privacy level (normal, personal, private, confidential)
  - `is_online_meeting` (optional `bool`): Whether this is an online meeting
  - `online_meeting_url` (optional `string`): URL for online meeting
  - `web_link` (optional `string`): Link to event in Outlook web interface

### create_event
**Tool Name:** Create Outlook Calendar Event
**Capabilities:** write
**Tags:** calendar, outlook, microsoft-graph
**Description:** Create a new calendar event in Outlook Calendar using Microsoft Graph

**Input:**
- `subject` (`string`): Event subject/title
- `body` (optional `string`): Event body content
- `body_content_type` (optional `BodyContentType`): Body content type ("text" or "html"). Defaults to "text"
- `start` (`string`): Start date-time (ISO 8601)
- `start_time_zone` (optional `string`): Start time zone (e.g., "UTC", "Pacific Standard Time"). Defaults to "UTC"
- `end` (`string`): End date-time (ISO 8601)
- `end_time_zone` (optional `string`): End time zone (e.g., "UTC", "Pacific Standard Time"). Defaults to "UTC"
- `location` (optional `string`): Location display name
- `attendees` (`Vec<string>`): Attendees to invite (email addresses)
- `is_all_day` (optional `bool`): Whether this is an all-day event
- `show_as` (optional `EventShowAs`): How the event should be shown (free, tentative, busy, oof, workingElsewhere, unknown)
- `is_online_meeting` (optional `bool`): Whether to create as an online meeting

**Output:**
- `event` (`Event`): The created event with all fields populated by Microsoft Graph

### update_event
**Tool Name:** Update Outlook Calendar Event
**Capabilities:** write
**Tags:** calendar, outlook, microsoft-graph
**Description:** Update an existing calendar event in Outlook Calendar using Microsoft Graph

**Input:**
- `event_id` (`string`): Event ID to update
- `subject` (optional `string`): New subject/title
- `body` (optional `string`): New body content
- `body_content_type` (optional `BodyContentType`): Body content type
- `start` (optional `string`): New start date-time (ISO 8601)
- `start_time_zone` (optional `string`): Start time zone
- `end` (optional `string`): New end date-time (ISO 8601)
- `end_time_zone` (optional `string`): End time zone
- `location` (optional `string`): New location

**Output:**
- `event` (`Event`): The updated event with all fields populated by Microsoft Graph

### cancel_event
**Tool Name:** Cancel Outlook Calendar Event
**Capabilities:** write
**Tags:** calendar, outlook, microsoft-graph
**Description:** Cancel a calendar event in Outlook Calendar using Microsoft Graph

**Input:**
- `event_id` (`string`): Event ID to cancel
- `comment` (optional `string`): Optional cancellation comment

**Output:**
- `cancelled` (`bool`): Confirmation that the event was cancelled

### get_free_busy
**Tool Name:** Get Outlook Calendar Free/Busy
**Capabilities:** read
**Tags:** calendar, outlook, microsoft-graph
**Description:** Get free/busy schedule information for users in Outlook Calendar using Microsoft Graph

**Input:**
- `schedules` (`Vec<string>`): Email addresses to query
- `start_time` (`string`): Start time (ISO 8601)
- `end_time` (`string`): End time (ISO 8601)
- `time_zone` (optional `string`): Time zone. Defaults to "UTC"
- `availability_view_interval` (optional `u32`): Availability view interval in minutes. Defaults to 30

**Output:**
- `schedules` (`Vec<ScheduleInformation>`): Array of schedule information with:
  - `schedule_id` (`string`): Email address for the schedule
  - `availability_view` (optional `string`): Availability view string
  - `schedule_items` (`Vec<ScheduleItem>`): Individual schedule items with status, start, and end times

## API Documentation

- **Base URL:** `https://graph.microsoft.com/v1.0` (configurable via `endpoint` credential)
- **API Documentation:** [Microsoft Graph API Calendar Documentation](https://learn.microsoft.com/en-us/graph/api/resources/calendar?view=graph-rest-1.0)

## Testing

Run tests with:

```bash
cargo test -p outlook-calendar
```

The test suite includes:
- Serialization roundtrip tests for enums
- Input validation tests
- Integration tests with mock server (using `wiremock`)
- Error handling tests

## Development

- **Crate:** `outlook-calendar`
- **Source:** `examples/calendars-scheduling/outlook-calendar`
