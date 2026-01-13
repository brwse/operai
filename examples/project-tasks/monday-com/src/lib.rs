//! project-tasks/monday-com integration for Operai Toolbox.

mod types;

use std::{collections::HashMap, fmt::Write};

use gql_client::Client as GqlClient;
use operai::{
    Context, JsonSchema, Result, anyhow::anyhow, define_user_credential, ensure, info, init,
    schemars, shutdown, tool,
};
use serde::{Deserialize, Serialize};
use types::{Board, BoardRef, Item, Update};

define_user_credential! {
    MondayCredential("monday") {
        api_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_API_ENDPOINT: &str = "https://api.monday.com/v2";

#[init]
async fn setup() -> Result<()> {
    info!("Monday.com integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Monday.com integration shutting down");
}

// ============================================================================
// LIST BOARDS
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListBoardsInput {
    /// Maximum number of boards to return. Defaults to 25.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Filter boards by state (e.g., "active", "archived", "deleted").
    #[serde(default)]
    pub state: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListBoardsOutput {
    pub boards: Vec<BoardSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BoardSummary {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListBoardsData {
    boards: Vec<Board>,
}

/// # List Monday.com Boards
///
/// Retrieves a list of boards from the user's Monday.com workspace. Use this
/// tool when you need to explore available boards, find a specific board by
/// name or ID, or get an overview of the workspace structure. Returns board
/// summaries including ID, name, description, and state
/// (active/archived/deleted).
///
/// ## When to use
/// - User wants to see what boards are available in their Monday.com workspace
/// - User needs to find a board ID before performing operations on items
/// - User wants to filter boards by state (e.g., only show active boards)
/// - User is exploring their Monday.com workspace structure
///
/// ## Key constraints
/// - The `limit` parameter must be between 1 and 100 (defaults to 25)
/// - Requires valid Monday.com API credentials
/// - Board IDs returned are required for other operations like listing or
///   creating items
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - project-management
/// - monday
///
/// # Errors
///
/// Returns an error if:
/// - The limit parameter is not between 1 and 100
/// - The Monday.com credentials are missing or invalid
/// - The API request fails due to network or server issues
/// - The Monday.com API returns an error response
#[tool]
pub async fn list_boards(ctx: Context, input: ListBoardsInput) -> Result<ListBoardsOutput> {
    let limit = input.limit.unwrap_or(25);
    ensure!(limit > 0 && limit <= 100, "limit must be between 1 and 100");

    let client = MondayClient::from_ctx(&ctx)?;

    let mut query = format!("query {{ boards(limit: {limit}");
    if let Some(state) = &input.state {
        let _ = write!(query, ", state: {state}");
    }
    query.push_str(") { id name description state } }");

    let data: ListBoardsData = client.execute_query(&query).await?;

    Ok(ListBoardsOutput {
        boards: data
            .boards
            .into_iter()
            .map(|b| BoardSummary {
                id: b.id,
                name: b.name,
                description: b.description,
                state: b.state,
            })
            .collect(),
    })
}

// ============================================================================
// LIST ITEMS
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListItemsInput {
    /// Board ID to list items from.
    pub board_id: String,
    /// Maximum number of items to return. Defaults to 25.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListItemsOutput {
    pub items: Vec<ItemSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ItemSummary {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub board: Option<BoardRef>,
}

#[derive(Debug, Deserialize)]
struct ListItemsData {
    boards: Vec<BoardWithItems>,
}

#[derive(Debug, Deserialize)]
struct BoardWithItems {
    items_page: ItemsPage,
}

#[derive(Debug, Deserialize)]
struct ItemsPage {
    items: Vec<Item>,
}

/// # List Monday.com Items
///
/// Retrieves items (tasks/rows) from a specific Monday.com board. Use this tool
/// when you need to view all items within a board, find a specific item by
/// name, or get an overview of the work tracked in a board. Returns item
/// summaries including ID, name, and board reference.
///
/// ## When to use
/// - User wants to see all tasks or items in a specific Monday.com board
/// - User needs to find an item ID before updating it or adding comments
/// - User wants to review the contents of a board
/// - User is looking for specific items by browsing the list
///
/// ## Key constraints
/// - Requires a valid `board_id` (obtain from `list_boards` tool)
/// - The `limit` parameter must be between 1 and 100 (defaults to 25)
/// - Returns only basic item information (ID, name, board reference)
/// - Requires valid Monday.com API credentials
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - project-management
/// - monday
///
/// # Errors
///
/// Returns an error if:
/// - The `board_id` is empty or contains only whitespace
/// - The limit parameter is not between 1 and 100
/// - The Monday.com credentials are missing or invalid
/// - The API request fails due to network or server issues
/// - The Monday.com API returns an error response
#[tool]
pub async fn list_items(ctx: Context, input: ListItemsInput) -> Result<ListItemsOutput> {
    ensure!(
        !input.board_id.trim().is_empty(),
        "board_id must not be empty"
    );
    let limit = input.limit.unwrap_or(25);
    ensure!(limit > 0 && limit <= 100, "limit must be between 1 and 100");

    let client = MondayClient::from_ctx(&ctx)?;

    let query = format!(
        r#"query {{ boards(ids: ["{}"]) {{ items_page(limit: {}) {{ items {{ id name board {{ id name }} }} }} }} }}"#,
        input.board_id, limit
    );

    let data: ListItemsData = client.execute_query(&query).await?;

    if let Some(board) = data.boards.first() {
        Ok(ListItemsOutput {
            items: board
                .items_page
                .items
                .iter()
                .map(|item| ItemSummary {
                    id: item.id.clone(),
                    name: item.name.clone(),
                    board: item.board.clone(),
                })
                .collect(),
        })
    } else {
        Ok(ListItemsOutput { items: vec![] })
    }
}

// ============================================================================
// CREATE ITEM
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateItemInput {
    /// Board ID where the item should be created.
    pub board_id: String,
    /// Item name/title.
    pub item_name: String,
    /// Optional column values as JSON object.
    #[serde(default)]
    pub column_values: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateItemOutput {
    pub id: String,
    pub name: String,
    pub board: BoardRef,
}

#[derive(Debug, Deserialize)]
struct CreateItemData {
    create_item: Item,
}

/// # Create Monday.com Item
///
/// Creates a new item (task/row) in a Monday.com board. Use this tool when the
/// user wants to add a new task, project item, or row to a board. Optionally
/// accepts column values to pre-populate fields like status, priority, dates,
/// or custom columns.
///
/// ## When to use
/// - User wants to create a new task or item in a Monday.com board
/// - User wants to add a new project item with specific field values
/// - User is creating tasks to be assigned or tracked
/// - User wants to add items to a backlog or task list
///
/// ## Key constraints
/// - Requires a valid `board_id` (obtain from `list_boards` tool)
/// - The `item_name` is required and will be the item's title/name
/// - `column_values` is optional but must be valid JSON if provided (format
///   depends on board column types)
/// - Requires valid Monday.com API credentials
/// - Returns the created item's ID and board reference
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - project-management
/// - monday
///
/// # Errors
///
/// Returns an error if:
/// - The `board_id` is empty or contains only whitespace
/// - The `item_name` is empty or contains only whitespace
/// - The Monday.com credentials are missing or invalid
/// - The API request fails due to network or server issues
/// - The Monday.com API returns an error response
#[tool]
pub async fn create_item(ctx: Context, input: CreateItemInput) -> Result<CreateItemOutput> {
    ensure!(
        !input.board_id.trim().is_empty(),
        "board_id must not be empty"
    );
    ensure!(
        !input.item_name.trim().is_empty(),
        "item_name must not be empty"
    );

    let client = MondayClient::from_ctx(&ctx)?;

    let escaped_name = escape_graphql_string(&input.item_name);
    let mut query = format!(
        r#"mutation {{ create_item(board_id: {}, item_name: "{}""#,
        input.board_id, escaped_name
    );

    if let Some(column_values) = &input.column_values {
        let escaped_values = escape_graphql_string(&column_values.to_string());
        let _ = write!(query, r#", column_values: "{escaped_values}""#);
    }

    query.push_str(") { id name board { id name } } }");

    let data: CreateItemData = client.execute_query(&query).await?;

    let item = data.create_item;
    Ok(CreateItemOutput {
        id: item.id,
        name: item.name,
        board: item.board.unwrap_or(BoardRef {
            id: input.board_id,
            name: String::new(),
        }),
    })
}

// ============================================================================
// UPDATE COLUMN
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateColumnInput {
    /// Board ID containing the item.
    pub board_id: String,
    /// Item ID to update.
    pub item_id: String,
    /// Column ID to update.
    pub column_id: String,
    /// New column value as JSON.
    pub value: serde_json::Value,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateColumnOutput {
    pub id: String,
    pub name: String,
    pub updated: bool,
}

#[derive(Debug, Deserialize)]
struct UpdateColumnData {
    change_column_value: Item,
}

/// # Update Monday.com Column
///
/// Updates a specific column value for an item in a Monday.com board. Use this
/// tool when you need to modify a single field of an item, such as changing
/// status, updating dates, setting priority, or modifying any other column
/// type. The value must be formatted as JSON according to the column type.
///
/// ## When to use
/// - User wants to change the status of a task (e.g., from "Working on it" to
///   "Done")
/// - User needs to update a specific field like due date, priority, or custom
///   column
/// - User wants to modify item properties without creating a new item
/// - User needs to set or update column values for an existing item
///
/// ## Key constraints
/// - Requires valid `board_id`, `item_id`, and `column_id` (obtain IDs from
///   `list_boards` and `list_items`)
/// - The `value` must be valid JSON formatted for the specific column type
/// - Common column IDs include "status", "date", "priority", "person", etc.
/// - Column IDs are board-specific and must match the board's column structure
/// - Requires valid Monday.com API credentials
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - project-management
/// - monday
///
/// # Errors
///
/// Returns an error if:
/// - The `board_id` is empty or contains only whitespace
/// - The `item_id` is empty or contains only whitespace
/// - The `column_id` is empty or contains only whitespace
/// - The Monday.com credentials are missing or invalid
/// - The API request fails due to network or server issues
/// - The Monday.com API returns an error response
#[tool]
pub async fn update_column(ctx: Context, input: UpdateColumnInput) -> Result<UpdateColumnOutput> {
    ensure!(
        !input.board_id.trim().is_empty(),
        "board_id must not be empty"
    );
    ensure!(
        !input.item_id.trim().is_empty(),
        "item_id must not be empty"
    );
    ensure!(
        !input.column_id.trim().is_empty(),
        "column_id must not be empty"
    );

    let client = MondayClient::from_ctx(&ctx)?;

    let escaped_value = escape_graphql_string(&input.value.to_string());
    let query = format!(
        r#"mutation {{ change_column_value(board_id: {}, item_id: {}, column_id: "{}", value: "{}") {{ id name }} }}"#,
        input.board_id, input.item_id, input.column_id, escaped_value
    );

    let data: UpdateColumnData = client.execute_query(&query).await?;

    let item = data.change_column_value;
    Ok(UpdateColumnOutput {
        id: item.id,
        name: item.name,
        updated: true,
    })
}

// ============================================================================
// ADD COMMENT (UPDATE)
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddCommentInput {
    /// Item ID to add the comment to.
    pub item_id: String,
    /// Comment text/body.
    pub body: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AddCommentOutput {
    pub id: String,
    pub body: String,
}

#[derive(Debug, Deserialize)]
struct AddCommentData {
    create_update: Update,
}

/// # Add Monday.com Comment
///
/// Adds a comment (update) to a Monday.com item. Use this tool when the user
/// wants to add a note, feedback, question, or any textual comment to an
/// existing task or item. Comments are visible to team members with access to
/// the item and support markdown formatting.
///
/// ## When to use
/// - User wants to add a note or comment to a task
/// - User needs to provide feedback on an item
/// - User wants to ask a question or add context to a task
/// - User is collaborating with team members on an item
///
/// ## Key constraints
/// - Requires a valid `item_id` (obtain from `list_items` tool)
/// - The `body` text is required and will be the comment content
/// - Comments appear in the item's updates feed
/// - Supports text content (markdown formatting may be supported)
/// - Requires valid Monday.com API credentials
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - project-management
/// - monday
///
/// # Errors
///
/// Returns an error if:
/// - The `item_id` is empty or contains only whitespace
/// - The `text` is empty or contains only whitespace
/// - The Monday.com credentials are missing or invalid
/// - The API request fails due to network or server issues
/// - The Monday.com API returns an error response
#[tool]
pub async fn add_comment(ctx: Context, input: AddCommentInput) -> Result<AddCommentOutput> {
    ensure!(
        !input.item_id.trim().is_empty(),
        "item_id must not be empty"
    );
    ensure!(!input.body.trim().is_empty(), "body must not be empty");

    let client = MondayClient::from_ctx(&ctx)?;

    let escaped_body = escape_graphql_string(&input.body);
    let query = format!(
        r#"mutation {{ create_update(item_id: {}, body: "{}") {{ id body }} }}"#,
        input.item_id, escaped_body
    );

    let data: AddCommentData = client.execute_query(&query).await?;

    let update = data.create_update;
    Ok(AddCommentOutput {
        id: update.id,
        body: update.body,
    })
}

// ============================================================================
// ASSIGN USER
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AssignUserInput {
    /// Board ID containing the item.
    pub board_id: String,
    /// Item ID to assign the user to.
    pub item_id: String,
    /// User ID to assign.
    pub user_id: String,
    /// Column ID for the people column. Defaults to "people".
    #[serde(default)]
    pub people_column_id: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AssignUserOutput {
    pub id: String,
    pub name: String,
    pub assigned: bool,
}

/// # Assign Monday.com User
///
/// Assigns a user to a Monday.com item by updating the item's people column.
/// Use this tool when the user wants to assign a task to a team member,
/// reassign ownership, or add someone to an item. This updates the designated
/// people column (defaults to "people" column) with the specified user.
///
/// ## When to use
/// - User wants to assign a task to a specific person
/// - User needs to change the assignee of an existing task
/// - User wants to add a team member to an item
/// - User is delegating work or reassigning ownership
///
/// ## Key constraints
/// - Requires valid `board_id` and `item_id` (obtain from `list_boards` and
///   `list_items`)
/// - Requires a valid `user_id` (Monday.com user ID, typically a numeric
///   string)
/// - Updates the people column specified by `people_column_id` (defaults to
///   "people")
/// - The people column must exist in the board
/// - Overwrites existing people in the column with the specified user
/// - Requires valid Monday.com API credentials
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - project-management
/// - monday
///
/// # Errors
///
/// Returns an error if:
/// - The `board_id` is empty or contains only whitespace
/// - The `item_id` is empty or contains only whitespace
/// - The `user_id` is empty or contains only whitespace
/// - The Monday.com credentials are missing or invalid
/// - The API request fails due to network or server issues
/// - The Monday.com API returns an error response
#[tool]
pub async fn assign_user(ctx: Context, input: AssignUserInput) -> Result<AssignUserOutput> {
    ensure!(
        !input.board_id.trim().is_empty(),
        "board_id must not be empty"
    );
    ensure!(
        !input.item_id.trim().is_empty(),
        "item_id must not be empty"
    );
    ensure!(
        !input.user_id.trim().is_empty(),
        "user_id must not be empty"
    );

    let people_column_id = input.people_column_id.as_deref().unwrap_or("people");

    let client = MondayClient::from_ctx(&ctx)?;

    // Build the people column value JSON
    let column_value = serde_json::json!({
        "personsAndTeams": [{ "id": input.user_id.parse::<i64>().unwrap_or(0), "kind": "person" }]
    });

    let escaped_value = escape_graphql_string(&column_value.to_string());
    let query = format!(
        r#"mutation {{ change_column_value(board_id: {}, item_id: {}, column_id: "{}", value: "{}") {{ id name }} }}"#,
        input.board_id, input.item_id, people_column_id, escaped_value
    );

    let data: UpdateColumnData = client.execute_query(&query).await?;

    let item = data.change_column_value;
    Ok(AssignUserOutput {
        id: item.id,
        name: item.name,
        assigned: true,
    })
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn escape_graphql_string(s: &str) -> String {
    s.replace('\\', r"\\")
        .replace('"', r#"\""#)
        .replace('\n', r"\n")
        .replace('\r', r"\r")
        .replace('\t', r"\t")
}

/// Normalizes a base URL by trimming whitespace and trailing slashes.
///
/// # Errors
///
/// Returns an error if the endpoint is empty or consists only of whitespace.
fn normalize_base_url(endpoint: &str) -> Result<String> {
    let trimmed = endpoint.trim();
    ensure!(!trimmed.is_empty(), "endpoint must not be empty");
    Ok(trimmed.trim_end_matches('/').to_string())
}

// ============================================================================
// MONDAY CLIENT
// ============================================================================

struct MondayClient {
    client: GqlClient,
}

impl std::fmt::Debug for MondayClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MondayClient").finish_non_exhaustive()
    }
}

impl MondayClient {
    /// Creates a new `MondayClient` from the tool context.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The Monday.com credentials are not found in the context
    /// - The `api_token` is empty or contains only whitespace
    /// - The endpoint URL is invalid
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = MondayCredential::get(ctx)?;
        ensure!(
            !cred.api_token.trim().is_empty(),
            "api_token must not be empty"
        );

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_API_ENDPOINT))?;

        let mut headers = HashMap::new();
        headers.insert("authorization", cred.api_token);

        Ok(Self {
            client: GqlClient::new_with_headers(&base_url, headers),
        })
    }

    /// Executes a GraphQL query against the Monday.com API.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails due to network issues
    /// - The GraphQL query returns errors
    /// - The response body cannot be parsed as JSON
    async fn execute_query<T: for<'de> Deserialize<'de>>(&self, query: &str) -> Result<T> {
        self.client
            .query::<T>(query)
            .await
            .map_err(|e| anyhow!("GraphQL error: {e}"))?
            .ok_or_else(|| anyhow!("No data in GraphQL response"))
    }
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_string_contains, header, method, path},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut monday_values = HashMap::new();
        monday_values.insert("api_token".to_string(), "test-token".to_string());
        monday_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("monday", monday_values)
    }

    // ========== Serialization roundtrip tests ==========

    #[test]
    fn test_board_summary_serialization_roundtrip() {
        let board = BoardSummary {
            id: "123".to_string(),
            name: "My Board".to_string(),
            description: Some("Test board".to_string()),
            state: Some("active".to_string()),
        };
        let json = serde_json::to_string(&board).unwrap();
        let parsed: BoardSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(board.id, parsed.id);
        assert_eq!(board.name, parsed.name);
    }

    #[test]
    fn test_escape_graphql_string_handles_quotes() {
        assert_eq!(
            escape_graphql_string(r#"test "quoted" text"#),
            r#"test \"quoted\" text"#
        );
    }

    #[test]
    fn test_escape_graphql_string_handles_newlines() {
        assert_eq!(escape_graphql_string("line1\nline2"), r"line1\nline2");
    }

    #[test]
    fn test_escape_graphql_string_handles_backslashes() {
        assert_eq!(escape_graphql_string(r"test\value"), r"test\\value");
    }

    // ========== Input validation tests ==========

    #[tokio::test]
    async fn test_list_boards_limit_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = list_boards(
            ctx,
            ListBoardsInput {
                limit: Some(0),
                state: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("limit must be between 1 and 100")
        );
    }

    #[tokio::test]
    async fn test_list_boards_limit_exceeds_max_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = list_boards(
            ctx,
            ListBoardsInput {
                limit: Some(101),
                state: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("limit must be between 1 and 100")
        );
    }

    #[tokio::test]
    async fn test_list_items_empty_board_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = list_items(
            ctx,
            ListItemsInput {
                board_id: "  ".to_string(),
                limit: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("board_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_create_item_empty_board_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = create_item(
            ctx,
            CreateItemInput {
                board_id: "  ".to_string(),
                item_name: "Test".to_string(),
                column_values: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("board_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_create_item_empty_name_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = create_item(
            ctx,
            CreateItemInput {
                board_id: "123".to_string(),
                item_name: "  ".to_string(),
                column_values: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("item_name must not be empty")
        );
    }

    #[tokio::test]
    async fn test_update_column_empty_ids_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = update_column(
            ctx,
            UpdateColumnInput {
                board_id: "  ".to_string(),
                item_id: "123".to_string(),
                column_id: "status".to_string(),
                value: serde_json::json!("Done"),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("board_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_add_comment_empty_item_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = add_comment(
            ctx,
            AddCommentInput {
                item_id: "  ".to_string(),
                body: "Test comment".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("item_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_add_comment_empty_body_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = add_comment(
            ctx,
            AddCommentInput {
                item_id: "123".to_string(),
                body: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("body must not be empty")
        );
    }

    // ========== Integration tests ==========

    #[tokio::test]
    async fn test_list_boards_success() {
        let server = MockServer::start().await;
        let response_body = r#"{"data":{"boards":[{"id":"123","name":"My Board","description":"Test","state":"active"}]}}"#;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", "test-token"))
            .and(body_string_contains("boards(limit: 10)"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = list_boards(
            ctx,
            ListBoardsInput {
                limit: Some(10),
                state: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.boards.len(), 1);
        assert_eq!(output.boards[0].id, "123");
        assert_eq!(output.boards[0].name, "My Board");
    }

    #[tokio::test]
    async fn test_list_items_success() {
        let server = MockServer::start().await;
        let response_body = r#"{"data":{"boards":[{"items_page":{"items":[{"id":"456","name":"Task 1","board":{"id":"123","name":"My Board"}}]}}]}}"#;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", "test-token"))
            .and(body_string_contains("items_page"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = list_items(
            ctx,
            ListItemsInput {
                board_id: "123".to_string(),
                limit: Some(10),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.items.len(), 1);
        assert_eq!(output.items[0].id, "456");
        assert_eq!(output.items[0].name, "Task 1");
    }

    #[tokio::test]
    async fn test_create_item_success() {
        let server = MockServer::start().await;
        let response_body = r#"{"data":{"create_item":{"id":"789","name":"New Task","board":{"id":"123","name":"My Board"}}}}"#;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(body_string_contains("create_item"))
            .and(body_string_contains("New Task"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = create_item(
            ctx,
            CreateItemInput {
                board_id: "123".to_string(),
                item_name: "New Task".to_string(),
                column_values: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.id, "789");
        assert_eq!(output.name, "New Task");
        assert_eq!(output.board.id, "123");
    }

    #[tokio::test]
    async fn test_add_comment_success() {
        let server = MockServer::start().await;
        let response_body = r#"{"data":{"create_update":{"id":"999","body":"Great work!"}}}"#;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(body_string_contains("create_update"))
            .and(body_string_contains("Great work!"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = add_comment(
            ctx,
            AddCommentInput {
                item_id: "456".to_string(),
                body: "Great work!".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.id, "999");
        assert_eq!(output.body, "Great work!");
    }

    #[tokio::test]
    async fn test_monday_api_error_returns_error() {
        let server = MockServer::start().await;
        let response_body =
            r#"{"errors":[{"message":"Invalid token","locations":[{"line":1,"column":1}]}]}"#;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let result = list_boards(
            ctx,
            ListBoardsInput {
                limit: Some(10),
                state: None,
            },
        )
        .await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Invalid token"));
    }

    #[tokio::test]
    async fn test_update_column_success() {
        let server = MockServer::start().await;
        let response_body = r#"{"data":{"change_column_value":{"id":"456","name":"Task 1"}}}"#;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(body_string_contains("change_column_value"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = update_column(
            ctx,
            UpdateColumnInput {
                board_id: "123".to_string(),
                item_id: "456".to_string(),
                column_id: "status".to_string(),
                value: serde_json::json!("Done"),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.id, "456");
        assert_eq!(output.name, "Task 1");
        assert!(output.updated);
    }

    #[tokio::test]
    async fn test_assign_user_success() {
        let server = MockServer::start().await;
        let response_body = r#"{"data":{"change_column_value":{"id":"456","name":"Task 1"}}}"#;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(body_string_contains("change_column_value"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = assign_user(
            ctx,
            AssignUserInput {
                board_id: "123".to_string(),
                item_id: "456".to_string(),
                user_id: "789".to_string(),
                people_column_id: Some("people".to_string()),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.id, "456");
        assert_eq!(output.name, "Task 1");
        assert!(output.assigned);
    }

    #[tokio::test]
    async fn test_assign_user_empty_user_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = assign_user(
            ctx,
            AssignUserInput {
                board_id: "123".to_string(),
                item_id: "456".to_string(),
                user_id: "  ".to_string(),
                people_column_id: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("user_id must not be empty")
        );
    }
}
