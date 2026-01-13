# Dropbox Integration for Operai Toolbox

This integration provides tools for interacting with Dropbox file storage through the Dropbox API v2.

## Overview

- **Search** for files and folders by name, extension, or content across your Dropbox
- **Download** files with metadata and base64-encoded content
- **Upload** files with configurable conflict resolution modes
- **Create** shared links with visibility controls and password protection
- **Move or rename** files and folders with auto-rename support

Primary use cases:
- File management workflows in Dropbox
- Content search and retrieval
- Automated file uploads and downloads
- Sharing and collaboration workflows

## Authentication

This integration uses OAuth2 bearer tokens for authentication with the Dropbox API v2. Tokens are ephemeral and may vary per-request, so this integration uses **user credentials** (not system credentials).

Credentials are supplied via the Operai Toolbox credential system using the `define_user_credential!` macro.

### Required Credentials

- **`access_token`**: OAuth2 access token for Dropbox API (required)
- **`api_base_url`**: Custom API endpoint for API requests (optional, defaults to `https://api.dropboxapi.com`)
- **`content_base_url`**: Custom API endpoint for content requests (optional, defaults to `https://content.dropboxapi.com`)

The optional URL fields are primarily used for testing with mock servers.

## Available Tools

### search
**Tool Name:** Search Files
**Capabilities:** read
**Tags:** file-storage, dropbox, search
**Description:** Search for files and folders in Dropbox by name, extension, or content

**Input:**
- `query` (string): The search query string (e.g., filename, extension, or content)
- `path` (optional string): The path to search within (defaults to root if not specified)
- `max_results` (optional number): Maximum number of results to return (1-1000, defaults to 100)
- `file_category` (optional string): Filter by file category: "image", "document", "pdf", "spreadsheet", "audio", "video", "folder", "other"

**Output:**
- `matches` (array): List of matching files and folders with metadata
- `has_more` (boolean): Whether there are more results available
- `cursor` (optional string): Cursor for pagination (use in subsequent requests)

### download
**Tool Name:** Download File
**Capabilities:** read
**Tags:** file-storage, dropbox, download
**Description:** Download a file from Dropbox and return its content as base64

**Input:**
- `path` (string): The path to the file to download (e.g., "/Documents/report.pdf")

**Output:**
- `metadata` (object): The file metadata (name, path, size, etc.)
- `content_base64` (string): Base64-encoded file content
- `content_type` (string): The content type/MIME type of the file

### upload
**Tool Name:** Upload File
**Capabilities:** write
**Tags:** file-storage, dropbox, upload
**Description:** Upload a file to Dropbox from base64-encoded content

**Input:**
- `path` (string): The destination path in Dropbox (e.g., "/Documents/report.pdf")
- `content_base64` (string): Base64-encoded file content
- `mode` (enum): How to handle conflicts with existing files: `add` (never overwrite), `overwrite`, or `update` (only if revision matches)
- `mute` (boolean): If true, files won't trigger desktop notifications (default: false)

**Output:**
- `metadata` (object): The metadata of the uploaded file
- `rev` (string): The revision identifier for this version

### share_link
**Tool Name:** Create Shared Link
**Capabilities:** write
**Tags:** file-storage, dropbox, sharing
**Description:** Create a shared link for a file or folder in Dropbox

**Input:**
- `path` (string): The path to the file or folder to share
- `visibility` (enum): Link visibility setting: `public` (anyone with link), `team_only` (team members only), or `password` (password-protected)
- `expires` (optional string): Expiration date in ISO 8601 format
- `password` (optional string): Password for password-protected links (required if visibility is "password")

**Output:**
- `url` (string): The shared link URL
- `direct_url` (string): The direct download URL (appends ?dl=1)
- `visibility` (enum): The visibility of the link
- `expires` (optional string): The expiration date (if set)
- `is_password_protected` (boolean): Whether the link is password protected
- `metadata` (object): Metadata of the shared file/folder

### move_rename
**Tool Name:** Move or Rename
**Capabilities:** write
**Tags:** file-storage, dropbox, move, rename
**Description:** Move a file/folder to a new location or rename it

**Input:**
- `from_path` (string): The current path of the file or folder
- `to_path` (string): The new path (for move) or new name in the same folder (for rename)
- `allow_overwrite` (boolean): If true, allows overwriting an existing file at the destination (default: false)
- `autorename` (boolean): If true and moving a folder, move it along with its contents (default: true)

**Output:**
- `metadata` (object): The metadata of the file/folder at its new location
- `from_path` (string): The original path before the move/rename

## API Documentation

- **Base URL:** `https://api.dropboxapi.com` (API requests), `https://content.dropboxapi.com` (content requests)
- **API Documentation:** [Dropbox HTTP Documentation](https://www.dropbox.com/developers/documentation/http/documentation)
- **API Explorer:** [Dropbox API v2 Explorer](https://dropbox.github.io/dropbox-api-v2-explorer/)

**API Endpoints Used:**
- Search: `POST /2/files/search_v2`
- Download: `POST /2/files/download`
- Upload: `POST /2/files/upload`
- Sharing: `POST /2/sharing/create_shared_link_with_settings`
- Move: `POST /2/files/move_v2`

## Testing

Run tests:
```bash
cargo test -p dropbox
```

The integration includes comprehensive tests:
- **Unit tests:** Serialization/deserialization and input validation
- **Integration tests:** Wiremock-based HTTP mock tests for all tools
- **14 total tests** covering success and error scenarios

## Development

- **Crate:** `dropbox`
- **Source:** `examples/file-storage/dropbox/src/`

### Implementation Details

- Uses `reqwest` for HTTP calls (no official Rust SDK)
- Authentication via OAuth2 Bearer tokens in `Authorization` header
- File operations use Dropbox's upload/download style:
  - Upload: JSON args in `Dropbox-API-Arg` header, binary body
  - Download: JSON args in `Dropbox-API-Arg` header, binary response with metadata in `Dropbox-API-Result` header
- Uses user credentials (ephemeral tokens) rather than system credentials
