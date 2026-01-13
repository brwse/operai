# Guru Integration for Operai Toolbox

Knowledge management integration for teams that enables searching, reading, creating, updating, and verifying knowledge cards in Guru.

## Overview

- Search and retrieve knowledge cards by keyword with optional filters for collection and verification status
- Create and update knowledge cards with HTML content, tags, and verification settings
- Verify cards to confirm content accuracy and track verification workflow

## Authentication

This integration uses **System Credentials** with HTTP Basic Authentication (user email as username, API token as password).

### Required Credentials

- `api_token`: Guru API token for authentication
- `user_email`: Guru user email associated with the API token
- `endpoint` (optional): Custom API endpoint (defaults to `https://api.getguru.com/api/v1`)

### Obtaining Credentials

1. Log in to your Guru account
2. Navigate to **Settings** → **Apps & Integrations** → **API Access**
3. Click **Generate User Token**
4. Copy the generated token
5. Use your Guru account email as `user_email`

**Note**: User tokens provide read/write access based on your Guru permissions.

## Available Tools

### search_cards
**Tool Name:** Search Guru Cards
**Capabilities:** read
**Tags:** docs, guru, search
**Description:** Search for knowledge cards in Guru by keyword, with optional filters for collection and verification status

**Input:**
- `query` (string): The search query string
- `collection_id` (string, optional): Optional collection ID to filter results
- `max_results` (number, optional): Maximum number of results to return (1-50, defaults to 20)
- `verification_status` (VerificationStatus, optional): Filter by card verification status. One of: `Trusted`, `NeedsVerification`, `Unverified`

**Output:**
- `cards` (array<CardSummary>): List of matching cards
  - `id` (string): Unique identifier for the card
  - `title` (string): Title of the card
  - `slug` (string): Slug for the card URL
  - `collection` (CollectionInfo, optional): Collection the card belongs to
  - `verification_status` (VerificationStatus): Current verification status
  - `relevance_score` (number): Relevance score from search (0.0-1.0)
  - `last_modified` (string): Last modified timestamp (ISO 8601)
- `total_count` (number): Total number of results available
- `query` (string): The query that was executed

### get_card
**Tool Name:** Get Guru Card
**Capabilities:** read
**Tags:** docs, guru
**Description:** Retrieve a specific Guru card by ID, including its full content and metadata

**Input:**
- `card_id` (string): The unique ID of the card to retrieve
- `include_content` (boolean, optional): Whether to include the full HTML content (defaults to true)

**Output:**
- `card` (Card): The retrieved card
  - `id` (string): Unique identifier for the card
  - `title` (string): Title of the card
  - `slug` (string): Slug for the card URL
  - `content` (string, optional): Full HTML content of the card (if requested)
  - `collection` (CollectionInfo, optional): Collection the card belongs to
  - `verification_status` (VerificationStatus): Current verification status
  - `owner` (UserInfo, optional): User who owns this card
  - `verifier` (UserInfo, optional): User who last verified this card
  - `verification_due_date` (string, optional): Date when verification is next due (ISO 8601)
  - `last_modified` (string): Last modified timestamp (ISO 8601)
  - `created_at` (string): Created timestamp (ISO 8601)
  - `tags` (array<string>): Tags associated with this card

### create_card
**Tool Name:** Create Guru Card
**Capabilities:** write
**Tags:** docs, guru
**Description:** Create a new knowledge card in Guru with the specified title, content, and metadata

**Input:**
- `title` (string): Title of the new card
- `content` (string): HTML content of the card
- `collection_id` (string): ID of the collection to create the card in
- `tags` (array<string>, optional): Optional tags to apply to the card
- `verification_interval_days` (number, optional): Optional verification interval in days
- `verifier_email` (string, optional): Email of the user to assign as verifier

**Output:**
- `card` (Card): The newly created card
- `web_url` (string): URL to view the card in Guru

### update_card
**Tool Name:** Update Guru Card
**Capabilities:** write
**Tags:** docs, guru
**Description:** Update an existing Guru card's title, content, tags, or other metadata

**Input:**
- `card_id` (string): The unique ID of the card to update
- `title` (string, optional): New title for the card
- `content` (string, optional): New HTML content for the card
- `tags` (array<string>, optional): New tags for the card (replaces existing tags if provided)
- `collection_id` (string, optional): New collection ID to move the card to
- `verification_interval_days` (number, optional): New verification interval in days

**Output:**
- `card` (Card): The updated card
- `modified_fields` (array<string>): Fields that were modified

### verify_card
**Tool Name:** Verify Guru Card
**Capabilities:** write
**Tags:** docs, guru, verification
**Description:** Mark a Guru card as verified, confirming its content is accurate and up-to-date

**Input:**
- `card_id` (string): The unique ID of the card to verify
- `comment` (string, optional): Optional comment explaining the verification

**Output:**
- `card_id` (string): The card ID that was verified
- `verification_status` (VerificationStatus): New verification status (should be Trusted after verification)
- `verified_by` (UserInfo): User who performed the verification
  - `email` (string): User's email address
  - `name` (string, optional): User's display name
- `verified_at` (string): Timestamp when the verification was performed (ISO 8601)
- `next_verification_due` (string): Next verification due date (ISO 8601)

## API Documentation

- Base URL: `https://api.getguru.com/api/v1`
- API Documentation: [Guru Developer Docs](https://developer.getguru.com/docs/getting-started)
- API Endpoints Reference: [Guru API Endpoints](https://developer.getguru.com/page/new-api-endpoints-5252022)

## Testing

Run tests:
```bash
cargo test -p brwse-guru
```

## Development

- Crate: brwse-guru
- Source: examples/docs-notes/guru
