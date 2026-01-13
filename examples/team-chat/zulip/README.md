# Zulip Integration for Operai Toolbox

Team chat and messaging capabilities for Zulip workspaces through Operai Toolbox.

## Overview

- **List and browse streams** (channels) in Zulip workspaces
- **Send messages** to stream topics or direct messages
- **Read message history** from specific topics
- **Resolve topics** by marking them as complete

**Primary use cases:**
- Automated notifications and alerts in Zulip streams
- Reading and analyzing conversation history
- Topic management and resolution tracking
- Integrating Zulip with AI agents and workflows

## Authentication

This integration uses **HTTP Basic Authentication** with email and API key credentials.

### Required Credentials

- **`email`**: Your Zulip account email address (used as Basic Auth username)
- **`api_key`**: Zulip API key (used as Basic Auth password)
- **`endpoint`** (optional): Custom API endpoint URL (defaults to `https://chat.zulip.org/api/v1`)

**How to get your API key:**
1. Go to Settings → Your Account → API keys
2. Generate a new API key for your bot or account
3. Copy the key and your email address to configure the credential

## Available Tools

### list_streams
**Tool Name:** List Zulip Streams
**Capabilities:** read
**Tags:** zulip, team-chat, streams
**Description:** List streams (channels) in a Zulip workspace

**Input:**
- `include_public` (optional, boolean): Include all public streams. Defaults to true.
- `include_subscribed` (boolean): Include subscribed streams only. Defaults to false.

**Output:**
- `streams` (array of Stream): List of streams with:
  - `stream_id` (integer): Unique stream identifier
  - `name` (string): Stream name
  - `description` (string): Stream description
  - `is_web_public` (boolean): Whether stream is publicly accessible
  - `is_announcement_only` (boolean): Whether only admins can post

### send_message
**Tool Name:** Send Zulip Message
**Capabilities:** write
**Tags:** zulip, team-chat, messaging
**Description:** Send a message to a Zulip stream topic

**Input:**
- `type` (string): Message type - either "stream" or "direct"
- `to` (optional, string): For stream messages - stream name or ID
- `topic` (optional, string): For stream messages - topic name
- `content` (string): Message content (supports Zulip markdown formatting)

**Output:**
- `id` (integer): ID of the created message

### read_topic
**Tool Name:** Read Zulip Topic
**Capabilities:** read
**Tags:** zulip, team-chat, messaging
**Description:** Read messages from a specific topic in a Zulip stream

**Input:**
- `stream` (string): Stream name or ID
- `topic` (string): Topic name
- `limit` (optional, integer): Maximum number of messages (1-5000). Defaults to 100.

**Output:**
- `messages` (array of Message): List of messages with:
  - `id` (integer): Message ID
  - `sender_id` (integer): Sender's user ID
  - `sender_full_name` (string): Sender's display name
  - `sender_email` (string): Sender's email address
  - `timestamp` (integer): Unix timestamp of message
  - `content` (string): Message content in markdown
  - `message_type` (string): Type - "stream" or "direct"
  - `stream_id` (optional, integer): Stream ID for stream messages
  - `topic` (optional, string): Topic name for stream messages

### resolve_topic
**Tool Name:** Resolve Zulip Topic
**Capabilities:** write
**Tags:** zulip, team-chat, topics
**Description:** Mark a Zulip topic as resolved by adding checkmark prefix

**Input:**
- `stream` (string): Stream name or ID
- `topic` (string): Topic name
- `propagate_mode` (optional, string): Propagate change to all messages in topic. Defaults to "change_all".

**Output:**
- `updated` (boolean): Whether the topic was updated
- `new_topic` (string): The new topic name with checkmark prefix

## API Documentation

- **Base URL:** `https://chat.zulip.org/api/v1`
- **API Documentation:** [https://zulip.com/api/](https://zulip.com/api/)

## Testing

Run tests:
```bash
cargo test -p brwse-zulip
```

## Development

- **Crate:** `brwse-zulip`
- **Source:** `examples/team-chat/zulip`
