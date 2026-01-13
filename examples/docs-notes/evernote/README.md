# Evernote Integration for Operai Toolbox

Manage Evernote notes through search, creation, updates, and tagging operations.

## Overview

This integration provides tools for managing Evernote notes including:

- **Search notes** using Evernote's powerful search grammar (keywords, `tag:`, `notebook:`, etc.)
- **Retrieve notes** with full content, metadata, and attachment information
- **Create notes** with optional notebook assignment and tags
- **Update notes** including title, content, and notebook location
- **Manage tags** on notes (add, remove, or replace)

## Authentication

This integration uses **OAuth2 Bearer Token** authentication via user credentials.

### Required Credentials

The following user credentials are configured:

- **`access_token`** (required): OAuth access token or developer token for Evernote API
- **`sandbox`** (optional): Set to `true` to use Evernote sandbox environment (defaults to `false` for production)

#### Getting an Access Token

1. **For Development**: Request a [Developer Token](https://dev.evernote.com/doc/articles/dev_tokens.php) from Evernote
2. **For Production**: Implement OAuth flow (see [Evernote OAuth Documentation](https://dev.evernote.com/doc/articles/authentication.php))

Developer tokens provide instant access but are limited to your personal account. For production apps, use OAuth to access user accounts.

## Available Tools

### search_notes

**Tool Name:** Search Notes
**Capabilities:** read
**Tags:** notes, evernote, search
**Description:** Search for notes in Evernote using the search grammar. Supports keywords, tag:, notebook:, created:, updated:, and other filters.

**Input:**

- `query` (string): Search query string using Evernote search grammar. Supports keywords, tags (tag:), notebook (notebook:), etc.
- `max_results` (optional u32): Maximum number of notes to return (1-250, default 25)
- `offset` (optional u32): Offset for pagination (default 0)
- `sort_order` (optional SortOrder): Sort order for results. Options: `created`, `updated`, `relevance`, `title`

**Output:**

- `notes` (array of NoteSummary): List of matching notes
- `total_count` (u32): Total number of notes matching the query
- `offset` (u32): Offset used in this search
- `request_id` (string): Request ID for tracking

### get_note

**Tool Name:** Get Note
**Capabilities:** read
**Tags:** notes, evernote
**Description:** Retrieve a specific note by its GUID, including content and metadata.

**Input:**

- `guid` (string): The unique identifier (GUID) of the note to retrieve
- `include_content` (boolean, default: true): Whether to include the full content (ENML)
- `include_resources` (optional boolean): Whether to include resource metadata (attachments)

**Output:**

- `note` (NoteDetails): The retrieved note details
  - `guid` (string): Unique identifier for the note
  - `title` (string): Title of the note
  - `content` (optional string): Content in ENML format (if requested)
  - `plain_text` (optional string): Plain text content (extracted from ENML)
  - `notebook_name` (optional string): Name of the notebook containing the note
  - `notebook_guid` (string): GUID of the notebook
  - `tags` (array of string): Tags applied to the note
  - `created` (i64): Timestamp when the note was created (Unix milliseconds)
  - `updated` (i64): Timestamp when the note was last updated (Unix milliseconds)
  - `author` (optional string): Author of the note
  - `source_url` (optional string): Source URL if the note was clipped from web
  - `resources` (array of NoteResource): Resources/attachments in the note
- `request_id` (string): Request ID for tracking

### create_note

**Tool Name:** Create Note
**Capabilities:** write
**Tags:** notes, evernote
**Description:** Create a new note in Evernote with optional notebook and tags.

**Input:**

- `title` (string): Title of the note
- `content` (string): Content of the note in plain text or ENML
- `is_enml` (optional boolean): Whether the content is already in ENML format
- `notebook` (optional string): Name or GUID of the notebook to create the note in. If not specified, uses the default notebook
- `tags` (optional array of string): Tags to apply to the note
- `source_url` (optional string): Source URL if clipping from web

**Output:**

- `guid` (string): GUID of the created note
- `title` (string): Title of the created note
- `notebook_guid` (string): Notebook GUID where the note was created
- `created` (i64): Timestamp when the note was created (Unix milliseconds)
- `request_id` (string): Request ID for tracking

### update_note

**Tool Name:** Update Note
**Capabilities:** write
**Tags:** notes, evernote
**Description:** Update an existing note's title, content, or notebook location.

**Input:**

- `guid` (string): GUID of the note to update
- `title` (optional string): New title for the note (if changing)
- `content` (optional string): New content for the note (if changing)
- `is_enml` (optional boolean): Whether the new content is in ENML format
- `notebook` (optional string): New notebook to move the note to (name or GUID)

**Output:**

- `guid` (string): GUID of the updated note
- `title` (string): Updated title
- `updated` (i64): Timestamp when the note was updated (Unix milliseconds)
- `request_id` (string): Request ID for tracking

### tag_note

**Tool Name:** Tag Note
**Capabilities:** write
**Tags:** notes, evernote, tags
**Description:** Add, remove, or replace tags on an Evernote note.

**Input:**

- `guid` (string): GUID of the note to tag
- `tags` (array of string): Tags to add, remove, or set
- `action` (TagAction, default: "add"): Action to perform on the tags. Options: `add`, `remove`, `replace`

**Output:**

- `guid` (string): GUID of the tagged note
- `tags` (array of string): Current tags on the note after the operation
- `updated` (i64): Timestamp when the note was updated (Unix milliseconds)
- `request_id` (string): Request ID for tracking

## API Documentation

- **Base URL:**
  - Production: `https://www.evernote.com`
  - Sandbox: `https://sandbox.evernote.com`
- **API Documentation:** [Evernote Developer Documentation](https://dev.evernote.com/doc/)

### Evernote Search Grammar

The `search_notes` tool supports Evernote's powerful search grammar:

- `tag:work` - Notes with "work" tag
- `notebook:"Project Notes"` - Notes in specific notebook
- `created:day-7` - Notes created in last 7 days
- `updated:day` - Notes updated today
- `intitle:meeting` - Notes with "meeting" in title
- `todo:true` - Notes with uncompleted checkboxes
- `resource:image/*` - Notes with images

See [Evernote Search Grammar](https://dev.evernote.com/doc/articles/search_grammar.php) for complete syntax.

### ENML Format

Evernote stores note content in **ENML** (Evernote Markup Language), an XML format based on XHTML. When creating or updating notes:

- Set `is_enml: false` (default) to send plain text
- Set `is_enml: true` if providing ENML-formatted content

ENML structure:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE en-note SYSTEM "http://xml.evernote.com/pub/enml2.dtd">
<en-note>
    Your content here
</en-note>
```

## Technical Notes

### API Architecture

Evernote uses **Apache Thrift** for their API, not traditional REST. This integration provides a simplified interface that would connect to Evernote's Thrift-based NoteStore service in a full implementation.

### Implementation Status

This is a **reference implementation** that demonstrates proper structure, validation, and patterns for Evernote integration. All tool functions include:

- **Input validation** following Evernote API constraints
- **Accurate type definitions** matching Evernote's Thrift API
- **Comprehensive documentation** with implementation guidance
- **Production-ready error handling patterns**

The functions currently return empty/fake data to demonstrate the structure. A full production implementation would require:

1. **Thrift Client**: Use `thrift` crate with generated code from [evernote-thrift IDL files](https://github.com/Evernote/evernote-thrift)
2. **HTTP Transport**: Configure Thrift HTTP client with proper endpoints (production vs sandbox)
3. **Error Handling**: Map Thrift exceptions (`EDAMUserException`, `EDAMNotFoundException`) to operai errors
4. **Notebook/Tag Resolution**: Resolve notebook/tag names to GUIDs via `NoteStore.listNotebooks` and `NoteStore.listTags`
5. **ENML Parsing**: Convert plain text to ENML format for create/update operations
6. **Authentication**: Handle OAuth tokens and developer tokens

**See individual function documentation in `src/lib.rs` for detailed implementation guidance**, including example Thrift call structures for each operation.

### Rate Limits

Evernote enforces rate limits:
- **API Key Rate Limit**: Varies by key type (basic vs. full access)
- **User Rate Limit**: Per-user limits on note operations
- See [Rate Limits Documentation](https://dev.evernote.com/doc/articles/rate_limits.php)

## Testing

Run tests:
```bash
cargo test -p evernote
```

All tests validate:
- Input deserialization and validation
- Output serialization
- Error handling for empty/invalid inputs
- Credential parsing

## Development

- **Crate:** `evernote`
- **Source:** `examples/docs-notes/evernote/src/`

## Resources

- [Evernote Developer Documentation](https://dev.evernote.com/doc/)
- [Core API Concepts](https://dev.evernote.com/doc/articles/core_concepts.php)
- [Creating Notes Guide](https://dev.evernote.com/doc/articles/creating_notes.php)
- [Search Documentation](https://dev.evernote.com/doc/articles/searching_notes.php)
- [Thrift IDL Files](https://github.com/Evernote/evernote-thrift)
