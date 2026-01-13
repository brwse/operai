# Notion Integration for Operai Toolbox

Interact with Notion pages, databases, and content blocks through the Notion API.

## Overview

- **Search** across all pages and databases shared with the integration
- **Create** new pages in databases or as children of existing pages
- **Read** page details and properties
- **Update** page properties and archive status
- **Append** content blocks (paragraphs, headings, lists, etc.) to pages

### Primary Use Cases

- Documentation management and knowledge base operations
- Automated content creation and page organization
- Integration with note-taking workflows
- Database-driven content management

## Authentication

This integration uses **OAuth2 Bearer Token** authentication. Credentials are supplied via the `define_user_credential!` macro and passed through the Operai Toolbox runtime.

### Required Credentials

- `access_token`: OAuth2 access token for Notion API (required)
- `endpoint`: Custom API endpoint URL (optional, defaults to `https://api.notion.com/v1`)

#### Obtaining an Access Token

1. Create an integration at [https://www.notion.so/my-integrations](https://www.notion.so/my-integrations)
2. Copy the "Internal Integration Token" (starts with `secret_`)
3. Share the integration with your workspace pages/databases

#### Example Configuration

```json
{
  "notion": {
    "access_token": "secret_abc123...",
    "endpoint": "https://api.notion.com/v1"
  }
}
```

## Available Tools

### search_pages_db
**Tool Name:** Search Notion Pages/Databases
**Capabilities:** read
**Tags:** docs, notion, search
**Description:** Search all pages and databases shared with the integration

**Input:**
- `query` (string): Search query string to filter by title
- `filter` (optional ObjectType): Filter by object type (`page` or `database`)
- `limit` (optional u32): Maximum number of results (1-100). Defaults to 10

**Output:**
- `results` (array[SearchResult]): Array of page or database summaries
- `has_more` (boolean): Whether more results exist beyond the current page

---

### get_page
**Tool Name:** Get Notion Page
**Capabilities:** read
**Tags:** docs, notion, page
**Description:** Retrieve a Notion page by ID with its properties

**Input:**
- `page_id` (string): Notion page ID

**Output:**
- `page` (PageDetail): Complete page object including properties, metadata, and parent information

---

### create_page
**Tool Name:** Create Notion Page
**Capabilities:** write
**Tags:** docs, notion, page
**Description:** Create a new page in a database or as a child of another page

**Input:**
- `parent_id` (string): Parent page or database ID
- `parent_is_database` (boolean): Whether parent is a database (true) or page (false)
- `title` (string): Page title
- `properties` (optional JSON): Optional properties (JSON object for database pages)

**Output:**
- `page_id` (string): Created page ID
- `url` (string): URL to access the page in Notion

---

### update_properties
**Tool Name:** Update Notion Page Properties
**Capabilities:** write
**Tags:** docs, notion, page
**Description:** Update properties of a Notion page

**Input:**
- `page_id` (string): Notion page ID
- `properties` (JSON): Properties to update (JSON object)
- `archived` (optional boolean): Whether to archive the page

**Output:**
- `updated` (boolean): Success indicator

---

### append_blocks
**Tool Name:** Append Blocks to Notion Page
**Capabilities:** write
**Tags:** docs, notion, blocks
**Description:** Append content blocks to a Notion page or block

**Input:**
- `block_id` (string): Block ID (page or block) to append children to
- `children` (array[BlockInput]): Array of block objects to append
  - `block_type` (string): Block type (e.g., `paragraph`, `heading_1`, `heading_2`, `heading_3`, `bulleted_list_item`, `numbered_list_item`, `quote`, `code`, `callout`, `toggle`)
  - `content` (string): Text content for the block

**Output:**
- `appended` (boolean): Success indicator
- `block_count` (number): Number of blocks appended

## API Documentation

- **Base URL:** `https://api.notion.com/v1`
- **API Version:** 2022-06-28
- **API Documentation:** [https://developers.notion.com/reference/intro](https://developers.notion.com/reference/intro)

### Required Headers

All API requests include these headers:
- `Authorization: Bearer {access_token}`
- `Notion-Version: 2022-06-28`
- `Content-Type: application/json`
- `Accept: application/json`

## Testing

Run tests:
```bash
cargo test -p brwse-notion
```

The test suite includes:
- Input validation tests (empty fields, invalid ranges)
- Serialization roundtrip tests for enums
- HTTP mock tests using `wiremock`
- Integration tests for all tool endpoints

## Development

- **Crate:** `brwse-notion`
- **Source:** `examples/docs-notes/notion/`

## References

- [Notion API Documentation](https://developers.notion.com/)
- [Search endpoint](https://developers.notion.com/reference/post-search)
- [Retrieve a page](https://developers.notion.com/reference/retrieve-a-page)
- [Create a page](https://developers.notion.com/reference/post-page)
- [Update page properties](https://developers.notion.com/reference/patch-page)
- [Append block children](https://developers.notion.com/reference/patch-block-children)
