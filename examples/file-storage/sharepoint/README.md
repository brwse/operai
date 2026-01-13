# SharePoint Integration for Operai Toolbox

Provides tools for interacting with Microsoft SharePoint document libraries via the Microsoft Graph API.

## Overview

- Search for documents across SharePoint sites using keywords or KQL syntax
- Upload files to SharePoint document libraries with conflict resolution
- Manage sharing permissions and generate sharing links for documents and folders
- Create folders and organize document libraries
- Integration uses Microsoft Graph API v1.0 for all operations

Primary use cases include document management automation, content migration, permission management, and building custom integrations with SharePoint Online.

## Authentication

This integration uses OAuth2 Bearer Token authentication (User-level credentials). Credentials are supplied through the `sharepoint` user credential configuration.

### Required Credentials

- `access_token` (required): OAuth2 access token for Microsoft Graph API
- `endpoint` (optional): Custom Graph API endpoint (defaults to `https://graph.microsoft.com/v1.0`)

### Microsoft Graph Permissions

Your OAuth2 application needs the following Microsoft Graph API permissions:

- `Sites.Read.All` or `Sites.ReadWrite.All` - Read/search SharePoint sites
- `Files.ReadWrite.All` - Upload and manage files
- `Files.ReadWrite.Selected` - For specific site access
- `Sites.ReadWrite.All` - Create folders and manage permissions

## Available Tools

### search_docs
**Tool Name:** Search Documents
**Capabilities:** read
**Tags:** (none specified)
**Description:** Search for documents in SharePoint using keywords or KQL syntax

**Input:**
- `query` (string, required): The search query string (supports SharePoint/KQL syntax)
- `site_id` (string, optional): The site ID or URL to search within (optional, searches all accessible sites if omitted)
- `drive_id` (string, optional): The drive ID (document library) to search within (optional)
- `limit` (number, optional): Maximum number of results to return (default: 25, max: 100)
- `file_type` (string, optional): Filter by file type (e.g., "docx", "pdf", "xlsx")

**Output:**
- `results` (array of SearchResultItem): List of matching documents
- `total_count` (number): Total number of matches (may be more than returned)
- `query` (string): The query that was executed

### upload
**Tool Name:** Upload File
**Capabilities:** write
**Tags:** (none specified)
**Description:** Upload a file to a SharePoint document library

**Input:**
- `site_id` (string, required): The site ID where the file will be uploaded
- `drive_id` (string, required): The drive ID (document library) to upload to
- `folder_path` (string, optional): The folder path within the drive (e.g., "/Reports/2024")
- `file_name` (string, required): The name for the uploaded file
- `content_base64` (string, required): Base64-encoded file content
- `mime_type` (string, optional): MIME type of the file (optional, will be inferred if not provided)
- `conflict_behavior` (string, optional): Conflict behavior: "fail", "replace", or "rename" (default: "fail")

**Output:**
- `id` (string): Unique identifier for the uploaded file
- `name` (string): Name of the uploaded file
- `path` (string): Full path to the file in SharePoint
- `web_url` (string): URL to access the file
- `size` (number): Size of the uploaded file in bytes
- `etag` (string): ETag for the uploaded file (for concurrency control)

### set_permissions
**Tool Name:** Set Permissions
**Capabilities:** write
**Tags:** (none specified)
**Description:** Set or update sharing permissions on a SharePoint document or folder

**Input:**
- `site_id` (string, required): The site ID containing the item
- `drive_id` (string, required): The drive ID containing the item
- `item_id` (string, required): The item ID (file or folder) to set permissions on
- `grants` (array of PermissionGrant, required): Permissions to grant
- `send_notification` (boolean, optional): Whether to send notification emails to recipients (default: true)
- `message` (string, optional): Optional message to include in notification emails

**PermissionGrant fields:**
- `recipient_type` (enum): "user", "group", or "anyone"
- `recipient` (string, optional): Email address or group ID (not required for "anyone" type)
- `role` (enum): "read", "write", or "owner"
- `expires_at` (string, optional): Expiration date for the permission (ISO 8601, optional)

**Output:**
- `item_id` (string): The item ID that permissions were set on
- `permissions` (array of AppliedPermission): List of permissions that were applied
- `notifications_sent` (boolean): Whether notifications were sent

### create_folder
**Tool Name:** Create Folder
**Capabilities:** write
**Tags:** (none specified)
**Description:** Create a new folder in a SharePoint document library

**Input:**
- `site_id` (string, required): The site ID where the folder will be created
- `drive_id` (string, required): The drive ID (document library) to create the folder in
- `parent_path` (string, optional): Parent folder path (e.g., "/Reports" or empty for root)
- `folder_name` (string, required): Name for the new folder
- `description` (string, optional): Description for the folder (stored in metadata)

**Output:**
- `id` (string): Unique identifier for the created folder
- `name` (string): Name of the created folder
- `path` (string): Full path to the folder
- `web_url` (string): URL to access the folder
- `created_at` (string): Creation timestamp (ISO 8601)

### get_link
**Tool Name:** Get Sharing Link
**Capabilities:** write
**Tags:** (none specified)
**Description:** Generate a sharing link for a SharePoint document or folder

**Input:**
- `site_id` (string, required): The site ID containing the item
- `drive_id` (string, required): The drive ID containing the item
- `item_id` (string, required): The item ID (file or folder) to get/create a link for
- `link_type` (enum, required): Type of link to create ("view", "edit", or "embed")
- `scope` (enum, required): Scope of the link ("anonymous", "organization", or "existing_access")
- `password` (string, optional): Password protection for the link (optional)
- `expires_at` (string, optional): Expiration date for the link (ISO 8601, optional)
- `block_download` (boolean, optional): Whether to block download for view links (optional)

**Output:**
- `link` (string): The sharing link URL
- `link_id` (string): Unique identifier for the sharing link
- `link_type` (string): Type of the link
- `scope` (string): Scope of the link
- `has_password` (boolean): Whether the link is password protected
- `expires_at` (string, optional): Expiration date if set
- `download_blocked` (boolean): Whether download is blocked

## API Documentation

- **Base URL:** `https://graph.microsoft.com/v1.0` (configurable via `endpoint` credential)
- **API Documentation:** [Microsoft Graph SharePoint API Documentation](https://learn.microsoft.com/en-us/graph/api/resources/sharepoint?view=graph-rest-1.0)

The integration uses the following Microsoft Graph v1.0 endpoints:

- **Search:** `POST /search/query` or `GET /sites/{site-id}/drive/root/search(q='{query}')`
- **Upload:** `PUT /sites/{site-id}/drives/{drive-id}/root:{path}:/content`
- **Create Folder:** `POST /sites/{site-id}/drives/{drive-id}/root/children`
- **Create Link:** `POST /sites/{site-id}/drives/{drive-id}/items/{item-id}/createLink`
- **Set Permissions:** `POST /sites/{site-id}/drives/{drive-id}/items/{item-id}/invite`

## Testing

Run tests:
```bash
cargo test -p brwse-sharepoint
```

## Development

- **Crate:** `brwse-sharepoint`
- **Source:** `examples/file-storage/sharepoint`

## Implementation Status

**Current Status:** ✅ Production Ready

The integration has:
- ✅ Complete HTTP client implementation with OAuth2 authentication
- ✅ Real Microsoft Graph API calls for all 5 tools
- ✅ Comprehensive error handling for Graph API responses
- ✅ Full test suite with serialization/validation tests
- ✅ Compiles and all tests pass

**API Endpoints Used:**
- Search: `GET /sites/{site-id}/drives/{drive-id}/root/search(q='{query}')`
- Upload: `PUT /sites/{site-id}/drives/{drive-id}/root:{path}:/content`
- Create Folder: `POST /sites/{site-id}/drives/{drive-id}/root/children`
- Create Link: `POST /sites/{site-id}/drives/{drive-id}/items/{item-id}/createLink`
- Set Permissions: `POST /sites/{site-id}/drives/{drive-id}/items/{item-id}/invite`

## References

- [Microsoft Graph SharePoint API Documentation](https://learn.microsoft.com/en-us/graph/api/resources/sharepoint?view=graph-rest-1.0)
- [SharePoint Concept Overview](https://learn.microsoft.com/en-us/graph/sharepoint-concept-overview)
- [Microsoft Graph Rust SDK (graph-rs-sdk)](https://github.com/sreeise/graph-rs-sdk)
