# Discord Integration for Operai Toolbox

Enables AI agents to interact with Discord servers through a comprehensive set of tools for channel management, messaging, thread operations, and file uploads.

## Overview

- **Channel Discovery**: List and explore all channels within a Discord server (guild)
- **Messaging Capabilities**: Post messages to channels, read message history with pagination, and reply to specific messages
- **Thread Management**: Create threads from messages or standalone, archive/unarchive, lock/unlock threads
- **File Operations**: Upload files as attachments with optional message content, spoiler support, and automatic content type detection

### Primary Use Cases

- Automated bot interactions for community management
- Message monitoring and moderation workflows
- Thread organization and archival automation
- File sharing and content distribution
- Integration with external systems through Discord webhooks

## Authentication

This integration uses a **Bot Token** for authentication with the Discord API. The bot token is passed as a Bearer token in the `Authorization` header for all HTTP requests.

### Required Credentials

The following credentials are configured via the `define_system_credential!` macro under the `discord` namespace:

- **`bot_token`** (required): Discord bot token for API authentication. Obtain this from the [Discord Developer Portal](https://discord.com/developers/applications) by creating a bot application.
- **`api_base_url`** (optional): Custom API base URL. Defaults to `https://discord.com/api/v10` if not provided.

**Note**: Bot tokens should be kept secure and never exposed in client-side code or version control.

## Available Tools

### list_channels

**Tool Name:** List Discord Channels
**Capabilities:** read
**Tags:** chat, discord
**Description:** Lists all channels in a Discord server

**Input:**
- `guild_id` (string): The ID of the guild (server) to list channels for

**Output:**
- `channels` (array of Channel): The list of channels in the guild
  - `id` (string): Channel ID
  - `channel_type` (ChannelType): Type of channel (text, voice, category, etc.)
  - `name` (string, optional): Channel name
  - `guild_id` (string, optional): Guild ID
  - `position` (integer, optional): Sorting position
  - `topic` (string, optional): Channel topic
  - `nsfw` (boolean, optional): Whether the channel is NSFW
  - `parent_id` (string, optional): ID of the parent category
- `count` (integer): The total number of channels returned

### post_message

**Tool Name:** Post Discord Message
**Capabilities:** write
**Tags:** chat, discord
**Description:** Posts a message to a Discord channel, optionally as a reply to another message

**Input:**
- `channel_id` (string): The ID of the channel to post the message to
- `content` (string): The content of the message to send (max 2000 characters)
- `reply_to` (string, optional): Message ID to reply to

**Output:**
- `message` (Message): The created message
  - `id` (string): Message ID
  - `channel_id` (string): Channel ID the message was sent in
  - `author` (User): Author of the message
    - `id` (string): User ID
    - `username` (string): Username (not unique across platform)
    - `global_name` (string, optional): User's display name
    - `avatar` (string, optional): User's avatar hash
    - `bot` (boolean, optional): Whether the user is a bot
  - `content` (string): Contents of the message
  - `timestamp` (string): When the message was sent (ISO8601 timestamp)
  - `edited_timestamp` (string, optional): When the message was edited
  - `tts` (boolean): Whether this was a TTS message
  - `mention_everyone` (boolean): Whether this message mentions everyone
  - `attachments` (array of Attachment): Attachments
  - `pinned` (boolean): Whether this message is pinned

### read_messages

**Tool Name:** Read Discord Messages
**Capabilities:** read
**Tags:** chat, discord
**Description:** Retrieves recent messages from a Discord channel with optional pagination

**Input:**
- `channel_id` (string): The ID of the channel to read messages from
- `limit` (integer, default 50): Maximum number of messages to retrieve (1-100)
- `before` (string, optional): Get messages before this message ID
- `after` (string, optional): Get messages after this message ID
- `around` (string, optional): Get messages around this message ID

**Output:**
- `messages` (array of Message): The list of messages retrieved (see Message structure above)
- `count` (integer): The number of messages returned
- `has_more` (boolean): Whether there are more messages available (approximation)

### manage_threads

**Tool Name:** Manage Discord Threads
**Capabilities:** write
**Tags:** chat, discord
**Description:** Creates, archives, locks, or unlocks threads in a Discord channel

**Input:**
- `action` (ThreadAction): The action to perform on the thread
  - `create_from_message`: Create a new thread from a message
  - `create_standalone`: Create a new thread without a starter message
  - `archive`: Archive the thread
  - `unarchive`: Unarchive the thread
  - `lock`: Lock the thread (prevent new messages)
  - `unlock`: Unlock the thread
- `channel_id` (string): The ID of the channel (for create) or thread ID (for other actions)
- `message_id` (string, optional): The message ID to create a thread from (required for `create_from_message`)
- `name` (string, optional): The name for the thread (required for create actions, max 100 characters)
- `auto_archive_duration` (integer, optional): Auto-archive duration in minutes (60, 1440, 4320, or 10080)

**Output:**
- `thread` (Thread, optional): The thread that was created or modified
  - `id` (string): Thread ID
  - `name` (string): Thread name
  - `channel_type` (ChannelType): Type of channel
  - `guild_id` (string, optional): Guild ID
  - `parent_id` (string, optional): ID of the parent channel
  - `archived` (boolean, optional): Whether the thread is archived
  - `locked` (boolean, optional): Whether the thread is locked
- `success` (boolean): Whether the action was successful
- `message` (string): A message describing the result

### upload_file

**Tool Name:** Upload Discord File
**Capabilities:** write
**Tags:** chat, discord
**Description:** Uploads a file to a Discord channel with an optional message

**Input:**
- `channel_id` (string): The ID of the channel to upload the file to
- `filename` (string): The filename for the uploaded file
- `content_base64` (string): The file content as base64-encoded data
- `message` (string, optional): Optional message content to accompany the file
- `spoiler` (boolean, default false): Whether to mark the file as a spoiler

**Output:**
- `message` (Message): The message containing the uploaded file (see Message structure above)

**File Size Limit**: Maximum 25MB for free users, 500MB for boosted servers

## API Documentation

- **Base URL**: `https://discord.com/api/v10` (configurable via `api_base_url` credential)
- **API Documentation**: [Discord API Documentation](https://discord.com/developers/docs/intro)

## Testing

Run tests:

```bash
cargo test -p brwse-discord
```

The integration includes comprehensive unit tests and integration tests using `wiremock` for HTTP mocking.

## Development

- **Crate**: `brwse-discord`
- **Source**: `examples/team-chat/discord/`
