//! spreadsheets/excel-online integration for Operai Toolbox.

mod types;

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
use types::{Range, Table};

define_user_credential! {
    ExcelCredential("excel_online") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_GRAPH_ENDPOINT: &str = "https://graph.microsoft.com/v1.0";

#[init]
async fn setup() -> Result<()> {
    info!("Excel Online integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Excel Online integration shutting down");
}

// ============================================================================
// Tool 1: Read Range
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadRangeInput {
    /// The ID or path of the workbook file in OneDrive/SharePoint.
    pub workbook_id: String,
    /// The worksheet name or ID.
    pub worksheet: String,
    /// The range address (e.g., "A1:D10"). If omitted, returns the used range.
    #[serde(default)]
    pub range: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReadRangeOutput {
    pub range: Range,
}

/// # Read Excel Online Range
///
/// Reads cell values from a specified range in an Excel workbook stored on
/// OneDrive or SharePoint.
///
/// Use this tool when a user needs to retrieve data from an Excel spreadsheet,
/// such as reading specific cells, entire rows/columns, or the used range of a
/// worksheet. This tool provides access to cell values, text representations,
/// formulas, and number formatting.
///
/// **When to use:**
/// - User wants to read data from an Excel Online workbook
/// - User needs to extract specific cell ranges or entire worksheets
/// - User wants to inspect cell formulas or formatting information
///
/// **Key considerations:**
/// - The workbook must exist in the user's OneDrive or SharePoint
/// - If `range` is omitted, returns the used range (all cells with data)
/// - Requires valid Microsoft Graph API authentication via access token
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - spreadsheet
/// - excel
/// - microsoft-graph
/// - excel-online
///
/// # Errors
///
/// Returns an error if:
/// - The `workbook_id` is empty or contains only whitespace
/// - The `worksheet` is empty or contains only whitespace
/// - The user credential is missing or the access token is empty
/// - The Microsoft Graph API request fails (network errors, authentication
///   failures, etc.)
/// - The API response cannot be parsed as a range
#[tool]
pub async fn read_range(ctx: Context, input: ReadRangeInput) -> Result<ReadRangeOutput> {
    ensure!(
        !input.workbook_id.trim().is_empty(),
        "workbook_id must not be empty"
    );
    ensure!(
        !input.worksheet.trim().is_empty(),
        "worksheet must not be empty"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let range_addr = input.range.as_deref().unwrap_or("usedRange");

    let url_path = format!(
        "/me/drive/items/{}/workbook/worksheets/{}/range(address='{}')",
        input.workbook_id, input.worksheet, range_addr
    );

    let range: GraphRange = client.get_json_path(&url_path, &[], &[]).await?;

    Ok(ReadRangeOutput {
        range: map_range(range),
    })
}

// ============================================================================
// Tool 2: Write Range
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteRangeInput {
    /// The ID or path of the workbook file in OneDrive/SharePoint.
    pub workbook_id: String,
    /// The worksheet name or ID.
    pub worksheet: String,
    /// The range address (e.g., "A1:D10").
    pub range: String,
    /// Values to write (2D array).
    pub values: Vec<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct WriteRangeOutput {
    pub updated: bool,
    pub range: Range,
}

/// # Write Excel Online Range
///
/// Writes values to a specified range of cells in an Excel workbook stored on
/// OneDrive or SharePoint.
///
/// Use this tool when a user needs to update or populate data in an Excel
/// spreadsheet. This tool allows writing multiple cells at once by providing a
/// 2D array of values. The values replace any existing content in the specified
/// range.
///
/// **When to use:**
/// - User wants to update existing cell values in an Excel Online workbook
/// - User needs to write data to a specific range of cells
/// - User wants to populate a worksheet with new data
///
/// **Key considerations:**
/// - The workbook must exist in the user's OneDrive or SharePoint
/// - The `values` parameter must be a 2D array (array of rows, where each row
///   is an array of cell values)
/// - Existing data in the specified range will be overwritten
/// - Requires valid Microsoft Graph API authentication via access token
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - spreadsheet
/// - excel
/// - microsoft-graph
/// - excel-online
///
/// # Errors
///
/// Returns an error if:
/// - The `workbook_id` is empty or contains only whitespace
/// - The `worksheet` is empty or contains only whitespace
/// - The `range` is empty or contains only whitespace
/// - The `values` array is empty
/// - The user credential is missing or the access token is empty
/// - The Microsoft Graph API request fails (network errors, authentication
///   failures, etc.)
/// - The API response cannot be parsed as a range
#[tool]
pub async fn write_range(ctx: Context, input: WriteRangeInput) -> Result<WriteRangeOutput> {
    ensure!(
        !input.workbook_id.trim().is_empty(),
        "workbook_id must not be empty"
    );
    ensure!(
        !input.worksheet.trim().is_empty(),
        "worksheet must not be empty"
    );
    ensure!(!input.range.trim().is_empty(), "range must not be empty");
    ensure!(!input.values.is_empty(), "values must not be empty");

    let client = GraphClient::from_ctx(&ctx)?;

    let url_path = format!(
        "/me/drive/items/{}/workbook/worksheets/{}/range(address='{}')",
        input.workbook_id, input.worksheet, input.range
    );

    let request = GraphRangeUpdateRequest {
        values: input.values,
    };

    let range: GraphRange = client.patch_json(&url_path, &request, &[]).await?;

    Ok(WriteRangeOutput {
        updated: true,
        range: map_range(range),
    })
}

// ============================================================================
// Tool 3: Append Row
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AppendRowInput {
    /// The ID or path of the workbook file in OneDrive/SharePoint.
    pub workbook_id: String,
    /// The worksheet name or ID.
    pub worksheet: String,
    /// The table name or ID to append to.
    pub table: String,
    /// Values to append as a new row.
    pub values: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AppendRowOutput {
    pub appended: bool,
    #[serde(default)]
    pub row_index: Option<i32>,
}

/// # Append Excel Online Row
///
/// Appends a new row of data to an Excel table in a workbook stored on
/// OneDrive or SharePoint.
///
/// Use this tool when a user needs to add data to an existing Excel table. This
/// tool automatically adds the new row to the end of the table, expanding the
/// table's data range. The append operation only works with Excel tables, not
/// arbitrary worksheet ranges.
///
/// **When to use:**
/// - User wants to add a new record to an Excel table
/// - User needs to append data to a structured data set in Excel
/// - User wants to automatically expand a table with new entries
///
/// **Key considerations:**
/// - The workbook must exist in the user's OneDrive or SharePoint
/// - The target must be an Excel table (created via "Insert > Table" or the
///   `create_table` tool)
/// - Cannot append to arbitrary worksheet ranges—only works with Excel tables
/// - The `values` array should match the table's column structure
/// - If you need to append to a non-table range, use `write_range` with a
///   specific range address
/// - Requires valid Microsoft Graph API authentication via access token
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - spreadsheet
/// - excel
/// - microsoft-graph
/// - excel-online
///
/// # Errors
///
/// Returns an error if:
/// - The `workbook_id` is empty or contains only whitespace
/// - The `worksheet` is empty or contains only whitespace
/// - The `table` is empty or contains only whitespace
/// - The `values` array is empty
/// - The user credential is missing or the access token is empty
/// - The Microsoft Graph API request fails (network errors, authentication
///   failures, etc.)
/// - The API response cannot be parsed as a table row
#[tool]
pub async fn append_row(ctx: Context, input: AppendRowInput) -> Result<AppendRowOutput> {
    ensure!(
        !input.workbook_id.trim().is_empty(),
        "workbook_id must not be empty"
    );
    ensure!(
        !input.worksheet.trim().is_empty(),
        "worksheet must not be empty"
    );
    ensure!(!input.table.trim().is_empty(), "table must not be empty");
    ensure!(!input.values.is_empty(), "values must not be empty");

    let client = GraphClient::from_ctx(&ctx)?;

    let url_path = format!(
        "/me/drive/items/{}/workbook/worksheets/{}/tables/{}/rows/add",
        input.workbook_id, input.worksheet, input.table
    );

    let request = GraphTableRowAddRequest {
        values: vec![input.values],
        index: None,
    };

    let row: GraphTableRow = client.post_json(&url_path, &request, &[]).await?;

    Ok(AppendRowOutput {
        appended: true,
        row_index: row.index,
    })
}

// ============================================================================
// Tool 4: Create Workbook
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateWorkbookInput {
    /// Name of the new workbook (must end with .xlsx).
    pub name: String,
    /// Optional parent folder ID. If omitted, creates in the root drive.
    #[serde(default)]
    pub parent_folder_id: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateWorkbookOutput {
    pub workbook_id: String,
    pub name: String,
    pub web_url: Option<String>,
}

/// # Create Excel Online Workbook
///
/// Creates a new Excel workbook (.xlsx file) in the user's OneDrive or
/// SharePoint.
///
/// Use this tool when a user needs to create a new Excel spreadsheet. The
/// workbook will be created in the user's OneDrive root folder or in a
/// specified parent folder. The created file can then be accessed and
/// manipulated using other Excel Online tools.
///
/// **When to use:**
/// - User wants to create a new Excel spreadsheet
/// - User needs to set up a new workbook for data entry or analysis
/// - User wants to create a workbook that will be populated with data
///
/// **Key considerations:**
/// - The workbook name must end with `.xlsx` extension
/// - By default, creates in the root of OneDrive; specify `parent_folder_id` to
///   create in a subfolder
/// - If a file with the same name exists, it will be automatically renamed
/// - Returns the workbook ID and web URL for accessing the created file
/// - Requires valid Microsoft Graph API authentication via access token
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - spreadsheet
/// - excel
/// - microsoft-graph
/// - excel-online
///
/// # Errors
///
/// Returns an error if:
/// - The `name` is empty or contains only whitespace
/// - The `name` does not end with `.xlsx` extension
/// - The user credential is missing or the access token is empty
/// - The Microsoft Graph API request fails (network errors, authentication
///   failures, etc.)
/// - The API response cannot be parsed as a drive item
#[tool]
pub async fn create_workbook(
    ctx: Context,
    input: CreateWorkbookInput,
) -> Result<CreateWorkbookOutput> {
    ensure!(!input.name.trim().is_empty(), "name must not be empty");
    ensure!(
        std::path::Path::new(&input.name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("xlsx")),
        "name must end with .xlsx"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let url_path = if let Some(ref folder_id) = input.parent_folder_id {
        format!("/me/drive/items/{folder_id}/children")
    } else {
        "/me/drive/root/children".to_string()
    };

    let request = GraphCreateFileRequest {
        name: input.name.clone(),
        file: GraphFileMetadata {},
        microsoft_graph_conflict_behavior: Some("rename".to_string()),
    };

    let file: GraphDriveItem = client.post_json(&url_path, &request, &[]).await?;

    Ok(CreateWorkbookOutput {
        workbook_id: file.id,
        name: file.name.unwrap_or(input.name),
        web_url: file.web_url,
    })
}

// ============================================================================
// Tool 5: Table Operations (List, Create, Delete)
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTablesInput {
    /// The ID or path of the workbook file in OneDrive/SharePoint.
    pub workbook_id: String,
    /// Optional worksheet name or ID. If omitted, lists all tables in the
    /// workbook.
    #[serde(default)]
    pub worksheet: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListTablesOutput {
    pub tables: Vec<Table>,
}

/// # List Excel Online Tables
///
/// Lists all Excel tables in a workbook stored on OneDrive or SharePoint.
///
/// Use this tool when a user needs to discover what tables exist in an Excel
/// workbook. This is useful for understanding the structure of a spreadsheet
/// before performing operations like appending rows or manipulating table data.
/// Tables can be filtered by worksheet.
///
/// **When to use:**
/// - User wants to see what tables are available in a workbook
/// - User needs to find table names or IDs before performing table operations
/// - User wants to understand the structure of an Excel workbook
/// - User needs to identify which worksheet contains specific tables
///
/// **Key considerations:**
/// - The workbook must exist in the user's OneDrive or SharePoint
/// - If `worksheet` is omitted, returns all tables in the entire workbook
/// - If `worksheet` is specified, returns only tables in that worksheet
/// - Returns table metadata (ID, name, header visibility, total row visibility,
///   style)
/// - Requires valid Microsoft Graph API authentication via access token
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - spreadsheet
/// - excel
/// - microsoft-graph
/// - excel-online
///
/// # Errors
///
/// Returns an error if:
/// - The `workbook_id` is empty or contains only whitespace
/// - The user credential is missing or the access token is empty
/// - The Microsoft Graph API request fails (network errors, authentication
///   failures, etc.)
/// - The API response cannot be parsed as a list of tables
#[tool]
pub async fn list_tables(ctx: Context, input: ListTablesInput) -> Result<ListTablesOutput> {
    ensure!(
        !input.workbook_id.trim().is_empty(),
        "workbook_id must not be empty"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let url_path = if let Some(ref worksheet) = input.worksheet {
        format!(
            "/me/drive/items/{}/workbook/worksheets/{}/tables",
            input.workbook_id, worksheet
        )
    } else {
        format!("/me/drive/items/{}/workbook/tables", input.workbook_id)
    };

    let response: GraphListResponse<GraphTable> = client.get_json_path(&url_path, &[], &[]).await?;

    Ok(ListTablesOutput {
        tables: response.value.into_iter().map(map_table).collect(),
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateTableInput {
    /// The ID or path of the workbook file in OneDrive/SharePoint.
    pub workbook_id: String,
    /// The worksheet name or ID.
    pub worksheet: String,
    /// The range address for the table (e.g., "A1:D10").
    pub range: String,
    /// Whether the range has headers.
    pub has_headers: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateTableOutput {
    pub table: Table,
}

/// # Create Excel Online Table
///
/// Creates a new Excel table in a worksheet within a workbook stored on
/// OneDrive or SharePoint.
///
/// Use this tool when a user needs to convert a range of cells into an Excel
/// table. Tables provide structured data handling with features like automatic
/// filtering, sorting, and the ability to append rows. Once created, tables can
/// be used with the `append_row` tool for adding data.
///
/// **When to use:**
/// - User wants to create a structured table from a data range
/// - User needs to enable table features (filtering, sorting, structured
///   references)
/// - User wants to prepare a range for row appending operations
/// - User needs to format data as a professional table with styling
///
/// **Key considerations:**
/// - The workbook must exist in the user's OneDrive or SharePoint
/// - The range must contain valid data (can be empty or populated)
/// - Tables are required for `append_row` operations
/// - Set `has_headers` to true if the first row contains column headers
/// - The table will be created with default Excel table styling
/// - Requires valid Microsoft Graph API authentication via access token
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - spreadsheet
/// - excel
/// - microsoft-graph
/// - excel-online
///
/// # Errors
///
/// Returns an error if:
/// - The `workbook_id` is empty or contains only whitespace
/// - The `worksheet` is empty or contains only whitespace
/// - The `range` is empty or contains only whitespace
/// - The user credential is missing or the access token is empty
/// - The Microsoft Graph API request fails (network errors, authentication
///   failures, etc.)
/// - The API response cannot be parsed as a table
#[tool]
pub async fn create_table(ctx: Context, input: CreateTableInput) -> Result<CreateTableOutput> {
    ensure!(
        !input.workbook_id.trim().is_empty(),
        "workbook_id must not be empty"
    );
    ensure!(
        !input.worksheet.trim().is_empty(),
        "worksheet must not be empty"
    );
    ensure!(!input.range.trim().is_empty(), "range must not be empty");

    let client = GraphClient::from_ctx(&ctx)?;

    let url_path = format!(
        "/me/drive/items/{}/workbook/worksheets/{}/tables/add",
        input.workbook_id, input.worksheet
    );

    let request = GraphTableAddRequest {
        address: input.range,
        has_headers: input.has_headers,
    };

    let table: GraphTable = client.post_json(&url_path, &request, &[]).await?;

    Ok(CreateTableOutput {
        table: map_table(table),
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteTableInput {
    /// The ID or path of the workbook file in OneDrive/SharePoint.
    pub workbook_id: String,
    /// The worksheet name or ID.
    pub worksheet: String,
    /// The table name or ID to delete.
    pub table: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DeleteTableOutput {
    pub deleted: bool,
}

/// # Delete Excel Online Table
///
/// Deletes an Excel table from a worksheet in a workbook stored on OneDrive
/// or SharePoint.
///
/// Use this tool when a user needs to remove a table from an Excel worksheet.
/// This operation removes the table structure and formatting, but preserves the
/// underlying data in the cells. The table's features (filtering, sorting,
/// structured references) will be removed.
///
/// **When to use:**
/// - User wants to remove a table structure while keeping the data
/// - User needs to clean up unused or unnecessary tables
/// - User wants to convert a table back to a regular cell range
///
/// **Key considerations:**
/// - The workbook must exist in the user's OneDrive or SharePoint
/// - Deleting the table removes the table object but not the cell data
/// - The table formatting will be removed
/// - This operation cannot be undone
/// - Use `list_tables` first to find the correct table name/ID
/// - Requires valid Microsoft Graph API authentication via access token
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - spreadsheet
/// - excel
/// - microsoft-graph
/// - excel-online
///
/// # Errors
///
/// Returns an error if:
/// - The `workbook_id` is empty or contains only whitespace
/// - The `worksheet` is empty or contains only whitespace
/// - The `table` is empty or contains only whitespace
/// - The user credential is missing or the access token is empty
/// - The Microsoft Graph API request fails (network errors, authentication
///   failures, etc.)
#[tool]
pub async fn delete_table(ctx: Context, input: DeleteTableInput) -> Result<DeleteTableOutput> {
    ensure!(
        !input.workbook_id.trim().is_empty(),
        "workbook_id must not be empty"
    );
    ensure!(
        !input.worksheet.trim().is_empty(),
        "worksheet must not be empty"
    );
    ensure!(!input.table.trim().is_empty(), "table must not be empty");

    let client = GraphClient::from_ctx(&ctx)?;

    let url_path = format!(
        "/me/drive/items/{}/workbook/worksheets/{}/tables/{}",
        input.workbook_id, input.worksheet, input.table
    );

    client.delete(&url_path, &[]).await?;

    Ok(DeleteTableOutput { deleted: true })
}

// ============================================================================
// Internal Graph API Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct GraphListResponse<T> {
    value: Vec<T>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphRange {
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    address_local: Option<String>,
    #[serde(default)]
    cell_count: Option<i32>,
    #[serde(default)]
    column_count: Option<i32>,
    #[serde(default)]
    column_index: Option<i32>,
    #[serde(default)]
    row_count: Option<i32>,
    #[serde(default)]
    row_index: Option<i32>,
    #[serde(default)]
    values: Option<Vec<Vec<serde_json::Value>>>,
    #[serde(default)]
    text: Option<Vec<Vec<String>>>,
    #[serde(default)]
    formulas: Option<Vec<Vec<serde_json::Value>>>,
    #[serde(default)]
    number_format: Option<Vec<Vec<String>>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphRangeUpdateRequest {
    values: Vec<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphTable {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    show_headers: Option<bool>,
    #[serde(default)]
    show_totals: Option<bool>,
    #[serde(default)]
    style: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphTableAddRequest {
    address: String,
    has_headers: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphTableRow {
    #[serde(default)]
    index: Option<i32>,
    #[serde(default)]
    values: Option<Vec<Vec<serde_json::Value>>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphTableRowAddRequest {
    values: Vec<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    index: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphDriveItem {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    web_url: Option<String>,
}

#[derive(Debug, Serialize)]
struct GraphFileMetadata {}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphCreateFileRequest {
    name: String,
    file: GraphFileMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "@microsoft.graph.conflictBehavior")]
    microsoft_graph_conflict_behavior: Option<String>,
}

// ============================================================================
// Mapping Functions
// ============================================================================

fn map_range(range: GraphRange) -> Range {
    Range {
        address: range.address,
        address_local: range.address_local,
        cell_count: range.cell_count,
        column_count: range.column_count,
        column_index: range.column_index,
        row_count: range.row_count,
        row_index: range.row_index,
        values: range.values,
        text: range.text,
        formulas: range.formulas,
        number_format: range.number_format,
    }
}

fn map_table(table: GraphTable) -> Table {
    Table {
        id: table.id,
        name: table.name,
        show_headers: table.show_headers,
        show_totals: table.show_totals,
        style: table.style,
    }
}

// ============================================================================
// Graph Client
// ============================================================================

#[derive(Debug, Clone)]
struct GraphClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl GraphClient {
    /// Creates a new `GraphClient` from the provided context.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The Excel credential is missing from the context
    /// - The access token is empty or contains only whitespace
    /// - The endpoint URL is invalid (empty after trimming)
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = ExcelCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_GRAPH_ENDPOINT))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            access_token: cred.access_token,
        })
    }

    async fn get_json_path<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        query: &[(&str, String)],
        extra_headers: &[(&str, &str)],
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let mut request = self.http.get(&url).query(query);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        let response = self.send_request(request).await?;
        Ok(response.json::<T>().await?)
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &TReq,
        extra_headers: &[(&str, &str)],
    ) -> Result<TRes> {
        let url = format!("{}{}", self.base_url, path);
        let mut request = self.http.post(&url).json(body);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        let response = self.send_request(request).await?;
        Ok(response.json::<TRes>().await?)
    }

    async fn patch_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &TReq,
        extra_headers: &[(&str, &str)],
    ) -> Result<TRes> {
        let url = format!("{}{}", self.base_url, path);
        let mut request = self.http.patch(&url).json(body);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        let response = self.send_request(request).await?;
        Ok(response.json::<TRes>().await?)
    }

    async fn delete(&self, path: &str, extra_headers: &[(&str, &str)]) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let mut request = self.http.delete(&url);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        self.send_request(request).await?;
        Ok(())
    }

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
            Err(operai::anyhow::anyhow!(
                "Microsoft Graph request failed ({status}): {body}"
            ))
        }
    }
}

/// Normalizes a Microsoft Graph endpoint URL by removing trailing slashes.
///
/// # Errors
///
/// Returns an error if:
/// - The endpoint is empty or contains only whitespace after trimming
fn normalize_base_url(endpoint: &str) -> Result<String> {
    let trimmed = endpoint.trim();
    ensure!(!trimmed.is_empty(), "endpoint must not be empty");
    Ok(trimmed.trim_end_matches('/').to_string())
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_string_contains, method, path_regex},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut excel_values = HashMap::new();
        excel_values.insert("access_token".to_string(), "test-token".to_string());
        excel_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("excel_online", excel_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        format!("{}/v1.0", server.uri())
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_range_serialization_roundtrip() {
        let range = Range {
            address: Some("A1:B2".to_string()),
            address_local: Some("A1:B2".to_string()),
            cell_count: Some(4),
            column_count: Some(2),
            column_index: Some(0),
            row_count: Some(2),
            row_index: Some(0),
            values: Some(vec![
                vec![serde_json::json!(1), serde_json::json!(2)],
                vec![serde_json::json!(3), serde_json::json!(4)],
            ]),
            text: Some(vec![
                vec!["1".to_string(), "2".to_string()],
                vec!["3".to_string(), "4".to_string()],
            ]),
            formulas: Some(vec![
                vec![serde_json::json!(null), serde_json::json!(null)],
                vec![serde_json::json!(null), serde_json::json!(null)],
            ]),
            number_format: Some(vec![
                vec!["General".to_string(), "General".to_string()],
                vec!["General".to_string(), "General".to_string()],
            ]),
        };
        let json = serde_json::to_string(&range).unwrap();
        let parsed: Range = serde_json::from_str(&json).unwrap();
        assert_eq!(range.address, parsed.address);
        assert_eq!(range.cell_count, parsed.cell_count);
    }

    #[test]
    fn test_table_serialization_roundtrip() {
        let table = Table {
            id: "table1".to_string(),
            name: Some("Table1".to_string()),
            show_headers: Some(true),
            show_totals: Some(false),
            style: Some("TableStyleMedium2".to_string()),
        };
        let json = serde_json::to_string(&table).unwrap();
        let parsed: Table = serde_json::from_str(&json).unwrap();
        assert_eq!(table.id, parsed.id);
        assert_eq!(table.name, parsed.name);
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://graph.microsoft.com/").unwrap();
        assert_eq!(result, "https://graph.microsoft.com");
    }

    #[test]
    fn test_normalize_base_url_empty_returns_error() {
        let result = normalize_base_url("");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must not be empty")
        );
    }

    // --- Input validation tests ---

    #[tokio::test]
    async fn test_read_range_empty_workbook_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = read_range(
            ctx,
            ReadRangeInput {
                workbook_id: "  ".to_string(),
                worksheet: "Sheet1".to_string(),
                range: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("workbook_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_read_range_empty_worksheet_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = read_range(
            ctx,
            ReadRangeInput {
                workbook_id: "workbook1".to_string(),
                worksheet: "  ".to_string(),
                range: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("worksheet must not be empty")
        );
    }

    #[tokio::test]
    async fn test_write_range_empty_range_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = write_range(
            ctx,
            WriteRangeInput {
                workbook_id: "workbook1".to_string(),
                worksheet: "Sheet1".to_string(),
                range: "  ".to_string(),
                values: vec![vec![serde_json::json!(1)]],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("range must not be empty")
        );
    }

    #[tokio::test]
    async fn test_write_range_empty_values_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = write_range(
            ctx,
            WriteRangeInput {
                workbook_id: "workbook1".to_string(),
                worksheet: "Sheet1".to_string(),
                range: "A1:A1".to_string(),
                values: vec![],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("values must not be empty")
        );
    }

    #[tokio::test]
    async fn test_write_range_empty_worksheet_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = write_range(
            ctx,
            WriteRangeInput {
                workbook_id: "workbook1".to_string(),
                worksheet: "  ".to_string(),
                range: "A1:A1".to_string(),
                values: vec![vec![serde_json::json!(1)]],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("worksheet must not be empty")
        );
    }

    #[tokio::test]
    async fn test_append_row_empty_worksheet_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = append_row(
            ctx,
            AppendRowInput {
                workbook_id: "workbook1".to_string(),
                worksheet: "  ".to_string(),
                table: "Table1".to_string(),
                values: vec![serde_json::json!(1)],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("worksheet must not be empty")
        );
    }

    #[tokio::test]
    async fn test_append_row_empty_table_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = append_row(
            ctx,
            AppendRowInput {
                workbook_id: "workbook1".to_string(),
                worksheet: "Sheet1".to_string(),
                table: "  ".to_string(),
                values: vec![serde_json::json!(1)],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("table must not be empty")
        );
    }

    #[tokio::test]
    async fn test_append_row_empty_values_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = append_row(
            ctx,
            AppendRowInput {
                workbook_id: "workbook1".to_string(),
                worksheet: "Sheet1".to_string(),
                table: "Table1".to_string(),
                values: vec![],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("values must not be empty")
        );
    }

    #[tokio::test]
    async fn test_create_workbook_empty_name_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = create_workbook(
            ctx,
            CreateWorkbookInput {
                name: "  ".to_string(),
                parent_folder_id: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("name must not be empty")
        );
    }

    #[tokio::test]
    async fn test_create_workbook_name_without_xlsx_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = create_workbook(
            ctx,
            CreateWorkbookInput {
                name: "workbook.txt".to_string(),
                parent_folder_id: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("name must end with .xlsx")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_read_range_success_returns_range() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "address": "Sheet1!A1:B2",
          "addressLocal": "Sheet1!A1:B2",
          "cellCount": 4,
          "columnCount": 2,
          "columnIndex": 0,
          "rowCount": 2,
          "rowIndex": 0,
          "values": [[1, 2], [3, 4]],
          "text": [["1", "2"], ["3", "4"]]
        }
        "#;

        Mock::given(method("GET"))
            .and(path_regex(r"/v1\.0/me/drive/items/.*/workbook/.*"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = read_range(
            ctx,
            ReadRangeInput {
                workbook_id: "workbook1".to_string(),
                worksheet: "Sheet1".to_string(),
                range: Some("A1:B2".to_string()),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.range.address.as_deref(), Some("Sheet1!A1:B2"));
        assert_eq!(output.range.cell_count, Some(4));
        assert!(output.range.values.is_some());
    }

    #[tokio::test]
    async fn test_write_range_success_returns_updated_range() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "address": "Sheet1!A1:A1",
          "values": [[42]]
        }
        "#;

        Mock::given(method("PATCH"))
            .and(path_regex(r"/v1\.0/me/drive/items/.*/workbook/.*"))
            .and(body_string_contains("\"values\":[[42]]"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = write_range(
            ctx,
            WriteRangeInput {
                workbook_id: "workbook1".to_string(),
                worksheet: "Sheet1".to_string(),
                range: "A1".to_string(),
                values: vec![vec![serde_json::json!(42)]],
            },
        )
        .await
        .unwrap();

        assert!(output.updated);
        assert_eq!(output.range.address.as_deref(), Some("Sheet1!A1:A1"));
    }

    #[tokio::test]
    async fn test_create_workbook_success_returns_workbook_id() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "workbook-123",
          "name": "MyWorkbook.xlsx",
          "webUrl": "https://example.com/workbook"
        }
        "#;

        Mock::given(method("POST"))
            .and(path_regex(r"/v1\.0/me/drive/.*/children"))
            .and(body_string_contains("\"name\":\"MyWorkbook.xlsx\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = create_workbook(
            ctx,
            CreateWorkbookInput {
                name: "MyWorkbook.xlsx".to_string(),
                parent_folder_id: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.workbook_id, "workbook-123");
        assert_eq!(output.name, "MyWorkbook.xlsx");
        assert_eq!(
            output.web_url.as_deref(),
            Some("https://example.com/workbook")
        );
    }

    #[tokio::test]
    async fn test_list_tables_success_returns_tables() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "value": [
            {
              "id": "table1",
              "name": "Table1",
              "showHeaders": true,
              "showTotals": false,
              "style": "TableStyleMedium2"
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path_regex(r"/v1\.0/me/drive/items/.*/workbook/.*tables"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_tables(
            ctx,
            ListTablesInput {
                workbook_id: "workbook1".to_string(),
                worksheet: Some("Sheet1".to_string()),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.tables.len(), 1);
        assert_eq!(output.tables[0].id, "table1");
        assert_eq!(output.tables[0].name.as_deref(), Some("Table1"));
    }

    #[tokio::test]
    async fn test_create_table_success_returns_table() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "table1",
          "name": "Table1",
          "showHeaders": true
        }
        "#;

        Mock::given(method("POST"))
            .and(path_regex(
                r"/v1\.0/me/drive/items/.*/workbook/.*tables/add",
            ))
            .and(body_string_contains("\"address\":\"A1:B10\""))
            .and(body_string_contains("\"hasHeaders\":true"))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = create_table(
            ctx,
            CreateTableInput {
                workbook_id: "workbook1".to_string(),
                worksheet: "Sheet1".to_string(),
                range: "A1:B10".to_string(),
                has_headers: true,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.table.id, "table1");
        assert_eq!(output.table.name.as_deref(), Some("Table1"));
    }

    #[tokio::test]
    async fn test_delete_table_success_returns_deleted() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("DELETE"))
            .and(path_regex(r"/v1\.0/me/drive/items/.*/workbook/.*tables/.*"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = delete_table(
            ctx,
            DeleteTableInput {
                workbook_id: "workbook1".to_string(),
                worksheet: "Sheet1".to_string(),
                table: "table1".to_string(),
            },
        )
        .await
        .unwrap();

        assert!(output.deleted);
    }

    #[tokio::test]
    async fn test_append_row_success_returns_appended() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "index": 5
        }
        "#;

        Mock::given(method("POST"))
            .and(path_regex(
                r"/v1\.0/me/drive/items/.*/workbook/.*tables/.*rows/add",
            ))
            .and(body_string_contains("\"values\":[[42]]"))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = append_row(
            ctx,
            AppendRowInput {
                workbook_id: "workbook1".to_string(),
                worksheet: "Sheet1".to_string(),
                table: "Table1".to_string(),
                values: vec![serde_json::json!(42)],
            },
        )
        .await
        .unwrap();

        assert!(output.appended);
        assert_eq!(output.row_index, Some(5));
    }

    #[tokio::test]
    async fn test_graph_error_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(401).set_body_raw(
                r#"{ "error": { "code": "InvalidAuthenticationToken", "message": "Bad token" } }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = read_range(
            ctx,
            ReadRangeInput {
                workbook_id: "workbook1".to_string(),
                worksheet: "Sheet1".to_string(),
                range: None,
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("401"));
    }
}
