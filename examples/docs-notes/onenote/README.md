# Microsoft OneNote Integration for Operai Toolbox

Interact with Microsoft OneNote notebooks, sections, and pages through the Microsoft Graph API.

## Overview

- **List** all OneNote notebooks for the authenticated user
- **Retrieve** OneNote pages with optional HTML content
- **Create** new pages with titles and HTML content in specific sections
- **Append** HTML content to existing pages
- **Search** for pages by keyword across all notebooks

### Primary Use Cases

- Documentation management and note organization
- Automated content creation in OneNote notebooks
- Integration with meeting notes and knowledge base workflows
- Content migration and synchronization

## Authentication

This integration uses **OAuth2 Bearer Token** authentication. Credentials are supplied via the `define_user_credential!` macro and passed through the Operai Toolbox runtime.

### Required Credentials

- `access_token`: OAuth2 access token for Microsoft Graph API (required)
- `endpoint`: Custom API endpoint URL (optional, defaults to `https://graph.microsoft.com/v1.0`)

#### Obtaining an Access Token

The access token must include OneNote permissions. Required Microsoft Graph API permissions:
- `Notes.Read` - Read OneNote notebooks and pages
- `Notes.ReadWrite` - Create and update OneNote content
- `Notes.Create` - Create new notebooks, sections, and pages

Obtain tokens through:
- [Azure Portal](https://portal.azure.com/) - Register an app and grant OneNote permissions
- [Microsoft Identity Platform](https://learn.microsoft.com/en-us/azure/active-directory/develop/quickstart-register-app) - OAuth 2.0 authorization code flow

#### Example Configuration

```json
{
  "onenote": {
    "access_token": "eyJ0eXAiOiJKV1QiLCJub25jZSI6...",
    "endpoint": "https://graph.microsoft.com/v1.0"
  }
}
```

## Available Tools

### list_notebooks
**Tool Name:** List OneNote Notebooks
**Capabilities:** read
**Tags:** docs, onenote, microsoft-graph
**Description:** List all OneNote notebooks for the authenticated user

**Input:**
- `limit` (optional u32): Maximum number of notebooks to return (1-100). Defaults to 20

**Output:**
- `notebooks` (array[Notebook]): Array of notebook objects
  - `id` (string): Notebook ID
  - `display_name` (optional string): Display name
  - `created_date_time` (optional string): ISO 8601 creation timestamp
  - `last_modified_date_time` (optional string): ISO 8601 modification timestamp
  - `is_default` (optional boolean): Whether this is the default notebook
  - `is_shared` (optional boolean): Whether this notebook is shared

---

### get_page
**Tool Name:** Get OneNote Page
**Capabilities:** read
**Tags:** docs, onenote, microsoft-graph
**Description:** Retrieve a OneNote page by ID, optionally including its full HTML content

**Input:**
- `page_id` (string): OneNote page ID
- `include_content` (boolean): When true, include the full HTML content of the page

**Output:**
- `page` (Page): Page object with metadata and content
  - `id` (string): Page ID
  - `title` (optional string): Page title
  - `created_date_time` (optional string): ISO 8601 creation timestamp
  - `last_modified_date_time` (optional string): ISO 8601 modification timestamp
  - `content_url` (optional string): URL to the page content
  - `content` (optional string): Full HTML content (if requested)
  - `level` (optional i32): Indentation level in the page hierarchy

---

### create_page
**Tool Name:** Create OneNote Page
**Capabilities:** write
**Tags:** docs, onenote, microsoft-graph
**Description:** Create a new OneNote page with a title and HTML content

**Input:**
- `title` (string): Title of the new page
- `content` (string): HTML content for the page body
- `section_id` (optional string): Optional section ID where the page should be created. If omitted, the page is created in the default notebook's default section

**Output:**
- `page` (Page): Created page object with metadata

---

### append_content
**Tool Name:** Append Content to OneNote Page
**Capabilities:** write
**Tags:** docs, onenote, microsoft-graph
**Description:** Append HTML content to an existing OneNote page

**Input:**
- `page_id` (string): OneNote page ID to update
- `content` (string): HTML content to append to the page

**Output:**
- `updated` (boolean): Success indicator

---

### search
**Tool Name:** Search OneNote
**Capabilities:** read
**Tags:** docs, onenote, microsoft-graph
**Description:** Search for OneNote pages by keyword

**Input:**
- `query` (string): Search query string
- `limit` (optional u32): Maximum number of results (1-100). Defaults to 20

**Output:**
- `pages` (array[PageSummary]): Array of matching page summaries
  - `id` (string): Page ID
  - `title` (optional string): Page title
  - `created_date_time` (optional string): ISO 8601 creation timestamp
  - `last_modified_date_time` (optional string): ISO 8601 modification timestamp
  - `content_url` (optional string): URL to the page content

## API Documentation

- **Base URL:** `https://graph.microsoft.com/v1.0`
- **API Documentation:** [https://learn.microsoft.com/en-us/graph/api/resources/onenote-api-overview](https://learn.microsoft.com/en-us/graph/api/resources/onenote-api-overview)

### Required Headers

All API requests include these headers:
- `Authorization: Bearer {access_token}`
- `Accept: application/json` or `text/html` (for content operations)
- `Content-Type: application/json` or `text/html` (for POST/PATCH)

### Key Endpoints

- `GET /me/onenote/notebooks` - List notebooks
- `GET /me/onenote/pages/{id}` - Get page metadata
- `GET /me/onenote/pages/{id}/content` - Get page HTML content
- `POST /me/onenote/pages` - Create a new page
- `POST /me/onenote/sections/{id}/pages` - Create page in specific section
- `PATCH /me/onenote/pages/{id}/content` - Update page content
- `GET /me/onenote/pages?$search={query}` - Search pages

## Testing

Run tests:
```bash
cargo test -p onenote
```

The test suite includes:
- Input validation tests (empty fields, invalid ranges)
- Serialization roundtrip tests for enums and structs
- HTTP mock tests using `wiremock`
- Integration tests for all tool endpoints

## Development

- **Crate:** `onenote`
- **Source:** `examples/docs-notes/onenote/`

## References

- [OneNote API Overview](https://learn.microsoft.com/en-us/graph/integrate-with-onenote)
- [Get OneNote Content](https://learn.microsoft.com/en-us/graph/onenote-get-content)
- [Create OneNote Pages](https://learn.microsoft.com/en-us/graph/onenote-create-page)
- [Update OneNote Content](https://learn.microsoft.com/en-us/graph/onenote-update-page)
- [Microsoft Graph API Permissions](https://learn.microsoft.com/en-us/graph/permissions-reference)
