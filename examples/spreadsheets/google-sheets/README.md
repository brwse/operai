# Google Sheets Integration for Operai Toolbox

A comprehensive integration for programmatically interacting with Google Sheets spreadsheets through the Operai Toolbox.

## Overview

The Google Sheets integration enables AI agents and tools to:

- Read cell values from spreadsheets with support for formatted, unformatted, and formula views
- Write and update data in specific ranges with automatic type conversion
- Append new rows to existing sheets for data collection workflows
- Create and manage sheets (tabs) within spreadsheets
- Evaluate basic formulas like SUM, addition, and multiplication

**Primary Use Cases:**
- Automated data entry and reporting
- Spreadsheet-based data pipelines and ETL workflows
- Dynamic content generation from spreadsheet data
- Integration with Google Workspace ecosystems

## Authentication

This integration uses OAuth2 Bearer Token authentication for secure access to Google Sheets API v4.

### Required Credentials

- `access_token`: OAuth2 access token for Google Sheets API (required)
- `endpoint`: Custom API endpoint URL (optional, defaults to `https://sheets.googleapis.com/v4`)

Credentials are supplied through the `GoogleSheetsCredential` user credential definition.

#### Example Credential

```json
{
  "access_token": "ya29.a0AfH6SMBx...",
  "endpoint": "https://sheets.googleapis.com/v4"
}
```

## Available Tools

### read_range
**Tool Name:** Read Range
**Capabilities:** read
**Tags:** spreadsheets, google-sheets, data
**Description:** Reads values from a specified range in a Google Sheets spreadsheet

**Input:**
- `spreadsheet_id` (string): The ID of the spreadsheet (from the URL)
- `range` (string): The A1 notation range to read (e.g., "Sheet1!A1:D10")
- `value_render_option` (optional, enum): How values should be represented in the output
  - `FormattedValue`: Values are calculated and formatted according to the cell's formatting (default)
  - `UnformattedValue`: Values are calculated but not formatted
  - `Formula`: Values are not calculated; formulas are returned as-is

**Output:**
- `range` (string): The range that was read in A1 notation
- `values` (array of array): The values in the range. Each inner vector represents a row
- `row_count` (number): Number of rows returned
- `column_count` (number): Number of columns returned (from the first row)

### write_range
**Tool Name:** Write Range
**Capabilities:** write
**Tags:** spreadsheets, google-sheets, data
**Description:** Writes values to a specified range in a Google Sheets spreadsheet

**Input:**
- `spreadsheet_id` (string): The ID of the spreadsheet
- `range` (string): The A1 notation range to write to (e.g., "Sheet1!A1:D10")
- `values` (array of array): The values to write. Each inner vector represents a row
- `value_input_option` (optional, enum): How input values should be interpreted
  - `Raw`: Values will be stored as-is
  - `UserEntered`: Values will be parsed as if typed into the UI, formulas are evaluated (default)

**Output:**
- `spreadsheet_id` (string): The spreadsheet ID
- `updated_range` (string): The range that was updated in A1 notation
- `updated_rows` (number): Number of rows updated
- `updated_columns` (number): Number of columns updated
- `updated_cells` (number): Total number of cells updated

### append_row
**Tool Name:** Append Row
**Capabilities:** write
**Tags:** spreadsheets, google-sheets, data
**Description:** Appends a new row of values to a Google Sheets spreadsheet

**Input:**
- `spreadsheet_id` (string): The ID of the spreadsheet
- `range` (string): The sheet name or A1 range to append to (e.g., "Sheet1" or "Sheet1!A:Z")
- `values` (array): The values for the new row
- `value_input_option` (optional, enum): How input values should be interpreted
  - `Raw`: Values will be stored as-is
  - `UserEntered`: Values will be parsed as if typed into the UI (default)
- `insert_data_option` (optional, enum): How the input data should be inserted
  - `InsertRows`: Data is appended after the last row with data (default)
  - `Overwrite`: Data overwrites existing cells

**Output:**
- `spreadsheet_id` (string): The spreadsheet ID
- `updated_range` (string): The range where the row was appended in A1 notation
- `appended_row` (number): The row number where the data was appended (1-indexed)
- `updated_cells` (number): Number of cells written

### create_sheet
**Tool Name:** Create Sheet
**Capabilities:** write
**Tags:** spreadsheets, google-sheets, management
**Description:** Creates a new sheet (tab) within a Google Sheets spreadsheet

**Input:**
- `spreadsheet_id` (string): The ID of the spreadsheet
- `title` (string): The title for the new sheet
- `row_count` (optional, number): Number of rows for the new sheet (defaults to 1000)
- `column_count` (optional, number): Number of columns for the new sheet (defaults to 26)
- `index` (optional, number): Index at which to insert the sheet (0-indexed)

**Output:**
- `spreadsheet_id` (string): The spreadsheet ID
- `sheet_id` (number): The ID of the newly created sheet
- `title` (string): The title of the newly created sheet
- `index` (number): The index of the sheet within the spreadsheet
- `row_count` (number): Number of rows in the sheet
- `column_count` (number): Number of columns in the sheet

### evaluate_formula
**Tool Name:** Evaluate Formula
**Capabilities:** read
**Tags:** spreadsheets, google-sheets, formulas
**Description:** Evaluates a basic Google Sheets formula and returns the result

**Input:**
- `spreadsheet_id` (string): The ID of the spreadsheet (for context and named ranges)
- `formula` (string): The formula to evaluate (e.g., "=SUM(1, 2, 3)" or "=A1+B1")
- `sheet_name` (optional, string): Sheet name for context when evaluating formulas with cell references

**Output:**
- `formula` (string): The original formula
- `result` (CellValue): The computed result (string, number, boolean, or null)
- `success` (boolean): Whether the formula was successfully evaluated
- `error` (optional, string): Error message if evaluation failed

## Cell Value Types

Cell values support the following types:
- **String**: Text values (e.g., `"Hello"`)
- **Number**: Numeric values (e.g., `42`, `3.14`)
- **Boolean**: True/false values (e.g., `true`, `false`)
- **Null**: Empty or null cells

## API Documentation

- **Base URL:** `https://sheets.googleapis.com/v4`
- **API Documentation:** [Google Sheets API v4 Documentation](https://developers.google.com/sheets/api)
- **Authentication:** [OAuth 2.0 for Google APIs](https://developers.google.com/identity/protocols/oauth2)

## Testing

Run tests with:

```bash
cargo test -p google-sheets
```

The test suite includes:
- Input/output serialization tests
- Tool execution tests with mock data
- Credential validation tests
- Enum deserialization tests for value options

## Development

- **Crate:** `google-sheets`
- **Source:** `examples/spreadsheets/google-sheets/src/`
- **Type:** Dynamic library (cdylib) for Operai Toolbox runtime loading

## Implementation Notes

This integration provides full HTTP client implementation using `reqwest` with:
- Automatic Bearer token authentication
- JSON request/response handling
- Error handling with descriptive messages
- Support for custom API endpoints

The formula evaluation tool currently supports:
- `=SUM(a, b, c, ...)` - Sum of comma-separated numbers
- Basic arithmetic: `=a + b` and `=a * b` (two operands only)

For production use, consider extending formula support and implementing API rate limiting.
