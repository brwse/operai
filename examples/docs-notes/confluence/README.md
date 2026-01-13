# Confluence Integration for Operai Toolbox

A comprehensive integration for Atlassian Confluence that enables searching, creating, updating, and managing pages, comments, and file attachments through the Operai Toolbox.

## Overview

The Confluence integration provides a complete toolkit for working with Confluence content within Operai Toolbox:

- **Search and Discovery**: Find pages using CQL (Confluence Query Language) with flexible filtering and pagination
- **Content Management**: Create, read, and update Confluence pages with support for multiple body formats (storage, view, ADF)
- **Collaboration**: Add comments to pages, including threaded replies to existing comments
- **File Attachments**: Upload and attach files to pages with base64 encoding support

### Primary Use Cases

- Automated documentation generation and maintenance
- Content migration between Confluence spaces or instances
- Bulk page updates and formatting operations
- Integration with AI-assisted content creation workflows
- Automated reporting and dashboard page population

## Authentication

This integration uses OAuth2 Bearer Token authentication for secure access to the Confluence REST API. Credentials are supplied through the Operai Toolbox credential system.

### Required Credentials

**Credential Type:** User Credential (per-request)

**Required Fields:**
- `access_token` (string): OAuth2 access token for Confluence API authentication

**Optional Fields:**
- `endpoint` (string): Confluence instance base URL (e.g., `https://yoursite.atlassian.net`)
  - Defaults to `https://confluence.atlassian.net/wiki/rest/api` if not specified
  - Automatically appends `/wiki/rest/api` if needed

#### Setup

1. **Obtain an OAuth2 access token for Confluence:**
   - For Confluence Cloud: Use OAuth 2.0 (3LO) or API tokens
   - For Confluence Data Center/Server: Use Personal Access Tokens (PATs)

2. **Pass the token per-request via user credentials:**
   ```json
   {
     "confluence": {
       "access_token": "your-oauth2-token-here",
       "endpoint": "https://yoursite.atlassian.net"
     }
   }
   ```

The access token must have appropriate permissions for the operations you intend to perform (read, create, update, delete, comment, attach).

## Available Tools

### search_pages

**Tool Name:** Search Confluence Pages

**Capabilities:** read

**Tags:** confluence, docs, search

**Description:** Search for Confluence pages using CQL (Confluence Query Language)

**Input:**

- `cql` (string, required): CQL (Confluence Query Language) query string. Examples: `"text ~ 'project update'"`, `"space = DEV AND type = page"`
- `limit` (uint32, optional): Maximum number of results to return (default: 25, max: 100)
- `start` (uint32, optional): Starting index for pagination (default: 0)

**Output:**

- `pages` (array of PageSummary): List of matching pages
  - `id` (string): Unique identifier of the page
  - `title` (string): Title of the page
  - `space_key` (string): Key of the space containing this page
  - `space_name` (string): Name of the space containing this page
  - `web_url` (string): Web UI URL for the page
  - `last_modified` (string): Last modification timestamp (ISO 8601)
  - `last_modifier` (string): Username of the last modifier
- `total_size` (uint32): Total number of results available
- `start` (uint32): Starting index of these results
- `limit` (uint32): Number of results returned

### get_page

**Tool Name:** Get Confluence Page

**Capabilities:** read

**Tags:** confluence, docs

**Description:** Retrieve a specific Confluence page by its ID, including content and metadata

**Input:**

- `page_id` (string, required): The unique identifier of the page to retrieve
- `include_body` (boolean, default: true): Whether to include the page body content
- `body_format` (string, optional): Body format: `"storage"` (raw), `"view"` (rendered HTML), or `"atlas_doc_format"` (ADF)

**Output:**

- `page` (PageDetails): The retrieved page details
  - `id` (string): Unique identifier of the page
  - `title` (string): Title of the page
  - `space_key` (string): Key of the space containing this page
  - `version` (uint32): Current version number
  - `body` (string, optional): Page body content (if requested)
  - `body_format` (string): Body format of the returned content
  - `web_url` (string): Web UI URL for the page
  - `api_url` (string): API URL for the page
  - `created_at` (string): Creation timestamp (ISO 8601)
  - `updated_at` (string): Last modification timestamp (ISO 8601)
  - `created_by` (string): Username of the page creator
  - `updated_by` (string): Username of the last modifier
  - `labels` (array of Label): Labels attached to the page
    - `name` (string): The label name
    - `prefix` (string): Label prefix (e.g., "global", "my")
  - `parent_id` (string, optional): Parent page ID (if not a root page)

### create_page

**Tool Name:** Create Confluence Page

**Capabilities:** write

**Tags:** confluence, docs

**Description:** Create a new Confluence page in a specified space

**Input:**

- `space_key` (string, required): Key of the space where the page will be created
- `title` (string, required): Title of the new page
- `body` (string, required): Body content of the page in storage format (XHTML-based)
- `parent_id` (string, optional): Parent page ID (optional, creates as child page)
- `labels` (array of string, default: []): Labels to attach to the page

**Output:**

- `page_id` (string): The ID of the newly created page
- `title` (string): The title of the created page
- `version` (uint32): The version number (always 1 for new pages)
- `web_url` (string): Web UI URL for the new page
- `api_url` (string): API URL for the new page

### update_page

**Tool Name:** Update Confluence Page

**Capabilities:** write

**Tags:** confluence, docs

**Description:** Update an existing Confluence page's title and/or content

**Input:**

- `page_id` (string, required): The ID of the page to update
- `title` (string, optional): New title for the page (optional, keeps existing if not provided)
- `body` (string, optional): New body content in storage format (optional, keeps existing if not provided)
- `current_version` (uint32, required): Current version number (required for optimistic locking)
- `version_message` (string, optional): Optional version message describing the change

**Output:**

- `page_id` (string): The ID of the updated page
- `title` (string): The title of the updated page
- `version` (uint32): The new version number after the update
- `web_url` (string): Web UI URL for the page

### add_comment

**Tool Name:** Add Confluence Comment

**Capabilities:** write

**Tags:** confluence, docs, comments

**Description:** Add a comment to a Confluence page, optionally as a reply to another comment

**Input:**

- `page_id` (string, required): The ID of the page to comment on
- `body` (string, required): Comment body in storage format (XHTML-based)
- `parent_comment_id` (string, optional): Parent comment ID for threaded replies (optional)

**Output:**

- `comment_id` (string): The ID of the newly created comment
- `page_id` (string): The ID of the page the comment was added to
- `web_url` (string): Web UI URL to view the comment
- `created_at` (string): Creation timestamp (ISO 8601)

### attach_file

**Tool Name:** Attach File to Confluence Page

**Capabilities:** write

**Tags:** confluence, docs, attachments

**Description:** Upload and attach a file to a Confluence page

**Input:**

- `page_id` (string, required): The ID of the page to attach the file to
- `filename` (string, required): Name of the file (including extension)
- `content_base64` (string, required): Base64-encoded file content
- `content_type` (string, required): MIME type of the file (e.g., "application/pdf", "image/png")
- `comment` (string, optional): Optional comment describing the attachment

**Output:**

- `attachment_id` (string): The ID of the newly created attachment
- `filename` (string): The filename of the attachment
- `size_bytes` (uint64): Size of the attachment in bytes
- `content_type` (string): MIME type of the attachment
- `download_url` (string): Download URL for the attachment
- `created_at` (string): Creation timestamp (ISO 8601)

## API Documentation

- **Base URL:** `https://confluence.atlassian.net/wiki/rest/api` (default, configurable via `endpoint` credential)
- **API Documentation:** [Confluence REST API Documentation](https://developer.atlassian.com/cloud/confluence/rest/)

The integration uses the Confluence REST API and supports both Confluence Cloud instances and self-hosted Confluence Data Center instances by configuring the `endpoint` credential.

### Content Formats

Confluence supports multiple content representations:

- **storage**: XHTML-based format used internally by Confluence (default for create/update)
- **view**: Rendered HTML suitable for display
- **atlas_doc_format** (ADF): JSON-based structured content format

When creating or updating pages, use the **storage** format for maximum compatibility.

### Examples

#### Search for pages about a project
```json
{
  "cql": "text ~ 'Q1 roadmap' AND space = PROJ",
  "limit": 10
}
```

#### Get page with ADF format
```json
{
  "page_id": "123456",
  "include_body": true,
  "body_format": "atlas_doc_format"
}
```

#### Create a child page
```json
{
  "space_key": "DEV",
  "title": "Architecture Overview",
  "body": "<p>This document describes...</p>",
  "parent_id": "789012",
  "labels": ["architecture", "documentation"]
}
```

#### Update page title only
```json
{
  "page_id": "123456",
  "title": "Updated Title",
  "current_version": 5
}
```

#### Add a threaded comment reply
```json
{
  "page_id": "123456",
  "body": "<p>I agree with this approach!</p>",
  "parent_comment_id": "comment-789"
}
```

#### Attach a PDF document
```json
{
  "page_id": "123456",
  "filename": "requirements.pdf",
  "content_base64": "JVBERi0xLjQKJeLjz9...",
  "content_type": "application/pdf",
  "comment": "Final requirements document"
}
```

## Testing

Run tests with:

```bash
cargo test -p brwse-confluence
```

The test suite includes comprehensive unit tests covering:
- Credential deserialization with all fields and access token only
- Input validation and deserialization for all tools
- Output serialization
- Business logic (limit capping to 100, version incrementing, URL normalization)
- Base64 encoding/decoding
- Edge cases and error handling

All 33 tests pass successfully.

## Development

- **Crate:** `brwse-confluence`
- **Source:** `examples/docs-notes/confluence/`
- **Language:** Rust
- **Dependencies:**
  - `operai`: Core toolbox runtime and macros
  - `reqwest`: HTTP client for API requests
  - `serde`/`serde_json`: Serialization and deserialization
  - `base64`: File content encoding for attachments
  - `schemars`: JSON schema generation for tool inputs/outputs

## Build

Build the dynamic library:

```bash
cargo build -p brwse-confluence
```

The compiled `.dylib` (macOS), `.so` (Linux), or `.dll` (Windows) can be loaded by the Operai Toolbox runtime.
