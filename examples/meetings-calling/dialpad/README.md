# Dialpad Integration for Operai Toolbox

AI-powered voice and messaging integration for Dialpad's cloud communication platform.

## Overview

- **Voice calling**: Initiate outbound calls through Dialpad's phone system
- **SMS messaging**: Send text messages to contacts via Dialpad
- **Call history**: Retrieve call logs and analytics data

### Primary Use Cases

- Automating outbound calling campaigns and callbacks
- Sending SMS notifications and alerts through Dialpad
- Retrieving call analytics and history for reporting

## Authentication

This integration uses **API Key authentication** via Bearer token. Credentials are supplied through the Operai Toolbox user credential system.

### Required Credentials

- **`api_key`**: Dialpad API key for authentication (required)
- **`endpoint`**: Custom API endpoint URL (optional, defaults to `https://dialpad.com/api/v2`)

The API key is used as a Bearer token in the `Authorization` header for all HTTP requests to the Dialpad API.

## Available Tools

### place_call
**Tool Name:** Place Dialpad Call
**Capabilities:** write
**Tags:** dialpad, phone, voice
**Description:** Initiate an outbound call using Dialpad. This causes the user's Dialpad application to start the call.

**Input:**
- `user_id` (string): The Dialpad user ID to initiate the call from
- `phone_number` (string): The phone number to call (E.164 format recommended, e.g., "+15551234567")
- `caller_id` (string, optional): Optional caller ID to use for the call (must be a Dialpad number you own)

**Output:**
- `call_url` (string): The URL of the initiated call

---

### send_sms
**Tool Name:** Send Dialpad SMS
**Capabilities:** write
**Tags:** dialpad, sms, messaging
**Description:** Send an SMS message using Dialpad

**Input:**
- `target` (string): The phone number or channel to send SMS to (E.164 format recommended for numbers)
- `text` (string): The message content to send
- `user_id` (string, optional): Optional user ID to send the SMS on behalf of. If not specified, uses default

**Output:**
- `success` (boolean): Whether the SMS was successfully queued
- `sms_id` (string, optional): Unique identifier for the SMS message (if provided)

---

### fetch_call_logs
**Tool Name:** Fetch Dialpad Call Logs
**Capabilities:** read
**Tags:** dialpad, phone, history
**Description:** Retrieve call history and logs from Dialpad. Only includes calls that have already concluded.

**Input:**
- `limit` (number, optional): Maximum number of call logs to retrieve (1-1000). Defaults to 100
- `start_date` (string, optional): Start date for call logs (ISO 8601 format, e.g., "2024-01-01T00:00:00Z")
- `end_date` (string, optional): End date for call logs (ISO 8601 format)

**Output:**
- `call_logs` (array): Array of call log entries, each containing:
  - `call_id` (string): Unique identifier for the call
  - `from_number` (string, optional): Caller phone number
  - `to_number` (string, optional): Called phone number
  - `direction` (string, optional): Call direction (inbound/outbound)
  - `duration` (number, optional): Call duration in seconds
  - `status` (string, optional): Call status (completed/missed/etc.)
  - `start_time` (string, optional): Call start timestamp (ISO 8601)
- `has_more` (boolean, optional): Whether there are more results available

## API Documentation

- **Base URL**: `https://dialpad.com/api/v2`
- **API Documentation**: [Dialpad Developer Documentation](https://developers.dialpad.com/)

## Testing

Run tests:
```bash
cargo test -p dialpad
```

The integration includes comprehensive unit and integration tests using `wiremock` for HTTP mocking, covering:
- Input validation for all tool parameters
- Serialization roundtrip tests
- API error handling
- Full request/response cycles

## Development

- **Crate**: `dialpad`
- **Source**: `examples/meetings-calling/dialpad/src/`
