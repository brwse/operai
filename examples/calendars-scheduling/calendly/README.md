# Calendly Integration for Operai Toolbox

Comprehensive integration with the Calendly scheduling and booking platform, enabling management of scheduled events, invitees, and scheduling links.

## Overview

- **List and query scheduled events** (bookings) with filtering by user, organization, date range, and status
- **Create scheduling links** for event types to enable easy booking
- **Cancel bookings** or generate rescheduling URLs for invitees
- **Retrieve detailed invitee information** including contact details and booking status

**Primary Use Cases:**
- Automating workflows around Calendly bookings and cancellations
- Integrating Calendly scheduling with other business tools
- Building custom dashboards for monitoring scheduled events
- Managing event bookings programmatically

## Authentication

This integration uses OAuth2 Bearer Token authentication. Credentials are supplied via the `define_user_credential!` macro and passed through the Operai Toolbox credential system.

### Required Credentials

The following credentials are defined in `CalendlyCredential`:

- **`access_token`** (required): OAuth2 access token for Calendly API authentication
- **`endpoint`** (optional): Custom API endpoint URL (defaults to `https://api.calendly.com`)

Credentials are identified by the credential name `"calendly"` in the user credentials map.

## Available Tools

### list_bookings
**Tool Name:** List Calendly Bookings
**Capabilities:** read
**Tags:** calendar, scheduling, calendly
**Description:** List Calendly bookings (scheduled events) with optional filters

**Input:**
- `user_uri` (optional string): Filter bookings for this Calendly user URI. Example: `https://api.calendly.com/users/AAAAAAAAAAAAAAAA`
- `organization_uri` (optional string): Filter bookings for this Calendly organization URI. Example: `https://api.calendly.com/organizations/AAAAAAAAAAAAAAAA`
- `min_start_time` (optional string): Earliest event start time to include (ISO 8601 format)
- `max_start_time` (optional string): Latest event start time to include (ISO 8601 format)
- `count` (optional u32): Limit results returned (1-100, Calendly supports pagination)
- `page_token` (optional string): Pagination token or cursor from a previous call
- `status` (optional string): Event status filter (e.g., `active`, `canceled`)

**Output:**
- `bookings` (array of Booking): List of scheduled events with details including URI, name, status, start/end times, location, and metadata
- `pagination` (optional Pagination): Pagination information including count, next_page, and previous_page URLs
- `request_id` (string): Unique identifier for the request

### create_scheduling_link
**Tool Name:** Create Calendly Scheduling Link
**Capabilities:** write
**Tags:** calendar, scheduling, calendly
**Description:** Create a Calendly scheduling link for a given event type

**Input:**
- `event_type_uri` (string): Event type URI to create a scheduling link for. Example: `https://api.calendly.com/event_types/AAAAAAAAAAAAAAAA`
- `max_event_count` (optional u32): Maximum number of times the scheduling link can be used (1-100, defaults to 1)

**Output:**
- `scheduling_link_uri` (string): Scheduling link resource URI
- `booking_url` (string): URL the end-user can visit to book
- `owner` (optional string): Owner URI for the scheduling link
- `owner_type` (optional string): Type of owner (typically "EventType")
- `max_event_count` (optional u32): Maximum number of bookings allowed
- `request_id` (string): Unique identifier for the request

### cancel_reschedule
**Tool Name:** Cancel or Reschedule Calendly Booking
**Capabilities:** write
**Tags:** calendar, scheduling, calendly
**Description:** Cancel a Calendly booking or generate a rescheduling URL for an invitee

**Input:**
- `action` (CancelRescheduleAction): Whether to cancel the event in Calendly or generate a rescheduling URL for the invitee. Values: `cancel` or `reschedule`
- `scheduled_event_uuid` (optional string): Scheduled event UUID (preferred for cancellation/rescheduling)
- `scheduled_event_uri` (optional string): Scheduled event URI. UUID will be extracted from this when provided
- `invitee_uuid` (optional string): Invitee UUID (required for `reschedule`)
- `invitee_uri` (optional string): Invitee URI (required for `reschedule` if `invitee_uuid` isn't provided). Example: `https://api.calendly.com/scheduled_events/{event_uuid}/invitees/{invitee_uuid}`
- `reason` (optional string): Optional cancellation reason

**Output:**
- `action` (CancelRescheduleAction): The action that was performed
- `cancellation` (optional Cancellation): Cancellation details including URI, canceled_at timestamp, reason, and scheduled_event_uri (only for `cancel` action)
- `reschedule_url` (optional string): Invitee rescheduling URL for `reschedule` action (web URL)
- `request_id` (string): Unique identifier for the request

### fetch_invitee_info
**Tool Name:** Fetch Calendly Invitee Information
**Capabilities:** read
**Tags:** calendar, scheduling, calendly
**Description:** Fetch detailed information about a specific Calendly invitee

**Input:**
- `scheduled_event_uuid` (optional string): Scheduled event UUID
- `scheduled_event_uri` (optional string): Scheduled event URI (UUID will be extracted from this when provided)
- `invitee_uuid` (optional string): Invitee UUID
- `invitee_uri` (optional string): Invitee URI (UUIDs will be extracted from this when provided)

**Output:**
- `invitee` (InviteeInfo): Detailed invitee information including URI, email, name, first/last name, status, cancel_url, reschedule_url, and timestamps
- `request_id` (string): Unique identifier for the request

## API Documentation

- **Base URL:** `https://api.calendly.com`
- **API Documentation:** [Calendly API v2 Documentation](https://developer.calendly.com/docs/getting-started-with-the-api)

## Testing

Run tests with:

```bash
cargo test -p brwse-calendly
```

The integration includes comprehensive unit tests covering:
- Serialization and deserialization of data structures
- URI parsing helper functions
- Input validation and edge cases
- HTTP client mocking with wiremock for API interactions

## Development

- **Crate:** `brwse-calendly`
- **Source:** `examples/calendars-scheduling/calendly/`
