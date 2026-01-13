# Gmail Integration for Operai Toolbox

A comprehensive Gmail integration for Operai Toolbox that enables AI agents to read, search, send, and manage emails through the Gmail API.

## Overview

This integration provides AI agents with full access to Gmail functionality including:

- **Email Reading**: Search and retrieve individual messages with metadata and body content
- **Email Sending**: Compose and send new emails with support for CC, BCC, and custom headers
- **Email Management**: Reply to messages, apply labels, and archive emails
- **Search Capabilities**: Full Gmail search syntax support for filtering and finding messages

Primary use cases:
- AI assistants that need to read and respond to emails on behalf of users
- Automated email triage and organization workflows
- Email-based notifications and alert handling
- Integration with customer support systems

## Authentication

This integration uses OAuth2 Bearer Token authentication. Credentials are supplied via the Operai Toolbox credential system.

### Required Credentials

The following credentials are configured via the `GmailCredential` definition:

- **`access_token`** (required): OAuth2 access token for Gmail API
  - Must have appropriate Gmail API scopes
  - Should be refreshed when expired

- **`endpoint`** (optional): Custom API endpoint URL
  - Defaults to `https://gmail.googleapis.com/gmail/v1`
  - Use this for testing with mock servers or custom proxies

## Available Tools

### search_messages

**Tool Name:** Search Gmail Messages
**Capabilities:** read
**Tags:** email, gmail, google
**Description:** Search Gmail messages using Gmail search syntax

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `query` | `String` | Search query string (Gmail search syntax) |
| `max_results` | `Option<u32>` | Maximum number of results (1-100). Defaults to 10 |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `messages` | `Vec<MessageSummary>` | List of matching message summaries |

### get_message

**Tool Name:** Get Gmail Message
**Capabilities:** read
**Tags:** email, gmail, google
**Description:** Fetch a single Gmail message by ID

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `message_id` | `String` | Gmail message ID |
| `include_body` | `bool` | Include full message body content |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `message` | `MessageDetail` | Complete message details |

### send_email

**Tool Name:** Send Gmail Email
**Capabilities:** write
**Tags:** email, gmail, google
**Description:** Send an email from the authenticated Gmail account

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `to` | `Vec<String>` | Recipient email addresses (To) |
| `cc` | `Vec<String>` | CC recipients (optional) |
| `bcc` | `Vec<String>` | BCC recipients (optional) |
| `subject` | `String` | Email subject |
| `body` | `String` | Email body text |
| `reply_to` | `Option<String>` | Reply-To email address (optional) |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `message_id` | `String` | ID of the sent message |
| `thread_id` | `String` | Thread ID of the sent message |

### reply

**Tool Name:** Reply to Gmail Message
**Capabilities:** write
**Tags:** email, gmail, google
**Description:** Reply to a Gmail message

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `message_id` | `String` | Gmail message ID to reply to |
| `body` | `String` | Reply text |
| `reply_all` | `bool` | When true, reply to all recipients |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `message_id` | `String` | ID of the reply message |
| `thread_id` | `String` | Thread ID of the conversation |

### label_message

**Tool Name:** Label Gmail Message
**Capabilities:** write
**Tags:** email, gmail, google
**Description:** Add or remove labels from a Gmail message

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `message_id` | `String` | Gmail message ID |
| `add_labels` | `Vec<String>` | Label IDs to add |
| `remove_labels` | `Vec<String>` | Label IDs to remove |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `message_id` | `String` | ID of the modified message |
| `labels` | `Vec<String>` | Updated list of label IDs |

### archive_message

**Tool Name:** Archive Gmail Message
**Capabilities:** write
**Tags:** email, gmail, google
**Description:** Archive a Gmail message (remove INBOX label)

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `message_id` | `String` | Gmail message ID to archive |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `message_id` | `String` | ID of the archived message |
| `archived` | `bool` | Confirmation that the message was archived |

## API Documentation

- **Base URL:** `https://gmail.googleapis.com/gmail/v1`
- **API Documentation:** [Gmail API Reference](https://developers.google.com/gmail/api/reference/rest)

## Testing

Run tests with:

```bash
cargo test -p brwse-gmail
```

The test suite includes:
- Unit tests for email address parsing and validation
- Input validation tests for all tools
- Integration tests with wiremock for API interaction testing

## Development

- **Crate:** `brwse-gmail`
- **Source:** `examples/email-inbox/gmail/`
