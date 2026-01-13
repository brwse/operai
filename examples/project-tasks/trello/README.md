# Trello Integration for Operai Toolbox

Manage Trello boards, cards, comments, and checklists through Operai Toolbox tools.

## Overview

- List and filter Trello boards and cards
- Create cards in lists with labels, due dates, and member assignments
- Move cards between lists and boards
- Add comments to cards and manage checklist items

**Primary use cases:**
- Task and project management automation
- Kanban workflow operations
- Trello integration with AI agents and tools

## Authentication

Uses **API Key + Token** authentication (system credential).

### Required Credentials

- `api_key`: Your Trello API key (32 hexadecimal characters)
- `api_token`: Your Trello authentication token

### Getting Credentials

1. Visit https://trello.com/power-ups/admin to create a Power-Up
2. Navigate to the API Key tab and select "Generate a new API Key"
3. To generate a token, visit: `https://trello.com/1/authorize?expiration=never&scope=read,write&response_type=token&key={YourAPIKey}`

### Manifest Configuration

Add the following to your `operai.toml` manifest:

```toml
[[tools]]
package = "trello"
[tools.credentials.trello]
api_key = "your_api_key_here"
api_token = "your_token_here"
```

## Available Tools

### list_boards
**Tool Name:** List Boards
**Description:** Lists all Trello boards accessible to the authenticated user

**Input:**
- `filter` (string, optional): Filter by status - "open", "closed", or "all". Defaults to "open"

**Output:**
- `boards` (array of Board): List of boards matching the filter
  - `id` (string): Unique identifier of the board
  - `name` (string): Name of the board
  - `desc` (string, optional): Optional description of the board
  - `url` (string): URL to access the board
  - `closed` (boolean): Whether the board is closed (archived)
- `count` (number): Total number of boards returned

### list_cards
**Tool Name:** List Cards
**Description:** Lists cards from a Trello board, optionally filtered by list

**Input:**
- `board_id` (string, required): ID of the board to list cards from
- `list_id` (string, optional): Optional list ID to filter cards by specific list
- `filter` (string, optional): Filter cards by status - "all", "open", or "closed". Defaults to "open"

**Output:**
- `cards` (array of Card): List of cards matching the criteria
  - `id` (string): Unique identifier of the card
  - `name` (string): Name/title of the card
  - `desc` (string, optional): Description of the card
  - `idList` (string): ID of the list this card belongs to
  - `idBoard` (string): ID of the board this card belongs to
  - `url` (string): URL to access the card
  - `pos` (number): Position of the card in the list
  - `due` (string, optional): Due date of the card (ISO 8601 format)
  - `dueComplete` (boolean): Whether the due date is complete
  - `closed` (boolean): Whether the card is closed (archived)
  - `labels` (array of Label): Labels attached to the card
- `count` (number): Total number of cards returned

### create_card
**Tool Name:** Create Card
**Description:** Creates a new card in a Trello list

**Input:**
- `list_id` (string, required): ID of the list to create the card in
- `name` (string, required): Name/title of the card
- `desc` (string, optional): Optional description of the card
- `pos` (string, optional): Optional position in the list: "top", "bottom", or a positive number
- `due` (string, optional): Optional due date in ISO 8601 format
- `label_ids` (array of string, optional): Optional list of label IDs to attach to the card
- `member_ids` (array of string, optional): Optional list of member IDs to assign to the card

**Output:**
- `card` (Card): The newly created card with all fields populated

### move_card
**Tool Name:** Move Card
**Description:** Moves a card to a different list (and optionally a different board)

**Input:**
- `card_id` (string, required): ID of the card to move
- `list_id` (string, required): ID of the destination list
- `pos` (string, optional): Optional position in the destination list: "top", "bottom", or a positive number
- `board_id` (string, optional): Optional board ID if moving to a list on a different board

**Output:**
- `card` (Card): The updated card after moving
- `previous_list_id` (string): The previous list ID the card was in

### add_comment
**Tool Name:** Add Comment
**Description:** Adds a comment to a Trello card

**Input:**
- `card_id` (string, required): ID of the card to comment on
- `text` (string, required): The comment text

**Output:**
- `comment` (Comment): The created comment
  - `id` (string): Unique identifier of the comment action
  - `text` (string): The comment text
  - `idMemberCreator` (string): ID of the member who made the comment
  - `date` (string): Date the comment was created (ISO 8601 format)

### add_checklist_item
**Tool Name:** Add Checklist Item
**Description:** Adds an item to a checklist on a Trello card

**Input:**
- `checklist_id` (string, required): ID of the checklist to add the item to
- `name` (string, required): Name/text of the checklist item
- `pos` (string, optional): Optional position: "top", "bottom", or a positive number
- `checked` (boolean, optional): Whether the item should be initially checked. Defaults to false
- `due` (string, optional): Optional due date for the checklist item in ISO 8601 format
- `member_id` (string, optional): Optional member ID to assign the checklist item to

**Output:**
- `check_item` (CheckItem): The created checklist item
  - `id` (string): Unique identifier of the check item
  - `name` (string): Name/text of the check item
  - `state` (string): State of the item: "complete" or "incomplete"
  - `pos` (number): Position of the item in the checklist
- `checklist_id` (string): ID of the checklist the item was added to

## API Documentation

- **Base URL:** `https://api.trello.com/1`
- **API Documentation:** [Trello REST API](https://developer.atlassian.com/cloud/trello/rest/)

### Common Endpoints

- `GET /1/members/me/boards` - List user boards
- `GET /1/boards/{id}/cards` - List cards on a board
- `GET /1/lists/{id}/cards` - List cards in a list
- `POST /1/cards` - Create a card
- `PUT /1/cards/{id}` - Update/move a card
- `POST /1/cards/{id}/actions/comments` - Add a comment
- `POST /1/checklists/{id}/checkItems` - Add checklist item

## Testing

Run tests with:
```bash
cargo test -p trello
```

The integration includes comprehensive tests covering:
- Credential deserialization validation
- Input/output serialization with Trello field naming conventions
- Tool behavior and type conversions
- All 37 tests verify proper API interaction patterns

## Development

- **Crate:** `trello`
- **Source:** `examples/project-tasks/trello`
