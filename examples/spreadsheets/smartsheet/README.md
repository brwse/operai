# Smartsheet Integration for Operai Toolbox

A Operai Toolbox integration for interacting with Smartsheet spreadsheets, enabling sheet management, row operations, commenting, and file attachments.

## Overview

This integration provides tools for:
- **Sheet Discovery**: List and browse Smartsheet sheets accessible to the authenticated user
- **Data Operations**: Read rows from sheets with optional filtering, and update cell values and formulas
- **Collaboration**: Add comments to rows and attach files from various sources (local uploads, URLs, cloud storage)
- **Flexible Access**: Support for pagination, column metadata, and filtering by column values

Primary use cases include project tracking, automated reporting, data synchronization, and collaborative workflows within Smartsheet sheets.

## Authentication

This integration uses OAuth2 Bearer Token authentication. Credentials are supplied via the Smartsheet user credential.

### Required Credentials

- `access_token`: OAuth2 access token for Smartsheet API (required)
- `endpoint`: Custom API endpoint URL (optional, defaults to `https://api.smartsheet.com/2.0`)

## Available Tools

### list_sheets

**Tool Name:** List Sheets
**Capabilities:** read
**Tags:** spreadsheet, smartsheet
**Description:** List all Smartsheet sheets accessible to the authenticated user

**Input:**
- `page_size` (optional `u32`): Maximum number of sheets to return (default: 100, max: 1000)
- `page` (optional `u32`): Page number for pagination (1-based)
- `include_trash` (optional `bool`): Include sheets in the Trash

**Output:**
- `sheets` (`Vec<SheetSummary>`): List of sheets
- `total_count` (`i32`): Total count of sheets

### read_rows

**Tool Name:** Read Rows
**Capabilities:** read
**Tags:** (none specified)
**Description:** Read rows from a Smartsheet sheet with optional filtering

**Input:**
- `sheet_id` (`u64`): ID of the sheet to read rows from
- `row_ids` (optional `Vec<u64>`): Specific row IDs to fetch. If empty, fetches all rows
- `filter` (optional `HashMap<String, String>`): Filter rows by column values. Key is column ID, value is the filter value
- `page_size` (optional `u32`): Maximum number of rows to return
- `page` (optional `u32`): Page number for pagination (1-based)
- `include_columns` (optional `bool`): Include column metadata in the response

**Output:**
- `sheet_id` (`u64`): ID of the sheet
- `sheet_name` (`String`): Name of the sheet
- `columns` (optional `Vec<Column>`): Column definitions (if `include_columns` was true)
- `rows` (`Vec<Row>`): Rows from the sheet
- `total_count` (`u32`): Total count of rows matching the criteria

### update_rows

**Tool Name:** Update Rows
**Capabilities:** (none specified)
**Tags:** (none specified)
**Description:** Update one or more rows in a Smartsheet sheet

**Input:**
- `sheet_id` (`u64`): ID of the sheet containing the rows
- `rows` (`Vec<RowUpdate>`): Rows to update
  - `id` (`u64`): ID of the row to update
  - `cells` (`Vec<CellUpdate>`): Cells to update in this row
    - `column_id` (`u64`): Column ID of the cell to update
    - `value` (optional `serde_json::Value`): New value for the cell
    - `formula` (optional `String`): Formula to set (overrides value if provided)
    - `hyperlink` (optional `Hyperlink`): Hyperlink to set on the cell
    - `strict` (optional `bool`): Set to true to clear the cell value
  - `parent_id` (optional `u64`): Move the row to be a child of this parent row
  - `sibling_id` (optional `u64`): Move the row to this position (above the specified sibling)
  - `expanded` (optional `bool`): Whether the row should be expanded
  - `locked` (optional `bool`): Whether the row is locked

**Output:**
- `sheet_id` (`u64`): ID of the sheet
- `results` (`Vec<RowUpdateResult>`): Results for each row update
  - `id` (`u64`): Row ID
  - `success` (`bool`): Whether the update was successful
  - `error` (optional `String`): Error message if the update failed
  - `version` (optional `u64`): Version number after the update
- `updated_count` (`u32`): Number of rows successfully updated
- `failed_count` (`u32`): Number of rows that failed to update

### comment

**Tool Name:** Add Comment
**Capabilities:** (none specified)
**Tags:** (none specified)
**Description:** Add a comment to a row in a Smartsheet sheet

**Input:**
- `sheet_id` (`u64`): ID of the sheet containing the row
- `row_id` (`u64`): ID of the row to comment on
- `text` (`String`): Text of the comment

**Output:**
- `comment_id` (`u64`): ID of the created comment
- `sheet_id` (`u64`): ID of the sheet
- `row_id` (`u64`): ID of the row
- `text` (`String`): Text of the comment
- `created_at` (`String`): Timestamp when the comment was created

### attach_file

**Tool Name:** Attach File
**Capabilities:** (none specified)
**Tags:** (none specified)
**Description:** Attach a file to a row in a Smartsheet sheet

**Input:**
- `sheet_id` (`u64`): ID of the sheet containing the row
- `row_id` (`u64`): ID of the row to attach the file to
- `name` (`String`): Name of the attachment
- `source_type` (`AttachmentSourceType`): Type of attachment source (`FILE`, `LINK`, `GOOGLE_DRIVE`, `ONE_DRIVE`, `DROPBOX`, `BOX`, `EVERNOTE`)
- `url` (optional `String`): URL of the file (for Link, Google Drive, OneDrive, Dropbox, Box, Evernote)
- `content` (optional `String`): Base64-encoded file content (for File type)
- `mime_type` (optional `String`): MIME type of the file
- `description` (optional `String`): Description of the attachment

**Output:**
- `attachment_id` (`u64`): ID of the created attachment
- `sheet_id` (`u64`): ID of the sheet
- `row_id` (`u64`): ID of the row
- `name` (`String`): Name of the attachment
- `mime_type` (optional `String`): MIME type of the attachment
- `size_in_kb` (optional `u64`): Size in KB
- `created_at` (`String`): Timestamp when the attachment was created

## API Documentation

- Base URL: `https://api.smartsheet.com/2.0`
- API Documentation: [Smartsheet API Docs](https://smartsheet-platform.github.io/api-docs/)

## Testing

Run tests:
```bash
cargo test -p brwse-smartsheet
```

## Development

- Crate: `brwse-smartsheet`
- Source: `examples/spreadsheets/smartsheet`
