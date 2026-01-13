# Box Integration for Operai Toolbox

This integration provides tools for interacting with Box cloud storage, enabling search, upload, download, folder management, sharing, and collaboration capabilities.

## Overview

- **File Management**: Search, upload, download, and organize files and folders in Box
- **Collaboration**: Create shared links and manage user permissions with role-based access control
- **Enterprise Integration**: Full OAuth2 authentication support for secure enterprise Box deployments

## Authentication

This integration uses OAuth2 Bearer token authentication. Credentials are supplied through the `BoxCredential` user credential definition.

### Required Credentials

- `access_token`: OAuth2 access token for Box API authentication (required)
- `endpoint`: Custom Box API endpoint URL (optional, defaults to `https://api.box.com/2.0`)

## Available Tools

### search
**Tool Name:** Search Box Files
**Capabilities:** read
**Tags:** file-storage, box, search
**Description:** Search for files and folders in Box

**Input:**
- `query` (String): Search query string to find files and folders
- `limit` (Option<u32>): Maximum number of results (1-200). Defaults to 30
- `file_extensions` (Vec<String>): File extensions to filter by (e.g., "pdf", "docx")

**Output:**
- `items` (Vec<SearchItem>): List of matching items with id, item_type, name, size, and path
- `total_count` (u32): Total number of search results

### download
**Tool Name:** Download Box File
**Capabilities:** read
**Tags:** file-storage, box, download
**Description:** Get a download URL for a Box file

**Input:**
- `file_id` (String): Box file ID to download

**Output:**
- `download_url` (String): Direct download URL for the file
- `file_name` (String): Name of the downloaded file

### upload
**Tool Name:** Upload Box File
**Capabilities:** write
**Tags:** file-storage, box, upload
**Description:** Upload a file to Box

**Input:**
- `parent_folder_id` (String): Parent folder ID where the file will be uploaded. Use "0" for root
- `file_name` (String): Name of the file to create
- `content_base64` (String): Base64-encoded file content

**Output:**
- `file_id` (String): ID of the uploaded file
- `file_name` (String): Name of the uploaded file

### create_folder
**Tool Name:** Create Box Folder
**Capabilities:** write
**Tags:** file-storage, box, folder
**Description:** Create a new folder in Box

**Input:**
- `name` (String): Name of the folder to create
- `parent_folder_id` (String): Parent folder ID. Use "0" for root

**Output:**
- `folder_id` (String): ID of the created folder
- `name` (String): Name of the folder

### share_link
**Tool Name:** Create Box Share Link
**Capabilities:** write
**Tags:** file-storage, box, share
**Description:** Create a shared link for a Box file or folder

**Input:**
- `item_id` (String): Item ID (file or folder) to share
- `item_type` (String): Item type: "file" or "folder"
- `access` (String): Access level: "open", "company", "collaborators"
- `password` (Option<String>): Optional password for the shared link

**Output:**
- `shared_link_url` (String): The generated shared link URL
- `access` (String): Access level of the shared link

### set_permissions
**Tool Name:** Set Box Permissions
**Capabilities:** write
**Tags:** file-storage, box, permissions
**Description:** Add a collaborator to a Box file or folder with specific permissions

**Input:**
- `item_id` (String): Item ID (file or folder) to set permissions on
- `item_type` (String): Item type: "file" or "folder"
- `user_email` (String): Email address of the user to grant access
- `role` (String): Role: "editor", "viewer", "previewer", "uploader", "previewer_uploader", "viewer_uploader", "co-owner"

**Output:**
- `collaboration_id` (String): ID of the created collaboration
- `role` (String): Role assigned to the user

## API Documentation

- **Base URL:** `https://api.box.com/2.0` (default)
- **Upload URL:** `https://upload.box.com/api/2.0` (for file uploads)
- **API Documentation:** [Box API Reference](https://developer.box.com/reference)

## Testing

Run tests:
```bash
cargo test -p box
```

Tests include:
- Input validation for all tool parameters
- Serialization/deserialization roundtrips
- HTTP integration tests using wiremock
- Error handling for API failures

## Development

- **Crate:** `box`
- **Source:** `examples/file-storage/box/`
