# Google Docs Integration for Operai Toolbox

Read, edit, and export Google Docs documents through the Operai Toolbox.

## Overview

- Retrieve Google Docs documents with full metadata and content
- Insert, update, and delete text within documents at specific locations
- Add comments to document ranges and export documents to multiple formats
- Support for PDF, TXT, HTML, DOCX, ODT, RTF, and EPUB export formats

Primary use cases include automated document content manipulation, bulk document processing, document format conversion, and integration with document workflows.

## Authentication

This integration uses OAuth2 Bearer Token authentication. Access tokens are validated and included in the Authorization header for all API requests to both Google Docs API and Google Drive API (for export functionality).

### Required Credentials

- **`access_token`**: OAuth2 access token for Google Docs API (required)
- **`endpoint`**: Custom API endpoint URL (optional, defaults to `https://docs.googleapis.com/v1`)

## Available Tools

### get_doc
**Tool Name:** Get Google Doc
**Capabilities:** read
**Tags:** docs, google, document
**Description:** Retrieve a Google Doc by ID, including its content and metadata

**Input:**
- `document_id` (string): The Google Docs document ID (from the document URL)
- `include_body` (boolean, optional): Whether to include the full document body content

**Output:**
- `document` (Document): Document structure containing document_id, title, body, and revision_id

### insert_text
**Tool Name:** Insert Text
**Capabilities:** write
**Tags:** docs, google, edit
**Description:** Insert text at a specific location in a Google Doc

**Input:**
- `document_id` (string): The Google Docs document ID
- `text` (string): The text to insert
- `location` (Location): The location (index) where text should be inserted. Index 1 is the start of the body
  - `index` (integer): The zero-based index in the document
  - `segment_id` (string, optional): Segment identifier for specific document sections

**Output:**
- `document_id` (string): The document ID that was updated
- `revision_id` (string): The revision identifier after the update

### update_text
**Tool Name:** Update Text
**Capabilities:** write
**Tags:** docs, google, edit
**Description:** Update or delete text in a specific range of a Google Doc

**Input:**
- `document_id` (string): The Google Docs document ID
- `range` (Range): The range of text to replace
  - `start_index` (integer): Start index of the range (must be >= 1)
  - `end_index` (integer): End index of the range (must be > start_index)
  - `segment_id` (string, optional): Segment identifier for specific document sections
- `new_text` (string): The new text to insert (or empty string to delete)

**Output:**
- `document_id` (string): The document ID that was updated
- `revision_id` (string): The revision identifier after the update

### add_comment
**Tool Name:** Add Comment
**Capabilities:** write
**Tags:** docs, google, comment
**Description:** Add a comment to a specific range in a Google Doc

**Input:**
- `document_id` (string): The Google Docs document ID
- `content` (string): The comment content
- `anchor` (Range): The range of text to attach the comment to
  - `start_index` (integer): Start index of the range (must be >= 1)
  - `end_index` (integer): End index of the range (must be > start_index)
  - `segment_id` (string, optional): Segment identifier for specific document sections

**Output:**
- `comment_id` (string): The ID of the created comment
- `document_id` (string): The document ID the comment was added to

### export_doc
**Tool Name:** Export Document
**Capabilities:** read
**Tags:** docs, google, export
**Description:** Export a Google Doc to PDF or other formats using Drive API

**Input:**
- `document_id` (string): The Google Docs document ID
- `format` (ExportFormat, optional): Export format (pdf, txt, html, docx, odt, rtf, epub, md). Defaults to pdf

**Output:**
- `document_id` (string): The document ID that was exported
- `format` (ExportFormat): The export format used
- `mime_type` (string): The MIME type of the exported file
- `download_url` (string): The URL to download the exported file

**Supported Export Formats:**
- `pdf` - PDF document (application/pdf)
- `txt` - Plain text (text/plain)
- `html` - Web Page HTML (application/zip) - HTML exports are packaged as ZIP files
- `docx` - Microsoft Word document (application/vnd.openxmlformats-officedocument.wordprocessingml.document)
- `odt` - OpenDocument text (application/vnd.oasis.opendocument.text)
- `rtf` - Rich Text Format (application/rtf)
- `epub` - EPUB eBook (application/epub+zip)
- `md` - Markdown (text/markdown)

## API Documentation

- Base URL (Docs API): `https://docs.googleapis.com/v1`
- Base URL (Drive API): `https://www.googleapis.com/drive/v3`
- API Documentation: [Google Docs API](https://developers.google.com/docs/api) | [Google Drive API](https://developers.google.com/drive/api)

## Testing

Run tests:

```bash
cargo test -p google-docs
```

## Development

- Crate: `google-docs`
- Source: `examples/docs-notes/google-docs/src/`
