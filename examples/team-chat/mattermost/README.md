# Mattermost Integration for Operai Toolbox

Brwse tool integration for Mattermost team chat platform that enables messaging, channel management, and file operations.

## Overview

This integration provides tools for interacting with Mattermost team chat:
- **List channels** the user has access to, with filtering by team and channel type
- **Post messages** to channels with Markdown support and file attachments
- **Reply to messages** in threads to enable conversational workflows
- **Read messages** from channels with pagination support
- **Upload files** for attachment to messages

### Primary Use Cases

- Automated notifications and alerts to Mattermost channels
- Chatbot implementations and conversational AI agents
- Integration with external systems that need to post updates
- Automated workflows that involve file sharing and collaboration

## Authentication

This integration uses **Bearer Token authentication** with a personal access token or bot token. Credentials are supplied via system credentials.

### Required Credentials

The following credentials are configured via the `MattermostCredential`:

- `access_token` (required): Personal access token or bot token for Mattermost API authentication
- `server_url` (required): Mattermost server URL (e.g., "https://mattermost.example.com")

### Obtaining a Personal Access Token

1. Enable personal access tokens in **System Console > Integrations > Integration Management**
2. For non-admin accounts, grant permissions via **System Console > User Management > Users** → search for user → **Manage Roles** → check "Allow this account to generate personal access tokens"
3. Go to **Profile > Security > Personal Access Tokens** → **Create Token** → enter description → **Save**

## Available Tools

### list_channels

**Tool Name:** List Channels
**Capabilities:** read
**Tags:** chat, mattermost
**Description:** Lists Mattermost channels the user has access to, optionally filtered by team or channel type

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `team_id` | `Option<String>` | Team ID to list channels for. If not specified, lists channels for all teams |
| `channel_type` | `Option<ChannelTypeFilter>` | Filter by channel type: "public", "private", or "direct" |
| `limit` | `Option<u32>` | Maximum number of channels to return (default: 50, max: 200) |
| `page` | `Option<u32>` | Page number for pagination (0-indexed) |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `channels` | `Vec<Channel>` | List of channels matching the filter criteria |
| `total_count` | `u32` | Total number of channels available (for pagination) |
| `has_more` | `bool` | Whether there are more channels available |

### post

**Tool Name:** Post Message
**Capabilities:** write
**Tags:** chat, mattermost
**Description:** Posts a new message to a Mattermost channel

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `channel_id` | `String` | Channel ID to post the message to |
| `message` | `String` | Message content (supports Markdown) |
| `file_ids` | `Option<Vec<String>>` | Optional list of file IDs to attach (from previous upload) |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `message` | `Message` | The posted message with metadata |
| `permalink` | `String` | Permalink to the message |

### reply

**Tool Name:** Reply to Message
**Capabilities:** write
**Tags:** chat, mattermost
**Description:** Replies to an existing message in a Mattermost thread

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `post_id` | `String` | ID of the root post to reply to |
| `message` | `String` | Reply message content (supports Markdown) |
| `file_ids` | `Option<Vec<String>>` | Optional list of file IDs to attach |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `message` | `Message` | The reply message that was posted |
| `permalink` | `String` | Permalink to the reply |
| `thread_id` | `String` | ID of the thread (same as the root post ID) |

### read

**Tool Name:** Read Messages
**Capabilities:** read
**Tags:** chat, mattermost
**Description:** Reads recent messages from a Mattermost channel

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `channel_id` | `String` | Channel ID to read messages from |
| `limit` | `Option<u32>` | Maximum number of messages to return (default: 30, max: 200) |
| `before` | `Option<String>` | Return messages before this post ID (for pagination) |
| `after` | `Option<String>` | Return messages after this post ID (for pagination) |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `messages` | `Vec<MessageWithThread>` | List of messages in the channel with author usernames |
| `channel` | `Channel` | Channel information |
| `has_more` | `bool` | Whether there are more messages available |

### upload

**Tool Name:** Upload File
**Capabilities:** write
**Tags:** chat, mattermost, files
**Description:** Uploads a file to a Mattermost channel for later attachment to messages

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `channel_id` | `String` | Channel ID to upload the file to |
| `filename` | `String` | File name |
| `content_base64` | `String` | File content as base64-encoded string |
| `mime_type` | `Option<String>` | Optional MIME type (auto-detected if not provided) |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `file` | `FileInfo` | Information about the uploaded file |
| `file_id` | `String` | File ID to use when attaching to a message |

## API Documentation

- **Base URL:** Configured via `server_url` credential (e.g., `https://mattermost.example.com`)
- **API Version:** v4 (`/api/v4`)
- **API Documentation:** [Mattermost API v4 Reference](https://api.mattermost.com/)

Key endpoints used by this integration:
- `GET /api/v4/users/me/channels` - List channels
- `POST /api/v4/posts` - Post messages
- `GET /api/v4/posts/{post_id}` - Get post details
- `GET /api/v4/channels/{channel_id}/posts` - Read messages
- `GET /api/v4/channels/{channel_id}` - Get channel info
- `GET /api/v4/users/{user_id}` - Get user info
- `POST /api/v4/files` - Upload files

## Testing

Run tests with:

```bash
cargo test -p mattermost
```

All tests are self-contained and use mock data with the `wiremock` library.

## Development

- **Crate:** `mattermost`
- **Source:** `examples/team-chat/mattermost/`
