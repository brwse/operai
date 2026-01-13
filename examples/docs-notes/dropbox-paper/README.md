# Dropbox Paper Integration for Operai Toolbox

Interact with Dropbox Paper documents to search, read, create, and comment on collaborative documents.

## Overview

This integration enables Operai Toolbox to work with Dropbox Paper documents through four main capabilities:

- Search for documents by query with filters for folder, modification date, and result limits
- Retrieve document content and metadata in Markdown or HTML format
- Create new documents or update existing ones with append or overwrite modes
- Add comments to documents or reply to existing comment threads

**Primary Use Cases:**
- Document discovery and content retrieval from Dropbox Paper
- Automated document creation and content updates
- Collaborative workflows with comment threading
- Content migration between document systems

## Authentication

This integration uses OAuth2 Bearer Token authentication. Credentials are supplied per-request through the `dropbox` user credential definition.

### Required Credentials

- **`access_token`** (required): OAuth2 access token for Dropbox API
- **`endpoint`** (optional): Custom API endpoint URL (defaults to `https://api.dropboxapi.com/2`)

**Example credential configuration:**
```json
{
  "access_token": "sl.your-dropbox-access-token",
  "endpoint": "https://api.dropboxapi.com/2"
}
```

To obtain an access token:
1. Create a Dropbox app in the [App Console](https://www.dropbox.com/developers/apps)
2. Configure OAuth2 redirect URIs
3. Complete the OAuth2 authorization flow
4. Use the access token in credential configuration

## Available Tools

### search_docs
**Tool Name:** Search Documents
**Capabilities:** read
**Tags:** docs, dropbox, paper, search
**Description:** Search for Dropbox Paper documents by query string with optional filters

**Input:**
- `query` (string): The search query string
- `limit` (optional uint32): Maximum number of results to return (1-100, default 20)
- `folder_id` (optional string): Filter by folder ID to search within a specific folder
- `modified_after` (optional string): Filter documents modified after this ISO 8601 timestamp

**Output:**
- `documents` (array of DocumentSummary): List of matching documents
  - `doc_id` (string): The unique document ID
  - `title` (string): The document title
  - `folder_id` (optional string): The folder ID containing this document
  - `last_modified` (string): When the document was last modified (ISO 8601)
  - `owner_email` (string): The owner's email address
  - `status` (string): Document status (active, archived, deleted)
- `has_more` (boolean): Whether there are more results available
- `cursor` (optional string): Cursor for pagination (if more results exist)

### get_doc
**Tool Name:** Get Document
**Capabilities:** read
**Tags:** docs, dropbox, paper
**Description:** Retrieve a Dropbox Paper document's content and metadata by ID

**Input:**
- `doc_id` (string): The unique document ID
- `export_format` (optional string): Export format: "markdown" (default) or "html"

**Output:**
- `doc_id` (string): The unique document ID
- `title` (string): The document title
- `content` (string): The document content in the requested format
- `format` (string): The export format used
- `revision` (uint64): Document revision number
- `created_at` (string): When the document was created (ISO 8601)
- `last_modified` (string): When the document was last modified (ISO 8601)
- `owner_email` (string): The owner's email address
- `sharing` (SharingSettings): Sharing settings for the document
  - `access_level` (string): Who can access: "private", "team", "public"
  - `link_sharing_enabled` (boolean): Whether link sharing is enabled
  - `share_link` (optional string): The shareable link URL (if enabled)

### upsert_doc
**Tool Name:** Create or Update Document
**Capabilities:** write
**Tags:** docs, dropbox, paper
**Description:** Create a new Dropbox Paper document or update an existing one

**Input:**
- `doc_id` (optional string): Document ID to update. If not provided, creates a new document
- `title` (optional string): The document title (required for new documents)
- `content` (string): The document content in Markdown format
- `folder_id` (optional string): Folder ID to create the document in (for new documents)
- `import_format` (optional string): Import format of the content: "markdown" (default) or "html"
- `update_mode` (optional string): Update mode: "overwrite" (default) or "append"

**Output:**
- `doc_id` (string): The document ID (newly created or existing)
- `title` (string): The document title
- `created` (boolean): Whether a new document was created (vs updated)
- `revision` (uint64): The new revision number
- `share_link` (string): Shareable link to the document

### add_comment
**Tool Name:** Add Comment
**Capabilities:** write
**Tags:** docs, dropbox, paper, comments
**Description:** Add a comment to a Dropbox Paper document or reply to an existing comment thread

**Input:**
- `doc_id` (string): The document ID to comment on
- `comment` (string): The comment text (supports Markdown formatting)
- `reply_to_comment_id` (optional string): Reply to an existing comment thread by comment ID

**Output:**
- `comment_id` (string): The newly created comment's ID
- `doc_id` (string): The document ID
- `is_reply` (boolean): Whether this is a reply to another comment
- `created_at` (string): When the comment was created (ISO 8601)

## API Documentation

- **Base API URL:** `https://api.dropboxapi.com/2`
- **Base Content URL:** `https://content.dropboxapi.com/2`
- **API Documentation:** [Dropbox API v2 Documentation](https://www.dropbox.com/developers/documentation/http/documentation)

## Testing

Run tests:
```bash
cargo test -p brwse-dropbox-paper
```

The integration includes comprehensive tests covering:
- Credential deserialization and validation
- Input validation for all tools
- Output serialization and structure
- Edge cases and error handling

## Development

- **Crate:** `brwse-dropbox-paper`
- **Source:** `examples/docs-notes/dropbox-paper/src/`
