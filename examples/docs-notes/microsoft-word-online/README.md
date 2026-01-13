# Microsoft Word Online Integration for Operai Toolbox

Interact with Microsoft Word documents stored in OneDrive and SharePoint through the Microsoft Graph API.

## Overview

The Microsoft Word Online integration enables Operai Toolbox to:

- **Read document metadata** - Retrieve document properties (name, size, URLs, timestamps)
- **Export documents** - Convert Word documents to PDF, HTML, TXT, RTF, or ODT formats

## Important Limitations

**Microsoft Graph API does NOT provide the following functionality for Word documents:**

- ❌ Reading document content (paragraphs, tables, text)
- ❌ Editing document content (inserting, replacing, or deleting paragraphs)
- ❌ Find-and-replace operations
- ❌ Document comments (creating, reading, or replying)

For advanced Word document manipulation, consider:
- **Office JavaScript API (Office.js)** - For building Word add-ins
- **Client-side libraries** - Download files and use libraries like `python-docx`
- **Microsoft Word Automation Services** - SharePoint on-premises solution

### Why These Limitations Exist

Microsoft Graph API provides REST endpoints for:
- ✅ **Excel** - Full workbook/worksheet/cell manipulation
- ✅ **OneNote** - Full notebook/page/section manipulation
- ⚠️ **Word** - **Only file-level operations** (metadata, download, upload, format conversion)

This is a current limitation of Microsoft Graph API. For more information, see:
- [StackOverflow: Is 'Word as a Service' possible via MS Graph API?](https://stackoverflow.com/questions/50509468/is-word-as-a-service-possible-via-ms-graph-api)
- [Microsoft Learn Q&A: Graph API for Microsoft Word](https://learn.microsoft.com/en-us/answers/questions/1424335/graph-api-for-microsoft-word)

## Authentication

This integration uses OAuth2 Bearer Token authentication with the Microsoft Graph API. Credentials are supplied via user credentials configuration.

### Required Credentials

- `access_token`: OAuth2 access token for Microsoft Graph API (required)
- `endpoint`: Custom API endpoint URL (optional, defaults to `https://graph.microsoft.com/v1.0`)

To obtain an access token, register an application in [Microsoft Entra ID (formerly Azure AD)](https://learn.microsoft.com/en-us/entra/identity-platform/) and request the appropriate Graph API scopes (e.g., `Files.Read`, `Files.ReadWrite`).

## Available Tools

### get_document
**Tool Name:** Get Word Document
**Capabilities:** read
**Tags:** docs, word, microsoft-graph
**Description:** Retrieves a Microsoft Word document's metadata from OneDrive or SharePoint

**Input:**
- `document_id` (string): The unique identifier of the document (item ID from OneDrive/SharePoint)
- `drive_id` (string, optional): The drive ID containing the document. If not provided, uses the user's default drive

**Output:**
- `id` (string): Document ID
- `name` (string): Document name
- `web_url` (string): URL to open the document in a browser
- `created_at` (string): ISO 8601 timestamp of document creation
- `modified_at` (string): ISO 8601 timestamp of last modification
- `last_modified_by` (string): Display name of the last modifier
- `size_bytes` (integer): File size in bytes

### export_document
**Tool Name:** Export Document
**Capabilities:** read
**Tags:** docs, word, microsoft-graph
**Description:** Exports a Microsoft Word document to various formats including PDF, HTML, TXT, RTF, and ODT

**Input:**
- `document_id` (string): The unique identifier of the document
- `drive_id` (string, optional): The drive ID containing the document
- `format` (enum): Export format. One of: `pdf`, `html`, `txt`, `rtf`, `odt`

**Output:**
- `document_id` (string): Document ID
- `format` (string): Export format
- `download_url` (string): URL to download the exported document
- `expires_at` (string): ISO 8601 timestamp when the download URL expires (typically 1 hour)
- `size_bytes` (integer): File size in bytes
- `filename` (string): Exported filename with extension

## API Documentation

- **Base URL:** `https://graph.microsoft.com/v1.0`
- **API Documentation:** [Microsoft Graph API REST API](https://learn.microsoft.com/en-us/graph/api/resources/overview?view=graph-rest-1.0)
- **Drive Items:** [Working with drives in Microsoft Graph](https://learn.microsoft.com/en-us/graph/api/resources/driveitem?view=graph-rest-1.0)
- **Format Conversion:** [Get item content](https://learn.microsoft.com/en-us/graph/api/driveitem-get-content?view=graph-rest-1.0)

## Testing

Run tests with:

```bash
cargo test -p microsoft-word-online
```

The integration uses `wiremock` for HTTP mocking in tests, covering:

- Input validation for all tools
- Successful API interactions
- Error handling (404, authentication failures)
- Data structure serialization/deserialization

## Development

- **Crate:** `microsoft-word-online`
- **Source:** `examples/docs-notes/microsoft-word-online/src/`
- **Types:** See `src/types.rs` for Microsoft Graph API response structures

## Example Use Cases

### Document Metadata Retrieval
```json
{
  "document_id": "01A2B3C4D5E6F7G8H9I0J1K2L3M4N5O6P7Q8R9S0T",
  "drive_id": "b!-Ik2sRPLDEWy_bR8l75jfeDcpXQcRKVOmcml10NQLQ1F2UVvTgEnTKi0GO59dbCL"
}
```

### Export to PDF
```json
{
  "document_id": "01A2B3C4D5E6F7G8H9I0J1K2L3M4N5O6P7Q8R9S0T",
  "format": "pdf"
}
```

## Resources

- [Microsoft Graph Documentation](https://learn.microsoft.com/en-us/graph/)
- [Microsoft Graph Explorer](https://developer.microsoft.com/en-us/graph/graph-explorer)
- [Office JavaScript API (for Word add-ins)](https://learn.microsoft.com/en-us/office/dev/add-ins/)
