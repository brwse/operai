# Airtable Integration for Operai Toolbox

Enables seamless interaction with Airtable bases, tables, and records through the Operai Toolbox.

## Overview

- **Read and browse** Airtable bases, tables, and records accessible to the authenticated user
- **Search and filter** records using Airtable formulas, sorting, and view-based queries
- **Create and update** records in any Airtable table with field-level precision
- **Attach files** to records by providing public URLs that Airtable downloads and stores

Primary use cases include data synchronization, content management automation, reporting, and integration workflows between Airtable and other systems.

## Authentication

This integration uses **API Key authentication** (Bearer Token) for the Airtable REST API. Credentials are supplied through system credentials configured in the Operai Toolbox environment.

### Required Credentials

- `api_key`: Airtable personal access token (PAT) or API key for authenticating requests
- `endpoint` (optional): Custom API endpoint URL (defaults to `https://api.airtable.com/v0`)

To obtain an API key, create a personal access token in your [Airtable Account Settings](https://airtable.com/create/tokens) with appropriate scopes for the bases you want to access.

## Available Tools

### list_bases
**Tool Name:** List Airtable Bases
**Capabilities:** read
**Tags:** airtable, bases
**Description:** List all bases accessible to the authenticated user

**Input:**
- `limit` (optional, u32): Maximum number of bases to return (1-100). Defaults to 100.
- `offset` (optional, String): Offset token for pagination

**Output:**
- `bases` (Vec<Base>): List of base summaries containing id, name, and permission_level
- `offset` (optional, String): Token for retrieving the next page of results

### list_tables
**Tool Name:** List Airtable Tables
**Capabilities:** read
**Tags:** airtable, tables
**Description:** List all tables in a base

**Input:**
- `base_id` (String): Base ID (starts with "app")

**Output:**
- `tables` (Vec<Table>): List of table metadata containing id, name, description, and primary_field_id

### search_records
**Tool Name:** Search Airtable Records
**Capabilities:** read
**Tags:** airtable, records, search
**Description:** Search and filter records in an Airtable table

**Input:**
- `base_id` (String): Base ID (starts with "app")
- `table_id_or_name` (String): Table ID or table name
- `filter_by_formula` (optional, String): Optional Airtable formula to filter records
- `max_records` (optional, u32): Maximum number of records to return (1-100). Defaults to 100.
- `sort` (Vec<String>, optional): Field names to sort by (prefix with "-" for descending)
- `view` (optional, String): View name or ID to use
- `offset` (optional, String): Offset token for pagination

**Output:**
- `records` (Vec<Record>): List of records containing id, fields (HashMap), and created_time
- `offset` (optional, String): Token for retrieving the next page of results

### create_record
**Tool Name:** Create Airtable Record
**Capabilities:** write
**Tags:** airtable, records, create
**Description:** Create a new record in an Airtable table

**Input:**
- `base_id` (String): Base ID (starts with "app")
- `table_id_or_name` (String): Table ID or table name
- `fields` (HashMap<String, serde_json::Value>): Field values for the new record

**Output:**
- `record` (Record): The created record containing id, fields, and created_time

### update_record
**Tool Name:** Update Airtable Record
**Capabilities:** write
**Tags:** airtable, records, update
**Description:** Update an existing record in an Airtable table

**Input:**
- `base_id` (String): Base ID (starts with "app")
- `table_id_or_name` (String): Table ID or table name
- `record_id` (String): Record ID (starts with "rec")
- `fields` (HashMap<String, serde_json::Value>): Field values to update

**Output:**
- `record` (Record): The updated record containing id, fields, and created_time

### attach_file
**Tool Name:** Attach File to Airtable Record
**Capabilities:** write
**Tags:** airtable, records, attachments
**Description:** Attach files to an attachment field in an Airtable record by providing public URLs

**Input:**
- `base_id` (String): Base ID (starts with "app")
- `table_id_or_name` (String): Table ID or table name
- `record_id` (String): Record ID (starts with "rec")
- `field_name` (String): Field name or ID of the attachment field
- `attachments` (Vec<Attachment>): Attachments to add (public URLs that Airtable will download and store)

**Output:**
- `record` (Record): The updated record containing id, fields, and created_time

## API Documentation

- **Base URL:** `https://api.airtable.com/v0`
- **API Documentation:** [Airtable REST API Documentation](https://airtable.com/developers/web/api)

## Testing

Run tests:
```bash
cargo test -p brwse-airtable
```

## Development

- **Crate:** brwse-airtable
- **Source:** examples/spreadsheets/airtable
