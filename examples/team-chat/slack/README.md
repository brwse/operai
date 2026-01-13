# Slack Integration for Operai Toolbox

Team chat integration for interacting with Slack workspaces, channels, and messages.

## Overview

- List and browse Slack channels (public and private)
- Post messages and reply to threads in channels
- Read recent message history from channels
- Upload and share files with channels

Primary use cases include team collaboration automation, message monitoring, and automated notifications within Slack workspaces.

## Authentication

This integration uses OAuth2 Bearer Token authentication. Credentials are supplied via user credentials in the Operai Toolbox runtime context.

### Required Credentials

- `access_token`: OAuth2 access token for Slack API (e.g., `xoxb-...` bot token)
- `endpoint` (optional): Custom API endpoint (defaults to `https://slack.com/api`)

## Available Tools

### list_channels
**Tool Name:** List Slack Channels
**Capabilities:** read
**Tags:** slack, team-chat, channels
**Description:** List channels in a Slack workspace

**Input:**
- `types` (optional `String`): Types of channels to include: `"public_channel"`, `"private_channel"`, or comma-separated list
- `limit` (optional `u32`): Maximum number of channels to return (1-1000). Defaults to 100.
- `exclude_archived` (`bool`): If true, exclude archived channels

**Output:**
- `channels` (`Vec<Channel>`): List of channels with id, name, is_private, is_archived, and num_members fields

### post_message
**Tool Name:** Post Slack Message
**Capabilities:** write
**Tags:** slack, team-chat, messaging
**Description:** Post a message to a Slack channel

**Input:**
- `channel` (`String`): Channel ID or name to post to
- `text` (`String`): Message text (supports Slack mrkdwn)
- `thread_ts` (optional `String`): Optional thread timestamp to reply in thread

**Output:**
- `ts` (`String`): Message timestamp
- `channel` (`String`): Channel ID where message was posted

### reply_in_thread
**Tool Name:** Reply in Slack Thread
**Capabilities:** write
**Tags:** slack, team-chat, messaging
**Description:** Reply to a message thread in Slack

**Input:**
- `channel` (`String`): Channel ID containing the thread
- `thread_ts` (`String`): Timestamp of the parent message
- `text` (`String`): Reply text

**Output:**
- `ts` (`String`): Message timestamp of the reply
- `thread_ts` (`String`): Thread timestamp of the parent message

### read_recent_messages
**Tool Name:** Read Recent Slack Messages
**Capabilities:** read
**Tags:** slack, team-chat, messaging
**Description:** Read recent messages from a Slack channel

**Input:**
- `channel` (`String`): Channel ID to read from
- `limit` (optional `u32`): Maximum number of messages (1-1000). Defaults to 20.

**Output:**
- `messages` (`Vec<Message>`): List of messages with ts, text, user, thread_ts, and reply_count fields

### upload_file
**Tool Name:** Upload File to Slack
**Capabilities:** write
**Tags:** slack, team-chat, files
**Description:** Upload a file to Slack channels

**Input:**
- `channels` (`Vec<String>`): Channels to share the file to
- `content` (`String`): Base64-encoded file content
- `filename` (`String`): Filename
- `title` (optional `String`): Optional title for the file

**Output:**
- `file` (`File`): Uploaded file details with id, name, mimetype, size, and permalink fields

## API Documentation

- Base URL: `https://slack.com/api`
- API Documentation: [Slack API Documentation](https://api.slack.com/docs)

## Testing

Run tests:
```bash
cargo test -p brwse-slack
```

## Development

- Crate: brwse-slack
- Source: examples/team-chat/slack
