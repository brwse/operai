# IMAP/SMTP Integration for Operai Toolbox

A generic email integration that enables sending and receiving messages through any IMAP/SMTP email server.

## Overview

This integration provides a universal interface for working with email accounts via standard IMAP and SMTP protocols:

- **Read emails** from any IMAP-compatible email server
- **Send emails** through any SMTP server
- **Manage messages** with mark as read and delete operations
- **List folders** and browse mailbox hierarchies

### Primary Use Cases

- Integrating with custom or self-hosted email servers
- Working with email providers that don't have OAuth2 APIs
- Testing and development with local email servers
- Legacy email system integration

## Authentication

This integration uses username/password authentication for both IMAP (incoming mail) and SMTP (outgoing mail). Credentials are supplied as system credentials through the Operai Toolbox configuration.

### Required Credentials

#### IMAP (Incoming Mail)

- **host**: IMAP server hostname (e.g., `imap.gmail.com`, `imap.fastmail.com`)
- **port**: IMAP server port (typically `993` for SSL/TLS)
- **username**: Email address or username for authentication
- **password**: Password or app-specific password
- **use_tls** (optional): Whether to use TLS/SSL (defaults to `true`)

#### SMTP (Outgoing Mail)

- **host**: SMTP server hostname (e.g., `smtp.gmail.com`, `smtp.fastmail.com`)
- **port**: SMTP server port (typically `587` for STARTTLS or `465` for SSL)
- **username**: Email address or username for authentication
- **password**: Password or app-specific password
- **use_starttls** (optional): Whether to use STARTTLS (defaults to `true`)

> **Note:** For email providers like Gmail, Fastmail, or Outlook, you may need to generate an app-specific password rather than using your account password.

## Available Tools

### list_folders

**Tool Name:** List Folders

**Capabilities:** read

**Tags:** email, imap, folders

**Description:** Lists all IMAP folders/mailboxes available in the email account

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| pattern | `Option<String>` | Optional pattern to filter folders (e.g., "*" for all, "INBOX*" for inbox subtree). Defaults to "*" |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| folders | `Vec<Folder>` | List of folders/mailboxes |
| count | `usize` | Total count of folders returned |

**Folder Object:**

| Field | Type | Description |
|-------|------|-------------|
| name | `String` | The full path/name of the folder |
| delimiter | `Option<String>` | Folder delimiter character (e.g., "/" or ".") |
| attributes | `Vec<String>` | Folder attributes (e.g., `\Noselect`, `\HasChildren`) |
| total_messages | `Option<u32>` | Total number of messages in the folder |
| unread_count | `Option<u32>` | Number of unread messages |

---

### fetch_message

**Tool Name:** Fetch Message

**Capabilities:** read

**Tags:** email, imap, messages

**Description:** Fetches a single email message by UID from the specified folder

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| folder | `String` | The folder/mailbox to fetch from (e.g., "INBOX") |
| uid | `u32` | The message UID to fetch |
| include_body | `Option<bool>` | Whether to include the full body (default: `true`). If false, only headers are returned |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| uid | `u32` | Message UID |
| message_id | `Option<String>` | Message ID header |
| subject | `Option<String>` | Subject of the message |
| from | `Option<EmailAddress>` | Sender of the message |
| to | `Vec<EmailAddress>` | Recipients (To field) |
| cc | `Vec<EmailAddress>` | CC recipients |
| date | `Option<String>` | Date the message was sent (RFC 2822 format) |
| body_text | `Option<String>` | Plain text body content |
| body_html | `Option<String>` | HTML body content |
| attachments | `Vec<Attachment>` | List of attachments |
| flags | `Vec<String>` | Message flags (e.g., "\Seen", "\Flagged") |
| is_read | `bool` | Whether the message has been read |

**EmailAddress Object:**

| Field | Type | Description |
|-------|------|-------------|
| name | `Option<String>` | Display name (e.g., "John Doe") |
| address | `String` | Email address (e.g., "john@example.com") |

**Attachment Object:**

| Field | Type | Description |
|-------|------|-------------|
| filename | `String` | Filename of the attachment |
| content_type | `String` | MIME type (e.g., "application/pdf") |
| size | `u64` | Size in bytes |
| content_id | `Option<String>` | Content ID for inline attachments |

---

### send_message

**Tool Name:** Send Message

**Capabilities:** write

**Tags:** email, smtp, send

**Description:** Sends an email message via SMTP

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| to | `Vec<EmailAddress>` | Recipients (To field) - required |
| cc | `Option<Vec<EmailAddress>>` | CC recipients |
| bcc | `Option<Vec<EmailAddress>>` | BCC recipients |
| subject | `String` | Email subject - required |
| body_text | `Option<String>` | Plain text body |
| body_html | `Option<String>` | HTML body |
| in_reply_to | `Option<String>` | Message ID to reply to (for threading) |
| references | `Option<Vec<String>>` | References header (for threading) |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| success | `bool` | Whether the message was sent successfully |
| message_id | `String` | Generated Message-ID for the sent message |
| recipients_count | `usize` | Number of recipients the message was sent to |

---

### mark_read

**Tool Name:** Mark Read

**Capabilities:** write

**Tags:** email, imap, update

**Description:** Marks one or more messages as read or unread

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| folder | `String` | The folder containing the message |
| uids | `Vec<u32>` | The message UID(s) to mark |
| read | `Option<bool>` | Whether to mark as read (`true`) or unread (`false`). Defaults to `true` |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| success | `bool` | Whether the operation was successful |
| updated_count | `usize` | Number of messages updated |
| marked_as_read | `bool` | The new read state applied |

---

### delete_message

**Tool Name:** Delete Message

**Capabilities:** write

**Tags:** email, imap, delete

**Description:** Deletes one or more messages (moves to Trash or permanently deletes)

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| folder | `String` | The folder containing the message(s) |
| uids | `Vec<u32>` | The message UID(s) to delete |
| permanent | `Option<bool>` | If `true`, permanently delete (expunge). If `false`, move to Trash. Defaults to `false` |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| success | `bool` | Whether the operation was successful |
| deleted_count | `usize` | Number of messages deleted |
| permanently_deleted | `bool` | Whether messages were permanently deleted or moved to trash |

## API Documentation

This integration uses standard IMAP and SMTP protocols:

- **IMAP Protocol:** [RFC 3501](https://datatracker.ietf.org/doc/html/rfc3501)
- **SMTP Protocol:** [RFC 5321](https://datatracker.ietf.org/doc/html/rfc5321)

### Common Email Server Settings

**Gmail:**
- IMAP: `imap.gmail.com:993` (TLS)
- SMTP: `smtp.gmail.com:587` (STARTTLS)
- Requires App Password: [Generate here](https://myaccount.google.com/apppasswords)

**Fastmail:**
- IMAP: `imap.fastmail.com:993` (TLS)
- SMTP: `smtp.fastmail.com:587` (STARTTLS)
- Requires App Password: [Generate here](https://www.fastmail.com/settings/addresses and passwords)

**Outlook/Office365:**
- IMAP: `outlook.office365.com:993` (TLS)
- SMTP: `smtp-mail.outlook.com:587` (STARTTLS)

## Testing

Run tests:

```bash
cargo test -p imap-smtp
```

The test suite includes comprehensive coverage for:
- Tool input/output serialization
- All five tool operations
- Credential validation
- Edge cases (empty lists, missing fields)

## Development

- **Crate:** `imap-smtp`
- **Source:** `examples/email-inbox/imap-smtp/`
- **Dependencies:**
  - `async-imap` (0.10) - Async IMAP client
  - `lettre` (0.11) - Email sending with SMTP
  - `mailparse` (0.15) - Email parsing
