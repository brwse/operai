//! Atlassian Confluence integration for Operai Toolbox.
//!
//! This integration provides tools for working with Confluence pages,
//! including searching, creating, updating, commenting, and file attachments.

use operai::{
    Context, JsonSchema, Result, anyhow, define_user_credential, ensure, info, init, schemars,
    shutdown, tool,
};
use serde::{Deserialize, Serialize};

define_user_credential! {
    ConfluenceCredential("confluence") {
        /// OAuth2 access token for Confluence API.
        access_token: String,
        #[optional]
        /// Confluence instance base URL (e.g., https://yoursite.atlassian.net).
        /// Defaults to Confluence Cloud if not specified.
        endpoint: Option<String>,
    }
}

const DEFAULT_ENDPOINT: &str = "https://confluence.atlassian.com";

/// Initialize the Confluence tool library.
#[init]
async fn setup() -> Result<()> {
    info!("Confluence integration initialized");
    Ok(())
}

/// Clean up resources when the library is unloaded.
#[shutdown]
fn cleanup() {
    info!("Confluence integration shutting down");
}

// =============================================================================
// Search Pages
// =============================================================================

/// Input for searching Confluence pages.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchPagesInput {
    /// CQL (Confluence Query Language) query string.
    /// Examples: "text ~ 'project update'", "space = DEV AND type = page"
    pub cql: String,
    /// Maximum number of results to return (default: 25, max: 100).
    #[serde(default)]
    pub limit: Option<u32>,
    /// Starting index for pagination (default: 0).
    #[serde(default)]
    pub start: Option<u32>,
}

/// A summary of a Confluence page in search results.
#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone)]
pub struct PageSummary {
    /// Unique identifier of the page.
    pub id: String,
    /// Title of the page.
    pub title: String,
    /// Key of the space containing this page.
    pub space_key: String,
    /// Name of the space containing this page.
    pub space_name: String,
    /// Web UI URL for the page.
    pub web_url: String,
    /// Last modification timestamp (ISO 8601).
    pub last_modified: String,
    /// Username of the last modifier.
    pub last_modifier: String,
}

/// Output from searching Confluence pages.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchPagesOutput {
    /// List of matching pages.
    pub pages: Vec<PageSummary>,
    /// Total number of results available.
    pub total_size: u32,
    /// Starting index of these results.
    pub start: u32,
    /// Number of results returned.
    pub limit: u32,
}

/// # Search Confluence Pages
///
/// Searches for Confluence pages using CQL (Confluence Query Language), a
/// powerful query syntax similar to SQL that allows filtering by space, content
/// type, labels, text content, and much more.
///
/// Use this tool when the user wants to find pages in their Confluence instance
/// based on specific criteria. This is ideal for discovering pages by content,
/// filtering by space, or finding pages with specific metadata.
///
/// **When to use this tool:**
/// - User wants to search for pages containing specific text or keywords
/// - User needs to find pages in a specific space
/// - User wants to filter pages by type, labels, or other metadata
/// - User needs to browse or discover content in Confluence
///
/// **Key inputs:**
/// - `cql`: Required. A CQL query string (e.g., "text ~ 'project update'",
///   "space = DEV AND type = page")
/// - `limit`: Optional. Maximum results to return (default: 25, max: 100)
/// - `start`: Optional. Starting index for pagination (default: 0)
///
/// **Returns:** A list of page summaries including title, space, last modified
/// info, and URLs.
///
/// **Common CQL examples:**
/// - Simple text search: `text ~ "api documentation"`
/// - Search in space: `space = "DEV" AND type = page`
/// - Search by label: `label = "important"`
/// - Recent pages: `type = page ORDER BY lastModified DESC`
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - confluence
/// - docs
/// - search
///
/// # Errors
///
/// Returns an error if:
/// - The provided CQL query is empty or contains only whitespace
/// - Credentials are not configured or the access token is invalid
/// - The Confluence API request fails due to network issues or server errors
/// - The API response cannot be parsed or contains unexpected data
#[tool]
pub async fn search_pages(ctx: Context, input: SearchPagesInput) -> Result<SearchPagesOutput> {
    ensure!(!input.cql.trim().is_empty(), "cql must not be empty");

    let limit = input.limit.unwrap_or(25).min(100);
    let start = input.start.unwrap_or(0);

    let client = ConfluenceClient::from_ctx(&ctx)?;

    let query = [
        ("cql", input.cql),
        ("limit", limit.to_string()),
        ("start", start.to_string()),
        ("expand", "space,history.lastUpdated,version".to_string()),
    ];

    let response: ConfluenceSearchResponse = client
        .get_json(client.url_with_segments(&["content", "search"])?, &query)
        .await?;

    let pages = response
        .results
        .into_iter()
        .map(|r| PageSummary {
            id: r.id,
            title: r.title,
            space_key: r.space.as_ref().map(|s| s.key.clone()).unwrap_or_default(),
            space_name: r.space.and_then(|s| s.name).unwrap_or_default(),
            web_url: r
                .links
                .webui
                .map(|w| format!("{}{}", client.base_url_without_api(), w))
                .unwrap_or_default(),
            last_modified: r
                .history
                .as_ref()
                .and_then(|h| h.last_updated.as_ref().and_then(|u| u.when.clone()))
                .unwrap_or_default(),
            last_modifier: r
                .history
                .and_then(|h| {
                    h.last_updated
                        .and_then(|u| u.by.and_then(|b| b.display_name))
                })
                .unwrap_or_default(),
        })
        .collect();

    Ok(SearchPagesOutput {
        total_size: response
            .total_size
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(0),
        pages,
        start,
        limit,
    })
}

// =============================================================================
// Get Page
// =============================================================================

/// Input for retrieving a specific Confluence page.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetPageInput {
    /// The unique identifier of the page to retrieve.
    pub page_id: String,
    /// Whether to include the page body content (default: true).
    #[serde(default = "default_true")]
    pub include_body: bool,
    /// Body format: `"storage"` (raw), `"view"` (rendered HTML), or
    /// `"atlas_doc_format"` (ADF).
    #[serde(default)]
    pub body_format: Option<String>,
}

fn default_true() -> bool {
    true
}

/// A label attached to a Confluence page.
#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone)]
pub struct Label {
    /// The label name.
    pub name: String,
    /// Label prefix (e.g., "global", "my").
    pub prefix: String,
}

/// Full details of a Confluence page.
#[derive(Debug, Serialize, JsonSchema)]
pub struct PageDetails {
    /// Unique identifier of the page.
    pub id: String,
    /// Title of the page.
    pub title: String,
    /// Key of the space containing this page.
    pub space_key: String,
    /// Current version number.
    pub version: u32,
    /// Page body content (if requested).
    pub body: Option<String>,
    /// Body format of the returned content.
    pub body_format: String,
    /// Web UI URL for the page.
    pub web_url: String,
    /// API URL for the page.
    pub api_url: String,
    /// Creation timestamp (ISO 8601).
    pub created_at: String,
    /// Last modification timestamp (ISO 8601).
    pub updated_at: String,
    /// Username of the page creator.
    pub created_by: String,
    /// Username of the last modifier.
    pub updated_by: String,
    /// Labels attached to the page.
    pub labels: Vec<Label>,
    /// Parent page ID (if not a root page).
    pub parent_id: Option<String>,
}

/// Output from retrieving a Confluence page.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GetPageOutput {
    /// The retrieved page details.
    pub page: PageDetails,
}

/// # Get Confluence Page
///
/// Retrieves a specific Confluence page by its ID, including full content,
/// metadata, and associated information such as labels, version history, and
/// parent pages.
///
/// Use this tool when the user wants to view or read the full content of a
/// specific Confluence page. This is the right tool when you have a page ID and
/// need to get the complete page details including the body content.
///
/// **When to use this tool:**
/// - User wants to read the content of a specific page (by ID)
/// - User needs to get page metadata (labels, version, author, timestamps)
/// - User wants to view the page in different formats (storage, view, or ADF)
/// - User needs to get page details before updating or commenting
///
/// **Key inputs:**
/// - `page_id`: Required. The unique identifier of the page to retrieve
/// - `include_body`: Optional. Whether to include page body content (default:
///   true)
/// - `body_format`: Optional. Format for body content - "storage" (raw XHTML),
///   "view" (rendered HTML), or "`atlas_doc_format`" (JSON ADF). Default:
///   "storage"
///
/// **Returns:** Full page details including title, space, version, body content
/// (if requested), creation/update timestamps, author information, labels,
/// parent page ID, and URLs.
///
/// **Note:** If you don't have a page ID, use the `search_pages` tool first to
/// find the page you're looking for.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - confluence
/// - docs
///
/// # Errors
///
/// Returns an error if:
/// - The provided page ID is empty or contains only whitespace
/// - Credentials are not configured or the access token is invalid
/// - The page with the given ID does not exist or is inaccessible
/// - The Confluence API request fails due to network issues or server errors
/// - The API response cannot be parsed or contains unexpected data
#[tool]
pub async fn get_page(ctx: Context, input: GetPageInput) -> Result<GetPageOutput> {
    ensure!(
        !input.page_id.trim().is_empty(),
        "page_id must not be empty"
    );

    let body_format = match input.body_format.as_deref() {
        Some("view") => "view",
        Some("atlas_doc_format") => "atlas_doc_format",
        _ => "storage",
    };

    let client = ConfluenceClient::from_ctx(&ctx)?;

    let mut expand = vec![
        "version",
        "space",
        "history",
        "metadata.labels",
        "ancestors",
    ];
    if input.include_body {
        expand.push(match body_format {
            "storage" | "view" | "atlas_doc_format" => "body.{body_format}",
            _ => "body.storage",
        });
    }

    let query = [("expand", expand.join(","))];

    let page: ConfluencePage = client
        .get_json(
            client.url_with_segments(&["content", &input.page_id])?,
            &query,
        )
        .await?;

    let body = if input.include_body {
        page.body
            .and_then(|b| match body_format {
                "view" => b.view,
                "atlas_doc_format" => b.atlas_doc_format,
                _ => b.storage,
            })
            .map(|c| c.value)
    } else {
        None
    };

    let labels = page
        .metadata
        .and_then(|m| m.labels)
        .and_then(|l| l.results)
        .unwrap_or_default()
        .into_iter()
        .map(|l| Label {
            name: l.name,
            prefix: l.prefix.unwrap_or_else(|| "global".to_string()),
        })
        .collect();

    let ancestors = page.ancestors.unwrap_or_default();
    let parent_id = ancestors.last().map(|a| a.id.clone());

    Ok(GetPageOutput {
        page: PageDetails {
            id: page.id,
            title: page.title,
            space_key: page
                .space
                .as_ref()
                .map(|s| s.key.clone())
                .unwrap_or_default(),
            version: page.version.map_or(1, |v| v.number),
            body,
            body_format: body_format.to_string(),
            web_url: page
                .links
                .webui
                .map(|w| format!("{}{}", client.base_url_without_api(), w))
                .unwrap_or_default(),
            api_url: page.links.self_link.unwrap_or_default(),
            created_at: page
                .history
                .as_ref()
                .and_then(|h| h.created_date.clone())
                .unwrap_or_default(),
            updated_at: page
                .history
                .as_ref()
                .and_then(|h| h.last_updated.as_ref().and_then(|u| u.when.clone()))
                .unwrap_or_default(),
            created_by: page
                .history
                .as_ref()
                .and_then(|h| h.created_by.as_ref().and_then(|u| u.display_name.clone()))
                .unwrap_or_default(),
            updated_by: page
                .history
                .and_then(|h| {
                    h.last_updated
                        .and_then(|u| u.by.and_then(|b| b.display_name))
                })
                .unwrap_or_default(),
            labels,
            parent_id,
        },
    })
}

// =============================================================================
// Create Page
// =============================================================================

/// Input for creating a new Confluence page.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreatePageInput {
    /// Key of the space where the page will be created.
    pub space_key: String,
    /// Title of the new page.
    pub title: String,
    /// Body content of the page in storage format (XHTML-based).
    pub body: String,
    /// Parent page ID (optional, creates as child page).
    #[serde(default)]
    pub parent_id: Option<String>,
    /// Labels to attach to the page.
    #[serde(default)]
    pub labels: Vec<String>,
}

/// Output from creating a Confluence page.
#[derive(Debug, Serialize, JsonSchema)]
pub struct CreatePageOutput {
    /// The ID of the newly created page.
    pub page_id: String,
    /// The title of the created page.
    pub title: String,
    /// The version number (always 1 for new pages).
    pub version: u32,
    /// Web UI URL for the new page.
    pub web_url: String,
    /// API URL for the new page.
    pub api_url: String,
}

/// # Create Confluence Page
///
/// Creates a new Confluence page in a specified space with the provided title,
/// content, and optional metadata such as parent page and labels.
///
/// Use this tool when the user wants to create a new documentation page or add
/// content to their Confluence wiki. The page body should be provided in
/// Confluence storage format (XHTML-based markup).
///
/// **When to use this tool:**
/// - User wants to create a new documentation page
/// - User needs to add a new page to a specific space
/// - User wants to create a child page under an existing parent page
/// - User needs to create pages with specific labels for categorization
///
/// **Key inputs:**
/// - `space_key`: Required. The key of the space where the page will be created
///   (e.g., "DEV", "DOC")
/// - `title`: Required. The title of the new page
/// - `body`: Required. Page content in storage format (XHTML-based Confluence
///   markup)
/// - `parent_id`: Optional. If provided, creates the page as a child of this
///   parent page
/// - `labels`: Optional. List of label names to attach to the page for
///   categorization
///
/// **Returns:** The newly created page ID, title, version (always 1 for new
/// pages), and URLs.
///
/// **Important notes:**
/// - The body must be in Confluence storage format (XHTML). For simple text,
///   wrap in `<p>` tags: `<p>Your content here</p>`
/// - The space key is case-sensitive and must match an existing space
/// - If a `parent_id` is provided, that page must exist and be accessible
/// - Labels help with organization and can be used for filtering in searches
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - confluence
/// - docs
///
/// # Errors
///
/// Returns an error if:
/// - The space key, title, or body is empty or contains only whitespace
/// - Credentials are not configured or the access token is invalid
/// - The specified space does not exist or is inaccessible
/// - The parent page ID is provided but does not exist or is inaccessible
/// - The Confluence API request fails due to network issues or server errors
/// - The API response cannot be parsed or contains unexpected data
#[tool]
pub async fn create_page(ctx: Context, input: CreatePageInput) -> Result<CreatePageOutput> {
    ensure!(
        !input.space_key.trim().is_empty(),
        "space_key must not be empty"
    );
    ensure!(!input.title.trim().is_empty(), "title must not be empty");
    ensure!(!input.body.trim().is_empty(), "body must not be empty");

    let client = ConfluenceClient::from_ctx(&ctx)?;

    let mut request = serde_json::json!({
        "type": "page",
        "title": input.title,
        "space": { "key": input.space_key },
        "body": {
            "storage": {
                "value": input.body,
                "representation": "storage"
            }
        }
    });

    if let Some(parent_id) = &input.parent_id {
        request["ancestors"] = serde_json::json!([{ "id": parent_id }]);
    }

    if !input.labels.is_empty() {
        request["metadata"] = serde_json::json!({
            "labels": input.labels.iter().map(|name| serde_json::json!({
                "prefix": "global",
                "name": name
            })).collect::<Vec<_>>()
        });
    }

    let page: ConfluencePage = client
        .post_json(client.url_with_segments(&["content"])?, &request)
        .await?;

    Ok(CreatePageOutput {
        page_id: page.id,
        title: page.title,
        version: page.version.map_or(1, |v| v.number),
        web_url: page
            .links
            .webui
            .map(|w| format!("{}{}", client.base_url_without_api(), w))
            .unwrap_or_default(),
        api_url: page.links.self_link.unwrap_or_default(),
    })
}

// =============================================================================
// Update Page
// =============================================================================

/// Input for updating an existing Confluence page.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdatePageInput {
    /// The ID of the page to update.
    pub page_id: String,
    /// New title for the page (optional, keeps existing if not provided).
    #[serde(default)]
    pub title: Option<String>,
    /// New body content in storage format (optional, keeps existing if not
    /// provided).
    #[serde(default)]
    pub body: Option<String>,
    /// Current version number (required for optimistic locking).
    pub current_version: u32,
    /// Optional version message describing the change.
    #[serde(default)]
    pub version_message: Option<String>,
}

/// Output from updating a Confluence page.
#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdatePageOutput {
    /// The ID of the updated page.
    pub page_id: String,
    /// The title of the updated page.
    pub title: String,
    /// The new version number after the update.
    pub version: u32,
    /// Web UI URL for the page.
    pub web_url: String,
}

/// # Update Confluence Page
///
/// Updates an existing Confluence page's title and/or content with optimistic
/// locking to prevent concurrent edit conflicts.
///
/// Use this tool when the user wants to modify an existing Confluence page. The
/// tool requires the current version number for optimistic locking, ensuring
/// that updates are only applied if no other changes have been made since the
/// page was fetched.
///
/// **When to use this tool:**
/// - User wants to edit the content of an existing page
/// - User needs to update the title of a page
/// - User wants to make both content and title changes
/// - User needs to add a version message describing the changes
///
/// **Key inputs:**
/// - `page_id`: Required. The ID of the page to update
/// - ``current_version``: Required. The current version number (for optimistic
///   locking - prevents overwriting concurrent edits)
/// - `title`: Optional. New title for the page (omitting keeps existing title)
/// - `body`: Optional. New body content in storage format (omitting keeps
///   existing content)
/// - `version_message`: Optional. A message describing the change (appears in
///   version history)
///
/// **Returns:** The updated page ID, title, new version number (incremented),
/// and URL.
///
/// **Important notes:**
/// - You must provide the current version number from a previous `get_page`
///   call
/// - The tool will fail if the `current_version` doesn't match (indicating
///   someone else edited the page)
/// - At least one of `title` or `body` must be provided
/// - If you only have the page title but not the ID, use `search_pages` first
///   to find the page ID
/// - The version number will be automatically incremented (`current_version` +
///   1)
///
/// **Typical workflow:**
/// 1. Use `get_page` to retrieve the current page and its version number
/// 2. Make your changes to the title/body
/// 3. Call `update_page` with the `current_version` from step 1
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - confluence
/// - docs
///
/// # Errors
///
/// Returns an error if:
/// - The page ID is empty or contains only whitespace
/// - Neither title nor body is provided for update
/// - Credentials are not configured or the access token is invalid
/// - The page with the given ID does not exist or is inaccessible
/// - The current version number does not match (conflict detected)
/// - The Confluence API request fails due to network issues or server errors
/// - The API response cannot be parsed or contains unexpected data
#[tool]
pub async fn update_page(ctx: Context, input: UpdatePageInput) -> Result<UpdatePageOutput> {
    ensure!(
        !input.page_id.trim().is_empty(),
        "page_id must not be empty"
    );
    ensure!(
        input.title.is_some() || input.body.is_some(),
        "must provide title or body to update"
    );

    let new_version = input.current_version + 1;
    let client = ConfluenceClient::from_ctx(&ctx)?;

    // First, fetch the current page to get the title if not updating it
    let query = [("expand", "body.storage,version".to_string())];
    let current_page: ConfluencePage = client
        .get_json(
            client.url_with_segments(&["content", &input.page_id])?,
            &query,
        )
        .await?;

    let title = input.title.unwrap_or(current_page.title.clone());
    let body = input.body.unwrap_or_else(|| {
        current_page
            .body
            .and_then(|b| b.storage)
            .map(|s| s.value)
            .unwrap_or_default()
    });

    let mut request = serde_json::json!({
        "version": { "number": new_version },
        "title": title,
        "type": "page",
        "body": {
            "storage": {
                "value": body,
                "representation": "storage"
            }
        }
    });

    if let Some(message) = input.version_message {
        request["version"]["message"] = serde_json::json!(message);
    }

    let page: ConfluencePage = client
        .put_json(
            client.url_with_segments(&["content", &input.page_id])?,
            &request,
        )
        .await?;

    Ok(UpdatePageOutput {
        page_id: page.id,
        title: page.title,
        version: page.version.map_or(new_version, |v| v.number),
        web_url: page
            .links
            .webui
            .map(|w| format!("{}{}", client.base_url_without_api(), w))
            .unwrap_or_default(),
    })
}

// =============================================================================
// Add Comment
// =============================================================================

/// Input for adding a comment to a Confluence page.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddCommentInput {
    /// The ID of the page to comment on.
    pub page_id: String,
    /// Comment body in storage format (XHTML-based).
    pub body: String,
    /// Parent comment ID for threaded replies (optional).
    #[serde(default)]
    pub parent_comment_id: Option<String>,
}

/// Output from adding a comment to a Confluence page.
#[derive(Debug, Serialize, JsonSchema)]
pub struct AddCommentOutput {
    /// The ID of the newly created comment.
    pub comment_id: String,
    /// The ID of the page the comment was added to.
    pub page_id: String,
    /// Web UI URL to view the comment.
    pub web_url: String,
    /// Creation timestamp (ISO 8601).
    pub created_at: String,
}

/// # Add Confluence Comment
///
/// Adds a comment to a Confluence page, with support for threaded replies to
/// existing comments.
///
/// Use this tool when the user wants to add feedback, questions, or notes to a
/// Confluence page. Comments can be top-level (added directly to the page) or
/// threaded (replies to existing comments).
///
/// **When to use this tool:**
/// - User wants to add a comment or feedback to a page
/// - User needs to reply to an existing comment on a page
/// - User wants to ask questions or provide suggestions on documentation
/// - User needs to add notes or annotations to a page
///
/// **Key inputs:**
/// - `page_id`: Required. The ID of the page to comment on
/// - `body`: Required. Comment content in storage format (XHTML-based
///   Confluence markup)
/// - `parent_comment_id`: Optional. If provided, creates a threaded reply to
///   this comment
///
/// **Returns:** The new comment ID, page ID, web URL to view the comment, and
/// creation timestamp.
///
/// **Important notes:**
/// - The body must be in Confluence storage format (XHTML). For simple text,
///   use: `<p>Your comment here</p>`
/// - To reply to an existing comment, you need that comment's ID (which may
///   require getting page details first)
/// - Comments appear in the page's comment section and are visible to users
///   with access
/// - Threaded replies are indented under the parent comment in the UI
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - confluence
/// - docs
/// - comments
///
/// # Errors
///
/// Returns an error if:
/// - The page ID or body is empty or contains only whitespace
/// - Credentials are not configured or the access token is invalid
/// - The page with the given ID does not exist or is inaccessible
/// - The parent comment ID is provided but does not exist or is inaccessible
/// - The Confluence API request fails due to network issues or server errors
/// - The API response cannot be parsed or contains unexpected data
#[tool]
pub async fn add_comment(ctx: Context, input: AddCommentInput) -> Result<AddCommentOutput> {
    ensure!(
        !input.page_id.trim().is_empty(),
        "page_id must not be empty"
    );
    ensure!(!input.body.trim().is_empty(), "body must not be empty");

    let client = ConfluenceClient::from_ctx(&ctx)?;

    let mut request = serde_json::json!({
        "type": "comment",
        "container": { "id": input.page_id, "type": "page" },
        "body": {
            "storage": {
                "value": input.body,
                "representation": "storage"
            }
        }
    });

    if let Some(parent_id) = &input.parent_comment_id {
        request["ancestors"] = serde_json::json!([{ "id": parent_id }]);
    }

    let comment: ConfluenceComment = client
        .post_json(client.url_with_segments(&["content"])?, &request)
        .await?;

    Ok(AddCommentOutput {
        comment_id: comment.id,
        page_id: input.page_id,
        web_url: comment
            .links
            .webui
            .map(|w| format!("{}{}", client.base_url_without_api(), w))
            .unwrap_or_default(),
        created_at: comment
            .history
            .and_then(|h| h.created_date)
            .unwrap_or_default(),
    })
}

// =============================================================================
// Attach File
// =============================================================================

/// Input for attaching a file to a Confluence page.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AttachFileInput {
    /// The ID of the page to attach the file to.
    pub page_id: String,
    /// Name of the file (including extension).
    pub filename: String,
    /// Base64-encoded file content.
    pub content_base64: String,
    /// MIME type of the file (e.g., "application/pdf", "image/png").
    pub content_type: String,
    /// Optional comment describing the attachment.
    #[serde(default)]
    pub comment: Option<String>,
}

/// Output from attaching a file to a Confluence page.
#[derive(Debug, Serialize, JsonSchema)]
pub struct AttachFileOutput {
    /// The ID of the newly created attachment.
    pub attachment_id: String,
    /// The filename of the attachment.
    pub filename: String,
    /// Size of the attachment in bytes.
    pub size_bytes: u64,
    /// MIME type of the attachment.
    pub content_type: String,
    /// Download URL for the attachment.
    pub download_url: String,
    /// Creation timestamp (ISO 8601).
    pub created_at: String,
}

/// # Attach Confluence File
///
/// Uploads and attaches a file to a Confluence page, supporting various file
/// types including documents, images, PDFs, and more.
///
/// Use this tool when the user wants to add files or attachments to a
/// Confluence page. This is ideal for including supporting documents, images,
/// diagrams, or any other files that enhance the page content.
///
/// **When to use this tool:**
/// - User wants to upload an image or screenshot to a page
/// - User needs to attach a PDF, document, or other file
/// - User wants to add supplementary materials to documentation
/// - User needs to include diagrams or visual assets on a page
///
/// **Key inputs:**
/// - `page_id`: Required. The ID of the page to attach the file to
/// - `filename`: Required. Name of the file including extension (e.g.,
///   "screenshot.png", "report.pdf")
/// - `content_base64`: Required. Base64-encoded file content
/// - `content_type`: Required. MIME type of the file (e.g., "image/png",
///   "application/pdf", "image/jpeg")
/// - `comment`: Optional. A description or note about the attachment
///
/// **Returns:** The attachment ID, filename, size in bytes, content type,
/// download URL, and creation timestamp.
///
/// **Important notes:**
/// - File content must be base64-encoded. For example, a PNG file should be
///   encoded as base64 before passing to this tool
/// - The `content_type` must match the actual file type (e.g., "image/png" for
///   PNG images, "application/pdf" for PDFs)
/// - Common MIME types:
///   - Images: "image/png", "image/jpeg", "image/gif", "image/svg+xml"
///   - Documents: "application/pdf", "application/msword",
///     "application/vnd.openxmlformats-officedocument.wordprocessingml.
///     document"
///   - Other: "text/plain", "text/csv", "application/zip"
/// - Attachments appear in the "Attachments" section of the page
/// - Large files may take longer to upload
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - confluence
/// - docs
/// - attachments
///
/// # Errors
///
/// Returns an error if:
/// - The page ID, filename, `content_base64`, or `content_type` is empty or
///   contains only whitespace
/// - Credentials are not configured or the access token is invalid
/// - The base64 content cannot be decoded (invalid base64 encoding)
/// - The page with the given ID does not exist or is inaccessible
/// - The Confluence API request fails due to network issues or server errors
/// - The API response cannot be parsed or contains unexpected data
#[tool]
pub async fn attach_file(ctx: Context, input: AttachFileInput) -> Result<AttachFileOutput> {
    ensure!(
        !input.page_id.trim().is_empty(),
        "page_id must not be empty"
    );
    ensure!(
        !input.filename.trim().is_empty(),
        "filename must not be empty"
    );
    ensure!(
        !input.content_base64.trim().is_empty(),
        "content_base64 must not be empty"
    );
    ensure!(
        !input.content_type.trim().is_empty(),
        "content_type must not be empty"
    );

    // Decode base64 content
    let decoded = base64_decode(&input.content_base64)?;
    let size_bytes = decoded.len() as u64;

    let client = ConfluenceClient::from_ctx(&ctx)?;

    // Build multipart form
    let mut form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(decoded)
            .file_name(input.filename.clone())
            .mime_str(&input.content_type)?,
    );

    if let Some(comment) = input.comment {
        form = form.text("comment", comment);
    }

    let attachment: ConfluenceAttachment = client
        .post_multipart(
            client.url_with_segments(&["content", &input.page_id, "child", "attachment"])?,
            form,
        )
        .await?;

    Ok(AttachFileOutput {
        attachment_id: attachment.id,
        filename: attachment.title,
        size_bytes,
        content_type: attachment.media_type.unwrap_or(input.content_type),
        download_url: attachment
            .links
            .download
            .map(|d| format!("{}{}", client.base_url_without_api(), d))
            .unwrap_or_default(),
        created_at: attachment
            .history
            .and_then(|h| h.created_date)
            .unwrap_or_default(),
    })
}

// =============================================================================
// HTTP Client
// =============================================================================

#[derive(Debug, Clone)]
struct ConfluenceClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl ConfluenceClient {
    /// Creates a new Confluence client from the provided context.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Credentials are not configured in the context
    /// - The access token is empty or contains only whitespace
    /// - The configured endpoint URL is invalid or cannot be normalized
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = ConfluenceCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let endpoint = cred.endpoint.as_deref().unwrap_or(DEFAULT_ENDPOINT);
        let base_url = normalize_base_url(endpoint)?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            access_token: cred.access_token,
        })
    }

    /// Constructs a URL by appending path segments to the base URL.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The base URL cannot be modified (it is not a valid URL that supports
    ///   path segments)
    fn url_with_segments(&self, segments: &[&str]) -> Result<reqwest::Url> {
        let mut url = reqwest::Url::parse(&self.base_url)?;
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|()| anyhow::anyhow!("base_url must be an absolute URL"))?;
            for segment in segments {
                path.push(segment);
            }
        }
        Ok(url)
    }

    fn base_url_without_api(&self) -> String {
        self.base_url.trim_end_matches("/wiki/rest/api").to_string()
    }

    /// Sends a GET request and deserializes the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails due to network issues or server errors
    /// - The response status code indicates failure
    /// - The response body cannot be parsed as JSON
    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        query: &[(&str, String)],
    ) -> Result<T> {
        let response = self.send_request(self.http.get(url).query(query)).await?;
        Ok(response.json::<T>().await?)
    }

    /// Sends a POST request with JSON body and deserializes the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The request body cannot be serialized as JSON
    /// - The HTTP request fails due to network issues or server errors
    /// - The response status code indicates failure
    /// - The response body cannot be parsed as JSON
    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
    ) -> Result<TRes> {
        let response = self.send_request(self.http.post(url).json(body)).await?;
        Ok(response.json::<TRes>().await?)
    }

    /// Sends a PUT request with JSON body and deserializes the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The request body cannot be serialized as JSON
    /// - The HTTP request fails due to network issues or server errors
    /// - The response status code indicates failure
    /// - The response body cannot be parsed as JSON
    async fn put_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
    ) -> Result<TRes> {
        let response = self.send_request(self.http.put(url).json(body)).await?;
        Ok(response.json::<TRes>().await?)
    }

    /// Sends a POST request with multipart form data and deserializes the JSON
    /// response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails due to network issues or server errors
    /// - The response status code indicates failure
    /// - The response body cannot be parsed as JSON
    /// - The response does not contain any attachment results
    async fn post_multipart<TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        form: reqwest::multipart::Form,
    ) -> Result<TRes> {
        let response = self
            .send_request(
                self.http
                    .post(url)
                    .multipart(form)
                    .header("X-Atlassian-Token", "no-check"),
            )
            .await?;

        // Confluence returns attachments as an array
        let response_text = response.text().await?;
        let results: ConfluenceAttachmentResults<TRes> = serde_json::from_str(&response_text)?;
        results
            .results
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No attachment returned from Confluence"))
    }

    /// Sends an HTTP request with authentication and returns the response or an
    /// error.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails due to network issues or server errors
    /// - The response status code indicates failure (includes status code and
    ///   response body in error message)
    async fn send_request(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let response = request
            .bearer_auth(&self.access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Confluence API request failed ({status}): {body}"
            ))
        }
    }
}

/// Normalizes a Confluence endpoint URL to include the API path.
///
/// # Errors
///
/// Returns an error if:
/// - The endpoint is empty or contains only whitespace
fn normalize_base_url(endpoint: &str) -> Result<String> {
    let trimmed = endpoint.trim().trim_end_matches('/');
    ensure!(!trimmed.is_empty(), "endpoint must not be empty");

    // Add /wiki/rest/api if not present
    if trimmed.ends_with("/wiki/rest/api") {
        Ok(trimmed.to_string())
    } else if trimmed.ends_with("/wiki") {
        Ok(format!("{trimmed}/rest/api"))
    } else {
        Ok(format!("{trimmed}/wiki/rest/api"))
    }
}

/// Decodes a base64-encoded string into bytes.
///
/// # Errors
///
/// Returns an error if:
/// - The input string is not valid base64 encoding
fn base64_decode(input: &str) -> Result<Vec<u8>> {
    use base64::{Engine as _, engine::general_purpose};
    general_purpose::STANDARD
        .decode(input)
        .map_err(|e| anyhow::anyhow!("Invalid base64 encoding: {e}"))
}

// =============================================================================
// API Response Types
// =============================================================================

#[derive(Debug, Deserialize)]
struct ConfluenceSearchResponse {
    results: Vec<ConfluenceSearchResult>,
    #[serde(default)]
    #[serde(rename = "totalSize")]
    total_size: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ConfluenceSearchResult {
    id: String,
    title: String,
    #[serde(default)]
    space: Option<ConfluenceSpace>,
    #[serde(default)]
    history: Option<ConfluenceHistory>,
    #[serde(rename = "_links")]
    links: ConfluenceLinks,
}

#[derive(Debug, Deserialize)]
struct ConfluencePage {
    id: String,
    title: String,
    #[serde(default)]
    space: Option<ConfluenceSpace>,
    #[serde(default)]
    body: Option<ConfluenceBody>,
    #[serde(default)]
    version: Option<ConfluenceVersion>,
    #[serde(default)]
    history: Option<ConfluenceHistory>,
    #[serde(default)]
    metadata: Option<ConfluenceMetadata>,
    #[serde(default)]
    ancestors: Option<Vec<ConfluenceAncestor>>,
    #[serde(rename = "_links")]
    links: ConfluenceLinks,
}

#[derive(Debug, Deserialize)]
struct ConfluenceComment {
    id: String,
    #[serde(default)]
    history: Option<ConfluenceHistory>,
    #[serde(rename = "_links")]
    links: ConfluenceLinks,
}

#[derive(Debug, Deserialize)]
struct ConfluenceAttachment {
    id: String,
    title: String,
    #[serde(default)]
    #[serde(rename = "mediaType")]
    media_type: Option<String>,
    #[serde(default)]
    history: Option<ConfluenceHistory>,
    #[serde(rename = "_links")]
    links: ConfluenceLinks,
}

#[derive(Debug, Deserialize)]
struct ConfluenceSpace {
    key: String,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfluenceBody {
    #[serde(default)]
    storage: Option<ConfluenceBodyContent>,
    #[serde(default)]
    view: Option<ConfluenceBodyContent>,
    #[serde(default)]
    #[serde(rename = "atlas_doc_format")]
    atlas_doc_format: Option<ConfluenceBodyContent>,
}

#[derive(Debug, Deserialize)]
struct ConfluenceBodyContent {
    value: String,
}

#[derive(Debug, Deserialize)]
struct ConfluenceVersion {
    number: u32,
}

#[derive(Debug, Deserialize)]
struct ConfluenceHistory {
    #[serde(default)]
    #[serde(rename = "createdDate")]
    created_date: Option<String>,
    #[serde(default)]
    #[serde(rename = "createdBy")]
    created_by: Option<ConfluenceUser>,
    #[serde(default)]
    #[serde(rename = "lastUpdated")]
    last_updated: Option<ConfluenceLastUpdated>,
}

#[derive(Debug, Deserialize)]
struct ConfluenceLastUpdated {
    #[serde(default)]
    when: Option<String>,
    #[serde(default)]
    by: Option<ConfluenceUser>,
}

#[derive(Debug, Deserialize)]
struct ConfluenceUser {
    #[serde(default)]
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfluenceMetadata {
    #[serde(default)]
    labels: Option<ConfluenceLabelResults>,
}

#[derive(Debug, Deserialize)]
struct ConfluenceLabelResults {
    #[serde(default)]
    results: Option<Vec<ConfluenceLabel>>,
}

#[derive(Debug, Deserialize)]
struct ConfluenceLabel {
    name: String,
    #[serde(default)]
    prefix: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfluenceAncestor {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ConfluenceLinks {
    #[serde(default)]
    webui: Option<String>,
    #[serde(default)]
    #[serde(rename = "self")]
    self_link: Option<String>,
    #[serde(default)]
    download: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConfluenceAttachmentResults<T> {
    results: Vec<T>,
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod integration_tests;

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // =========================================================================
    // Credential Tests
    // =========================================================================

    #[test]
    fn test_confluence_credential_deserializes_with_all_fields() {
        let json = r#"{
            "access_token": "token123",
            "endpoint": "https://mysite.atlassian.net"
        }"#;

        let cred: ConfluenceCredential = serde_json::from_str(json).unwrap();

        assert_eq!(cred.access_token, "token123");
        assert_eq!(
            cred.endpoint.as_deref(),
            Some("https://mysite.atlassian.net")
        );
    }

    #[test]
    fn test_confluence_credential_deserializes_with_access_token_only() {
        let json = r#"{
            "access_token": "token123"
        }"#;

        let cred: ConfluenceCredential = serde_json::from_str(json).unwrap();

        assert_eq!(cred.access_token, "token123");
        assert_eq!(cred.endpoint, None);
    }

    #[test]
    fn test_confluence_credential_missing_access_token_returns_error() {
        let json = r#"{
            "endpoint": "https://mysite.atlassian.net"
        }"#;

        let err = serde_json::from_str::<ConfluenceCredential>(json).unwrap_err();
        assert!(err.to_string().contains("missing field"));
    }

    // =========================================================================
    // Search Pages Tests
    // =========================================================================

    #[test]
    fn test_search_pages_input_deserializes_with_cql_only() {
        let json = r#"{ "cql": "text ~ 'test'" }"#;

        let input: SearchPagesInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.cql, "text ~ 'test'");
        assert_eq!(input.limit, None);
        assert_eq!(input.start, None);
    }

    #[test]
    fn test_search_pages_input_deserializes_with_all_fields() {
        let json = r#"{
            "cql": "space = DEV AND type = page",
            "limit": 50,
            "start": 10
        }"#;

        let input: SearchPagesInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.cql, "space = DEV AND type = page");
        assert_eq!(input.limit, Some(50));
        assert_eq!(input.start, Some(10));
    }

    #[test]
    fn test_search_pages_output_serializes_correctly() {
        let output = SearchPagesOutput {
            pages: vec![PageSummary {
                id: "123".to_string(),
                title: "Test Page".to_string(),
                space_key: "DEV".to_string(),
                space_name: "Development".to_string(),
                web_url: "https://example.atlassian.net/wiki/spaces/DEV/pages/123".to_string(),
                last_modified: "2024-01-15T10:30:00Z".to_string(),
                last_modifier: "jdoe".to_string(),
            }],
            total_size: 1,
            start: 0,
            limit: 25,
        };

        let json = serde_json::to_value(&output).unwrap();

        assert_eq!(json["pages"][0]["id"], "123");
        assert_eq!(json["pages"][0]["title"], "Test Page");
        assert_eq!(json["total_size"], 1);
    }

    // =========================================================================
    // Get Page Tests
    // =========================================================================

    #[test]
    fn test_get_page_input_deserializes_with_page_id_only() {
        let json = r#"{ "page_id": "12345" }"#;

        let input: GetPageInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.page_id, "12345");
        assert!(input.include_body);
        assert_eq!(input.body_format, None);
    }

    #[test]
    fn test_get_page_input_deserializes_with_all_fields() {
        let json = r#"{
            "page_id": "12345",
            "include_body": false,
            "body_format": "view"
        }"#;

        let input: GetPageInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.page_id, "12345");
        assert!(!input.include_body);
        assert_eq!(input.body_format.as_deref(), Some("view"));
    }

    // =========================================================================
    // Create Page Tests
    // =========================================================================

    #[test]
    fn test_create_page_input_deserializes_minimal() {
        let json = r#"{
            "space_key": "DEV",
            "title": "New Page",
            "body": "<p>Content</p>"
        }"#;

        let input: CreatePageInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.space_key, "DEV");
        assert_eq!(input.title, "New Page");
        assert_eq!(input.body, "<p>Content</p>");
        assert_eq!(input.parent_id, None);
        assert!(input.labels.is_empty());
    }

    #[test]
    fn test_create_page_input_deserializes_with_optional_fields() {
        let json = r#"{
            "space_key": "DEV",
            "title": "New Page",
            "body": "<p>Content</p>",
            "parent_id": "parent123",
            "labels": ["important", "draft"]
        }"#;

        let input: CreatePageInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.parent_id.as_deref(), Some("parent123"));
        assert_eq!(input.labels, vec!["important", "draft"]);
    }

    // =========================================================================
    // Update Page Tests
    // =========================================================================

    #[test]
    fn test_update_page_input_deserializes_minimal() {
        let json = r#"{
            "page_id": "12345",
            "current_version": 5
        }"#;

        let input: UpdatePageInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.page_id, "12345");
        assert_eq!(input.current_version, 5);
        assert_eq!(input.title, None);
        assert_eq!(input.body, None);
        assert_eq!(input.version_message, None);
    }

    #[test]
    fn test_update_page_input_deserializes_with_all_fields() {
        let json = r#"{
            "page_id": "12345",
            "title": "Updated Title",
            "body": "<p>Updated content</p>",
            "current_version": 5,
            "version_message": "Fixed typos"
        }"#;

        let input: UpdatePageInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.title.as_deref(), Some("Updated Title"));
        assert_eq!(input.body.as_deref(), Some("<p>Updated content</p>"));
        assert_eq!(input.version_message.as_deref(), Some("Fixed typos"));
    }

    // =========================================================================
    // Add Comment Tests
    // =========================================================================

    #[test]
    fn test_add_comment_input_deserializes_minimal() {
        let json = r#"{
            "page_id": "12345",
            "body": "<p>Great work!</p>"
        }"#;

        let input: AddCommentInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.page_id, "12345");
        assert_eq!(input.body, "<p>Great work!</p>");
        assert_eq!(input.parent_comment_id, None);
    }

    #[test]
    fn test_add_comment_input_deserializes_with_parent() {
        let json = r#"{
            "page_id": "12345",
            "body": "<p>I agree!</p>",
            "parent_comment_id": "comment456"
        }"#;

        let input: AddCommentInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.parent_comment_id.as_deref(), Some("comment456"));
    }

    // =========================================================================
    // Attach File Tests
    // =========================================================================

    #[test]
    fn test_attach_file_input_deserializes_minimal() {
        let json = r#"{
            "page_id": "12345",
            "filename": "report.pdf",
            "content_base64": "SGVsbG8gV29ybGQ=",
            "content_type": "application/pdf"
        }"#;

        let input: AttachFileInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.page_id, "12345");
        assert_eq!(input.filename, "report.pdf");
        assert_eq!(input.content_base64, "SGVsbG8gV29ybGQ=");
        assert_eq!(input.content_type, "application/pdf");
        assert_eq!(input.comment, None);
    }

    #[test]
    fn test_attach_file_input_deserializes_with_comment() {
        let json = r#"{
            "page_id": "12345",
            "filename": "image.png",
            "content_base64": "iVBORw0KGgo=",
            "content_type": "image/png",
            "comment": "Project logo"
        }"#;

        let input: AttachFileInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.comment.as_deref(), Some("Project logo"));
    }

    // =========================================================================
    // Page Summary Tests
    // =========================================================================

    #[test]
    fn test_page_summary_serializes_correctly() {
        let summary = PageSummary {
            id: "456".to_string(),
            title: "Project Overview".to_string(),
            space_key: "PROJ".to_string(),
            space_name: "Project Space".to_string(),
            web_url: "https://example.atlassian.net/wiki/spaces/PROJ/pages/456".to_string(),
            last_modified: "2024-02-01T09:00:00Z".to_string(),
            last_modifier: "alice".to_string(),
        };

        let json = serde_json::to_value(&summary).unwrap();

        assert_eq!(
            json,
            json!({
                "id": "456",
                "title": "Project Overview",
                "space_key": "PROJ",
                "space_name": "Project Space",
                "web_url": "https://example.atlassian.net/wiki/spaces/PROJ/pages/456",
                "last_modified": "2024-02-01T09:00:00Z",
                "last_modifier": "alice"
            })
        );
    }

    #[test]
    fn test_label_serializes_correctly() {
        let label = Label {
            name: "important".to_string(),
            prefix: "global".to_string(),
        };

        let json = serde_json::to_value(&label).unwrap();

        assert_eq!(
            json,
            json!({
                "name": "important",
                "prefix": "global"
            })
        );
    }

    // =========================================================================
    // URL normalization tests
    // =========================================================================

    #[test]
    fn test_normalize_base_url_adds_api_path() {
        let result = normalize_base_url("https://mysite.atlassian.net").unwrap();
        assert_eq!(result, "https://mysite.atlassian.net/wiki/rest/api");
    }

    #[test]
    fn test_normalize_base_url_preserves_complete_path() {
        let result = normalize_base_url("https://mysite.atlassian.net/wiki/rest/api").unwrap();
        assert_eq!(result, "https://mysite.atlassian.net/wiki/rest/api");
    }

    #[test]
    fn test_normalize_base_url_adds_rest_api_to_wiki() {
        let result = normalize_base_url("https://mysite.atlassian.net/wiki").unwrap();
        assert_eq!(result, "https://mysite.atlassian.net/wiki/rest/api");
    }

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://mysite.atlassian.net/").unwrap();
        assert_eq!(result, "https://mysite.atlassian.net/wiki/rest/api");
    }

    #[test]
    fn test_normalize_base_url_empty_returns_error() {
        let result = normalize_base_url("");
        assert!(result.is_err());
    }

    // =========================================================================
    // Base64 decoding tests
    // =========================================================================

    #[test]
    fn test_base64_decode_valid() {
        let result = base64_decode("SGVsbG8gV29ybGQ=").unwrap();
        assert_eq!(result, b"Hello World");
    }

    #[test]
    fn test_base64_decode_invalid_returns_error() {
        let result = base64_decode("not-valid-base64!!!");
        assert!(result.is_err());
    }
}
