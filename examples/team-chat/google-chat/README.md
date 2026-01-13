# Google Chat Integration for Operai Toolbox

Interact with Google Chat spaces, messages, and users through the Operai Toolbox.

## Overview

- Lists Google Chat spaces (rooms, group chats, and direct messages) accessible to the authenticated user
- Posts messages to spaces and threads with optional user mentions
- Reads message threads and conversation history
- Uploads files as message attachments

**Primary use cases:**
- Automated notifications and alerts in Google Chat spaces
- Reading and responding to team messages
- File sharing and collaboration workflows
- Custom chatbot integrations

## Authentication

This integration uses OAuth2 Bearer Token authentication. Credentials are supplied via user credentials under the `google_chat` namespace.

### Required Credentials

- `access_token`: OAuth2 access token for Google Chat API (required)
- `endpoint` (optional): Custom API endpoint (defaults to `https://chat.googleapis.com/v1`)

### Environment Setup

For testing, you can set up credentials via your application's OAuth flow to obtain:
- `access_token`: OAuth2 access token from Google
- `endpoint` (optional): Custom endpoint for testing (defaults to `https://chat.googleapis.com/v1`)

## Available Tools

### list_spaces
**Tool Name:** List Google Chat Spaces
**Capabilities:** read
**Tags:** chat, google-chat, collaboration
**Description:** Lists all Google Chat spaces (rooms and direct messages) accessible to the authenticated user

**Input:**
- `page_size` (optional `u32`): Maximum number of spaces to return (1-1000). Defaults to 100.
- `page_token` (optional `string`): Token for the next page of results.
- `filter` (optional `string`): Optional filter (e.g., "spaceType = SPACE" or "spaceType = `DIRECT_MESSAGE`").

**Output:**
- `spaces` (array of `Space`): List of Google Chat spaces
  - `name` (string): Resource name (format: `spaces/{space}`)
  - `display_name` (optional string): Display name of the space
  - `kind` (optional `SpaceType`): Type of space (Space, GroupChat, DirectMessage)
- `next_page_token` (optional string): Token for retrieving the next page

### post_message
**Tool Name:** Post Google Chat Message
**Capabilities:** write
**Tags:** chat, google-chat, collaboration
**Description:** Posts a new message to a Google Chat space or thread

**Input:**
- `space_name` (string): The space to post the message to (e.g., "spaces/AAAA1234")
- `text` (string): The text content of the message
- `thread_name` (optional string): Optional thread to reply to. If not specified, creates a new thread.

**Output:**
- `message` (`Message`): The created message
  - `name` (string): Resource name (format: `spaces/{space}/messages/{message}`)
  - `text` (optional string): Message text content
  - `sender` (optional `User`): Message sender information
  - `create_time` (optional string): Message creation timestamp
  - `thread` (optional `Thread`): Thread information

### read_thread
**Tool Name:** Read Google Chat Thread
**Capabilities:** read
**Tags:** chat, google-chat, collaboration
**Description:** Reads messages from a Google Chat space or thread

**Input:**
- `space_name` (string): The space containing the messages (e.g., "spaces/AAAA1234")
- `thread_name` (optional string): Optional filter to list messages in a specific thread
- `page_size` (optional `u32`): Maximum number of messages to return (1-1000). Defaults to 50.
- `page_token` (optional string): Token for the next page of results

**Output:**
- `messages` (array of `Message`): List of messages in the space/thread
- `next_page_token` (optional string): Token for retrieving the next page

### mention_user
**Tool Name:** Mention User in Google Chat
**Capabilities:** write
**Tags:** chat, google-chat, collaboration
**Description:** Posts a message that mentions (notifies) a specific user in a Google Chat space

**Input:**
- `space_name` (string): The space to post the message to (e.g., "spaces/AAAA1234")
- `user_name` (string): The user to mention (user resource name, e.g., "users/123456")
- `text` (string): The message text (mention will be added automatically)
- `thread_name` (optional string): Optional thread to reply to

**Output:**
- `message` (`Message`): The created message with user mention annotation

### upload_file
**Tool Name:** Upload File to Google Chat
**Capabilities:** write
**Tags:** chat, google-chat, collaboration, upload
**Description:** Uploads a file to a Google Chat space as an attachment

**Input:**
- `space_name` (string): The space to upload the file to (e.g., "spaces/AAAA1234")
- `filename` (string): The filename for the uploaded file
- `content_base64` (string): Base64-encoded file content
- `content_type` (string): MIME type of the file (e.g., "image/png", "application/pdf")
- `message_text` (optional string): Optional message text to accompany the file
- `thread_name` (optional string): Optional thread to upload to

**Output:**
- `message` (`Message`): The created message with attachment

## API Documentation

- **Base URL:** `https://chat.googleapis.com/v1`
- **API Documentation:** [Google Chat API Reference](https://developers.google.com/chat/api/reference/rest)

## Testing

Run tests:

```bash
cargo test -p brwse-google-chat
```

The integration includes comprehensive unit and integration tests using wiremock for HTTP mocking.

Tests cover:
- Input validation (empty fields, invalid ranges)
- API request formatting
- Response parsing
- Error handling
- HTTP status codes

## Development

- **Crate:** `brwse-google-chat`
- **Source:** `examples/team-chat/google-chat/`

## Architecture

The implementation follows the established pattern from similar integrations:

- **Authentication**: Uses `define_user_credential!` macro for OAuth2 access tokens (ephemeral per-request tokens)
- **HTTP Client**: Uses `reqwest` for all API calls with bearer token authentication
- **Types**: All Google Chat API types defined in `src/types.rs` following the camelCase API convention
- **Error Handling**: Proper validation and error messages for all inputs
- **Testing**: Comprehensive tests using `wiremock` for HTTP mocking

## Dependencies

- `operai` - Core toolbox SDK
- `reqwest` - HTTP client for API calls
- `serde` - JSON serialization/deserialization
- `tokio` - Async runtime
- `base64-simd` - Fast base64 encoding/decoding for file uploads
- `urlencoding` - URL encoding for upload parameters
- `wiremock` - HTTP mocking for tests

## Status

✅ **Complete and Verified** - All 5 required tools are implemented with real HTTP calls to the Google Chat API, verified against official documentation.

## Implementation Notes

### File Upload
The file upload feature uses the Google Chat API's [media.upload endpoint](https://developers.google.com/workspace/chat/api/reference/rest/v1/media/upload), which requires:
- Upload URI: `https://chat.googleapis.com/upload/v1/{parent}/attachments:upload`
- Filename passed as a query parameter
- File data sent as binary in the request body
- Response contains `attachmentDataRef` with `resourceName` and `attachmentUploadToken`

### User Mentions
User mentions use the [Annotation](https://developers.google.com/workspace/chat/api/reference/rest/v1/spaces.messages#Annotation) format with:
- Text formatted as `<users/{user_id}> message text`
- `type: "USER_MENTION"` annotation
- `startIndex` and `length` covering the full mention text including angle brackets
- `userMention` metadata containing the user reference
