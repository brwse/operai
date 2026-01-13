# Coda Integration for Operai Toolbox

This integration enables reading and writing to Coda documents, tables, and rows through the Operai Toolbox.

## Overview

The Coda integration for Operai Toolbox provides tools to:
- List and query Coda documents accessible to the authenticated user
- Read rows from Coda tables with optional filtering
- Insert or update rows in Coda tables (upsert)
- Add comments to table rows

**Primary use cases:**
- Syncing data between Coda docs and external systems
- Building automations that read from or write to Coda tables
- Creating custom workflows that interact with Coda documents

## Authentication

This integration uses **API Key (Bearer Token)** authentication. Credentials are supplied as system-level credentials.

### Required Credentials

- `api_key`: Coda API bearer token for authentication
- `endpoint` (optional): Custom API endpoint (defaults to `https://coda.io/apis/v1`)

### Getting Your API Key

1. Log in to your Coda account
2. Go to Account Settings
3. Navigate to the API section
4. Generate a new API token
5. Add the token to your `operai.toml` manifest:

```toml
[[tools]]
package = "brwse-coda"
[tools.credentials.coda]
api_key = "your_api_key_here"
```

## Available Tools

### list_docs
**Tool Name:** List Coda Docs
**Capabilities:** read
**Tags:** coda, spreadsheet, docs
**Description:** List Coda documents accessible to the authenticated user

**Input:**
- `limit` (optional, u32): Maximum number of docs to return (1-100). Defaults to 25.

**Output:**
- `docs` (array of DocSummary): List of document summaries
  - `id` (string): Document ID
  - `name` (string): Document name
  - `browser_link` (string): URL to open document in browser
  - `owner_name` (optional, string): Name of document owner
  - `created_at` (optional, string): ISO 8601 timestamp when document was created
  - `updated_at` (optional, string): ISO 8601 timestamp when document was last updated

### query_table
**Tool Name:** Query Coda Table
**Capabilities:** read
**Tags:** coda, spreadsheet, table, query
**Description:** Query rows from a Coda table with optional filtering

**Input:**
- `doc_id` (string): Document ID
- `table_id_or_name` (string): Table ID or name
- `query` (optional, string): Optional query string to filter rows
- `limit` (optional, u32): Maximum number of rows to return (1-500). Defaults to 100.

**Output:**
- `rows` (array of RowData): List of table rows
  - `id` (string): Row ID
  - `name` (string): Row name
  - `index` (integer): Row index
  - `values` (object): Map of column names to cell values
  - `browser_link` (string): URL to open row in browser
  - `created_at` (optional, string): ISO 8601 timestamp when row was created
  - `updated_at` (optional, string): ISO 8601 timestamp when row was last updated

### upsert_row
**Tool Name:** Upsert Coda Row
**Capabilities:** write
**Tags:** coda, spreadsheet, table, upsert
**Description:** Insert or update rows in a Coda table

**Input:**
- `doc_id` (string): Document ID
- `table_id_or_name` (string): Table ID or name
- `rows` (array of RowValues): Rows to insert or update
  - `cells` (object): Map of column names to values
- `key_columns` (optional, array of string): Optional column names to use as unique keys for upserts

**Output:**
- `request_id` (string): Request ID for tracking
- `added_row_ids` (array of string): IDs of newly added rows

### add_comment
**Tool Name:** Add Coda Comment
**Capabilities:** write
**Tags:** coda, spreadsheet, comment
**Description:** Add a comment to a row in a Coda table

**Input:**
- `doc_id` (string): Document ID
- `table_id_or_name` (string): Table ID or name
- `row_id` (string): Row ID to comment on
- `content` (string): Comment content

**Output:**
- `comment_id` (string): ID of the created comment
- `created_at` (string): ISO 8601 timestamp when comment was created

## API Documentation

- **Base URL:** `https://coda.io/apis/v1`
- **API Documentation:** [Coda Developers Portal](https://coda.io/developers)
- **API Reference:** [Coda API v1 Reference](https://coda.io/developers/apis/v1)

## Rate Limits

The Coda API has rate limits. If you exceed the limit, you'll receive a 429 (Too Many Requests) error. The integration does not automatically handle rate limiting - implement backoff and retry logic in your application if needed.

## Testing

Run tests with:

```bash
cargo test -p brwse-coda
```

The integration includes comprehensive unit tests and integration tests using wiremock for HTTP mocking.

## Development

- **Crate:** `brwse-coda`
- **Source:** `examples/spreadsheets/coda/src/`

## Examples

### Listing All Documents

```bash
# Ensure your credentials are configured in the manifest

# List documents
operai call list_docs --input '{"limit": 10}'
```

### Querying a Table

```bash
# Query all rows in a table
operai call query_table --input '{
  "doc_id": "doc-abc123",
  "table_id_or_name": "Tasks",
  "limit": 100
}'

# Query with a filter
operai call query_table --input '{
  "doc_id": "doc-abc123",
  "table_id_or_name": "Tasks",
  "query": "Status = \"In Progress\"",
  "limit": 50
}'
```

### Upserting Rows

```bash
# Insert a new row
operai call upsert_row --input '{
  "doc_id": "doc-abc123",
  "table_id_or_name": "Tasks",
  "rows": [
    {
      "cells": {
        "Task Name": "New task",
        "Status": "To Do"
      }
    }
  ]
}'

# Update existing rows based on a key column
operai call upsert_row --input '{
  "doc_id": "doc-abc123",
  "table_id_or_name": "Tasks",
  "rows": [
    {
      "cells": {
        "Task Name": "Existing task",
        "Status": "Done"
      }
    }
  ],
  "key_columns": ["Task Name"]
}'
```

### Adding a Comment

```bash
operai call add_comment --input '{
  "doc_id": "doc-abc123",
  "table_id_or_name": "Tasks",
  "row_id": "row-xyz789",
  "content": "This looks good to me!"
}'
```
