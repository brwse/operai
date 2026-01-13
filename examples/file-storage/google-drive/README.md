# Google Drive Integration for Operai Toolbox

Comprehensive file storage and management integration for Google Drive through the Operai Toolbox.

## Overview

The Google Drive integration enables AI agents to interact with Google Drive through a comprehensive set of tools:

- **File Discovery**: Search for files using Google Drive's powerful query syntax
- **File Operations**: Upload, download, rename, and move files
- **Sharing & Permissions**: Create sharing links and manage file permissions
- **Full CRUD Support**: Complete read and write access to Drive files and metadata

### Primary Use Cases

- Automatically processing documents stored in Google Drive
- Organizing files into folders and applying batch operations
- Generating and sharing documents with specific users or publicly
- Building workflows that integrate with Google Workspace

## Authentication

This integration uses OAuth2 Bearer Token authentication with user credentials. The access token is validated and passed to the Google Drive API via the `Authorization` header.

### Required Credentials

The following user credentials are configured via the `GoogleDriveCredential` definition:

- **`access_token`** (required): OAuth2 access token for Google Drive API
- **`endpoint`** (optional): Custom API endpoint (defaults to `https://www.googleapis.com/drive/v3`)

### OAuth2 Scopes

Your access token must include one of these scopes:

- `https://www.googleapis.com/auth/drive` - Full access to Google Drive
- `https://www.googleapis.com/auth/drive.file` - Per-file access to files created by the app
- `https://www.googleapis.com/auth/drive.readonly` - Read-only access (for read-only tools)

## Available Tools

### search_files

**Tool Name:** Search Google Drive Files

**Capabilities:** read

**Tags:** file-storage, google-drive, search

**Description:** Search for files in Google Drive using query syntax

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `query` | `String` | Search query using Google Drive search syntax (e.g., `"name contains 'report'"`, `"mimeType = 'application/pdf'"`) |
| `limit` | `Option<u32>` | Maximum number of results (1-100). Defaults to 10 |
| `fields` | `Option<String>` | Fields to include in response. Defaults to common fields |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `files` | `Vec<DriveFile>` | List of matching files with metadata |
| `next_page_token` | `Option<String>` | Token for pagination if more results exist |

### download_file

**Tool Name:** Download Google Drive File

**Capabilities:** read

**Tags:** file-storage, google-drive, download

**Description:** Download a file from Google Drive by ID

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `file_id` | `String` | File ID to download |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `content` | `String` | Base64-encoded file content |
| `file_name` | `String` | Name of the downloaded file |
| `mime_type` | `String` | MIME type of the file |
| `size_bytes` | `usize` | Size of the file in bytes |

### upload_file

**Tool Name:** Upload File to Google Drive

**Capabilities:** write

**Tags:** file-storage, google-drive, upload

**Description:** Upload a file to Google Drive

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | File name |
| `content` | `String` | Base64-encoded file content |
| `mime_type` | `Option<String>` | MIME type of the file (defaults to `"application/octet-stream"`) |
| `parents` | `Vec<String>` | Parent folder IDs |
| `description` | `Option<String>` | File description |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `file_id` | `String` | ID of the uploaded file |
| `name` | `String` | Name of the uploaded file |
| `web_view_link` | `Option<String>` | Web view link for the file |

### share_file

**Tool Name:** Share Google Drive File

**Capabilities:** write

**Tags:** file-storage, google-drive, share

**Description:** Create a sharing link or permission for a Google Drive file

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `file_id` | `String` | File ID to share |
| `role` | `PermissionRole` | Permission role: `owner`, `organizer`, `fileOrganizer`, `writer`, `commenter`, or `reader` |
| `type` | `PermissionType` | Permission type: `user`, `group`, `domain`, or `anyone` |
| `email_address` | `Option<String>` | Email address for user/group permissions (required for `user` and `group` types) |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `permission_id` | `String` | ID of the created permission |
| `web_view_link` | `Option<String>` | Web view link for the shared file |

### move_file

**Tool Name:** Move Google Drive File

**Capabilities:** write

**Tags:** file-storage, google-drive, move

**Description:** Move a file to a different folder in Google Drive

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `file_id` | `String` | File ID to move |
| `destination_folder_id` | `String` | Destination folder ID |
| `remove_from_parents` | `Option<bool>` | Remove from all current parent folders. Defaults to `true` |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `file_id` | `String` | ID of the moved file |
| `parents` | `Vec<String>` | Updated list of parent folder IDs |

### rename_file

**Tool Name:** Rename Google Drive File

**Capabilities:** write

**Tags:** file-storage, google-drive, rename

**Description:** Rename a file in Google Drive

**Input:**

| Field | Type | Description |
|-------|------|-------------|
| `file_id` | `String` | File ID to rename |
| `new_name` | `String` | New file name |

**Output:**

| Field | Type | Description |
|-------|------|-------------|
| `file_id` | `String` | ID of the renamed file |
| `name` | `String` | New file name |

## API Documentation

- **Base URL:** `https://www.googleapis.com/drive/v3`
- **API Documentation:** [Google Drive API v3 Reference](https://developers.google.com/drive/api/v3/reference)
- **Search Syntax:** [Search Files in Google Drive](https://developers.google.com/drive/api/v3/search-files)
- **MIME Types:** [Supported MIME Types](https://developers.google.com/drive/api/v3/mime-types)

## Testing

Run tests with:

```bash
cargo test -p google-drive
```

The test suite includes:
- Serialization roundtrip tests for enums
- URL normalization validation
- Input validation for all tool parameters
- Integration tests with mock HTTP server

## Development

- **Crate:** `google-drive`
- **Source:** `examples/file-storage/google-drive/`
- **Type:** Dynamic library (`cdylib`) for runtime loading
