//! Evernote integration for Operai Toolbox.
//!
//! This integration provides tools for managing Evernote notes including
//! searching, creating, updating, and tagging notes.

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

define_user_credential! {
    EvernoteCredential("evernote") {
        /// OAuth access token or developer token for Evernote API.
        access_token: String,
        /// Whether to use sandbox environment (defaults to false for production).
        #[optional]
        sandbox: Option<bool>,
    }
}

/// Initialize the Evernote integration.
#[init]
async fn setup() -> Result<()> {
    info!("Evernote integration initialized");
    Ok(())
}

/// Clean up resources when the library is unloaded.
#[shutdown]
fn cleanup() {
    info!("Evernote integration shutting down");
}

// =============================================================================
// Search Notes Tool
// =============================================================================

/// Input for searching notes.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchNotesInput {
    /// Search query string using Evernote search grammar.
    /// Supports keywords, tags (tag:), notebook (notebook:), etc.
    pub query: String,
    /// Maximum number of notes to return (1-250, default 25).
    #[serde(default)]
    pub max_results: Option<u32>,
    /// Offset for pagination (default 0).
    #[serde(default)]
    pub offset: Option<u32>,
    /// Sort order for results.
    #[serde(default)]
    pub sort_order: Option<SortOrder>,
}

/// Sort order for search results.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    /// Sort by creation date (newest first).
    Created,
    /// Sort by last updated date (newest first).
    Updated,
    /// Sort by relevance to search query.
    Relevance,
    /// Sort by title alphabetically.
    Title,
}

/// A note summary returned from search.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NoteSummary {
    /// Unique identifier for the note.
    pub guid: String,
    /// Title of the note.
    pub title: String,
    /// Name of the notebook containing the note.
    pub notebook_name: Option<String>,
    /// Tags applied to the note.
    pub tags: Vec<String>,
    /// Timestamp when the note was created (Unix milliseconds).
    pub created: i64,
    /// Timestamp when the note was last updated (Unix milliseconds).
    pub updated: i64,
    /// Content snippet/preview.
    pub snippet: Option<String>,
}

/// Output from searching notes.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchNotesOutput {
    /// List of matching notes.
    pub notes: Vec<NoteSummary>,
    /// Total number of notes matching the query.
    pub total_count: u32,
    /// Offset used in this search.
    pub offset: u32,
    /// Request ID for tracking.
    pub request_id: String,
}

/// # Search Evernote Notes
///
/// Searches for notes in the user's Evernote account using Evernote's search
/// grammar. This is the primary tool for discovering and locating notes based
/// on content, tags, notebooks, creation date, update date, and other
/// attributes.
///
/// Use this tool when a user wants to:
/// - Find notes containing specific keywords or phrases
/// - Locate notes with specific tags (e.g., "tag:work")
/// - Filter notes by notebook (e.g., "notebook:Projects")
/// - Search within date ranges (e.g., "created:20240101-20241231")
/// - Combine multiple search criteria
///
/// The tool returns a list of note summaries including title, notebook, tags,
/// timestamps, and content snippets. Use `get_note` to retrieve full note
/// content.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - notes
/// - evernote
/// - search
///
/// # Errors
///
/// Returns an error if:
/// - The query string is empty or contains only whitespace
/// - The `max_results` value exceeds the allowed range (1-250)
#[tool]
pub async fn search_notes(ctx: Context, input: SearchNotesInput) -> Result<SearchNotesOutput> {
    ensure!(!input.query.trim().is_empty(), "query must not be empty");

    let max_results = input.max_results.unwrap_or(25).clamp(1, 250);
    let offset = input.offset.unwrap_or(0);

    info!(
        "Searching notes with query: {} (max: {}, offset: {})",
        input.query, max_results, offset
    );

    // Reference implementation: validates input but returns empty results.
    // A production implementation would call Evernote's NoteStore.findNotesMetadata
    // API.
    Ok(SearchNotesOutput {
        notes: vec![],
        total_count: 0,
        offset,
        request_id: ctx.request_id().to_string(),
    })
}

// =============================================================================
// Get Note Tool
// =============================================================================

/// Input for getting a specific note.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetNoteInput {
    /// The unique identifier (GUID) of the note to retrieve.
    pub guid: String,
    /// Whether to include the full content (ENML).
    #[serde(default = "default_true")]
    pub include_content: bool,
    /// Whether to include resource metadata (attachments).
    #[serde(default)]
    pub include_resources: Option<bool>,
}

fn default_true() -> bool {
    true
}

/// A resource/attachment in a note.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NoteResource {
    /// Unique identifier for the resource.
    pub guid: String,
    /// MIME type of the resource.
    pub mime_type: String,
    /// Filename if available.
    pub filename: Option<String>,
    /// Size in bytes.
    pub size: u32,
}

/// Full note details.
#[derive(Debug, Serialize, JsonSchema)]
pub struct NoteDetails {
    /// Unique identifier for the note.
    pub guid: String,
    /// Title of the note.
    pub title: String,
    /// Content in ENML format (if requested).
    pub content: Option<String>,
    /// Plain text content (extracted from ENML).
    pub plain_text: Option<String>,
    /// Name of the notebook containing the note.
    pub notebook_name: Option<String>,
    /// GUID of the notebook.
    pub notebook_guid: String,
    /// Tags applied to the note.
    pub tags: Vec<String>,
    /// Timestamp when the note was created (Unix milliseconds).
    pub created: i64,
    /// Timestamp when the note was last updated (Unix milliseconds).
    pub updated: i64,
    /// Author of the note.
    pub author: Option<String>,
    /// Source URL if the note was clipped from web.
    pub source_url: Option<String>,
    /// Resources/attachments in the note.
    pub resources: Vec<NoteResource>,
}

/// Output from getting a note.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GetNoteOutput {
    /// The retrieved note details.
    pub note: NoteDetails,
    /// Request ID for tracking.
    pub request_id: String,
}

/// # Get Evernote Note
///
/// Retrieves a specific note from Evernote by its unique identifier (GUID).
/// Returns the complete note details including title, content, metadata, tags,
/// and attached resources.
///
/// Use this tool when:
/// - A user wants to read the full content of a specific note
/// - Displaying detailed note information after a search
/// - Retrieving note content for editing or reference
/// - Getting note metadata (author, source URL, creation/update times)
///
/// The note content can be returned in ENML format (Evernote Markup Language),
/// as plain text extracted from ENML, or both. Attachments/resource metadata
/// can optionally be included.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - notes
/// - evernote
///
/// # Errors
///
/// Returns an error if:
/// - The GUID is empty or contains only whitespace
/// - The note with the specified GUID does not exist
#[tool]
pub async fn get_note(ctx: Context, input: GetNoteInput) -> Result<GetNoteOutput> {
    ensure!(!input.guid.trim().is_empty(), "guid must not be empty");

    let include_content = input.include_content;
    let include_resources = input.include_resources.unwrap_or(false);

    info!(
        "Getting note: {} (content: {}, resources: {})",
        input.guid, include_content, include_resources
    );

    // Reference implementation: validates input but returns empty note data.
    // A production implementation would call Evernote's
    // NoteStore.getNoteWithResultSpec API.
    Ok(GetNoteOutput {
        note: NoteDetails {
            guid: input.guid.clone(),
            title: String::new(),
            content: if include_content {
                Some(String::new())
            } else {
                None
            },
            plain_text: if include_content {
                Some(String::new())
            } else {
                None
            },
            notebook_name: None,
            notebook_guid: String::new(),
            tags: vec![],
            created: 0,
            updated: 0,
            author: None,
            source_url: None,
            resources: vec![],
        },
        request_id: ctx.request_id().to_string(),
    })
}

// =============================================================================
// Create Note Tool
// =============================================================================

/// Input for creating a new note.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateNoteInput {
    /// Title of the note.
    pub title: String,
    /// Content of the note in plain text or ENML.
    pub content: String,
    /// Whether the content is already in ENML format.
    #[serde(default)]
    pub is_enml: Option<bool>,
    /// Name or GUID of the notebook to create the note in.
    /// If not specified, uses the default notebook.
    #[serde(default)]
    pub notebook: Option<String>,
    /// Tags to apply to the note.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// Source URL if clipping from web.
    #[serde(default)]
    pub source_url: Option<String>,
}

/// Output from creating a note.
#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateNoteOutput {
    /// GUID of the created note.
    pub guid: String,
    /// Title of the created note.
    pub title: String,
    /// Notebook GUID where the note was created.
    pub notebook_guid: String,
    /// Timestamp when the note was created (Unix milliseconds).
    pub created: i64,
    /// Request ID for tracking.
    pub request_id: String,
}

/// # Create Evernote Note
///
/// Creates a new note in the user's Evernote account with the specified title,
/// content, and optional metadata including notebook placement, tags, and
/// source URL.
///
/// Use this tool when a user wants to:
/// - Save a new note or quick thought
/// - Create a note in a specific notebook
/// - Clip web content (provide `source_url`)
/// - Create notes with initial tags for organization
/// - Save formatted content using ENML markup
///
/// The note can be created in the default notebook or a specific notebook (by
/// name or GUID). Content can be provided as plain text or ENML (Evernote
/// Markup Language) for rich formatting. Tags can be applied at creation time.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - notes
/// - evernote
///
/// # Errors
///
/// Returns an error if:
/// - The title is empty or contains only whitespace
/// - The content is empty or contains only whitespace
/// - The specified notebook does not exist
#[tool]
pub async fn create_note(ctx: Context, input: CreateNoteInput) -> Result<CreateNoteOutput> {
    ensure!(!input.title.trim().is_empty(), "title must not be empty");
    ensure!(
        !input.content.trim().is_empty(),
        "content must not be empty"
    );

    let is_enml = input.is_enml.unwrap_or(false);

    info!(
        "Creating note: {} (enml: {}, notebook: {:?}, tags: {:?})",
        input.title, is_enml, input.notebook, input.tags
    );

    // Reference implementation: validates input but returns fake data.
    // A production implementation would call Evernote's NoteStore.createNote API.
    Ok(CreateNoteOutput {
        guid: "new-note-guid".to_string(),
        title: input.title,
        notebook_guid: "default-notebook-guid".to_string(),
        created: 0,
        request_id: ctx.request_id().to_string(),
    })
}

// =============================================================================
// Update Note Tool
// =============================================================================

/// Input for updating an existing note.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateNoteInput {
    /// GUID of the note to update.
    pub guid: String,
    /// New title for the note (if changing).
    #[serde(default)]
    pub title: Option<String>,
    /// New content for the note (if changing).
    #[serde(default)]
    pub content: Option<String>,
    /// Whether the new content is in ENML format.
    #[serde(default)]
    pub is_enml: Option<bool>,
    /// New notebook to move the note to (name or GUID).
    #[serde(default)]
    pub notebook: Option<String>,
}

/// Output from updating a note.
#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateNoteOutput {
    /// GUID of the updated note.
    pub guid: String,
    /// Updated title.
    pub title: String,
    /// Timestamp when the note was updated (Unix milliseconds).
    pub updated: i64,
    /// Request ID for tracking.
    pub request_id: String,
}

/// # Update Evernote Note
///
/// Updates an existing Evernote note's properties including title, content,
/// or notebook location. Only the fields specified in the request are modified;
/// unspecified fields remain unchanged.
///
/// Use this tool when a user wants to:
/// - Edit the title or content of an existing note
/// - Move a note to a different notebook
/// - Append or modify note content
/// - Update notes with new information
/// - Reorganize notes across notebooks
///
/// This tool performs partial updates - you can update just the title, just the
/// content, or both. The notebook field allows moving notes between notebooks.
/// Content can be provided as plain text or ENML for rich formatting.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - notes
/// - evernote
///
/// # Errors
///
/// Returns an error if:
/// - The GUID is empty or contains only whitespace
/// - The title is provided but empty or contains only whitespace
/// - The content is provided but empty or contains only whitespace
/// - The notebook is provided but empty or contains only whitespace
/// - The note with the specified GUID does not exist
/// - The target notebook does not exist
#[tool]
pub async fn update_note(ctx: Context, input: UpdateNoteInput) -> Result<UpdateNoteOutput> {
    ensure!(!input.guid.trim().is_empty(), "guid must not be empty");

    if let Some(ref title) = input.title {
        ensure!(
            !title.trim().is_empty(),
            "title must not be empty if provided"
        );
    }
    if let Some(ref content) = input.content {
        ensure!(
            !content.trim().is_empty(),
            "content must not be empty if provided"
        );
    }
    if let Some(ref notebook) = input.notebook {
        ensure!(
            !notebook.trim().is_empty(),
            "notebook must not be empty if provided"
        );
    }

    info!(
        "Updating note: {} (title: {:?}, content_len: {:?}, notebook: {:?})",
        input.guid,
        input.title,
        input.content.as_ref().map(String::len),
        input.notebook
    );

    // Reference implementation: validates input but returns fake data.
    // A production implementation would call Evernote's NoteStore.updateNote API.
    Ok(UpdateNoteOutput {
        guid: input.guid,
        title: input.title.unwrap_or_else(|| "Untitled".to_string()),
        updated: 0,
        request_id: ctx.request_id().to_string(),
    })
}

// =============================================================================
// Tag Note Tool
// =============================================================================

/// Action to perform on tags.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TagAction {
    /// Add tags to the note.
    Add,
    /// Remove tags from the note.
    Remove,
    /// Replace all tags with the specified list.
    Replace,
}

/// Input for tagging a note.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TagNoteInput {
    /// GUID of the note to tag.
    pub guid: String,
    /// Tags to add, remove, or set.
    pub tags: Vec<String>,
    /// Action to perform on the tags.
    #[serde(default = "default_tag_action")]
    pub action: TagAction,
}

fn default_tag_action() -> TagAction {
    TagAction::Add
}

/// Output from tagging a note.
#[derive(Debug, Serialize, JsonSchema)]
pub struct TagNoteOutput {
    /// GUID of the tagged note.
    pub guid: String,
    /// Current tags on the note after the operation.
    pub tags: Vec<String>,
    /// Timestamp when the note was updated (Unix milliseconds).
    pub updated: i64,
    /// Request ID for tracking.
    pub request_id: String,
}

/// # Tag Evernote Note
///
/// Manages tags on an Evernote note by adding, removing, or replacing tags.
/// Tags are labels used for organizing and categorizing notes for easy
/// retrieval.
///
/// Use this tool when a user wants to:
/// - Add tags to a note for organization (Add action)
/// - Remove specific tags from a note (Remove action)
/// - Replace all existing tags with a new set (Replace action)
/// - Organize notes into categories or projects
/// - Apply labels for filtering and search
///
/// The tool supports three actions:
/// - `add`: Appends new tags to the note's existing tags
/// - `remove`: Removes specified tags from the note
/// - `replace`: Replaces all tags with the provided list
///
/// Returns the complete list of tags on the note after the operation.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - notes
/// - evernote
/// - tags
///
/// # Errors
///
/// Returns an error if:
/// - The GUID is empty or contains only whitespace
/// - The tags array is empty
/// - Any tag string is empty or contains only whitespace
/// - The note with the specified GUID does not exist
#[tool]
pub async fn tag_note(ctx: Context, input: TagNoteInput) -> Result<TagNoteOutput> {
    ensure!(!input.guid.trim().is_empty(), "guid must not be empty");
    ensure!(!input.tags.is_empty(), "tags must not be empty");
    ensure!(
        input.tags.iter().all(|tag| !tag.trim().is_empty()),
        "tags must not contain empty strings"
    );

    let action = input.action.clone();

    info!(
        "Tagging note: {} (action: {:?}, tags: {:?})",
        input.guid, action, input.tags
    );

    // Reference implementation: validates input but returns fake data.
    // A production implementation would:
    // 1. Fetch the current note via NoteStore.getNote
    // 2. Modify tags based on action
    // 3. Update the note via NoteStore.updateNote
    Ok(TagNoteOutput {
        guid: input.guid,
        tags: input.tags,
        updated: 0,
        request_id: ctx.request_id().to_string(),
    })
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // =========================================================================
    // Credential Tests
    // =========================================================================

    #[test]
    fn test_credential_deserializes_with_required_access_token() {
        let json = r#"{ "access_token": "token123" }"#;
        let cred: EvernoteCredential = serde_json::from_str(json).unwrap();
        assert_eq!(cred.access_token, "token123");
        assert_eq!(cred.sandbox, None);
    }

    #[test]
    fn test_credential_deserializes_with_optional_sandbox() {
        let json = r#"{ "access_token": "token123", "sandbox": true }"#;
        let cred: EvernoteCredential = serde_json::from_str(json).unwrap();
        assert_eq!(cred.access_token, "token123");
        assert_eq!(cred.sandbox, Some(true));
    }

    #[test]
    fn test_credential_missing_access_token_returns_error() {
        let json = r#"{ "sandbox": true }"#;
        let err = serde_json::from_str::<EvernoteCredential>(json).unwrap_err();
        assert!(err.to_string().contains("missing field `access_token`"));
    }

    // =========================================================================
    // Validation Tests
    // =========================================================================

    #[tokio::test]
    async fn test_search_notes_empty_query_returns_error() {
        let ctx = Context::empty();
        let input = SearchNotesInput {
            query: "   ".to_string(),
            max_results: None,
            offset: None,
            sort_order: None,
        };

        let result = search_notes(ctx, input).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("query must not be empty")
        );
    }

    #[tokio::test]
    async fn test_get_note_empty_guid_returns_error() {
        let ctx = Context::empty();
        let input = GetNoteInput {
            guid: "  ".to_string(),
            include_content: true,
            include_resources: None,
        };

        let result = get_note(ctx, input).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("guid must not be empty")
        );
    }

    #[tokio::test]
    async fn test_create_note_empty_title_returns_error() {
        let ctx = Context::empty();
        let input = CreateNoteInput {
            title: "  ".to_string(),
            content: "Content".to_string(),
            is_enml: None,
            notebook: None,
            tags: None,
            source_url: None,
        };

        let result = create_note(ctx, input).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("title must not be empty")
        );
    }

    #[tokio::test]
    async fn test_create_note_empty_content_returns_error() {
        let ctx = Context::empty();
        let input = CreateNoteInput {
            title: "Title".to_string(),
            content: "  ".to_string(),
            is_enml: None,
            notebook: None,
            tags: None,
            source_url: None,
        };

        let result = create_note(ctx, input).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("content must not be empty")
        );
    }

    #[tokio::test]
    async fn test_update_note_empty_guid_returns_error() {
        let ctx = Context::empty();
        let input = UpdateNoteInput {
            guid: "  ".to_string(),
            title: Some("New Title".to_string()),
            content: None,
            is_enml: None,
            notebook: None,
        };

        let result = update_note(ctx, input).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("guid must not be empty")
        );
    }

    #[tokio::test]
    async fn test_update_note_empty_title_if_provided_returns_error() {
        let ctx = Context::empty();
        let input = UpdateNoteInput {
            guid: "note-123".to_string(),
            title: Some("  ".to_string()),
            content: None,
            is_enml: None,
            notebook: None,
        };

        let result = update_note(ctx, input).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("title must not be empty")
        );
    }

    #[tokio::test]
    async fn test_tag_note_empty_guid_returns_error() {
        let ctx = Context::empty();
        let input = TagNoteInput {
            guid: "  ".to_string(),
            tags: vec!["tag1".to_string()],
            action: TagAction::Add,
        };

        let result = tag_note(ctx, input).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("guid must not be empty")
        );
    }

    #[tokio::test]
    async fn test_tag_note_empty_tags_array_returns_error() {
        let ctx = Context::empty();
        let input = TagNoteInput {
            guid: "note-123".to_string(),
            tags: vec![],
            action: TagAction::Add,
        };

        let result = tag_note(ctx, input).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("tags must not be empty")
        );
    }

    #[tokio::test]
    async fn test_tag_note_empty_tag_string_returns_error() {
        let ctx = Context::empty();
        let input = TagNoteInput {
            guid: "note-123".to_string(),
            tags: vec!["tag1".to_string(), "  ".to_string()],
            action: TagAction::Add,
        };

        let result = tag_note(ctx, input).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("tags must not contain empty strings")
        );
    }

    // =========================================================================
    // Search Notes Tests
    // =========================================================================

    #[test]
    fn test_search_notes_input_deserializes_minimal() {
        let json = r#"{ "query": "tag:work" }"#;
        let input: SearchNotesInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.query, "tag:work");
        assert_eq!(input.max_results, None);
        assert_eq!(input.offset, None);
        assert!(input.sort_order.is_none());
    }

    #[test]
    fn test_search_notes_input_deserializes_full() {
        let json = r#"{
            "query": "notebook:Projects",
            "max_results": 50,
            "offset": 10,
            "sort_order": "updated"
        }"#;
        let input: SearchNotesInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.query, "notebook:Projects");
        assert_eq!(input.max_results, Some(50));
        assert_eq!(input.offset, Some(10));
        assert!(matches!(input.sort_order, Some(SortOrder::Updated)));
    }

    #[test]
    fn test_search_notes_input_missing_query_returns_error() {
        let json = r#"{ "max_results": 10 }"#;
        let err = serde_json::from_str::<SearchNotesInput>(json).unwrap_err();
        assert!(err.to_string().contains("missing field `query`"));
    }

    #[tokio::test]
    async fn test_search_notes_returns_empty_results() {
        let ctx = Context::with_metadata("req-123", "sess-456", "user-789");
        let input = SearchNotesInput {
            query: "tag:test".to_string(),
            max_results: None,
            offset: None,
            sort_order: None,
        };

        let output = search_notes(ctx, input).await.unwrap();

        assert!(output.notes.is_empty());
        assert_eq!(output.total_count, 0);
        assert_eq!(output.offset, 0);
        assert_eq!(output.request_id, "req-123");
    }

    #[tokio::test]
    async fn test_search_notes_respects_offset() {
        let ctx = Context::with_metadata("req-123", "", "");
        let input = SearchNotesInput {
            query: "test".to_string(),
            max_results: Some(10),
            offset: Some(20),
            sort_order: None,
        };

        let output = search_notes(ctx, input).await.unwrap();

        assert_eq!(output.offset, 20);
    }

    #[test]
    fn test_search_notes_output_serializes_correctly() {
        let output = SearchNotesOutput {
            notes: vec![NoteSummary {
                guid: "note-123".to_string(),
                title: "Test Note".to_string(),
                notebook_name: Some("Work".to_string()),
                tags: vec!["important".to_string()],
                created: 1_700_000_000_000,
                updated: 1_700_001_000_000,
                snippet: Some("This is a test...".to_string()),
            }],
            total_count: 1,
            offset: 0,
            request_id: "req-123".to_string(),
        };

        let json = serde_json::to_value(output).unwrap();
        assert_eq!(json["notes"][0]["guid"], "note-123");
        assert_eq!(json["notes"][0]["title"], "Test Note");
        assert_eq!(json["total_count"], 1);
    }

    // =========================================================================
    // Get Note Tests
    // =========================================================================

    #[test]
    fn test_get_note_input_deserializes_minimal() {
        let json = r#"{ "guid": "note-abc" }"#;
        let input: GetNoteInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.guid, "note-abc");
        assert!(input.include_content);
        assert_eq!(input.include_resources, None);
    }

    #[test]
    fn test_get_note_input_deserializes_full() {
        let json = r#"{
            "guid": "note-xyz",
            "include_content": false,
            "include_resources": true
        }"#;
        let input: GetNoteInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.guid, "note-xyz");
        assert!(!input.include_content);
        assert_eq!(input.include_resources, Some(true));
    }

    #[test]
    fn test_get_note_input_missing_guid_returns_error() {
        let json = r#"{ "include_content": true }"#;
        let err = serde_json::from_str::<GetNoteInput>(json).unwrap_err();
        assert!(err.to_string().contains("missing field `guid`"));
    }

    #[tokio::test]
    async fn test_get_note_returns_note_with_content() {
        let ctx = Context::with_metadata("req-456", "", "");
        let input = GetNoteInput {
            guid: "test-note".to_string(),
            include_content: true,
            include_resources: Some(false),
        };

        let output = get_note(ctx, input).await.unwrap();

        assert_eq!(output.note.guid, "test-note");
        assert!(output.note.content.is_some());
        assert_eq!(output.request_id, "req-456");
    }

    #[tokio::test]
    async fn test_get_note_without_content() {
        let ctx = Context::empty();
        let input = GetNoteInput {
            guid: "test-note".to_string(),
            include_content: false,
            include_resources: None,
        };

        let output = get_note(ctx, input).await.unwrap();

        assert!(output.note.content.is_none());
        assert!(output.note.plain_text.is_none());
    }

    // =========================================================================
    // Create Note Tests
    // =========================================================================

    #[test]
    fn test_create_note_input_deserializes_minimal() {
        let json = r#"{ "title": "My Note", "content": "Hello world" }"#;
        let input: CreateNoteInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.title, "My Note");
        assert_eq!(input.content, "Hello world");
        assert_eq!(input.is_enml, None);
        assert_eq!(input.notebook, None);
        assert_eq!(input.tags, None);
    }

    #[test]
    fn test_create_note_input_deserializes_full() {
        let json = r#"{
            "title": "Project Notes",
            "content": "<?xml version=\"1.0\"?><en-note>Content</en-note>",
            "is_enml": true,
            "notebook": "Work",
            "tags": ["project", "important"],
            "source_url": "https://example.com"
        }"#;
        let input: CreateNoteInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.title, "Project Notes");
        assert_eq!(input.is_enml, Some(true));
        assert_eq!(input.notebook.as_deref(), Some("Work"));
        assert_eq!(input.tags.as_ref().unwrap().len(), 2);
        assert_eq!(input.source_url.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn test_create_note_input_missing_title_returns_error() {
        let json = r#"{ "content": "Hello" }"#;
        let err = serde_json::from_str::<CreateNoteInput>(json).unwrap_err();
        assert!(err.to_string().contains("missing field `title`"));
    }

    #[test]
    fn test_create_note_input_missing_content_returns_error() {
        let json = r#"{ "title": "Test" }"#;
        let err = serde_json::from_str::<CreateNoteInput>(json).unwrap_err();
        assert!(err.to_string().contains("missing field `content`"));
    }

    #[tokio::test]
    async fn test_create_note_returns_new_guid() {
        let ctx = Context::with_metadata("req-789", "", "");
        let input = CreateNoteInput {
            title: "New Note".to_string(),
            content: "Content here".to_string(),
            is_enml: None,
            notebook: None,
            tags: Some(vec!["test".to_string()]),
            source_url: None,
        };

        let output = create_note(ctx, input).await.unwrap();

        assert_eq!(output.guid, "new-note-guid");
        assert_eq!(output.title, "New Note");
        assert_eq!(output.request_id, "req-789");
    }

    #[tokio::test]
    async fn test_create_note_with_notebook() {
        let ctx = Context::empty();
        let input = CreateNoteInput {
            title: "Work Note".to_string(),
            content: "Work content".to_string(),
            is_enml: None,
            notebook: Some("Work".to_string()),
            tags: None,
            source_url: None,
        };

        let output = create_note(ctx, input).await.unwrap();

        assert!(!output.notebook_guid.is_empty());
    }

    // =========================================================================
    // Update Note Tests
    // =========================================================================

    #[test]
    fn test_update_note_input_deserializes_minimal() {
        let json = r#"{ "guid": "note-123" }"#;
        let input: UpdateNoteInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.guid, "note-123");
        assert_eq!(input.title, None);
        assert_eq!(input.content, None);
    }

    #[test]
    fn test_update_note_input_deserializes_full() {
        let json = r#"{
            "guid": "note-123",
            "title": "Updated Title",
            "content": "Updated content",
            "is_enml": false,
            "notebook": "Archive"
        }"#;
        let input: UpdateNoteInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.guid, "note-123");
        assert_eq!(input.title.as_deref(), Some("Updated Title"));
        assert_eq!(input.content.as_deref(), Some("Updated content"));
        assert_eq!(input.notebook.as_deref(), Some("Archive"));
    }

    #[test]
    fn test_update_note_input_missing_guid_returns_error() {
        let json = r#"{ "title": "New Title" }"#;
        let err = serde_json::from_str::<UpdateNoteInput>(json).unwrap_err();
        assert!(err.to_string().contains("missing field `guid`"));
    }

    #[tokio::test]
    async fn test_update_note_returns_updated_note() {
        let ctx = Context::with_metadata("req-update", "", "");
        let input = UpdateNoteInput {
            guid: "note-to-update".to_string(),
            title: Some("New Title".to_string()),
            content: None,
            is_enml: None,
            notebook: None,
        };

        let output = update_note(ctx, input).await.unwrap();

        assert_eq!(output.guid, "note-to-update");
        assert_eq!(output.title, "New Title");
        assert_eq!(output.request_id, "req-update");
    }

    #[tokio::test]
    async fn test_update_note_with_no_title_defaults_to_untitled() {
        let ctx = Context::empty();
        let input = UpdateNoteInput {
            guid: "note-123".to_string(),
            title: None,
            content: Some("New content".to_string()),
            is_enml: None,
            notebook: None,
        };

        let output = update_note(ctx, input).await.unwrap();

        assert_eq!(output.title, "Untitled");
    }

    // =========================================================================
    // Tag Note Tests
    // =========================================================================

    #[test]
    fn test_tag_note_input_deserializes_minimal() {
        let json = r#"{ "guid": "note-123", "tags": ["work", "urgent"] }"#;
        let input: TagNoteInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.guid, "note-123");
        assert_eq!(input.tags, vec!["work", "urgent"]);
        assert!(matches!(input.action, TagAction::Add));
    }

    #[test]
    fn test_tag_note_input_deserializes_with_action() {
        let json = r#"{
            "guid": "note-456",
            "tags": ["archive"],
            "action": "remove"
        }"#;
        let input: TagNoteInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.guid, "note-456");
        assert!(matches!(input.action, TagAction::Remove));
    }

    #[test]
    fn test_tag_note_input_replace_action() {
        let json = r#"{
            "guid": "note-789",
            "tags": ["new-tag"],
            "action": "replace"
        }"#;
        let input: TagNoteInput = serde_json::from_str(json).unwrap();
        assert!(matches!(input.action, TagAction::Replace));
    }

    #[test]
    fn test_tag_note_input_missing_guid_returns_error() {
        let json = r#"{ "tags": ["test"] }"#;
        let err = serde_json::from_str::<TagNoteInput>(json).unwrap_err();
        assert!(err.to_string().contains("missing field `guid`"));
    }

    #[test]
    fn test_tag_note_input_missing_tags_returns_error() {
        let json = r#"{ "guid": "note-123" }"#;
        let err = serde_json::from_str::<TagNoteInput>(json).unwrap_err();
        assert!(err.to_string().contains("missing field `tags`"));
    }

    #[tokio::test]
    async fn test_tag_note_adds_tags() {
        let ctx = Context::with_metadata("req-tag", "", "");
        let input = TagNoteInput {
            guid: "note-to-tag".to_string(),
            tags: vec!["project".to_string(), "important".to_string()],
            action: TagAction::Add,
        };

        let output = tag_note(ctx, input).await.unwrap();

        assert_eq!(output.guid, "note-to-tag");
        assert_eq!(output.tags.len(), 2);
        assert_eq!(output.request_id, "req-tag");
    }

    #[tokio::test]
    async fn test_tag_note_removes_tags() {
        let ctx = Context::empty();
        let input = TagNoteInput {
            guid: "note-123".to_string(),
            tags: vec!["old-tag".to_string()],
            action: TagAction::Remove,
        };

        let output = tag_note(ctx, input).await.unwrap();

        assert_eq!(output.guid, "note-123");
    }

    #[tokio::test]
    async fn test_tag_note_replaces_all_tags() {
        let ctx = Context::empty();
        let input = TagNoteInput {
            guid: "note-456".to_string(),
            tags: vec!["only-tag".to_string()],
            action: TagAction::Replace,
        };

        let output = tag_note(ctx, input).await.unwrap();

        assert_eq!(output.tags, vec!["only-tag"]);
    }

    #[test]
    fn test_tag_action_serializes_correctly() {
        assert_eq!(serde_json::to_string(&TagAction::Add).unwrap(), r#""add""#);
        assert_eq!(
            serde_json::to_string(&TagAction::Remove).unwrap(),
            r#""remove""#
        );
        assert_eq!(
            serde_json::to_string(&TagAction::Replace).unwrap(),
            r#""replace""#
        );
    }

    // =========================================================================
    // Output Serialization Tests
    // =========================================================================

    #[test]
    fn test_note_resource_serializes_correctly() {
        let resource = NoteResource {
            guid: "res-123".to_string(),
            mime_type: "image/png".to_string(),
            filename: Some("screenshot.png".to_string()),
            size: 1024,
        };

        let json = serde_json::to_value(resource).unwrap();
        assert_eq!(json["guid"], "res-123");
        assert_eq!(json["mime_type"], "image/png");
        assert_eq!(json["filename"], "screenshot.png");
        assert_eq!(json["size"], 1024);
    }

    #[test]
    fn test_create_note_output_serializes_correctly() {
        let output = CreateNoteOutput {
            guid: "new-123".to_string(),
            title: "Test Note".to_string(),
            notebook_guid: "nb-456".to_string(),
            created: 1_700_000_000_000,
            request_id: "req-789".to_string(),
        };

        let json = serde_json::to_value(output).unwrap();
        assert_eq!(
            json,
            json!({
                "guid": "new-123",
                "title": "Test Note",
                "notebook_guid": "nb-456",
                "created": 1_700_000_000_000_i64,
                "request_id": "req-789"
            })
        );
    }

    #[test]
    fn test_sort_order_deserializes_all_variants() {
        assert!(matches!(
            serde_json::from_str::<SortOrder>(r#""created""#).unwrap(),
            SortOrder::Created
        ));
        assert!(matches!(
            serde_json::from_str::<SortOrder>(r#""updated""#).unwrap(),
            SortOrder::Updated
        ));
        assert!(matches!(
            serde_json::from_str::<SortOrder>(r#""relevance""#).unwrap(),
            SortOrder::Relevance
        ));
        assert!(matches!(
            serde_json::from_str::<SortOrder>(r#""title""#).unwrap(),
            SortOrder::Title
        ));
    }
}
