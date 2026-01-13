# Google Calendar Integration for Operai Toolbox

Manage Google Calendar events and query availability directly from your Operai Toolbox workflows.

## Overview

- **Event Management**: Create, read, update, and cancel calendar events
- **Availability Queries**: Query free/busy information across multiple calendars
- **Flexible Scheduling**: Support for both timed events and all-day events with timezone handling

## Authentication

This integration uses **OAuth2 Bearer Token** authentication. Credentials are supplied via user-provided configuration.

### Required Credentials

- `access_token`: OAuth2 access token for Google Calendar API
- `endpoint` (optional): Custom API base URL (defaults to `https://www.googleapis.com/calendar/v3`)

## Available Tools

### list_events
**Tool Name:** List events
**Capabilities:** read
**Tags:** calendar, events, scheduling
**Description:** List events on a Google Calendar within a time range

**Input:**
- `calendar_id` (string): Calendar ID (use "primary" for the authenticated user's primary calendar)
- `time_min` (string): RFC3339 lower bound (inclusive), e.g. "2026-01-11T00:00:00Z"
- `time_max` (string): RFC3339 upper bound (exclusive), e.g. "2026-01-18T00:00:00Z"
- `q` (string, optional): Free-text search query
- `page_token` (string, optional): Page token from a previous response
- `max_results` (integer, optional): Maximum number of results (1-2500)
- `single_events` (boolean, default: true): Expand recurring events into instances

**Output:**
- `events` (array): List of calendar events with id, summary, description, location, start/end times, attendees, status, and html_link
- `next_page_token` (string, optional): Token for retrieving the next page of results

### create_event
**Tool Name:** Create event
**Capabilities:** write
**Tags:** calendar, events, scheduling
**Description:** Create an event on a Google Calendar

**Input:**
- `calendar_id` (string): Calendar ID (use "primary" for the authenticated user's primary calendar)
- `summary` (string): Event summary/title
- `description` (string, optional): Event description
- `location` (string, optional): Event location
- `start` (EventTime): Event start time/date with either `date_time` (RFC3339 timestamp) or `date` (YYYY-MM-DD for all-day events)
- `end` (EventTime): Event end time/date (must use same type as start)
- `attendees` (array of strings, optional): Attendee email addresses
- `send_updates` (string, optional): Whether to send updates to attendees: "all", "externalOnly", or "none"

**Output:**
- `event` (CalendarEvent): The created event with all fields including id, status, summary, description, location, start/end times, attendees, and html_link

### update_event
**Tool Name:** Update event
**Capabilities:** write
**Tags:** calendar, events, scheduling
**Description:** Update an event on a Google Calendar

**Input:**
- `calendar_id` (string): Calendar ID (use "primary" for the authenticated user's primary calendar)
- `event_id` (string): Event ID
- `summary` (string, optional): Updated event summary/title
- `description` (string, optional): Updated description
- `location` (string, optional): Updated location
- `start` (EventTime, optional): Updated start time/date
- `end` (EventTime, optional): Updated end time/date
- `attendees` (array of strings, optional): Updated attendee email addresses (replaces attendee list when provided)
- `send_updates` (string, optional): Whether to send updates to attendees: "all", "externalOnly", or "none"

**Output:**
- `event` (CalendarEvent): The updated event with all fields

### cancel
**Tool Name:** Cancel event
**Capabilities:** write
**Tags:** calendar, events, scheduling
**Description:** Cancel (delete) an event on a Google Calendar

**Input:**
- `calendar_id` (string): Calendar ID (use "primary" for the authenticated user's primary calendar)
- `event_id` (string): Event ID
- `send_updates` (string, optional): Whether to send updates to attendees: "all", "externalOnly", or "none"

**Output:**
- `cancelled` (boolean): True if the event was successfully cancelled

### free_busy
**Tool Name:** Free/busy
**Capabilities:** read
**Tags:** calendar, availability, scheduling
**Description:** Query free/busy availability for one or more calendars

**Input:**
- `time_min` (string): RFC3339 lower bound (inclusive), e.g. "2026-01-11T00:00:00Z"
- `time_max` (string): RFC3339 upper bound (exclusive), e.g. "2026-01-18T00:00:00Z"
- `calendar_ids` (array of strings): Calendar IDs to query
- `time_zone` (string, optional): IANA time zone name for the response

**Output:**
- `calendars` (array): List of calendars with calendar_id and busy intervals (each with start/end RFC3339 timestamps)

## API Documentation

- **Base URL:** https://www.googleapis.com/calendar/v3
- **API Documentation:** [Google Calendar API v3](https://developers.google.com/calendar/api/v3/reference)

## Testing

Run tests with:

```bash
cargo test -p brwse-google-calendar
```

The integration includes comprehensive unit tests and API integration tests using wiremock for HTTP mocking.

## Development

- **Crate:** brwse-google-calendar
- **Source:** examples/calendars-scheduling/google-calendar/src/
