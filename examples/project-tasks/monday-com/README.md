# Monday.com Integration for Operai Toolbox

This integration provides tools for managing Monday.com workspaces, including boards, items, columns, comments, and user assignments.

## Overview

- Interact with Monday.com boards, items, and updates through GraphQL API queries and mutations
- Create, read, and update tasks (items) and their column values
- Assign users to items and add comments for collaboration

**Primary Use Cases:**
- Project management automation and task tracking
- Integration of Monday.com with AI agents and workflows
- Programmatic board and item management for custom tools

## Authentication

This integration uses **API Token authentication** (user credential). You need a personal API token from your Monday.com account to authenticate requests.

### Getting Your API Token

1. Log in to your Monday.com account
2. Click on your profile picture in the top right corner
3. Select **Developers**
4. Click **API** token section and copy your personal token

### Required Credentials

The integration uses the `MondayCredential` user credential with the following fields:

- **`api_token`** (required): Personal API token for Monday.com API. Obtain from your Monday.com account settings under Developers > API tokens.
- **`endpoint`** (optional): Custom API endpoint URL. Defaults to `https://api.monday.com/v2` if not provided.

Credentials are supplied through the `MondayCredential` user credential in the toolbox runtime.

## Available Tools

### list_boards
**Tool Name:** List Monday.com Boards
**Capabilities:** read
**Tags:** project-management, monday
**Description:** List boards in the Monday.com workspace

**Input:**
- `limit` (optional, `u32`): Maximum number of boards to return. Defaults to 25.
- `state` (optional, `String`): Filter boards by state (e.g., "active", "archived", "deleted").

**Output:**
- `boards` (`Vec<BoardSummary>`): List of board summaries containing:
  - `id` (`String`): Board ID
  - `name` (`String`): Board name
  - `description` (`Option<String>`): Board description
  - `state` (`Option<String>`): Board state

### list_items
**Tool Name:** List Monday.com Items
**Capabilities:** read
**Tags:** project-management, monday
**Description:** List items (tasks/rows) from a Monday.com board

**Input:**
- `board_id` (`String`): Board ID to list items from
- `limit` (optional, `u32`): Maximum number of items to return. Defaults to 25.

**Output:**
- `items` (`Vec<ItemSummary>`): List of item summaries containing:
  - `id` (`String`): Item ID
  - `name` (`String`): Item name
  - `board` (`Option<BoardRef>`): Reference to parent board with `id` and `name`

### create_item
**Tool Name:** Create Monday.com Item
**Capabilities:** write
**Tags:** project-management, monday
**Description:** Create a new item (task/row) in a Monday.com board

**Input:**
- `board_id` (`String`): Board ID where the item should be created
- `item_name` (`String`): Item name/title
- `column_values` (optional, `serde_json::Value`): Optional column values as JSON object

**Output:**
- `id` (`String`): Created item ID
- `name` (`String`): Item name
- `board` (`BoardRef`): Reference to parent board with `id` and `name`

### update_column
**Tool Name:** Update Monday.com Column
**Capabilities:** write
**Tags:** project-management, monday
**Description:** Update a column value for an item in a Monday.com board

**Input:**
- `board_id` (`String`): Board ID containing the item
- `item_id` (`String`): Item ID to update
- `column_id` (`String`): Column ID to update
- `value` (`serde_json::Value`): New column value as JSON

**Output:**
- `id` (`String`): Updated item ID
- `name` (`String`): Item name
- `updated` (`bool`): Confirmation that the update succeeded

### add_comment
**Tool Name:** Add Monday.com Comment
**Capabilities:** write
**Tags:** project-management, monday
**Description:** Add a comment (update) to a Monday.com item

**Input:**
- `item_id` (`String`): Item ID to add the comment to
- `body` (`String`): Comment text/body

**Output:**
- `id` (`String`): Created comment ID
- `body` (`String`): Comment body text

### assign_user
**Tool Name:** Assign Monday.com User
**Capabilities:** write
**Tags:** project-management, monday
**Description:** Assign a user to an item in Monday.com

**Input:**
- `board_id` (`String`): Board ID containing the item
- `item_id` (`String`): Item ID to assign the user to
- `user_id` (`String`): User ID to assign
- `people_column_id` (optional, `String`): Column ID for the people column. Defaults to "people".

**Output:**
- `id` (`String`): Updated item ID
- `name` (`String`): Item name
- `assigned` (`bool`): Confirmation that the assignment succeeded

## API Documentation

- **Base URL:** `https://api.monday.com/v2`
- **API Documentation:** [Monday.com GraphQL API Reference](https://developer.monday.com/api-reference/docs/introduction-to-graphql)
- **API Token Setup:** [Monday.com Authentication Guide](https://developer.monday.com/api-reference/docs/authentication)

## Testing

Run tests with:
```bash
cargo test -p brwse-monday-com
```

Tests use mock data via `wiremock` and do not require actual Monday.com credentials.

## Development

- **Crate:** `brwse-monday-com`
- **Source:** `examples/project-tasks/monday-com/src/`
