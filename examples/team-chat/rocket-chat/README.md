# Rocket.Chat Integration for Operai Toolbox

Enables AI agents to interact with Rocket.Chat workspaces for team communication and collaboration.

## Overview

- **Channel Discovery**: List and browse joined channels in your Rocket.Chat workspace
- **Message Operations**: Read channel history, post new messages, and reply to threads
- **File Upload**: Upload files to channels with base64-encoded content

**Primary Use Cases:**
- Automated notifications and alerts to Rocket.Chat channels
- Reading and analyzing channel conversations
- Posting updates and responding to threads
- Sharing files and documents in channels

## Authentication

Uses Rocket.Chat REST API authentication via personal access tokens (X-Auth-Token and X-User-Id headers).

### Required Credentials

- **`auth_token`** (required): Rocket.Chat REST API auth token (sent as `X-Auth-Token`)
- **`user_id`** (required): Rocket.Chat user ID (sent as `X-User-Id`)
- **`endpoint`** (required): Base URL for your Rocket.Chat instance (e.g., `https://chat.example.com`)

### Obtaining Credentials

1. Log in to your Rocket.Chat instance
2. Go to **My Account** → **Personal Access Tokens**
3. Create a new token with appropriate permissions
4. Use your User ID (found in **My Account** → **My Info**) and the generated token

## Available Tools

### list_channels
**Tool Name:** List Rocket.Chat Channels
**Capabilities:** read
**Tags:** team-chat, rocket-chat
**Description:** List Rocket.Chat channels the authenticated user has joined

**Input:**
- `offset` (optional, u32): Number of channels to skip for pagination (>= 0)
- `count` (optional, u32): Maximum number of channels to return (1-100). Defaults to 100

**Output:**
- `channels` (Vec<Channel>): List of channels with id, name, display_name, and room_type

---

### post
**Tool Name:** Post Message to Rocket.Chat
**Capabilities:** write
**Tags:** team-chat, rocket-chat
**Description:** Post a message to a Rocket.Chat channel (by room ID or channel name)

**Input:**
- `room_id` (optional, String): Rocket.Chat room ID to post into (preferred)
- `channel` (optional, String): Rocket.Chat channel name (with or without `#`). Alternative to `room_id`
- `text` (required, String): Message text

**Output:**
- `message` (Message): Posted message with id, room_id, text, timestamp, thread_id, and user

**Note:** Must provide either `room_id` or `channel`. Channel names are automatically normalized with `#` prefix.

---

### reply
**Tool Name:** Reply to Rocket.Chat Thread
**Capabilities:** write
**Tags:** team-chat, rocket-chat
**Description:** Reply to a Rocket.Chat thread (tmid) in a room

**Input:**
- `room_id` (required, String): Rocket.Chat room ID containing the thread
- `thread_id` (required, String): Message ID to reply to (sent as `tmid`)
- `text` (required, String): Reply text

**Output:**
- `message` (Message): Reply message with id, room_id, text, timestamp, thread_id, and user

---

### read
**Tool Name:** Read Rocket.Chat Messages
**Capabilities:** read
**Tags:** team-chat, rocket-chat
**Description:** Read recent messages from a Rocket.Chat channel by room ID

**Input:**
- `room_id` (required, String): Rocket.Chat room ID to read from
- `count` (optional, u32): Number of messages to return (1-100). Defaults to 20
- `offset` (optional, u32): Number of messages to skip for pagination (>= 0)

**Output:**
- `messages` (Vec<Message>): List of messages with id, room_id, text, timestamp, thread_id, and user

---

### upload
**Tool Name:** Upload File to Rocket.Chat
**Capabilities:** write
**Tags:** team-chat, rocket-chat
**Description:** Upload a file to a Rocket.Chat room (base64-encoded payload)

**Input:**
- `room_id` (required, String): Rocket.Chat room ID to upload into
- `file_name` (required, String): File name to use for the upload
- `file_base64` (required, String): File contents as base64
- `message` (optional, String): Optional message to accompany the file (sent as `msg`)
- `description` (optional, String): Optional description for the uploaded file

**Output:**
- `message` (Message): Upload confirmation message with file attachment details

## API Documentation

- **Base URL**: User-provided via `endpoint` credential (e.g., `https://chat.example.com`)
- **API Documentation**: [Rocket.Chat REST API Documentation](https://developer.rocket.chat/reference/api/rest-api)
- **Authentication**: [Rocket.Chat Authentication API](https://developer.rocket.chat/reference/api/rest-api/endpoints/messaging/authentication)
- **Channels**: [Channels API Endpoints](https://developer.rocket.chat/reference/api/rest-api/endpoints/team-collaboration/channels)
- **Chat**: [Chat API Endpoints](https://developer.rocket.chat/reference/api/rest-api/endpoints/messaging/chat-endpoints)
- **File Upload**: [Upload Media Files to Room](https://developer.rocket.chat/apidocs/upload-media-files-to-a-room)

## Example Usage

```json
{
  "tool": "post",
  "input": {
    "channel": "#general",
    "text": "Hello from Brwse!"
  }
}
```

```json
{
  "tool": "list_channels",
  "input": {
    "count": 50,
    "offset": 0
  }
}
```

```json
{
  "tool": "read",
  "input": {
    "room_id": "GENERAL",
    "count": 20
  }
}
```

## Testing

Run tests:

```bash
cargo test -p brwse-rocket-chat
```

The integration includes comprehensive unit tests and integration tests using `wiremock` for API mocking, including:
- Input validation tests
- Serialization roundtrip tests
- HTTP mock tests with wiremock
- Edge case handling (empty strings, newlines, boundary values)

## Development

- **Crate**: `brwse-rocket-chat`
- **Source**: `examples/team-chat/rocket-chat/src/`

**Key Data Structures:**
- `Channel`: Represents a Rocket.Chat channel/room
- `Message`: Represents a message with user info and attachments
- `User`: Represents a Rocket.Chat user

**Implementation Notes:**
- Uses `reqwest` for HTTP requests to the Rocket.Chat REST API
- Authentication via custom headers (`X-Auth-Token` and `X-User-Id`)
- Supports both room IDs and channel names (with `#` prefix) for posting
- File uploads use multipart form data with base64-encoded content
- Comprehensive error handling with descriptive messages
- Full test coverage with wiremock-based HTTP mocking
