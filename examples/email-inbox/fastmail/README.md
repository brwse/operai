# Fastmail Integration for Operai Toolbox

Email operations for Fastmail using the JMAP (JSON Meta Application Protocol) API.

## Overview

This integration enables Operai Toolbox to interact with Fastmail email accounts through the JMAP protocol:

- Search, retrieve, and send email messages
- Reply to existing messages and organize mailboxes
- Apply labels and keywords to messages for organization
- Move messages between mailboxes

Primary use cases include email triage, automated responses, message search and retrieval, and email workflow automation.

## Authentication

This integration uses API token authentication for Fastmail's JMAP API. Credentials are supplied through the Operai Toolbox credential system.

### Required Credentials

- `api_token`: Fastmail API token for authentication. Generate this in Fastmail Settings -> Passwords & Security -> Integrations.

### Optional Credentials

- `endpoint`: Custom JMAP API endpoint (defaults to `https://api.fastmail.com/jmap/api/`)
- `account_id`: JMAP account ID (defaults to `primary`)

## Available Tools

### search_mail
**Tool Name:** Search Fastmail
**Capabilities:** read
**Tags:** email, fastmail, jmap
**Description:** Search Fastmail messages using JMAP Email/query

**Input:**
- `query` (string): Search query string (searches across subject, from, to, and body)
- `limit` (optional u32): Maximum number of results (1-100). Defaults to 25.

**Output:**
- `messages` (array of EmailSummary): List of matching email summaries with id, subject, from, received_at, preview, keywords, and mailbox_ids

### get_message
**Tool Name:** Get Fastmail Message
**Capabilities:** read
**Tags:** email, fastmail, jmap
**Description:** Fetch a single Fastmail message by ID using JMAP Email/get

**Input:**
- `message_id` (string): JMAP email ID
- `include_body` (boolean): When true, include the full message body content (text and HTML)

**Output:**
- `message` (Email): Full email with id, subject, from, to, cc, bcc, received_at, sent_at, preview, text_body, html_body, keywords, and mailbox_ids

### send_email
**Tool Name:** Send Fastmail Email
**Capabilities:** write
**Tags:** email, fastmail, jmap
**Description:** Send an email from the authenticated mailbox using JMAP

**Input:**
- `to` (array of string): One or more "To" recipients (email addresses)
- `cc` (optional array of string): Optional CC recipients (email addresses)
- `bcc` (optional array of string): Optional BCC recipients (email addresses)
- `subject` (string): Email subject
- `body` (string): Email body content (plain text)
- `sent_mailbox_id` (optional string): Mailbox ID where the sent email should be stored (e.g., "Sent"). If not provided, uses default Sent mailbox.

**Output:**
- `email_id` (string): ID of the created email draft
- `submission_id` (string): ID of the email submission

### reply
**Tool Name:** Reply to Fastmail Message
**Capabilities:** write
**Tags:** email, fastmail, jmap
**Description:** Reply (or reply-all) to a Fastmail message using JMAP

**Input:**
- `message_id` (string): JMAP email ID to reply to
- `body` (string): Reply text to include
- `reply_all` (boolean): When true, reply-all instead of reply

**Output:**
- `email_id` (string): ID of the created reply email
- `submission_id` (string): ID of the email submission

### move
**Tool Name:** Move Fastmail Message
**Capabilities:** write
**Tags:** email, fastmail, jmap
**Description:** Move a message to a different Fastmail mailbox using JMAP Email/set

**Input:**
- `message_id` (string): JMAP email ID to move
- `destination_mailbox_id` (string): Destination mailbox ID

**Output:**
- `updated` (boolean): True if the message was successfully updated

### label
**Tool Name:** Label Fastmail Message
**Capabilities:** write
**Tags:** email, fastmail, jmap
**Description:** Add or remove a keyword/label on a Fastmail message using JMAP Email/set

**Input:**
- `message_id` (string): JMAP email ID to label
- `keyword` (string): Keyword/label to add or remove (e.g., "$flagged", "$seen", or custom labels)
- `add` (boolean): When true, adds the keyword. When false, removes it.

**Output:**
- `updated` (boolean): True if the message was successfully updated

## API Documentation

- **Base URL:** `https://api.fastmail.com/jmap/api/`
- **API Documentation:** [Fastmail JMAP Reference](https://jmap.io/) and [Fastmail Developer Docs](https://www.fastmail.com/developer/)

## Testing

Run tests:
```bash
cargo test -p brwse-fastmail
```

## Development

- **Crate:** `brwse-fastmail`
- **Source:** `examples/email-inbox/fastmail/`
