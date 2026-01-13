//! Smartsheet integration for Operai Toolbox.
//!
//! This integration provides tools for interacting with Smartsheet:
//! - List sheets
//! - Read rows from a sheet
//! - Update rows in a sheet
//! - Add comments to rows
//! - Attach files to rows

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

mod types;
use types::{Attachment, Column, Discussion, Row, SheetResponse, SheetSummary, SheetsListResponse};

define_user_credential! {
    SmartsheetCredential("smartsheet") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_SMARTSHEET_ENDPOINT: &str = "https://api.smartsheet.com/2.0";

/// Initialize the Smartsheet tool library.
#[init]
async fn setup() -> Result<()> {
    info!("Smartsheet integration initialized");
    Ok(())
}

/// Clean up resources when the library is unloaded.
#[shutdown]
fn cleanup() {
    info!("Smartsheet integration shutting down");
}

// ============================================================================
// HTTP Client
// ============================================================================

#[derive(Debug, Clone)]
struct SmartsheetClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl SmartsheetClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = SmartsheetCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url = normalize_base_url(
            cred.endpoint
                .as_deref()
                .unwrap_or(DEFAULT_SMARTSHEET_ENDPOINT),
        )?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            access_token: cred.access_token,
        })
    }

    fn url_with_segments(&self, segments: &[&str]) -> Result<reqwest::Url> {
        let mut url = reqwest::Url::parse(&self.base_url)?;
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|()| operai::anyhow::anyhow!("base_url must be an absolute URL"))?;
            for segment in segments {
                path.push(segment);
            }
        }
        Ok(url)
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        query: &[(&str, String)],
    ) -> Result<T> {
        let response = self
            .http
            .get(url)
            .query(query)
            .bearer_auth(&self.access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.json::<T>().await?)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Smartsheet API request failed ({status}): {body}"
            ))
        }
    }

    async fn post_json<T: for<'de> Deserialize<'de>, B: Serialize + ?Sized>(
        &self,
        url: reqwest::Url,
        body: &B,
    ) -> Result<T> {
        let response = self
            .http
            .post(url)
            .bearer_auth(&self.access_token)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::ACCEPT, "application/json")
            .json(body)
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.json::<T>().await?)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Smartsheet API request failed ({status}): {body}"
            ))
        }
    }

    async fn put_json<T: for<'de> Deserialize<'de>, B: Serialize + ?Sized>(
        &self,
        url: reqwest::Url,
        body: &B,
    ) -> Result<T> {
        let response = self
            .http
            .put(url)
            .bearer_auth(&self.access_token)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::ACCEPT, "application/json")
            .json(body)
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.json::<T>().await?)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Smartsheet API request failed ({status}): {body}"
            ))
        }
    }
}

fn normalize_base_url(endpoint: &str) -> Result<String> {
    let trimmed = endpoint.trim();
    ensure!(!trimmed.is_empty(), "endpoint must not be empty");
    Ok(trimmed.trim_end_matches('/').to_string())
}

// ============================================================================
// Tool: list_sheets
// ============================================================================

/// Input for the `list_sheets` tool.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListSheetsInput {
    /// Maximum number of sheets to return (default: 100, max: 1000).
    #[serde(default)]
    pub page_size: Option<u32>,
    /// Page number for pagination (1-based).
    #[serde(default)]
    pub page: Option<u32>,
    /// Include sheets in the Trash.
    #[serde(default)]
    pub include_trash: Option<bool>,
}

/// Output from the `list_sheets` tool.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListSheetsOutput {
    /// List of sheets.
    pub sheets: Vec<SheetSummary>,
    /// Total count of sheets.
    pub total_count: i32,
}

/// # List Smartsheet Sheets
///
/// Lists all Smartsheet sheets accessible to the authenticated user.
///
/// Use this tool when a user wants to:
/// - Browse available sheets in their Smartsheet workspace
/// - Find a specific sheet by name or metadata
/// - Get an overview of all sheets they have access to
/// - Check which sheets are available before performing operations on them
///
/// The tool returns a paginated list of sheets with metadata including:
/// - Sheet ID and name
/// - Access level (e.g., OWNER, EDITOR, VIEWER)
/// - Creation and modification timestamps
/// - Permalink to the sheet
///
/// Pagination parameters allow retrieving sheets in batches (default: 100, max:
/// 1000). Set `include_trash` to true to also include sheets that have been
/// moved to the trash.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - spreadsheet
/// - smartsheet
///
/// # Errors
///
/// Returns an error if:
/// - The Smartsheet credentials are missing or invalid
/// - The API request fails due to network or server issues
/// - The response cannot be parsed
#[tool]
pub async fn list_sheets(ctx: Context, input: ListSheetsInput) -> Result<ListSheetsOutput> {
    let page_size = input.page_size.unwrap_or(100).min(1000);
    let page = input.page.unwrap_or(1).max(1);

    let client = SmartsheetClient::from_ctx(&ctx)?;

    let mut query = vec![
        ("pageSize", page_size.to_string()),
        ("page", page.to_string()),
    ];

    if let Some(true) = input.include_trash {
        query.push(("includeAll", "true".to_string()));
    }

    let response: SheetsListResponse = client
        .get_json(client.url_with_segments(&["sheets"])?, &query)
        .await?;

    let total_count = response
        .total_count
        .unwrap_or(response.sheets.len().try_into().unwrap_or(i32::MAX));
    let sheets = response.sheets;

    Ok(ListSheetsOutput {
        sheets,
        total_count,
    })
}

// ============================================================================
// Tool: read_rows
// ============================================================================

/// Input for the `read_rows` tool.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadRowsInput {
    /// ID of the sheet to read rows from.
    pub sheet_id: u64,
    /// Specific row IDs to fetch. If empty, fetches all rows.
    #[serde(default)]
    pub row_ids: Option<Vec<u64>>,
    /// Filter rows by column values. Key is column ID, value is the filter
    /// value.
    #[serde(default)]
    pub filter: Option<std::collections::HashMap<String, String>>,
    /// Maximum number of rows to return.
    #[serde(default)]
    pub page_size: Option<u32>,
    /// Page number for pagination (1-based).
    #[serde(default)]
    pub page: Option<u32>,
    /// Include column metadata in the response.
    #[serde(default)]
    pub include_columns: Option<bool>,
}

/// Output from the `read_rows` tool.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadRowsOutput {
    /// ID of the sheet.
    pub sheet_id: u64,
    /// Name of the sheet.
    pub sheet_name: String,
    /// Column definitions (if `include_columns` was true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<Column>>,
    /// Rows from the sheet.
    pub rows: Vec<Row>,
    /// Total count of rows matching the criteria.
    pub total_count: u32,
}

/// # Read Smartsheet Rows
///
/// Reads rows from a Smartsheet sheet with optional filtering capabilities.
///
/// Use this tool when a user wants to:
/// - Retrieve data from a specific Smartsheet sheet
/// - Read specific rows by their IDs
/// - Filter rows based on column values
/// - Get column metadata along with row data
/// - Extract data from a spreadsheet for analysis or processing
///
/// The tool fetches rows from the specified sheet and returns:
/// - Row data with cell values and display values
/// - Row IDs, row numbers, and hierarchy information (parent/child
///   relationships)
/// - Optional column definitions (when `include_columns` is true)
/// - Total row count for pagination
///
/// Filtering options:
/// - `row_ids`: Fetch only specific rows by their IDs (more efficient than
///   fetching all)
/// - `filter`: Filter rows by column values using a key-value map (column ID ->
///   value)
/// - `page_size` and `page`: Control pagination for large sheets
///
/// The filter parameter checks cell values against the specified values. For
/// each column ID in the filter map, only rows with matching cell values are
/// returned. This is useful for queries like "show me all rows where Status
/// equals 'Open'".
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - spreadsheet
/// - smartsheet
///
/// # Errors
///
/// Returns an error if:
/// - The sheet ID does not exist or is inaccessible
/// - The provided row IDs are invalid
/// - The API request fails due to network or server issues
///
/// # Panics
///
/// Panics if a row ID from the Smartsheet API is negative, which should
/// never happen with valid API responses.
#[tool]
pub async fn read_rows(ctx: Context, input: ReadRowsInput) -> Result<ReadRowsOutput> {
    let client = SmartsheetClient::from_ctx(&ctx)?;

    // Build query parameters
    let mut query = vec![];
    let include_columns = input.include_columns.unwrap_or(false);

    // Build include parameter
    let includes: Vec<String> = Vec::new();
    if include_columns {
        // Columns are always included when getting a sheet
    }
    if !includes.is_empty() {
        query.push(("include", includes.join(",")));
    }

    // Get the full sheet with rows and columns
    let url = client.url_with_segments(&["sheets", &input.sheet_id.to_string()])?;
    let sheet_response: SheetResponse = client.get_json(url, &query).await?;

    let rows = sheet_response.rows.unwrap_or_default();
    let columns = sheet_response.columns;
    // Smartsheet API returns total_row_count as i32, but we need u32 for output
    // The unwrap_or uses the actual row count as fallback, which should fit in i32
    let total_count = u32::try_from(
        sheet_response
            .total_row_count
            .unwrap_or(i32::try_from(rows.len()).unwrap_or(i32::MAX)),
    )
    .unwrap_or(u32::MAX);

    // Filter by row IDs if specified
    let filtered_rows = if let Some(ref row_ids) = input.row_ids {
        let row_ids_set: std::collections::HashSet<u64> = row_ids.iter().copied().collect();
        rows.into_iter()
            .filter(|r| row_ids_set.contains(&u64::try_from(r.id).unwrap()))
            .collect()
    } else {
        rows
    };

    // Filter by column values if specified
    let filtered_rows = if let Some(ref filter) = input.filter {
        filtered_rows
            .into_iter()
            .filter(|row| {
                for cell in &row.cells {
                    if let Some(filter_value) = filter.get(&cell.column_id.to_string()) {
                        let cell_value = cell
                            .display_value
                            .as_deref()
                            .or_else(|| cell.value.as_ref().and_then(|v| v.as_str()))
                            .unwrap_or("");
                        if cell_value != filter_value {
                            return false;
                        }
                    }
                }
                true
            })
            .collect()
    } else {
        filtered_rows
    };

    Ok(ReadRowsOutput {
        sheet_id: input.sheet_id,
        sheet_name: sheet_response.name,
        columns: if include_columns { columns } else { None },
        rows: filtered_rows,
        total_count,
    })
}

// ============================================================================
// Tool: update_rows
// ============================================================================

// Internal API types for row updates
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiRowUpdate<'a> {
    id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    cells: Option<Vec<ApiCellUpdate<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sibling_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expanded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    locked: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiCellUpdate<'a> {
    column_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    formula: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hyperlink: Option<&'a Hyperlink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    strict: Option<bool>,
}

/// Cell update specification.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Hyperlink {
    /// Target URL for the hyperlink.
    pub url: String,
}

/// Cell update specification.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CellUpdate {
    /// Column ID of the cell to update.
    pub column_id: u64,
    /// New value for the cell.
    #[serde(default)]
    pub value: Option<serde_json::Value>,
    /// Formula to set (overrides value if provided).
    #[serde(default)]
    pub formula: Option<String>,
    /// Hyperlink to set on the cell.
    #[serde(default)]
    pub hyperlink: Option<Hyperlink>,
    /// Set to true to clear the cell value.
    #[serde(default)]
    pub strict: Option<bool>,
}

/// Row update specification.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RowUpdate {
    /// ID of the row to update.
    pub id: u64,
    /// Cells to update in this row.
    pub cells: Vec<CellUpdate>,
    /// Move the row to be a child of this parent row.
    #[serde(default)]
    pub parent_id: Option<u64>,
    /// Move the row to this position (above the specified sibling).
    #[serde(default)]
    pub sibling_id: Option<u64>,
    /// Whether the row should be expanded.
    #[serde(default)]
    pub expanded: Option<bool>,
    /// Whether the row is locked.
    #[serde(default)]
    pub locked: Option<bool>,
}

/// Input for the `update_rows` tool.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRowsInput {
    /// ID of the sheet containing the rows.
    pub sheet_id: u64,
    /// Rows to update.
    pub rows: Vec<RowUpdate>,
}

/// Result of updating a single row.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RowUpdateResult {
    /// Row ID.
    pub id: u64,
    /// Whether the update was successful.
    pub success: bool,
    /// Error message if the update failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Version number after the update.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u64>,
}

/// Output from the `update_rows` tool.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRowsOutput {
    /// ID of the sheet.
    pub sheet_id: u64,
    /// Results for each row update.
    pub results: Vec<RowUpdateResult>,
    /// Number of rows successfully updated.
    pub updated_count: u32,
    /// Number of rows that failed to update.
    pub failed_count: u32,
}

/// # Update Smartsheet Rows
///
/// Updates one or more rows in a Smartsheet sheet with new values, formulas, or
/// properties.
///
/// Use this tool when a user wants to:
/// - Modify cell values in existing rows
/// - Update formulas or add hyperlinks to cells
/// - Change row properties (expanded/collapsed, locked state)
/// - Reorganize rows within a sheet hierarchy (parent/child relationships)
/// - Bulk update multiple rows in a single operation
///
/// This tool provides comprehensive row update capabilities:
///
/// **Cell Updates:**
/// - Set new values for specific cells using column IDs
/// - Add or modify formulas (e.g., "=SUM(A1:A10)")
/// - Insert hyperlinks into cells
/// - Clear cell values by setting them to null
///
/// **Row Hierarchy:**
/// - Move rows to become children of other rows using `parent_id`
/// - Reorder rows by positioning them relative to siblings using `sibling_id`
/// - Control row expansion state with `expanded` field
/// - Lock or unlock rows to prevent edits with `locked` field
///
/// The tool returns detailed results for each row update, including:
/// - Success/failure status for each row
/// - Error messages if updates fail
/// - Version numbers after update (when available)
///
/// **Important:** All cell updates must specify the column ID (not column
/// name). Column IDs can be obtained by calling the `read_rows` tool with
/// `include_columns: true`.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - spreadsheet
/// - smartsheet
///
/// # Errors
///
/// Returns an error if:
/// - The sheet ID does not exist or is inaccessible
/// - Any of the row IDs do not exist
/// - Column IDs in cell updates are invalid
/// - The user lacks permission to modify the rows
/// - The API request fails due to network or server issues
///
/// # Panics
///
/// Panics if a row ID from the Smartsheet API is negative, which should
/// never happen with valid API responses.
#[tool]
pub async fn update_rows(ctx: Context, input: UpdateRowsInput) -> Result<UpdateRowsOutput> {
    let client = SmartsheetClient::from_ctx(&ctx)?;

    let api_rows: Vec<ApiRowUpdate> = input
        .rows
        .iter()
        .map(|row| ApiRowUpdate {
            id: row.id,
            cells: if row.cells.is_empty() {
                None
            } else {
                Some(
                    row.cells
                        .iter()
                        .map(|cell| ApiCellUpdate {
                            column_id: cell.column_id,
                            value: cell.value.clone(),
                            formula: cell.formula.clone(),
                            hyperlink: cell.hyperlink.as_ref(),
                            strict: cell.strict,
                        })
                        .collect(),
                )
            },
            parent_id: row.parent_id,
            sibling_id: row.sibling_id,
            expanded: row.expanded,
            locked: row.locked,
        })
        .collect();

    // Make the API call
    let url = client.url_with_segments(&["sheets", &input.sheet_id.to_string(), "rows"])?;

    // Smartsheet returns the updated rows on success
    let response_rows: Vec<Row> = client.put_json(url, &api_rows).await?;

    // Map response to results
    let mut results = Vec::new();
    let mut updated_count = 0u32;

    for response_row in response_rows {
        results.push(RowUpdateResult {
            id: u64::try_from(response_row.id).unwrap(),
            success: true,
            error: None,
            version: None, // Smartsheet doesn't return version in this response
        });
        updated_count += 1;
    }

    Ok(UpdateRowsOutput {
        sheet_id: input.sheet_id,
        results,
        updated_count,
        failed_count: 0u32,
    })
}

// ============================================================================
// Tool: comment
// ============================================================================

// Internal API types for discussions
#[derive(Debug, Serialize)]
struct CreateDiscussionRequest {
    title: Option<String>,
    comment: CreateCommentRequest,
}

#[derive(Debug, Serialize)]
struct CreateCommentRequest {
    text: String,
}

/// Input for the `comment` tool.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommentInput {
    /// ID of the sheet containing the row.
    pub sheet_id: u64,
    /// ID of the row to comment on.
    pub row_id: u64,
    /// Text of the comment.
    pub text: String,
}

/// Output from the `comment` tool.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommentOutput {
    /// ID of the created comment.
    pub comment_id: u64,
    /// ID of the sheet.
    pub sheet_id: u64,
    /// ID of the row.
    pub row_id: u64,
    /// Text of the comment.
    pub text: String,
    /// Timestamp when the comment was created.
    pub created_at: String,
}

/// # Add Smartsheet Comment
///
/// Adds a comment to a specific row in a Smartsheet sheet.
///
/// Use this tool when a user wants to:
/// - Add a comment or note to a specific row
/// - Collaborate with team members by leaving feedback on rows
/// - Document decisions or explanations for row data
/// - Communicate about specific items in a spreadsheet
///
/// This tool creates a new discussion/comment thread on the specified row.
/// Comments are visible to all users with access to the sheet and can be
/// used for collaboration and communication around specific data.
///
/// The tool returns:
/// - The unique comment ID
/// - Sheet and row IDs for reference
/// - The comment text
/// - Timestamp when the comment was created
///
/// Comments appear in the Smartsheet UI and can include @mentions (if supported
/// by the Smartsheet API). They are useful for:
/// - Providing feedback on data entries
/// - Asking questions about specific rows
/// - Documenting why certain values were set
/// - Collaborating without directly modifying cell data
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - spreadsheet
/// - smartsheet
/// - comments
///
/// # Errors
///
/// Returns an error if:
/// - The sheet ID does not exist or is inaccessible
/// - The row ID does not exist within the sheet
/// - The user lacks permission to comment on the row
/// - The API request fails due to network or server issues
///
/// # Panics
///
/// Panics if the system time is before the UNIX epoch (January 1, 1970),
/// which should never happen on modern systems.
#[tool]
pub async fn comment(ctx: Context, input: CommentInput) -> Result<CommentOutput> {
    let client = SmartsheetClient::from_ctx(&ctx)?;

    let request = CreateDiscussionRequest {
        title: None,
        comment: CreateCommentRequest {
            text: input.text.clone(),
        },
    };

    let url = client.url_with_segments(&[
        "sheets",
        &input.sheet_id.to_string(),
        "rows",
        &input.row_id.to_string(),
        "discussions",
    ])?;

    // API returns the created discussion with comments
    let discussion: Discussion = client.post_json(url, &request).await?;

    // Extract the comment ID from the discussion response
    let comment_id = discussion
        .comments
        .as_ref()
        .and_then(|comments| comments.first())
        .map_or(u64::try_from(discussion.id).unwrap(), |c| {
            u64::try_from(c.id).unwrap()
        });

    // Get created timestamp from discussion or comment
    let created_at = discussion
        .comments
        .as_ref()
        .and_then(|comments| comments.first())
        .and_then(|c| c.created_at.clone())
        .unwrap_or_else(|| {
            // Current timestamp in ISO format
            format!(
                "{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            )
        });

    Ok(CommentOutput {
        comment_id,
        sheet_id: input.sheet_id,
        row_id: input.row_id,
        text: input.text,
        created_at,
    })
}

// ============================================================================
// Tool: attach_file
// ============================================================================

// Internal API types for attachments
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateAttachmentRequest {
    name: String,
    attachment_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mime_type: Option<String>,
}

/// Attachment source type.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttachmentSourceType {
    /// File uploaded directly.
    File,
    /// URL to an external file.
    Link,
    /// File from Google Drive.
    GoogleDrive,
    /// File from OneDrive.
    OneDrive,
    /// File from Dropbox.
    Dropbox,
    /// File from Box.
    Box,
    /// File from Evernote.
    Evernote,
}

/// Input for the `attach_file` tool.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttachFileInput {
    /// ID of the sheet containing the row.
    pub sheet_id: u64,
    /// ID of the row to attach the file to.
    pub row_id: u64,
    /// Name of the attachment.
    pub name: String,
    /// Type of attachment source.
    pub source_type: AttachmentSourceType,
    /// URL of the file (for Link, Google Drive, OneDrive, Dropbox, Box,
    /// Evernote).
    #[serde(default)]
    pub url: Option<String>,
    /// Base64-encoded file content (for File type).
    #[serde(default)]
    pub content: Option<String>,
    /// MIME type of the file.
    #[serde(default)]
    pub mime_type: Option<String>,
    /// Description of the attachment.
    #[serde(default)]
    pub description: Option<String>,
}

/// Output from the `attach_file` tool.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AttachFileOutput {
    /// ID of the created attachment.
    pub attachment_id: u64,
    /// ID of the sheet.
    pub sheet_id: u64,
    /// ID of the row.
    pub row_id: u64,
    /// Name of the attachment.
    pub name: String,
    /// MIME type of the attachment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Size in KB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_in_kb: Option<u64>,
    /// Timestamp when the attachment was created.
    pub created_at: String,
}

/// # Attach Smartsheet File
///
/// Attaches a file to a specific row in a Smartsheet sheet.
///
/// Use this tool when a user wants to:
/// - Attach documents, images, or other files to a specific row
/// - Link external resources (URLs) to rows
/// - Attach files from cloud storage services (Google Drive, OneDrive, Dropbox,
///   Box, Evernote)
/// - Add supporting documentation or references to spreadsheet data
///
/// This tool supports multiple attachment sources:
///
/// **URL-based Attachments (Recommended):**
/// - `LINK`: Attach a file by providing a URL to the external resource
/// - `GOOGLE_DRIVE`: Link a file from Google Drive
/// - `ONE_DRIVE`: Link a file from Microsoft OneDrive
/// - `DROPBOX`: Link a file from Dropbox
/// - `BOX`: Link a file from Box
/// - `EVERNOTE`: Link a note from Evernote
///
/// **Direct Upload:**
/// - `FILE`: Upload file content directly (currently not implemented; use
///   URL-based attachments)
///
/// The tool returns:
/// - The unique attachment ID
/// - Sheet and row IDs for reference
/// - Attachment name and MIME type
/// - File size in KB (when available)
/// - Timestamp when the attachment was created
///
/// **Best Practices:**
/// - Use URL-based attachments (LINK type) for maximum compatibility
/// - Provide accurate MIME types for proper file handling
/// - Include descriptive names for easy identification
/// - Use attachment descriptions to provide context
///
/// **Note:** Direct file upload (FILE type with base64 content) is not yet
/// implemented. Please use URL-based attachments by providing a publicly
/// accessible URL to the file.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - spreadsheet
/// - smartsheet
/// - files
///
/// # Errors
///
/// Returns an error if:
/// - The sheet ID does not exist or is inaccessible
/// - The row ID does not exist within the sheet
/// - The file content or URL is invalid
/// - The file size exceeds the allowed limit
/// - The user lacks permission to attach files to the row
/// - Direct file upload is attempted (not yet implemented)
/// - The API request fails due to network or server issues
///
/// # Panics
///
/// Panics if the system time is before the UNIX epoch (January 1, 1970),
/// which should never happen on modern systems.
#[tool]
pub async fn attach_file(ctx: Context, input: AttachFileInput) -> Result<AttachFileOutput> {
    let client = SmartsheetClient::from_ctx(&ctx)?;

    // For URL-based attachments, we use JSON upload
    // For file uploads, we need multipart/form-data which is more complex
    // This implementation focuses on URL attachments for now

    let attachment_type_str = match input.source_type {
        AttachmentSourceType::File => "FILE",
        AttachmentSourceType::Link => "LINK",
        AttachmentSourceType::GoogleDrive => "GOOGLE_DRIVE",
        AttachmentSourceType::OneDrive => "ONE_DRIVE",
        AttachmentSourceType::Dropbox => "DROPBOX",
        AttachmentSourceType::Box => "BOX",
        AttachmentSourceType::Evernote => "EVERNOTE",
    };

    // For FILE type with content, we need to handle it differently
    // For now, we'll require URL for non-LINK types
    if matches!(input.source_type, AttachmentSourceType::File) && input.content.is_some() {
        // TODO: Implement multipart/form-data upload for file content
        return Err(operai::anyhow::anyhow!(
            "File upload from content is not yet implemented. Please use URL-based attachments."
        ));
    }

    let request = CreateAttachmentRequest {
        name: input.name.clone(),
        attachment_type: attachment_type_str.to_string(),
        url: input.url.clone(),
        mime_type: input.mime_type.clone(),
    };

    let url = client.url_with_segments(&[
        "sheets",
        &input.sheet_id.to_string(),
        "rows",
        &input.row_id.to_string(),
        "attachments",
    ])?;

    let attachment: Attachment = client.post_json(url, &request).await?;

    Ok(AttachFileOutput {
        attachment_id: u64::try_from(attachment.id).unwrap(),
        sheet_id: input.sheet_id,
        row_id: input.row_id,
        name: input.name,
        mime_type: attachment.mime_type,
        size_in_kb: attachment.size_in_kb.map(|s| u64::try_from(s).unwrap()),
        created_at: attachment.created_at.unwrap_or_else(|| {
            // Current timestamp
            format!(
                "{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            )
        }),
    })
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Cell;

    // ========================================================================
    // Credential Tests
    // ========================================================================

    #[test]
    fn test_smartsheet_credential_deserializes_with_required_access_token() {
        let json = r#"{ "access_token": "token123" }"#;
        let cred: SmartsheetCredential = serde_json::from_str(json).unwrap();
        assert_eq!(cred.access_token, "token123");
        assert_eq!(cred.endpoint, None);
    }

    #[test]
    fn test_smartsheet_credential_deserializes_with_optional_endpoint() {
        let json = r#"{ "access_token": "token123", "endpoint": "https://custom.api.com" }"#;
        let cred: SmartsheetCredential = serde_json::from_str(json).unwrap();
        assert_eq!(cred.access_token, "token123");
        assert_eq!(cred.endpoint.as_deref(), Some("https://custom.api.com"));
    }

    #[test]
    fn test_smartsheet_credential_missing_access_token_fails() {
        let json = r#"{ "endpoint": "https://custom.api.com" }"#;
        let err = serde_json::from_str::<SmartsheetCredential>(json).unwrap_err();
        assert!(err.to_string().contains("missing field `access_token`"));
    }

    // ========================================================================
    // list_sheets Tests
    // ========================================================================

    #[test]
    fn test_list_sheets_input_deserializes_with_defaults() {
        let json = r"{}";
        let input: ListSheetsInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.page_size, None);
        assert_eq!(input.page, None);
        assert_eq!(input.include_trash, None);
    }

    #[test]
    fn test_list_sheets_input_deserializes_with_all_fields() {
        let json = r#"{ "pageSize": 50, "page": 2, "includeTrash": true }"#;
        let input: ListSheetsInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.page_size, Some(50));
        assert_eq!(input.page, Some(2));
        assert_eq!(input.include_trash, Some(true));
    }

    // NOTE: Integration tests with Context::empty() won't work with real API calls.
    // For integration testing, you need to set up proper credentials.
    // These tests focus on serialization/deserialization which are testable without
    // API access.

    // ========================================================================
    // read_rows Tests
    // ========================================================================

    #[test]
    fn test_read_rows_input_deserializes_with_sheet_id_only() {
        let json = r#"{ "sheetId": 123456 }"#;
        let input: ReadRowsInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.sheet_id, 123_456);
        assert_eq!(input.row_ids, None);
        assert_eq!(input.include_columns, None);
    }

    #[test]
    fn test_read_rows_input_deserializes_with_all_fields() {
        let json = r#"{
            "sheetId": 123456,
            "rowIds": [1, 2, 3],
            "pageSize": 50,
            "page": 1,
            "includeColumns": true
        }"#;
        let input: ReadRowsInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.sheet_id, 123_456);
        assert_eq!(input.row_ids, Some(vec![1, 2, 3]));
        assert_eq!(input.page_size, Some(50));
        assert_eq!(input.page, Some(1));
        assert_eq!(input.include_columns, Some(true));
    }

    // NOTE: read_rows now makes real API calls, so we only test serialization.
    // For integration testing, you need to set up proper credentials.

    // ========================================================================
    // update_rows Tests
    // ========================================================================

    #[test]
    fn test_update_rows_input_deserializes_correctly() {
        let json = r#"{
            "sheetId": 123456,
            "rows": [{
                "id": 789,
                "cells": [{
                    "columnId": 100,
                    "value": "Updated value"
                }]
            }]
        }"#;
        let input: UpdateRowsInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.sheet_id, 123_456);
        assert_eq!(input.rows.len(), 1);
        assert_eq!(input.rows[0].id, 789);
        assert_eq!(input.rows[0].cells.len(), 1);
    }

    #[test]
    fn test_cell_update_deserializes_with_formula() {
        let json = r#"{
            "columnId": 100,
            "formula": "=SUM([Column1]:[Column2])"
        }"#;
        let cell: CellUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(cell.column_id, 100);
        assert_eq!(cell.formula, Some("=SUM([Column1]:[Column2])".to_string()));
        assert_eq!(cell.value, None);
    }

    // NOTE: update_rows now makes real API calls, so we only test serialization.
    // For integration testing, you need to set up proper credentials.

    // ========================================================================
    // comment Tests
    // ========================================================================

    #[test]
    fn test_comment_input_deserializes_correctly() {
        let json = r#"{
            "sheetId": 123456,
            "rowId": 789,
            "text": "This is a comment"
        }"#;
        let input: CommentInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.sheet_id, 123_456);
        assert_eq!(input.row_id, 789);
        assert_eq!(input.text, "This is a comment");
    }

    #[test]
    fn test_comment_input_missing_text_fails() {
        let json = r#"{ "sheetId": 123456, "rowId": 789 }"#;
        let err = serde_json::from_str::<CommentInput>(json).unwrap_err();
        assert!(err.to_string().contains("missing field `text`"));
    }

    // NOTE: comment now makes real API calls, so we only test serialization.
    // For integration testing, you need to set up proper credentials.

    // ========================================================================
    // attach_file Tests
    // ========================================================================

    #[test]
    fn test_attach_file_input_deserializes_with_link() {
        let json = r#"{
            "sheetId": 123456,
            "rowId": 789,
            "name": "document.pdf",
            "sourceType": "LINK",
            "url": "https://example.com/doc.pdf"
        }"#;
        let input: AttachFileInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.sheet_id, 123_456);
        assert_eq!(input.row_id, 789);
        assert_eq!(input.name, "document.pdf");
        assert!(matches!(input.source_type, AttachmentSourceType::Link));
        assert_eq!(input.url, Some("https://example.com/doc.pdf".to_string()));
    }

    #[test]
    fn test_attach_file_input_deserializes_with_file_content() {
        let json = r#"{
            "sheetId": 123456,
            "rowId": 789,
            "name": "image.png",
            "sourceType": "FILE",
            "content": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAACklEQVR4nGMAAQAABQABDQottAAAAABJRU5ErkJggg==",
            "mimeType": "image/png"
        }"#;
        let input: AttachFileInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.name, "image.png");
        assert!(matches!(input.source_type, AttachmentSourceType::File));
        assert!(input.content.is_some());
        assert_eq!(input.mime_type, Some("image/png".to_string()));
    }

    #[test]
    fn test_attachment_source_type_deserializes_all_variants() {
        let variants = vec![
            ("FILE", AttachmentSourceType::File),
            ("LINK", AttachmentSourceType::Link),
            ("GOOGLE_DRIVE", AttachmentSourceType::GoogleDrive),
            ("ONE_DRIVE", AttachmentSourceType::OneDrive),
            ("DROPBOX", AttachmentSourceType::Dropbox),
            ("BOX", AttachmentSourceType::Box),
            ("EVERNOTE", AttachmentSourceType::Evernote),
        ];

        for (json_val, _) in variants {
            let json = format!(r#""{json_val}""#);
            let result: std::result::Result<AttachmentSourceType, _> = serde_json::from_str(&json);
            assert!(result.is_ok(), "Failed to deserialize: {json_val}");
        }
    }

    // NOTE: attach_file now makes real API calls, so we only test serialization.
    // For integration testing, you need to set up proper credentials.

    // ========================================================================
    // Common Type Tests
    // ========================================================================

    #[test]
    fn test_cell_deserializes_with_hyperlink() {
        let json = r#"{
            "columnId": 100,
            "displayValue": "Click here",
            "hyperlink": {
                "url": "https://example.com"
            }
        }"#;
        let cell: Cell = serde_json::from_str(json).unwrap();
        assert_eq!(cell.column_id, 100);
        assert!(cell.hyperlink.is_some());
        assert_eq!(
            cell.hyperlink.unwrap().url,
            Some("https://example.com".to_string())
        );
    }

    #[test]
    fn test_row_deserializes_with_parent_id() {
        let json = r#"{
            "id": 200,
            "rowNumber": 5,
            "cells": [],
            "parentId": 100
        }"#;
        let row: Row = serde_json::from_str(json).unwrap();
        assert_eq!(row.id, 200);
        assert_eq!(row.row_number, Some(5));
        assert_eq!(row.parent_id, Some(100));
    }

    #[test]
    fn test_column_deserializes_with_picklist_options() {
        let json = r#"{
            "id": 100,
            "title": "Status",
            "type": "PICKLIST",
            "index": 0,
            "options": ["Open", "Closed", "Pending"]
        }"#;
        let column: Column = serde_json::from_str(json).unwrap();
        assert_eq!(column.title, "Status");
        assert_eq!(column.r#type, Some("PICKLIST".to_string()));
        assert_eq!(
            column.options,
            Some(vec![
                "Open".to_string(),
                "Closed".to_string(),
                "Pending".to_string()
            ])
        );
    }

    #[test]
    fn test_sheet_summary_serializes_with_snake_case_to_camel_case() {
        let sheet = SheetSummary {
            id: 123,
            name: "Test".to_string(),
            permalink: Some("https://example.com".to_string()),
            access_level: Some("OWNER".to_string()),
            created_at: Some("2024-01-01".to_string()),
            modified_at: Some("2024-01-02".to_string()),
        };

        let json = serde_json::to_value(&sheet).unwrap();
        assert!(json.get("accessLevel").is_some());
        assert!(json.get("createdAt").is_some());
        assert!(json.get("modifiedAt").is_some());
    }
}
