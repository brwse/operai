# Slab Integration for Operai Toolbox

Interact with Slab, a knowledge base platform for teams, enabling search, retrieval, creation, and modification of articles and comments.

## Overview

- **Search articles** across your Slab workspace by keyword with optional filtering by topic and author
- **Retrieve full article content** including metadata, contributors, and optionally comments
- **Create and update articles** with Markdown content, topic organization, and draft/publish control
- **Add comments** to articles or reply to existing comments for collaboration

**Primary Use Cases:**
- Building AI-powered knowledge base assistants that can search and retrieve documentation
- Automating documentation workflows, such as creating articles from templates or external sources
- Enabling collaborative features like commenting on documentation
- Synchronizing content between Slab and other documentation systems

## Authentication

Slab uses OAuth2-style bearer token authentication. Credentials are supplied via the `SlabCredential` user credential definition.

### Required Credentials

- **`access_token`** (required): OAuth2 access token for Slab API authentication

The access token is sent as a Bearer token in the `Authorization` header for all GraphQL API requests to `https://api.slab.com/v1/graphql`.

### Obtaining Credentials

1. Navigate to your Slab team settings
2. Click "Developer" in the left sidebar
3. Copy your API token from the Developer Tools section

**Note:** API access is only available to customers on Slab's Business or Enterprise plans.

## Available Tools

### search_articles
**Tool Name:** Search Articles
**Capabilities:** read
**Tags:** docs, slab, search
**Description:** Search for articles in Slab by keyword, with optional filters for topic and author

**Input:**
- `query` (string, required): The search query string
- `limit` (optional uint32): Maximum number of results to return (default: 20, max: 100)
- `topic_id` (optional string): Filter by topic ID
- `author_id` (optional string): Filter by author user ID

**Output:**
- `results` (array of ArticleSearchResult): The list of matching articles
  - `id` (string): The unique identifier of the article
  - `title` (string): The title of the article
  - `snippet` (string): A snippet of the article content with search terms highlighted
  - `topic` (optional TopicInfo): The topic this article belongs to
    - `id` (string): The unique identifier of the topic
    - `name` (string): The name of the topic
  - `author` (UserInfo): The author of the article
    - `id` (string): The unique identifier of the user
    - `name` (string): The display name of the user
    - `email` (optional string): The email address of the user
  - `updated_at` (string): When the article was last updated (ISO 8601 format)
- `total_count` (uint32): Total number of results matching the query
- `request_id` (string): The request ID for this operation

### get_article
**Tool Name:** Get Article
**Capabilities:** read
**Tags:** docs, slab
**Description:** Retrieve a specific article from Slab by its ID, including content and optionally comments

**Input:**
- `article_id` (string, required): The unique identifier of the article to retrieve
- `include_content` (boolean, optional, default: true): Whether to include the full content
- `include_comments` (boolean, optional, default: false): Whether to include comments on the article

**Output:**
- `id` (string): The unique identifier of the article
- `title` (string): The title of the article
- `content` (optional string): The full content of the article in Markdown format
- `topic` (optional TopicInfo): The topic this article belongs to
  - `id` (string): The unique identifier of the topic
  - `name` (string): The name of the topic
- `author` (UserInfo): The author of the article
  - `id` (string): The unique identifier of the user
  - `name` (string): The display name of the user
  - `email` (optional string): The email address of the user
- `contributors` (array of UserInfo): List of contributors to the article
- `created_at` (string): When the article was created (ISO 8601 format)
- `updated_at` (string): When the article was last updated (ISO 8601 format)
- `version` (uint32): The current version number of the article
- `comments` (optional array of Comment): Comments on the article (if requested)
  - `id` (string): The unique identifier of the comment
  - `content` (string): The content of the comment
  - `author` (UserInfo): The author of the comment
  - `created_at` (string): When the comment was created (ISO 8601 format)
  - `parent_id` (optional string): Parent comment ID if this is a reply
- `request_id` (string): The request ID for this operation

### create_article
**Tool Name:** Create Article
**Capabilities:** write
**Tags:** docs, slab
**Description:** Create a new article in Slab with the specified title, content, and optional topic

**Input:**
- `title` (string, required): The title of the article
- `content` (string, required): The content of the article in Markdown format
- `topic_id` (optional string): The ID of the topic to place the article in
- `publish` (boolean, optional, default: false): Whether to publish the article immediately (default: false, saves as draft)

**Output:**
- `id` (string): The unique identifier of the created article
- `title` (string): The title of the created article
- `url` (string): The URL to view the article in Slab
- `is_published` (boolean): Whether the article is published or a draft
- `request_id` (string): The request ID for this operation

### update_article
**Tool Name:** Update Article
**Capabilities:** write
**Tags:** docs, slab
**Description:** Update an existing article in Slab, modifying title, content, topic, or publish status

**Input:**
- `article_id` (string, required): The unique identifier of the article to update
- `title` (optional string): The new title (keeps current if not provided)
- `content` (optional string): The new content in Markdown format (keeps current if not provided)
- `topic_id` (optional string): The new topic ID (keeps current if not provided)
- `publish` (optional boolean): Whether to publish the article (only applies if currently a draft)

**Output:**
- `id` (string): The unique identifier of the updated article
- `title` (string): The current title of the article
- `url` (string): The URL to view the article in Slab
- `version` (uint32): The new version number after the update
- `is_published` (boolean): Whether the article is published or a draft
- `request_id` (string): The request ID for this operation

### add_comment
**Tool Name:** Add Comment
**Capabilities:** write
**Tags:** docs, slab
**Description:** Add a comment to an article in Slab, optionally as a reply to an existing comment

**Input:**
- `article_id` (string, required): The unique identifier of the article to comment on
- `content` (string, required): The content of the comment
- `parent_comment_id` (optional string): The ID of the parent comment if this is a reply

**Output:**
- `comment_id` (string): The unique identifier of the created comment
- `article_id` (string): The article the comment was added to
- `created_at` (string): When the comment was created (ISO 8601 format)
- `request_id` (string): The request ID for this operation

## API Documentation

- **Base URL:** `https://api.slab.com/v1/graphql`
- **API Type:** GraphQL
- **Authentication:** Bearer token in Authorization header
- **API Documentation:** [Slab Developer Tools Help](https://help.slab.com/en/articles/6545629-developer-tools-api-webhooks)
- **Public Schema:** [Slab GraphQL Schema (Apollo Studio)](https://studio.apollographql.com/public/Slab/home)

## Testing

Run tests:

```bash
cargo test -p slab
```

The integration includes comprehensive unit tests covering:
- Input validation (empty fields, required parameters)
- Credential deserialization
- Output serialization
- Type roundtrips

## Development

- **Crate:** `slab`
- **Source:** `examples/docs-notes/slab/`
