//! Trello integration for Operai Toolbox.
//!
//! This integration provides tools for managing Trello boards, cards, comments,
//! and checklists.

use std::sync::OnceLock;

use operai::{
    Context, JsonSchema, Result, bail, define_system_credential, info, init, schemars, shutdown,
    tool,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};

mod types;

define_system_credential! {
    TrelloCredential("trello") {
        api_key: String,
        api_token: String,
    }
}

/// HTTP client for Trello API requests.
static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

fn http_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .build()
            .expect("Failed to create HTTP client")
    })
}

/// Trello API base URL.
const TRELLO_API_BASE: &str = "https://api.trello.com/1";

/// Initialize the Trello integration.
///
/// # Errors
///
/// This function returns an error if the initialization process fails.
#[init]
async fn setup() -> Result<()> {
    info!("Trello integration initialized");
    Ok(())
}

/// Clean up resources when the integration is unloaded.
#[shutdown]
fn cleanup() {
    info!("Trello integration shutting down");
}

// ============================================================================
// Common Types
// ============================================================================

/// Represents a Trello board.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Board {
    /// Unique identifier of the board.
    pub id: String,
    /// Name of the board.
    pub name: String,
    /// Optional description of the board.
    #[serde(default)]
    pub desc: Option<String>,
    /// URL to access the board.
    pub url: String,
    /// Whether the board is closed (archived).
    #[serde(default)]
    pub closed: bool,
}

/// Represents a Trello list within a board.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct List {
    /// Unique identifier of the list.
    pub id: String,
    /// Name of the list.
    pub name: String,
    /// ID of the board this list belongs to.
    #[serde(rename = "idBoard")]
    pub id_board: String,
    /// Position of the list on the board.
    pub pos: f64,
    /// Whether the list is closed (archived).
    #[serde(default)]
    pub closed: bool,
}

/// Represents a Trello card.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Card {
    /// Unique identifier of the card.
    pub id: String,
    /// Name/title of the card.
    pub name: String,
    /// Description of the card.
    #[serde(default)]
    pub desc: Option<String>,
    /// ID of the list this card belongs to.
    #[serde(rename = "idList")]
    pub id_list: String,
    /// ID of the board this card belongs to.
    #[serde(rename = "idBoard")]
    pub id_board: String,
    /// URL to access the card.
    pub url: String,
    /// Position of the card in the list.
    pub pos: f64,
    /// Due date of the card (ISO 8601 format).
    #[serde(default)]
    pub due: Option<String>,
    /// Whether the due date is complete.
    #[serde(rename = "dueComplete", default)]
    pub due_complete: bool,
    /// Whether the card is closed (archived).
    #[serde(default)]
    pub closed: bool,
    /// Labels attached to the card.
    #[serde(default)]
    pub labels: Vec<Label>,
}

/// Represents a label on a card.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Label {
    /// Unique identifier of the label.
    pub id: String,
    /// Name of the label.
    #[serde(default)]
    pub name: Option<String>,
    /// Color of the label.
    pub color: String,
}

/// Represents a comment (action) on a card.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Comment {
    /// Unique identifier of the comment action.
    pub id: String,
    /// The comment text.
    pub text: String,
    /// ID of the member who made the comment.
    #[serde(rename = "idMemberCreator")]
    pub id_member_creator: String,
    /// Date the comment was created (ISO 8601 format).
    pub date: String,
}

/// Represents a checklist on a card.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Checklist {
    /// Unique identifier of the checklist.
    pub id: String,
    /// Name of the checklist.
    pub name: String,
    /// ID of the card this checklist belongs to.
    #[serde(rename = "idCard")]
    pub id_card: String,
    /// Items in the checklist.
    #[serde(rename = "checkItems", default)]
    pub check_items: Vec<CheckItem>,
}

/// Represents an item in a checklist.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CheckItem {
    /// Unique identifier of the check item.
    pub id: String,
    /// Name/text of the check item.
    pub name: String,
    /// State of the item: "complete" or "incomplete".
    pub state: String,
    /// Position of the item in the checklist.
    pub pos: f64,
}

/// Error response from Trello API.
#[derive(Debug, Deserialize)]
struct TrelloErrorResponse {
    error: Option<String>,
}

/// Helper function to make authenticated GET requests to Trello API.
async fn trello_get<T: for<'de> Deserialize<'de>>(
    credential: &TrelloCredential,
    path: &str,
    params: &[(&str, &str)],
) -> Result<T> {
    let mut query_params = vec![
        ("key", credential.api_key.as_str()),
        ("token", credential.api_token.as_str()),
    ];
    query_params.extend_from_slice(params);

    let url = format!("{TRELLO_API_BASE}{path}");
    let response: reqwest::Response = http_client()
        .get(&url)
        .query(&query_params)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send request: {e}"))?;

    let status = response.status();
    let response_text: String = response
        .text()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read response: {e}"))?;

    if !status.is_success() {
        let error_msg =
            if let Ok(error) = serde_json::from_str::<TrelloErrorResponse>(&response_text) {
                error.error.unwrap_or_else(|| response_text.clone())
            } else {
                response_text.clone()
            };
        bail!("Trello API error ({status}): {error_msg}");
    }

    serde_json::from_str(&response_text)
        .map_err(|e| anyhow::anyhow!("Failed to parse response: {e}"))
}

/// Helper function to make authenticated POST requests to Trello API.
async fn trello_post<T: for<'de> Deserialize<'de>>(
    credential: &TrelloCredential,
    path: &str,
    params: &[(&str, &str)],
) -> Result<T> {
    let mut query_params = vec![
        ("key", credential.api_key.as_str()),
        ("token", credential.api_token.as_str()),
    ];
    query_params.extend_from_slice(params);

    let url = format!("{TRELLO_API_BASE}{path}");
    let response: reqwest::Response = http_client()
        .post(&url)
        .form(&query_params)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send request: {e}"))?;

    let status = response.status();
    let response_text: String = response
        .text()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read response: {e}"))?;

    if !status.is_success() {
        let error_msg =
            if let Ok(error) = serde_json::from_str::<TrelloErrorResponse>(&response_text) {
                error.error.unwrap_or_else(|| response_text.clone())
            } else {
                response_text.clone()
            };
        bail!("Trello API error ({status}): {error_msg}");
    }

    serde_json::from_str(&response_text)
        .map_err(|e| anyhow::anyhow!("Failed to parse response: {e}"))
}

/// Helper function to make authenticated PUT requests to Trello API.
async fn trello_put<T: for<'de> Deserialize<'de>>(
    credential: &TrelloCredential,
    path: &str,
    params: &[(&str, &str)],
) -> Result<T> {
    let mut query_params = vec![
        ("key", credential.api_key.as_str()),
        ("token", credential.api_token.as_str()),
    ];
    query_params.extend_from_slice(params);

    let url = format!("{TRELLO_API_BASE}{path}");
    let response: reqwest::Response = http_client()
        .put(&url)
        .form(&query_params)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send request: {e}"))?;

    let status = response.status();
    let response_text: String = response
        .text()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read response: {e}"))?;

    if !status.is_success() {
        let error_msg =
            if let Ok(error) = serde_json::from_str::<TrelloErrorResponse>(&response_text) {
                error.error.unwrap_or_else(|| response_text.clone())
            } else {
                response_text.clone()
            };
        bail!("Trello API error ({status}): {error_msg}");
    }

    serde_json::from_str(&response_text)
        .map_err(|e| anyhow::anyhow!("Failed to parse response: {e}"))
}

// ============================================================================
// Tool: List Boards
// ============================================================================

/// Input for listing boards.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListBoardsInput {
    /// Filter boards by status. Options: "all", "open", "closed". Defaults to
    /// "open".
    #[serde(default = "default_board_filter")]
    pub filter: String,
}

fn default_board_filter() -> String {
    "open".to_string()
}

/// Output from listing boards.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ListBoardsOutput {
    /// List of boards matching the filter.
    pub boards: Vec<Board>,
    /// Total number of boards returned.
    pub count: usize,
}

/// # List Trello Boards
///
/// Retrieves all Trello boards accessible to the authenticated user, with
/// optional filtering by board status.
///
/// Use this tool when a user wants to:
/// - View all their Trello boards
/// - Find a specific board by name or description
/// - List only open (active) boards or include archived/closed boards
/// - Get board IDs needed for other Trello operations
///
/// The tool supports three filter modes:
/// - "open": Returns only active, non-archived boards (default)
/// - "closed": Returns only archived/closed boards
/// - "all": Returns all boards regardless of status
///
/// Returns board details including ID, name, description, URL, and closed
/// status.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - project-management
/// - trello
///
/// # Errors
///
/// This function returns an error if:
/// - The Trello API credentials are invalid or expired
/// - Network connectivity issues prevent reaching the Trello API
/// - The Trello API returns an error response (e.g., rate limiting, service
///   unavailability)
/// - The response from Trello cannot be parsed or deserialized
#[tool]
pub async fn list_boards(ctx: Context, input: ListBoardsInput) -> Result<ListBoardsOutput> {
    let credential = TrelloCredential::get(&ctx)?;
    let filter = input.filter.as_str();

    let boards: Vec<Board> = trello_get(
        &credential,
        &format!("/members/me/boards?filter={filter}"),
        &[],
    )
    .await?;

    let count = boards.len();
    Ok(ListBoardsOutput { boards, count })
}

// ============================================================================
// Tool: List Cards
// ============================================================================

/// Input for listing cards.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListCardsInput {
    /// ID of the board to list cards from.
    pub board_id: String,
    /// Optional list ID to filter cards by specific list.
    #[serde(default)]
    pub list_id: Option<String>,
    /// Filter cards by status. Options: "all", "open", "closed". Defaults to
    /// "open".
    #[serde(default = "default_card_filter")]
    pub filter: String,
}

fn default_card_filter() -> String {
    "open".to_string()
}

/// Output from listing cards.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ListCardsOutput {
    /// List of cards matching the criteria.
    pub cards: Vec<Card>,
    /// Total number of cards returned.
    pub count: usize,
}

/// # List Trello Cards
///
/// Retrieves cards from a Trello board, with optional filtering by list and
/// card status.
///
/// Use this tool when a user wants to:
/// - View all cards on a specific board
/// - See cards in a particular list (e.g., "In Progress", "Done")
/// - Find cards by name, description, or labels
/// - Get card IDs needed for updating, moving, or commenting on cards
/// - Review task assignments, due dates, and card metadata
///
/// This tool can operate in two modes:
/// 1. **Board-wide**: Returns all cards from a board when only `board_id` is
///    provided
/// 2. **List-specific**: Returns only cards from a specific list when `list_id`
///    is also provided
///
/// Cards can be filtered by status:
/// - "open": Returns only active, non-archived cards (default)
/// - "closed": Returns only archived cards
/// - "all": Returns all cards regardless of status
///
/// Returns card details including ID, name, description, list/board membership,
/// position, due date, labels, and URL.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - project-management
/// - trello
///
/// # Errors
///
/// This function returns an error if:
/// - The provided `board_id` or `list_id` does not exist or is inaccessible
/// - The Trello API credentials are invalid or expired
/// - Network connectivity issues prevent reaching the Trello API
/// - The Trello API returns an error response (e.g., rate limiting, service
///   unavailability)
/// - The response from Trello cannot be parsed or deserialized
#[tool]
pub async fn list_cards(ctx: Context, input: ListCardsInput) -> Result<ListCardsOutput> {
    let credential = TrelloCredential::get(&ctx)?;

    let cards: Vec<Card> = if let Some(list_id) = &input.list_id {
        // GET /1/lists/{id}/cards
        trello_get(
            &credential,
            &format!("/lists/{list_id}/cards"),
            &[("filter", input.filter.as_str())],
        )
        .await?
    } else {
        // GET /1/boards/{id}/cards
        trello_get(
            &credential,
            &format!("/boards/{}/cards", input.board_id),
            &[("filter", input.filter.as_str())],
        )
        .await?
    };

    let count = cards.len();
    Ok(ListCardsOutput { cards, count })
}

// ============================================================================
// Tool: Create Card
// ============================================================================

/// Input for creating a card.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateCardInput {
    /// ID of the list to create the card in.
    pub list_id: String,
    /// Name/title of the card.
    pub name: String,
    /// Optional description of the card.
    #[serde(default)]
    pub desc: Option<String>,
    /// Optional position in the list: "top", "bottom", or a positive number.
    #[serde(default)]
    pub pos: Option<String>,
    /// Optional due date in ISO 8601 format.
    #[serde(default)]
    pub due: Option<String>,
    /// Optional list of label IDs to attach to the card.
    #[serde(default)]
    pub label_ids: Vec<String>,
    /// Optional list of member IDs to assign to the card.
    #[serde(default)]
    pub member_ids: Vec<String>,
}

/// Output from creating a card.
#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateCardOutput {
    /// The newly created card.
    pub card: Card,
}

/// # Create Trello Card
///
/// Creates a new card in a Trello list with optional metadata including
/// description, position, due date, labels, and member assignments.
///
/// Use this tool when a user wants to:
/// - Create a new task or work item
/// - Add a card to a specific list (e.g., "To Do", "In Progress")
/// - Set due dates for time-sensitive tasks
/// - Assign labels for categorization (e.g., priority, bug, feature)
/// - Assign team members to a card
/// - Position the card at the top, bottom, or a specific location in the list
///
/// **Key requirements:**
/// - `list_id`: The ID of the list where the card will be created (obtained
///   from `list_cards` or board structure)
/// - `name`: A descriptive title for the card (required)
///
/// **Optional enhancements:**
/// - `desc`: Detailed description of the task or work item
/// - `pos`: Card position ("top", "bottom", or a numeric value)
/// - `due`: ISO 8601 formatted due date (e.g., "2024-12-31T23:59:59.000Z")
/// - `label_ids`: Array of label IDs to categorize the card
/// - `member_ids`: Array of member IDs to assign to the card
///
/// Returns the created card with all generated IDs and metadata.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - project-management
/// - trello
///
/// # Errors
///
/// This function returns an error if:
/// - The provided `list_id` does not exist or is inaccessible
/// - The Trello API credentials are invalid or expired
/// - Network connectivity issues prevent reaching the Trello API
/// - The Trello API returns an error response (e.g., rate limiting, service
///   unavailability)
/// - The provided `label_ids` or `member_ids` are invalid or inaccessible
/// - The due date is in an invalid format
/// - The response from Trello cannot be parsed or deserialized
#[tool]
pub async fn create_card(ctx: Context, input: CreateCardInput) -> Result<CreateCardOutput> {
    let credential = TrelloCredential::get(&ctx)?;

    let mut params = vec![
        ("name", input.name.as_str()),
        ("idList", input.list_id.as_str()),
    ];

    if let Some(desc) = &input.desc {
        params.push(("desc", desc.as_str()));
    }
    if let Some(pos) = &input.pos {
        params.push(("pos", pos.as_str()));
    }
    if let Some(due) = &input.due {
        params.push(("due", due.as_str()));
    }

    // Handle label_ids - Trello API expects multiple idLabels parameters
    for label_id in &input.label_ids {
        params.push(("idLabels", label_id.as_str()));
    }

    // Handle member_ids - Trello API expects multiple idMembers parameters
    for member_id in &input.member_ids {
        params.push(("idMembers", member_id.as_str()));
    }

    let card: Card = trello_post(&credential, "/cards", &params).await?;

    Ok(CreateCardOutput { card })
}

// ============================================================================
// Tool: Move Card
// ============================================================================

/// Input for moving a card to a different list.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct MoveCardInput {
    /// ID of the card to move.
    pub card_id: String,
    /// ID of the destination list.
    pub list_id: String,
    /// Optional position in the destination list: "top", "bottom", or a
    /// positive number.
    #[serde(default)]
    pub pos: Option<String>,
    /// Optional board ID if moving to a list on a different board.
    #[serde(default)]
    pub board_id: Option<String>,
}

/// Output from moving a card.
#[derive(Debug, Serialize, JsonSchema)]
pub struct MoveCardOutput {
    /// The updated card after moving.
    pub card: Card,
    /// The previous list ID the card was in.
    pub previous_list_id: String,
}

/// # Move Trello Card
///
/// Moves a card to a different list within the same board or to a list on a
/// different board, with optional position control.
///
/// Use this tool when a user wants to:
/// - Advance a card through workflow stages (e.g., "To Do" → "In Progress" →
///   "Done")
/// - Reorder cards within a list (prioritize by moving to top/bottom)
/// - Transfer a card to a different board
/// - Reposition a card at a specific location in the target list
///
/// **Key requirements:**
/// - `card_id`: The ID of the card to move (obtained from `list_cards`)
/// - `list_id`: The ID of the destination list
///
/// **Optional controls:**
/// - `pos`: Position in the target list ("top", "bottom", or a numeric value)
/// - `board_id`: Required only when moving to a list on a different board
///
/// **Common use cases:**
/// - Progress tracking: Move cards through sequential lists as work progresses
/// - Prioritization: Move important cards to the "top" position
/// - Cross-board workflows: Transfer cards between project boards
/// - Cleanup: Archive cards by moving to a "Done" or "Archive" list
///
/// Returns the updated card with its new location and the previous list ID for
/// reference.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - project-management
/// - trello
///
/// # Errors
///
/// This function returns an error if:
/// - The provided `card_id` does not exist or is inaccessible
/// - The provided `list_id` does not exist or is inaccessible
/// - The provided `board_id` (if specified) does not exist or is inaccessible
/// - The Trello API credentials are invalid or expired
/// - Network connectivity issues prevent reaching the Trello API
/// - The Trello API returns an error response (e.g., rate limiting, service
///   unavailability)
/// - The response from Trello cannot be parsed or deserialized
#[tool]
pub async fn move_card(ctx: Context, input: MoveCardInput) -> Result<MoveCardOutput> {
    let credential = TrelloCredential::get(&ctx)?;

    // First, get the current card to retrieve the previous list ID
    let current_card: Card =
        trello_get(&credential, &format!("/cards/{}", input.card_id), &[]).await?;
    let previous_list_id = current_card.id_list.clone();

    let mut params = vec![("idList", input.list_id.as_str())];

    if let Some(pos) = &input.pos {
        params.push(("pos", pos.as_str()));
    }
    if let Some(board_id) = &input.board_id {
        params.push(("idBoard", board_id.as_str()));
    }

    let card: Card = trello_put(&credential, &format!("/cards/{}", input.card_id), &params).await?;

    Ok(MoveCardOutput {
        card,
        previous_list_id,
    })
}

// ============================================================================
// Tool: Add Comment
// ============================================================================

/// Trello API response for comment actions.
#[derive(Debug, Deserialize)]
struct TrelloAction {
    id: String,
    #[serde(rename = "idMemberCreator")]
    id_member_creator: String,
    date: String,
    data: ActionData,
}

/// Data field within Trello API action response.
#[derive(Debug, Deserialize)]
struct ActionData {
    text: String,
}

/// Input for adding a comment to a card.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddCommentInput {
    /// ID of the card to comment on.
    pub card_id: String,
    /// The comment text.
    pub text: String,
}

/// Output from adding a comment.
#[derive(Debug, Serialize, JsonSchema)]
pub struct AddCommentOutput {
    /// The created comment.
    pub comment: Comment,
}

/// # Add Trello Comment
///
/// Adds a text comment to a Trello card for collaboration and communication.
///
/// Use this tool when a user wants to:
/// - Add notes, questions, or feedback to a card
/// - Communicate with team members about a specific task
/// - Document progress, decisions, or updates on a card
/// - Provide context or clarification for a work item
/// - Collaborate asynchronously on tasks
///
/// **Key requirements:**
/// - `card_id`: The ID of the card to comment on (obtained from `list_cards`)
/// - `text`: The comment text to add
///
/// **Best practices:**
/// - Be concise and specific in comments
/// - Mention relevant team members using @mentions (if supported)
/// - Use comments to explain why something was done, not just what was done
/// - Reference related cards, attachments, or external resources when helpful
///
/// **Note:** Comments appear in the card's activity feed and are visible to all
/// members with access to the card.
///
/// Returns the created comment with its ID, author information, and timestamp.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - project-management
/// - trello
///
/// # Errors
///
/// This function returns an error if:
/// - The provided `card_id` does not exist or is inaccessible
/// - The Trello API credentials are invalid or expired
/// - Network connectivity issues prevent reaching the Trello API
/// - The Trello API returns an error response (e.g., rate limiting, service
///   unavailability)
/// - The comment text exceeds Trello's length limits
/// - The response from Trello cannot be parsed or deserialized
#[tool]
pub async fn add_comment(ctx: Context, input: AddCommentInput) -> Result<AddCommentOutput> {
    let credential = TrelloCredential::get(&ctx)?;

    let action: TrelloAction = trello_post(
        &credential,
        &format!("/cards/{}/actions/comments", input.card_id),
        &[("text", input.text.as_str())],
    )
    .await?;

    let comment = Comment {
        id: action.id,
        text: action.data.text,
        id_member_creator: action.id_member_creator,
        date: action.date,
    };

    Ok(AddCommentOutput { comment })
}

// ============================================================================
// Tool: Add Checklist Item
// ============================================================================

/// Input for adding an item to a checklist.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddChecklistItemInput {
    /// ID of the checklist to add the item to.
    pub checklist_id: String,
    /// Name/text of the checklist item.
    pub name: String,
    /// Optional position: "top", "bottom", or a positive number.
    #[serde(default)]
    pub pos: Option<String>,
    /// Whether the item should be initially checked. Defaults to false.
    #[serde(default)]
    pub checked: bool,
    /// Optional due date for the checklist item in ISO 8601 format.
    #[serde(default)]
    pub due: Option<String>,
    /// Optional member ID to assign the checklist item to.
    #[serde(default)]
    pub member_id: Option<String>,
}

/// Output from adding a checklist item.
#[derive(Debug, Serialize, JsonSchema)]
pub struct AddChecklistItemOutput {
    /// The created checklist item.
    pub check_item: CheckItem,
    /// ID of the checklist the item was added to.
    pub checklist_id: String,
}

/// # Add Trello Checklist Item
///
/// Adds a new item to an existing checklist on a Trello card, with optional
/// state, position, due date, and member assignment.
///
/// Use this tool when a user wants to:
/// - Break down a card into smaller, trackable subtasks
/// - Create a step-by-step task list within a card
/// - Add action items or requirements to a checklist
/// - Assign specific checklist items to team members
/// - Set due dates for individual checklist items
/// - Pre-populate a checklist with items for recurring tasks
///
/// **Key requirements:**
/// - `checklist_id`: The ID of the checklist to add the item to
/// - `name`: The text describing the checklist item
///
/// **Optional enhancements:**
/// - `pos`: Position in the checklist ("top", "bottom", or a numeric value)
/// - `checked`: Initial state (true = pre-checked as complete, false =
///   incomplete)
/// - `due`: ISO 8601 formatted due date for this specific item
/// - `member_id`: Assign this checklist item to a specific team member
///
/// **Common use cases:**
/// - Task breakdown: Decompose complex tasks into actionable steps
/// - Definition of Done: Create completion criteria for a card
/// - Recurring processes: Template checklists for repeated workflows
/// - Individual accountability: Assign specific subtasks to different team
///   members
///
/// **Note:** The checklist must exist before adding items. Use Trello's UI or
/// other tools to create the checklist first.
///
/// Returns the created checklist item with its generated ID and state.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - project-management
/// - trello
///
/// # Errors
///
/// This function returns an error if:
/// - The provided `checklist_id` does not exist or is inaccessible
/// - The Trello API credentials are invalid or expired
/// - Network connectivity issues prevent reaching the Trello API
/// - The Trello API returns an error response (e.g., rate limiting, service
///   unavailability)
/// - The provided `member_id` (if specified) is invalid or inaccessible
/// - The due date is in an invalid format
/// - The response from Trello cannot be parsed or deserialized
#[tool]
pub async fn add_checklist_item(
    ctx: Context,
    input: AddChecklistItemInput,
) -> Result<AddChecklistItemOutput> {
    let credential = TrelloCredential::get(&ctx)?;

    let mut params = vec![("name", input.name.as_str())];

    if let Some(pos) = &input.pos {
        params.push(("pos", pos.as_str()));
    }

    let state = if input.checked {
        "complete"
    } else {
        "incomplete"
    };
    params.push(("state", state));

    if let Some(due) = &input.due {
        params.push(("due", due.as_str()));
    }
    if let Some(member_id) = &input.member_id {
        params.push(("idMember", member_id.as_str()));
    }

    let check_item: CheckItem = trello_post(
        &credential,
        &format!("/checklists/{}/checkItems", input.checklist_id),
        &params,
    )
    .await?;

    Ok(AddChecklistItemOutput {
        check_item,
        checklist_id: input.checklist_id,
    })
}

operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Credential Tests
    // ========================================================================

    #[test]
    fn test_trello_credential_deserializes_with_required_fields() {
        let json = r#"{ "api_key": "abc123", "api_token": "token456" }"#;

        let cred: TrelloCredential = serde_json::from_str(json).unwrap();

        assert_eq!(cred.api_key, "abc123");
        assert_eq!(cred.api_token, "token456");
    }

    #[test]
    fn test_trello_credential_missing_api_key_returns_error() {
        let json = r#"{ "api_token": "token456" }"#;

        let err = serde_json::from_str::<TrelloCredential>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `api_key`"));
    }

    #[test]
    fn test_trello_credential_missing_api_token_returns_error() {
        let json = r#"{ "api_key": "abc123" }"#;

        let err = serde_json::from_str::<TrelloCredential>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `api_token`"));
    }

    // ========================================================================
    // List Boards Tests
    // ========================================================================

    #[test]
    fn test_list_boards_input_deserializes_with_default_filter() {
        let json = r"{}";

        let input: ListBoardsInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.filter, "open");
    }

    #[test]
    fn test_list_boards_input_deserializes_with_custom_filter() {
        let json = r#"{ "filter": "all" }"#;

        let input: ListBoardsInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.filter, "all");
    }

    #[tokio::test]
    async fn test_list_boards_output_serializes_correctly() {
        let output = ListBoardsOutput {
            boards: vec![Board {
                id: "board_1".to_string(),
                name: "My Board".to_string(),
                desc: Some("A test board".to_string()),
                url: "https://trello.com/b/abc123".to_string(),
                closed: false,
            }],
            count: 1,
        };

        let json = serde_json::to_value(&output).unwrap();

        assert_eq!(json["count"], 1);
        assert_eq!(json["boards"][0]["id"], "board_1");
        assert_eq!(json["boards"][0]["name"], "My Board");
    }

    // ========================================================================
    // List Cards Tests
    // ========================================================================

    #[test]
    fn test_list_cards_input_deserializes_with_required_board_id() {
        let json = r#"{ "board_id": "board_123" }"#;

        let input: ListCardsInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.board_id, "board_123");
        assert_eq!(input.list_id, None);
        assert_eq!(input.filter, "open");
    }

    #[test]
    fn test_list_cards_input_deserializes_with_all_fields() {
        let json = r#"{ "board_id": "board_123", "list_id": "list_456", "filter": "all" }"#;

        let input: ListCardsInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.board_id, "board_123");
        assert_eq!(input.list_id, Some("list_456".to_string()));
        assert_eq!(input.filter, "all");
    }

    #[test]
    fn test_list_cards_input_missing_board_id_returns_error() {
        let json = r#"{ "list_id": "list_456" }"#;

        let err = serde_json::from_str::<ListCardsInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `board_id`"));
    }

    #[tokio::test]
    async fn test_list_cards_output_serializes_correctly() {
        let output = ListCardsOutput {
            cards: vec![Card {
                id: "card_1".to_string(),
                name: "Task 1".to_string(),
                desc: Some("Do something".to_string()),
                id_list: "list_1".to_string(),
                id_board: "board_1".to_string(),
                url: "https://trello.com/c/xyz".to_string(),
                pos: 16384.0,
                due: Some("2024-12-31T23:59:59.000Z".to_string()),
                due_complete: false,
                closed: false,
                labels: vec![Label {
                    id: "label_1".to_string(),
                    name: Some("Priority".to_string()),
                    color: "red".to_string(),
                }],
            }],
            count: 1,
        };

        let json = serde_json::to_value(&output).unwrap();

        assert_eq!(json["count"], 1);
        assert_eq!(json["cards"][0]["name"], "Task 1");
        assert_eq!(json["cards"][0]["labels"][0]["color"], "red");
    }

    // ========================================================================
    // Create Card Tests
    // ========================================================================

    #[test]
    fn test_create_card_input_deserializes_with_required_fields() {
        let json = r#"{ "list_id": "list_123", "name": "New Task" }"#;

        let input: CreateCardInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.list_id, "list_123");
        assert_eq!(input.name, "New Task");
        assert_eq!(input.desc, None);
        assert!(input.label_ids.is_empty());
    }

    #[test]
    fn test_create_card_input_deserializes_with_all_fields() {
        let json = r#"{
            "list_id": "list_123",
            "name": "New Task",
            "desc": "Task description",
            "pos": "top",
            "due": "2024-12-31T23:59:59.000Z",
            "label_ids": ["label_1", "label_2"],
            "member_ids": ["member_1"]
        }"#;

        let input: CreateCardInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.list_id, "list_123");
        assert_eq!(input.name, "New Task");
        assert_eq!(input.desc, Some("Task description".to_string()));
        assert_eq!(input.pos, Some("top".to_string()));
        assert_eq!(input.due, Some("2024-12-31T23:59:59.000Z".to_string()));
        assert_eq!(input.label_ids, vec!["label_1", "label_2"]);
        assert_eq!(input.member_ids, vec!["member_1"]);
    }

    #[test]
    fn test_create_card_input_missing_name_returns_error() {
        let json = r#"{ "list_id": "list_123" }"#;

        let err = serde_json::from_str::<CreateCardInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `name`"));
    }

    #[test]
    fn test_create_card_output_serializes_correctly() {
        let output = CreateCardOutput {
            card: Card {
                id: "card_new".to_string(),
                name: "Test Card".to_string(),
                desc: None,
                id_list: "list_123".to_string(),
                id_board: "board_1".to_string(),
                url: "https://trello.com/c/card_new".to_string(),
                pos: 65536.0,
                due: None,
                due_complete: false,
                closed: false,
                labels: vec![],
            },
        };

        let json = serde_json::to_value(&output).unwrap();

        assert!(json["card"]["id"].is_string());
        assert_eq!(json["card"]["name"], "Test Card");
        assert_eq!(json["card"]["closed"], false);
    }

    // ========================================================================
    // Move Card Tests
    // ========================================================================

    #[test]
    fn test_move_card_input_deserializes_with_required_fields() {
        let json = r#"{ "card_id": "card_123", "list_id": "list_456" }"#;

        let input: MoveCardInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.card_id, "card_123");
        assert_eq!(input.list_id, "list_456");
        assert_eq!(input.pos, None);
        assert_eq!(input.board_id, None);
    }

    #[test]
    fn test_move_card_input_deserializes_with_all_fields() {
        let json = r#"{
            "card_id": "card_123",
            "list_id": "list_456",
            "pos": "bottom",
            "board_id": "board_789"
        }"#;

        let input: MoveCardInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.card_id, "card_123");
        assert_eq!(input.list_id, "list_456");
        assert_eq!(input.pos, Some("bottom".to_string()));
        assert_eq!(input.board_id, Some("board_789".to_string()));
    }

    #[test]
    fn test_move_card_input_missing_card_id_returns_error() {
        let json = r#"{ "list_id": "list_456" }"#;

        let err = serde_json::from_str::<MoveCardInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `card_id`"));
    }

    #[test]
    fn test_move_card_output_serializes_correctly() {
        let output = MoveCardOutput {
            card: Card {
                id: "card_123".to_string(),
                name: "Moved Card".to_string(),
                desc: None,
                id_list: "list_new".to_string(),
                id_board: "board_1".to_string(),
                url: "https://trello.com/c/card_moved".to_string(),
                pos: 65536.0,
                due: None,
                due_complete: false,
                closed: false,
                labels: vec![],
            },
            previous_list_id: "list_old".to_string(),
        };

        let json = serde_json::to_value(&output).unwrap();

        assert_eq!(json["card"]["id"], "card_123");
        assert_eq!(json["card"]["idList"], "list_new");
        assert_eq!(json["previous_list_id"], "list_old");
    }

    // ========================================================================
    // Add Comment Tests
    // ========================================================================

    #[test]
    fn test_add_comment_input_deserializes_correctly() {
        let json = r#"{ "card_id": "card_123", "text": "This is a comment" }"#;

        let input: AddCommentInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.card_id, "card_123");
        assert_eq!(input.text, "This is a comment");
    }

    #[test]
    fn test_add_comment_input_missing_text_returns_error() {
        let json = r#"{ "card_id": "card_123" }"#;

        let err = serde_json::from_str::<AddCommentInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `text`"));
    }

    #[test]
    fn test_add_comment_input_missing_card_id_returns_error() {
        let json = r#"{ "text": "A comment" }"#;

        let err = serde_json::from_str::<AddCommentInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `card_id`"));
    }

    #[test]
    fn test_add_comment_output_serializes_correctly() {
        let output = AddCommentOutput {
            comment: Comment {
                id: "comment_new".to_string(),
                text: "Test comment".to_string(),
                id_member_creator: "member_1".to_string(),
                date: "2024-01-15T10:30:00.000Z".to_string(),
            },
        };

        let json = serde_json::to_value(&output).unwrap();

        assert_eq!(json["comment"]["text"], "Test comment");
        assert!(json["comment"]["id"].is_string());
        assert!(json["comment"]["idMemberCreator"].is_string());
    }

    // ========================================================================
    // Add Checklist Item Tests
    // ========================================================================

    #[test]
    fn test_add_checklist_item_input_deserializes_with_required_fields() {
        let json = r#"{ "checklist_id": "checklist_123", "name": "Task item" }"#;

        let input: AddChecklistItemInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.checklist_id, "checklist_123");
        assert_eq!(input.name, "Task item");
        assert!(!input.checked);
        assert_eq!(input.pos, None);
    }

    #[test]
    fn test_add_checklist_item_input_deserializes_with_all_fields() {
        let json = r#"{
            "checklist_id": "checklist_123",
            "name": "Task item",
            "pos": "top",
            "checked": true,
            "due": "2024-12-31T00:00:00.000Z",
            "member_id": "member_1"
        }"#;

        let input: AddChecklistItemInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.checklist_id, "checklist_123");
        assert_eq!(input.name, "Task item");
        assert_eq!(input.pos, Some("top".to_string()));
        assert!(input.checked);
        assert_eq!(input.due, Some("2024-12-31T00:00:00.000Z".to_string()));
        assert_eq!(input.member_id, Some("member_1".to_string()));
    }

    #[test]
    fn test_add_checklist_item_input_missing_name_returns_error() {
        let json = r#"{ "checklist_id": "checklist_123" }"#;

        let err = serde_json::from_str::<AddChecklistItemInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `name`"));
    }

    #[test]
    fn test_add_checklist_item_output_serializes_correctly() {
        let output = AddChecklistItemOutput {
            check_item: CheckItem {
                id: "checkitem_new".to_string(),
                name: "Test item".to_string(),
                state: "incomplete".to_string(),
                pos: 65536.0,
            },
            checklist_id: "checklist_123".to_string(),
        };

        let json = serde_json::to_value(&output).unwrap();

        assert_eq!(json["check_item"]["name"], "Test item");
        assert_eq!(json["check_item"]["state"], "incomplete");
        assert_eq!(json["checklist_id"], "checklist_123");
    }

    // ========================================================================
    // Type Serialization/Deserialization Tests
    // ========================================================================

    #[test]
    fn test_board_deserializes_from_trello_api_response() {
        let json = r#"{
            "id": "5abc123",
            "name": "Project Board",
            "desc": "Board description",
            "url": "https://trello.com/b/5abc123/project-board",
            "closed": false
        }"#;

        let board: Board = serde_json::from_str(json).unwrap();

        assert_eq!(board.id, "5abc123");
        assert_eq!(board.name, "Project Board");
        assert_eq!(board.desc, Some("Board description".to_string()));
        assert!(!board.closed);
    }

    #[test]
    fn test_card_deserializes_with_trello_field_names() {
        let json = r#"{
            "id": "card_1",
            "name": "Task",
            "desc": null,
            "idList": "list_1",
            "idBoard": "board_1",
            "url": "https://trello.com/c/abc",
            "pos": 16384.0,
            "due": "2024-12-31T00:00:00.000Z",
            "dueComplete": false,
            "closed": false,
            "labels": []
        }"#;

        let card: Card = serde_json::from_str(json).unwrap();

        assert_eq!(card.id_list, "list_1");
        assert_eq!(card.id_board, "board_1");
        assert!(!card.due_complete);
    }

    #[test]
    fn test_checklist_deserializes_with_nested_items() {
        let json = r#"{
            "id": "checklist_1",
            "name": "To Do",
            "idCard": "card_1",
            "checkItems": [
                { "id": "item_1", "name": "First task", "state": "complete", "pos": 1.0 },
                { "id": "item_2", "name": "Second task", "state": "incomplete", "pos": 2.0 }
            ]
        }"#;

        let checklist: Checklist = serde_json::from_str(json).unwrap();

        assert_eq!(checklist.id, "checklist_1");
        assert_eq!(checklist.check_items.len(), 2);
        assert_eq!(checklist.check_items[0].state, "complete");
        assert_eq!(checklist.check_items[1].state, "incomplete");
    }

    #[test]
    fn test_label_serializes_with_optional_name() {
        let label_with_name = Label {
            id: "label_1".to_string(),
            name: Some("Bug".to_string()),
            color: "red".to_string(),
        };

        let label_without_name = Label {
            id: "label_2".to_string(),
            name: None,
            color: "blue".to_string(),
        };

        let json1 = serde_json::to_value(&label_with_name).unwrap();
        let json2 = serde_json::to_value(&label_without_name).unwrap();

        assert_eq!(json1["name"], "Bug");
        assert_eq!(json2["name"], serde_json::Value::Null);
    }
}
