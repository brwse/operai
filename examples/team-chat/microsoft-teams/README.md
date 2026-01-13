# Microsoft Teams Integration for Operai Toolbox

Interact with Microsoft Teams through the Microsoft Graph API, enabling team collaboration, messaging, and meeting management within the Operai Toolbox platform.

## Overview

The Microsoft Teams integration provides:

- **Team and Channel Discovery**: List all teams the user is a member of and discover channels within specific teams
- **Message Management**: Post messages to channels, reply to existing messages, and read channel message history
- **Meeting Scheduling**: Create and schedule online Teams meetings with attendees

**Primary Use Cases:**
- Automated notifications and status updates to Teams channels
- Reading and monitoring channel messages for processing
- Scheduling and managing Teams meetings programmatically
- Building chatbots and automation workflows that interact with Teams

## Authentication

This integration uses OAuth2 Bearer Token authentication with the Microsoft Graph API.

### Required Credentials

The integration uses the `TeamsCredential` credential type with the following fields:

- **`access_token`** (required): OAuth2 access token for Microsoft Graph API
  - Must have appropriate permissions for Teams resources (e.g., `ChannelMessage.Send.All`, `Group.Read.All`, `OnlineMeetings.ReadWrite`)
  - Tokens can be obtained via Azure AD OAuth2 flows

- **`endpoint`** (optional): Custom Microsoft Graph API endpoint
  - Defaults to `https://graph.microsoft.com/v1.0`
  - Use this to target sovereign clouds or specific environments

## Available Tools

### list_teams_channels

**Tool Name:** List Teams and Channels

**Capabilities:** read

**Tags:** teams, microsoft-teams, microsoft-graph, chat

**Description:** List Microsoft Teams teams and channels accessible to the user

**Input:**
- `team_id` (optional string): Team ID to list channels for a specific team. If not provided, lists all teams the user is a member of
- `limit` (optional u32): Maximum number of results (1-50). Defaults to 10

**Output:**
- `teams` (array of Team): List of teams (when team_id is not provided)
  - `id` (string): Unique identifier for the team
  - `display_name` (optional string): Display name of the team
  - `description` (optional string): Description of the team
- `channels` (array of Channel): List of channels (when team_id is provided)
  - `id` (string): Unique identifier for the channel
  - `display_name` (optional string): Display name of the channel
  - `description` (optional string): Description of the channel
  - `web_url` (optional string): URL to the channel in the Teams web client

### post_message

**Tool Name:** Post Teams Message

**Capabilities:** write

**Tags:** teams, microsoft-teams, microsoft-graph, chat

**Description:** Post a message to a Microsoft Teams channel

**Input:**
- `team_id` (string): Team ID where the channel is located
- `channel_id` (string): Channel ID to post the message to
- `content` (string): Message content
- `content_type` (optional BodyContentType): Content type ("text" or "html"). Defaults to "text"

**Output:**
- `message_id` (string): Unique identifier for the created message
- `created_date_time` (optional string): ISO 8601 timestamp of message creation

### reply

**Tool Name:** Reply to Teams Message

**Capabilities:** write

**Tags:** teams, microsoft-teams, microsoft-graph, chat

**Description:** Reply to a message in a Microsoft Teams channel

**Input:**
- `team_id` (string): Team ID where the channel is located
- `channel_id` (string): Channel ID where the message exists
- `message_id` (string): Message ID to reply to
- `content` (string): Reply content
- `content_type` (optional BodyContentType): Content type ("text" or "html"). Defaults to "text"

**Output:**
- `reply_id` (string): Unique identifier for the created reply
- `created_date_time` (optional string): ISO 8601 timestamp of reply creation

### read_messages

**Tool Name:** Read Teams Messages

**Capabilities:** read

**Tags:** teams, microsoft-teams, microsoft-graph, chat

**Description:** Read messages from a Microsoft Teams channel

**Input:**
- `team_id` (string): Team ID where the channel is located
- `channel_id` (string): Channel ID to read messages from
- `limit` (optional u32): Maximum number of messages to retrieve (1-50). Defaults to 10

**Output:**
- `messages` (array of ChatMessage): List of messages from the channel
  - `id` (string): Unique identifier for the message
  - `created_date_time` (optional string): ISO 8601 timestamp of message creation
  - `last_modified_date_time` (optional string): ISO 8601 timestamp of last modification
  - `from` (optional IdentitySet): Sender information
    - `user` (optional Identity): User details
      - `display_name` (optional string): Display name of the user
      - `id` (optional string): Unique identifier for the user
  - `body` (optional ItemBody): Message body content
    - `content_type` (BodyContentType): Type of content ("text" or "html")
    - `content` (string): The actual message content
  - `web_url` (optional string): URL to the message in the Teams web client

### schedule_meeting

**Tool Name:** Schedule Teams Meeting

**Capabilities:** write

**Tags:** teams, microsoft-teams, microsoft-graph, meeting, calendar

**Description:** Schedule a Microsoft Teams meeting

**Input:**
- `subject` (string): Meeting subject/title
- `start_date_time` (string): Meeting start date and time (ISO 8601 format)
- `end_date_time` (string): Meeting end date and time (ISO 8601 format)
- `time_zone` (optional string): Time zone (e.g., "Pacific Standard Time"). Defaults to "UTC"
- `attendees` (array of string): List of attendee email addresses (note: not currently supported by the underlying API)

**Output:**
- `meeting_id` (string): Unique identifier for the created meeting
- `join_url` (optional string): URL for participants to join the Teams meeting

**Note:** This tool creates a standalone online meeting that is not associated with a calendar event. The meeting will not appear on the user's calendar and the `attendees` parameter is reserved for future use. To create calendar-backed meetings with attendees, use the Microsoft Graph Calendar API directly.

## API Documentation

- **Base URL:** `https://graph.microsoft.com/v1.0` (default)
- **API Documentation:** [Microsoft Graph API Documentation](https://learn.microsoft.com/en-us/graph/api/resources/teams-api-overview)

## Testing

Run tests with cargo:

```bash
cd examples/team-chat/microsoft-teams
cargo test
```

The test suite includes:
- Serialization roundtrip tests for enums
- Input validation tests (empty strings, limit ranges)
- Integration tests with mock server (wiremock) for all tools
- Error handling tests

## Development

- **Crate:** `brwse-microsoft-teams`
- **Source:** `examples/team-chat/microsoft-teams/`
- **Protocol:** Microsoft Graph API v1.0
- **Authentication:** OAuth2 Bearer Token
