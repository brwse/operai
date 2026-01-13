# Proton Mail Integration for Operai Toolbox

A reference implementation for Proton Mail integration that enables searching, reading, sending, and managing email messages through the Operai Toolbox framework.

## Overview

This integration provides a comprehensive interface to Proton Mail's email capabilities within Operai Toolbox:

- **Search & Retrieve**: Search messages by keyword and fetch full message details including subject, sender, recipients, and body content
- **Send & Reply**: Compose and send new emails, or reply to existing messages with support for reply-all
- **Message Organization**: Move messages between folders and apply custom labels for organization
- **Full Metadata Access**: Access message metadata including timestamps, size, read/unread status, and starred status

> **Note**: This is a reference implementation. Proton Mail does not provide an official public REST API, and this implementation follows patterns from unofficial community APIs. Actual usage would need to be adapted based on real Proton Mail API access.

## Authentication

This integration uses OAuth2 Bearer Token authentication. The access token is provided as a Bearer token in the HTTP Authorization header.

### Required Credentials

The integration uses the following credentials from the `define_user_credential!` macro:

- **`access_token`** (required): OAuth2 access token for authenticating with the Proton Mail API
- **`endpoint`** (optional): Custom API endpoint URL. Defaults to `https://mail.proton.me/api` if not provided

Credentials are supplied via the user credential context in Operai Toolbox under the `proton` namespace.

## Available Tools

### search_mail
**Tool Name:** Search Proton Mail
**Capabilities:** read
**Tags:** email, proton, proton-mail
**Description:** Search Proton Mail messages

**Input:**
- `query` (string): Search query string (searches in subject, from, to)
- `limit` (optional u32): Maximum number of results (1-100). Defaults to 50

**Output:**
- `messages` (Vec<MessageSummary>): List of matching message summaries
- `total` (i32): Total count of matching messages

---

### get_message
**Tool Name:** Get Proton Message
**Capabilities:** read
**Tags:** email, proton, proton-mail
**Description:** Fetch a single Proton Mail message by ID

**Input:**
- `message_id` (string): Proton Mail message ID

**Output:**
- `message` (Message): Full message details including body, headers, and metadata

---

### send_email
**Tool Name:** Send Proton Email
**Capabilities:** write
**Tags:** email, proton, proton-mail
**Description:** Send an email from the authenticated Proton Mail account

**Input:**
- `to` (Vec<string>): One or more "To" recipients (email addresses)
- `cc` (optional Vec<string>): Optional CC recipients (email addresses)
- `bcc` (optional Vec<string>): Optional BCC recipients (email addresses)
- `subject` (string): Email subject
- `body` (string): Email body content
- `mime_type` (optional string): MIME type ("text/plain" or "text/html"). Defaults to "text/plain"

**Output:**
- `sent` (bool): Confirmation that the email was sent
- `message_id` (string): ID of the sent message

---

### reply
**Tool Name:** Reply to Proton Message
**Capabilities:** write
**Tags:** email, proton, proton-mail
**Description:** Reply (or reply-all) to a Proton Mail message

**Input:**
- `message_id` (string): Proton Mail message ID to reply to
- `body` (string): Reply body content
- `reply_all` (optional bool): When true, reply-all instead of reply

**Output:**
- `sent` (bool): Confirmation that the reply was sent
- `message_id` (string): ID of the sent reply message

---

### move
**Tool Name:** Move Proton Message
**Capabilities:** write
**Tags:** email, proton, proton-mail
**Description:** Move a Proton Mail message to a different folder

**Input:**
- `message_id` (string): Proton Mail message ID to move
- `folder_id` (string): Destination folder/label ID (e.g., "0" for inbox, "6" for trash, "1" for drafts)

**Output:**
- `moved` (bool): Confirmation that the message was moved
- `message_id` (string): ID of the moved message
- `folder_id` (string): ID of the destination folder

---

### label
**Tool Name:** Label Proton Message
**Capabilities:** write
**Tags:** email, proton, proton-mail
**Description:** Apply a label to a Proton Mail message

**Input:**
- `message_id` (string): Proton Mail message ID to label
- `label_id` (string): Label ID to apply

**Output:**
- `labeled` (bool): Confirmation that the label was applied
- `message_id` (string): ID of the labeled message
- `label_id` (string): ID of the applied label

## API Documentation

- **Base URL**: `https://mail.proton.me/api` (default)
- **API Version**: 3 (via `x-pm-apiversion` header)
- **Authentication**: Bearer token in `Authorization` header

The integration sends additional headers for API identification:
- `x-pm-apiversion: 3`
- `x-pm-appversion: web-mail@5.0`

> **Note**: Proton Mail does not provide official public API documentation. This implementation is based on community reverse-engineering and may require updates to match actual API behavior.

## Testing

Run tests using the standard Cargo test command:

```bash
cargo test -p brwse-proton-mail
```

The test suite includes:
- Serialization roundtrip tests for data structures
- Input validation tests for all tools
- Integration tests using wiremock for HTTP mocking
- URL normalization tests

## Development

- **Crate**: `brwse-proton-mail`
- **Source**: `examples/email-inbox/proton-mail/src/`
- **Type Definitions**: See `types.rs` for complete data structure definitions including Message, MessageSummary, Recipient, and Label types
