# OneDrive Integration for Operai Toolbox

Microsoft OneDrive file storage integration using the Microsoft Graph API.

## Overview

- Provides read and write access to OneDrive files and folders through Microsoft Graph API
- Supports searching, downloading, uploading, sharing, moving, and renaming files
- Enables file management operations with proper OAuth2 Bearer Token authentication

Primary use cases:
- File storage and retrieval workflows
- Document management and sharing
- Automated file organization in OneDrive

## Authentication

This integration uses OAuth2 Bearer Token authentication via Microsoft Graph API. Credentials are supplied per-request through user credentials.

### Required Credentials

- `access_token`: OAuth2 access token for Microsoft Graph API with OneDrive permissions
- `endpoint` (optional): Custom Microsoft Graph API endpoint (defaults to `https://graph.microsoft.com/v1.0`)

**Required Microsoft Graph API Scopes**:
- `Files.ReadWrite.All` or `Files.ReadWrite` - For full read/write access
- `Files.Read.All` or `Files.Read` - For read-only access

## Available Tools

### search_files

**Tool Name:** Search OneDrive Files
**Capabilities:** read
**Tags:** files, onedrive, microsoft-graph
**Description:** Search for files and folders in OneDrive using Microsoft Graph

**Input:**
- `query` (string, required): Search query string (searches file names and content)
- `limit` (number, optional): Maximum number of results (1-50). Defaults to 10

**Output:**
- `items` (array of DriveItem): List of matching drive items with metadata including id, name, size, createdDateTime, lastModifiedDateTime, webUrl, folder, file, downloadUrl, and parentReference

### download

**Tool Name:** Download OneDrive File
**Capabilities:** read
**Tags:** files, onedrive, microsoft-graph, download
**Description:** Get download URL for a file in OneDrive using Microsoft Graph

**Input:**
- `item_id_or_path` (string, required): File ID or path (e.g., "/Documents/file.txt")

**Output:**
- `download_url` (string): Direct download URL for the file
- `item` (DriveItem): Drive item metadata including id, name, size, createdDateTime, lastModifiedDateTime, webUrl, folder, file, downloadUrl, and parentReference

### upload

**Tool Name:** Upload OneDrive File
**Capabilities:** write
**Tags:** files, onedrive, microsoft-graph, upload
**Description:** Upload a file to OneDrive using Microsoft Graph (simple upload, max 4MB)

**Input:**
- `parent_folder_path` (string, required): Parent folder ID or path. Use "/" for root
- `file_name` (string, required): Name of the file to create
- `content_base64` (string, required): Base64-encoded file content

**Output:**
- `item` (DriveItem): Created drive item metadata

**Note:** For files larger than 250MB, use the OneDrive upload session API (not currently implemented).

### share_link

**Tool Name:** Create OneDrive Sharing Link
**Capabilities:** write
**Tags:** files, onedrive, microsoft-graph, sharing
**Description:** Create a sharing link for a file or folder in OneDrive using Microsoft Graph

**Input:**
- `item_id` (string, required): File or folder ID
- `link_type` (string, required): Link type: "view" or "edit"
- `scope` (string, optional): Link scope: "anonymous" or "organization". Defaults to "anonymous"

**Output:**
- `link` (SharingLink): Sharing link information including linkType, scope, and webUrl

### move

**Tool Name:** Move OneDrive Item
**Capabilities:** write
**Tags:** files, onedrive, microsoft-graph
**Description:** Move a file or folder to a different location in OneDrive using Microsoft Graph

**Input:**
- `item_id` (string, required): File or folder ID to move
- `destination_folder_id` (string, required): Destination folder ID

**Output:**
- `item` (DriveItem): Updated drive item metadata

### rename

**Tool Name:** Rename OneDrive Item
**Capabilities:** write
**Tags:** files, onedrive, microsoft-graph
**Description:** Rename a file or folder in OneDrive using Microsoft Graph

**Input:**
- `item_id` (string, required): File or folder ID to rename
- `new_name` (string, required): New name for the item

**Output:**
- `item` (DriveItem): Updated drive item metadata

## API Documentation

- Base URL: `https://graph.microsoft.com/v1.0`
- API Documentation: [Microsoft Graph OneDrive API](https://learn.microsoft.com/en-us/graph/api/resources/onedrive)
  - [DriveItem resource type](https://learn.microsoft.com/en-us/graph/api/resources/driveitem)
  - [Upload files](https://learn.microsoft.com/en-us/graph/api/driveitem-put-content)
  - [Create sharing links](https://learn.microsoft.com/en-us/graph/api/driveitem-createlink)

## Testing

Run tests:
```bash
cargo test -p brwse-onedrive
```

Tests include:
- Input validation tests for all tools
- Serialization roundtrip tests for data structures
- HTTP mock tests using wiremock
- Error handling tests

## Development

- Crate: brwse-onedrive
- Source: examples/file-storage/onedrive
