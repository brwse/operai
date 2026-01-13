# Cal.com Integration for Operai Toolbox

Scheduling infrastructure for managing Cal.com bookings, availability, and event types through the Operai Toolbox.

## Overview

- **Booking Management**: List, cancel, and reschedule bookings with filtering by status
- **Availability Control**: Create and manage availability schedules with timezone support
- **Booking Links**: Generate booking URLs for event types

## Authentication

This integration uses API Key authentication via the `Authorization: Bearer` header.

### Required Credentials

- `api_key`: Cal.com API key for authenticating requests (required)
- `endpoint`: Custom API endpoint URL (optional, defaults to `https://api.cal.com/v2`)

Configure credentials using the `cal_com` user credential namespace.

## Available Tools

### list_bookings
**Tool Name:** List Cal.com Bookings
**Capabilities:** read
**Tags:** calendar, scheduling, cal.com
**Description:** List bookings from Cal.com with optional filtering

**Input:**
- `status` (optional): Filter by booking status (`Accepted`, `Pending`, `Cancelled`, `Rejected`)
- `limit` (optional): Limit number of results (1-100). Defaults to 20.
- `page` (optional): Page number for pagination. Defaults to 1.

**Output:**
- `bookings`: Array of booking summaries with ID, UID, start/end times, duration, status, and event type
- `pagination` (optional): Pagination metadata including total items, current page, total pages, and next page flag

### cancel_booking
**Tool Name:** Cancel Cal.com Booking
**Capabilities:** write
**Tags:** calendar, scheduling, cal.com
**Description:** Cancel a Cal.com booking with a reason

**Input:**
- `booking_uid`: Booking UID to cancel (String)
- `cancellation_reason`: Cancellation reason (String)

**Output:**
- `cancelled`: Confirmation that booking was cancelled (bool)
- `booking_uid`: The cancelled booking UID (String)

### reschedule_booking
**Tool Name:** Reschedule Cal.com Booking
**Capabilities:** write
**Tags:** calendar, scheduling, cal.com
**Description:** Reschedule a Cal.com booking to a new time

**Input:**
- `booking_uid`: Booking UID to reschedule (String)
- `start`: New start time (ISO 8601 format) (String)
- `reschedule_reason` (optional): Reschedule reason (String)

**Output:**
- `rescheduled`: Confirmation that booking was rescheduled (bool)
- `booking_uid`: The rescheduled booking UID (String)
- `new_start`: The new start time (String)

### set_availability
**Tool Name:** Set Cal.com Availability
**Capabilities:** write
**Tags:** calendar, scheduling, cal.com, availability
**Description:** Create or update availability schedule in Cal.com

**Input:**
- `name`: Schedule name (String)
- `time_zone`: Time zone (e.g., "America/New_York") (String)
- `schedule`: Availability schedule entries - array of objects with:
  - `days`: Days of the week (e.g., "Monday", "Tuesday") (Vec<String>)
  - `start_time`: Start time (HH:MM format, e.g., "09:00") (String)
  - `end_time`: End time (HH:MM format, e.g., "17:00") (String)
- `is_default` (optional): Set as default schedule (bool)

**Output:**
- `schedule_id`: The created/updated schedule ID (i64)
- `name`: The schedule name (String)

### create_booking_link
**Tool Name:** Create Cal.com Booking Link
**Capabilities:** read
**Tags:** calendar, scheduling, cal.com
**Description:** Generate a booking link for a Cal.com event type

**Input:**
- `event_type_id`: Event type ID to create link for (i64)
- `link` (optional): Optional custom link identifier (String)

**Output:**
- `booking_url`: The generated booking URL (String)
- `event_type_id`: The event type ID (i64)

## API Documentation

- **Base URL**: `https://api.cal.com/v2`
- **API Version**: `2024-08-13`
- **API Documentation**: [Cal.com API v2 Documentation](https://cal.com/docs/api-reference/v2)

## Testing

Run tests:
```bash
cargo test -p brwse-cal-com
```

## Development

- **Crate**: brwse-cal-com
- **Source**: `examples/calendars-scheduling/cal-com/src/`
