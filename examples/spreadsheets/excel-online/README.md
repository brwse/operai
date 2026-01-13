# Excel Online Integration for Operai Toolbox

Interact with Excel Online workbooks stored in OneDrive and SharePoint through the Microsoft Graph API.

## Overview

- **Read and write** cell ranges in Excel workbooks using Microsoft Graph API
- **Manage tables** within worksheets (list, create, delete)
- **Create new workbooks** directly in OneDrive/SharePoint
- **Append rows** to worksheets or tables dynamically

Primary use cases:
- Automated spreadsheet data entry and retrieval
- Report generation and data synchronization
- Integration with Microsoft 365 ecosystem

## Authentication

This integration uses OAuth2 Bearer Token authentication via Microsoft Graph API. Credentials are supplied through the Operai Toolbox credential system under the `excel_online` key.

### Required Credentials

**Credential Key:** `excel_online`

- **`access_token`** (required): OAuth2 access token for Microsoft Graph API with appropriate Excel/OneDrive permissions
- **`endpoint`** (optional): Custom API endpoint URL (defaults to `https://graph.microsoft.com/v1.0`)

**Required Microsoft Graph Permissions:**
- `Files.ReadWrite` or `Files.ReadWrite.All` (for OneDrive access)
- `Sites.ReadWrite.All` (for SharePoint site access)

## Available Tools

### read_range
**Tool Name:** Read Excel Range
**Capabilities:** read
**Tags:** spreadsheet, excel, microsoft-graph
**Description:** Read a range of cells from an Excel workbook using Microsoft Graph

**Input:**
- `workbook_id` (string): The ID or path of the workbook file in OneDrive/SharePoint
- `worksheet` (string): The worksheet name or ID
- `range` (string, optional): The range address (e.g., "A1:D10"). If omitted, returns the used range

**Output:**
- `range` (Range): A range object containing:
  - `address` (string, optional): The range address in A1 notation
  - `address_local` (string, optional): The localized range address
  - `cell_count` (integer, optional): Total number of cells in the range
  - `column_count` (integer, optional): Number of columns
  - `column_index` (integer, optional): Zero-based column index
  - `row_count` (integer, optional): Number of rows
  - `row_index` (integer, optional): Zero-based row index
  - `values` (array, optional): 2D array of cell values
  - `text` (array, optional): 2D array of cell text values
  - `formulas` (array, optional): 2D array of cell formulas
  - `number_format` (array, optional): 2D array of number format codes

### write_range
**Tool Name:** Write Excel Range
**Capabilities:** write
**Tags:** spreadsheet, excel, microsoft-graph
**Description:** Write values to a range of cells in an Excel workbook using Microsoft Graph

**Input:**
- `workbook_id` (string): The ID or path of the workbook file in OneDrive/SharePoint
- `worksheet` (string): The worksheet name or ID
- `range` (string): The range address (e.g., "A1:D10")
- `values` (array of arrays): Values to write (2D array)

**Output:**
- `updated` (boolean): Whether the range was successfully updated
- `range` (Range): The updated range object (see read_range for structure)

### append_row
**Tool Name:** Append Excel Row
**Capabilities:** write
**Tags:** spreadsheet, excel, microsoft-graph
**Description:** Append a new row to an Excel table using Microsoft Graph

**Input:**
- `workbook_id` (string): The ID or path of the workbook file in OneDrive/SharePoint
- `worksheet` (string): The worksheet name or ID
- `table` (string): The table name or ID to append to
- `values` (array): Values to append as a new row

**Output:**
- `appended` (boolean): Whether the row was successfully appended
- `row_index` (integer, optional): The index of the appended row

**Note:** Microsoft Graph API only supports appending rows to Excel tables, not to arbitrary worksheet ranges. To append data to a non-table range, use write_range with a specific range address.

### create_workbook
**Tool Name:** Create Excel Workbook
**Capabilities:** write
**Tags:** spreadsheet, excel, microsoft-graph
**Description:** Create a new Excel workbook in OneDrive/SharePoint using Microsoft Graph

**Input:**
- `name` (string): Name of the new workbook (must end with .xlsx)
- `parent_folder_id` (string, optional): Optional parent folder ID. If omitted, creates in the root drive

**Output:**
- `workbook_id` (string): The ID of the created workbook
- `name` (string): The name of the workbook
- `web_url` (string, optional): The web URL to open the workbook in Excel Online

### list_tables
**Tool Name:** List Excel Tables
**Capabilities:** read
**Tags:** spreadsheet, excel, microsoft-graph
**Description:** List tables in an Excel workbook using Microsoft Graph

**Input:**
- `workbook_id` (string): The ID or path of the workbook file in OneDrive/SharePoint
- `worksheet` (string, optional): Optional worksheet name or ID. If omitted, lists all tables in the workbook

**Output:**
- `tables` (array of Table): List of table objects, each containing:
  - `id` (string): The table identifier
  - `name` (string, optional): The table name
  - `show_headers` (boolean, optional): Whether the table shows headers
  - `show_totals` (boolean, optional): Whether the table shows totals row
  - `style` (string, optional): The table style name

### create_table
**Tool Name:** Create Excel Table
**Capabilities:** write
**Tags:** spreadsheet, excel, microsoft-graph
**Description:** Create a table in an Excel worksheet using Microsoft Graph

**Input:**
- `workbook_id` (string): The ID or path of the workbook file in OneDrive/SharePoint
- `worksheet` (string): The worksheet name or ID
- `range` (string): The range address for the table (e.g., "A1:D10")
- `has_headers` (boolean): Whether the range has headers

**Output:**
- `table` (Table): The created table object (see list_tables for structure)

### delete_table
**Tool Name:** Delete Excel Table
**Capabilities:** write
**Tags:** spreadsheet, excel, microsoft-graph
**Description:** Delete a table from an Excel worksheet using Microsoft Graph

**Input:**
- `workbook_id` (string): The ID or path of the workbook file in OneDrive/SharePoint
- `worksheet` (string): The worksheet name or ID
- `table` (string): The table name or ID to delete

**Output:**
- `deleted` (boolean): Whether the table was successfully deleted

## API Documentation

- **Base URL:** `https://graph.microsoft.com/v1.0` (configurable via `endpoint` credential)
- **API Documentation:** [Microsoft Graph Excel API](https://learn.microsoft.com/en-us/graph/api/resources/excel?view=graph-rest-1.0)

## Testing

Run tests with:

```bash
cargo test -p excel-online
```

The integration includes comprehensive unit tests and integration tests using wiremock for mocking Microsoft Graph API responses.

## Development

- **Crate:** `excel-online`
- **Source:** `examples/spreadsheets/excel-online/src/`
