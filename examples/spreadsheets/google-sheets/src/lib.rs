//! Google Sheets integration for Operai Toolbox.
//!
//! This integration provides tools for interacting with Google Sheets
//! spreadsheets:
//! - Read data from a range of cells
//! - Write data to a range of cells
//! - Append rows to a sheet
//! - Create new sheets within a spreadsheet
//! - Evaluate basic formulas

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

mod types;
use types::{
    AddSheet, AddSheetRequest, BatchUpdateResponse, GridProperties, SheetProperties,
    SheetsAppendResponse, SheetsValueRange, SheetsValueRangeInput, UpdateValuesResponse,
};

// Google Sheets API requires OAuth2 credentials
define_user_credential! {
    GoogleSheetsCredential("google_sheets") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_SHEETS_ENDPOINT: &str = "https://sheets.googleapis.com/v4";

/// Initialize the Google Sheets tool library.
#[init]
async fn setup() -> Result<()> {
    info!("Google Sheets integration initialized");
    Ok(())
}

/// Clean up resources when the library is unloaded.
#[shutdown]
fn cleanup() {
    info!("Google Sheets integration shutting down");
}

// ============================================================================
// Read Range Tool
// ============================================================================

/// Input for reading a range from a Google Sheet.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadRangeInput {
    /// The ID of the spreadsheet (from the URL).
    pub spreadsheet_id: String,
    /// The A1 notation range to read (e.g., "Sheet1!A1:D10").
    pub range: String,
    /// How values should be represented in the output.
    #[serde(default)]
    pub value_render_option: Option<ValueRenderOption>,
}

/// How values should be rendered in the output.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ValueRenderOption {
    /// Values will be calculated and formatted according to the cell's
    /// formatting.
    #[default]
    FormattedValue,
    /// Values will be calculated but not formatted.
    UnformattedValue,
    /// Values will not be calculated. The reply includes formulas.
    Formula,
}

/// Output from reading a range.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ReadRangeOutput {
    /// The range that was read in A1 notation.
    pub range: String,
    /// The values in the range. Each inner vector represents a row.
    pub values: Vec<Vec<CellValue>>,
    /// Number of rows returned.
    pub row_count: usize,
    /// Number of columns returned (from the first row).
    pub column_count: usize,
}

/// A cell value that can be a string, number, boolean, or null.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(untagged)]
pub enum CellValue {
    /// A string value.
    String(String),
    /// A numeric value.
    Number(f64),
    /// A boolean value.
    Boolean(bool),
    /// An empty or null cell.
    Null,
}

/// # Read Google Sheets Range
///
/// Reads cell values from a specified range in a Google Sheets spreadsheet
/// using the Google Sheets API. Use this tool when the user wants to retrieve
/// data from a spreadsheet, such as reading a table, extracting specific cells,
/// or getting the contents of a named range.
///
/// The `spreadsheet_id` can be found in the URL: `https://docs.google.com/spreadsheets/d/{SPREADSHEET_ID}/edit`.
/// The range should be in A1 notation (e.g., "Sheet1!A1:D10" or "Sheet1!A:Z"
/// for entire columns).
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - spreadsheets
/// - google-sheets
/// - data
///
/// # Errors
///
/// Returns an error if:
/// - The `spreadsheet_id` is empty
/// - The `range` is empty
/// - The Google Sheets credential is not found or invalid
/// - The API request fails (network errors, authentication failures, rate
///   limits)
/// - The response cannot be parsed
#[tool]
pub async fn read_range(ctx: Context, input: ReadRangeInput) -> Result<ReadRangeOutput> {
    ensure!(
        !input.spreadsheet_id.trim().is_empty(),
        "spreadsheet_id must not be empty"
    );
    ensure!(!input.range.trim().is_empty(), "range must not be empty");

    let client = SheetsClient::from_ctx(&ctx)?;

    let value_render = match input
        .value_render_option
        .as_ref()
        .unwrap_or(&ValueRenderOption::FormattedValue)
    {
        ValueRenderOption::FormattedValue => "FORMATTED_VALUE",
        ValueRenderOption::UnformattedValue => "UNFORMATTED_VALUE",
        ValueRenderOption::Formula => "FORMULA",
    };

    let query = [("valueRenderOption", value_render.to_string())];

    let response: SheetsValueRange = client
        .get_json(
            client.url_with_segments(&[
                "spreadsheets",
                &input.spreadsheet_id,
                "values",
                &input.range,
            ])?,
            &query,
        )
        .await?;

    let values: Vec<Vec<CellValue>> = response
        .values
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|cell| match cell {
                    serde_json::Value::String(s) => CellValue::String(s),
                    serde_json::Value::Number(n) => CellValue::Number(n.as_f64().unwrap_or(0.0)),
                    serde_json::Value::Bool(b) => CellValue::Boolean(b),
                    serde_json::Value::Null => CellValue::Null,
                    _ => CellValue::String(cell.to_string()),
                })
                .collect()
        })
        .collect();

    let row_count = values.len();
    let column_count = values.first().map_or(0, Vec::len);

    Ok(ReadRangeOutput {
        range: response.range,
        values,
        row_count,
        column_count,
    })
}

// ============================================================================
// Write Range Tool
// ============================================================================

/// Input for writing values to a range in a Google Sheet.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteRangeInput {
    /// The ID of the spreadsheet.
    pub spreadsheet_id: String,
    /// The A1 notation range to write to (e.g., "Sheet1!A1:D10").
    pub range: String,
    /// The values to write. Each inner vector represents a row.
    pub values: Vec<Vec<CellValue>>,
    /// How input values should be interpreted.
    #[serde(default)]
    pub value_input_option: Option<ValueInputOption>,
}

/// How input values should be interpreted.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ValueInputOption {
    /// Values will be stored as-is.
    Raw,
    /// Values will be parsed as if typed into the UI (formulas evaluated).
    #[default]
    UserEntered,
}

/// Output from writing a range.
#[derive(Debug, Serialize, JsonSchema)]
pub struct WriteRangeOutput {
    /// The spreadsheet ID.
    pub spreadsheet_id: String,
    /// The range that was updated in A1 notation.
    pub updated_range: String,
    /// Number of rows updated.
    pub updated_rows: usize,
    /// Number of columns updated.
    pub updated_columns: usize,
    /// Total number of cells updated.
    pub updated_cells: usize,
}

/// # Write Google Sheets Range
///
/// Writes cell values to a specified range in a Google Sheets spreadsheet using
/// the Google Sheets API. Use this tool when the user wants to update or
/// populate data in a spreadsheet, such as writing a table of data, updating
/// specific cells, or setting values in a named range.
///
/// The `spreadsheet_id` can be found in the URL: `https://docs.google.com/spreadsheets/d/{SPREADSHEET_ID}/edit`.
/// The range should be in A1 notation (e.g., "Sheet1!A1:D10"). The values array
/// should contain rows of data, where each inner vector represents a row and
/// each `CellValue` can be a string, number, boolean, or null.
///
/// By default, values are interpreted as if typed into the UI (formulas are
/// evaluated). Use the `value_input_option` parameter to control this behavior.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - spreadsheets
/// - google-sheets
/// - data
///
/// # Errors
///
/// Returns an error if:
/// - The `spreadsheet_id` is empty
/// - The `range` is empty
/// - The `values` array is empty
/// - The Google Sheets credential is not found or invalid
/// - The API request fails (network errors, authentication failures, rate
///   limits)
/// - The response cannot be parsed
#[tool]
pub async fn write_range(ctx: Context, input: WriteRangeInput) -> Result<WriteRangeOutput> {
    ensure!(
        !input.spreadsheet_id.trim().is_empty(),
        "spreadsheet_id must not be empty"
    );
    ensure!(!input.range.trim().is_empty(), "range must not be empty");
    ensure!(!input.values.is_empty(), "values must not be empty");

    let client = SheetsClient::from_ctx(&ctx)?;

    let value_input = match input
        .value_input_option
        .as_ref()
        .unwrap_or(&ValueInputOption::UserEntered)
    {
        ValueInputOption::Raw => "RAW",
        ValueInputOption::UserEntered => "USER_ENTERED",
    };

    let api_values: Vec<Vec<serde_json::Value>> = input
        .values
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| match cell {
                    CellValue::String(s) => serde_json::Value::String(s.clone()),
                    CellValue::Number(n) => serde_json::json!(n),
                    CellValue::Boolean(b) => serde_json::Value::Bool(*b),
                    CellValue::Null => serde_json::Value::Null,
                })
                .collect()
        })
        .collect();

    let request_body = SheetsValueRangeInput {
        range: input.range.clone(),
        major_dimension: "ROWS".to_string(),
        values: api_values,
    };

    let query = [("valueInputOption", value_input.to_string())];

    let response: UpdateValuesResponse = client
        .put_json(
            client.url_with_segments(&[
                "spreadsheets",
                &input.spreadsheet_id,
                "values",
                &input.range,
            ])?,
            &request_body,
            &query,
        )
        .await?;

    Ok(WriteRangeOutput {
        spreadsheet_id: response.spreadsheet_id,
        updated_range: response.updated_range,
        updated_rows: response.updated_rows as usize,
        updated_columns: response.updated_columns as usize,
        updated_cells: response.updated_cells as usize,
    })
}

// ============================================================================
// Append Row Tool
// ============================================================================

/// Input for appending a row to a Google Sheet.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AppendRowInput {
    /// The ID of the spreadsheet.
    pub spreadsheet_id: String,
    /// The sheet name or A1 range to append to (e.g., "Sheet1" or
    /// "Sheet1!A:Z").
    pub range: String,
    /// The values for the new row.
    pub values: Vec<CellValue>,
    /// How input values should be interpreted.
    #[serde(default)]
    pub value_input_option: Option<ValueInputOption>,
    /// How the input data should be inserted.
    #[serde(default)]
    pub insert_data_option: Option<InsertDataOption>,
}

/// How the input data should be inserted.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InsertDataOption {
    /// Data is appended after the last row with data.
    #[default]
    InsertRows,
    /// Data overwrites existing cells.
    Overwrite,
}

/// Output from appending a row.
#[derive(Debug, Serialize, JsonSchema)]
pub struct AppendRowOutput {
    /// The spreadsheet ID.
    pub spreadsheet_id: String,
    /// The range where the row was appended in A1 notation.
    pub updated_range: String,
    /// The row number where the data was appended (1-indexed).
    pub appended_row: usize,
    /// Number of cells written.
    pub updated_cells: usize,
}

/// # Append Google Sheets Row
///
/// Appends a new row of values to the end of a Google Sheets spreadsheet using
/// the Google Sheets API. Use this tool when the user wants to add a new record
/// to a spreadsheet table, log data entries, or append data to an existing
/// dataset without overwriting existing content.
///
/// The `spreadsheet_id` can be found in the URL: `https://docs.google.com/spreadsheets/d/{SPREADSHEET_ID}/edit`.
/// The range can be a sheet name (e.g., "Sheet1") or an A1 notation range
/// (e.g., "Sheet1!A:Z"). The values array should contain the cell values for
/// the new row in order.
///
/// By default, the row is appended after the last row with data in the sheet.
/// Use the `insert_data_option` parameter to control whether to insert new rows
/// or overwrite existing cells.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - spreadsheets
/// - google-sheets
/// - data
///
/// # Errors
///
/// Returns an error if:
/// - The `spreadsheet_id` is empty
/// - The `range` is empty
/// - The `values` array is empty
/// - The Google Sheets credential is not found or invalid
/// - The API request fails (network errors, authentication failures, rate
///   limits)
/// - The response cannot be parsed or doesn't contain the expected update
///   information
#[tool]
pub async fn append_row(ctx: Context, input: AppendRowInput) -> Result<AppendRowOutput> {
    ensure!(
        !input.spreadsheet_id.trim().is_empty(),
        "spreadsheet_id must not be empty"
    );
    ensure!(!input.range.trim().is_empty(), "range must not be empty");
    ensure!(!input.values.is_empty(), "values must not be empty");

    let client = SheetsClient::from_ctx(&ctx)?;

    let value_input = match input
        .value_input_option
        .as_ref()
        .unwrap_or(&ValueInputOption::UserEntered)
    {
        ValueInputOption::Raw => "RAW",
        ValueInputOption::UserEntered => "USER_ENTERED",
    };

    let insert_data = match input
        .insert_data_option
        .as_ref()
        .unwrap_or(&InsertDataOption::InsertRows)
    {
        InsertDataOption::InsertRows => "INSERT_ROWS",
        InsertDataOption::Overwrite => "OVERWRITE",
    };

    let api_values: Vec<serde_json::Value> = input
        .values
        .iter()
        .map(|cell| match cell {
            CellValue::String(s) => serde_json::Value::String(s.clone()),
            CellValue::Number(n) => serde_json::json!(n),
            CellValue::Boolean(b) => serde_json::Value::Bool(*b),
            CellValue::Null => serde_json::Value::Null,
        })
        .collect();

    let request_body = SheetsValueRangeInput {
        range: input.range.clone(),
        major_dimension: "ROWS".to_string(),
        values: vec![api_values],
    };

    let query = [
        ("valueInputOption", value_input.to_string()),
        ("insertDataOption", insert_data.to_string()),
    ];

    let response: SheetsAppendResponse = client
        .post_json(
            client.url_with_segments(&[
                "spreadsheets",
                &input.spreadsheet_id,
                "values",
                &input.range,
                ":append",
            ])?,
            &request_body,
            &query,
        )
        .await?;

    // Extract row number from updated_range (e.g., "Sheet1!A10:C10" -> 10)
    let appended_row = response
        .updates
        .updated_range
        .split('!')
        .nth(1)
        .and_then(|part| part.chars().find(char::is_ascii_digit))
        .and_then(|_| {
            response
                .updates
                .updated_range
                .chars()
                .filter(char::is_ascii_digit)
                .collect::<String>()
                .parse::<usize>()
                .ok()
        })
        .unwrap_or(1);

    Ok(AppendRowOutput {
        spreadsheet_id: response.spreadsheet_id,
        updated_range: response.updates.updated_range,
        appended_row,
        updated_cells: response.updates.updated_cells as usize,
    })
}

// ============================================================================
// Create Sheet Tool
// ============================================================================

/// Input for creating a new sheet within a spreadsheet.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateSheetInput {
    /// The ID of the spreadsheet.
    pub spreadsheet_id: String,
    /// The title for the new sheet.
    pub title: String,
    /// Optional number of rows for the new sheet.
    #[serde(default)]
    pub row_count: Option<u32>,
    /// Optional number of columns for the new sheet.
    #[serde(default)]
    pub column_count: Option<u32>,
    /// Optional index at which to insert the sheet (0-indexed).
    #[serde(default)]
    pub index: Option<u32>,
}

/// Output from creating a new sheet.
#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateSheetOutput {
    /// The spreadsheet ID.
    pub spreadsheet_id: String,
    /// The ID of the newly created sheet.
    pub sheet_id: u64,
    /// The title of the newly created sheet.
    pub title: String,
    /// The index of the sheet within the spreadsheet.
    pub index: u32,
    /// Number of rows in the sheet.
    pub row_count: u32,
    /// Number of columns in the sheet.
    pub column_count: u32,
}

/// # Create Google Sheets Sheet
///
/// Creates a new sheet (tab) within a Google Sheets spreadsheet using the
/// Google Sheets API. Use this tool when the user wants to add a new worksheet
/// to an existing spreadsheet, organize data into separate tabs, or create
/// additional workspaces for different datasets.
///
/// The `spreadsheet_id` can be found in the URL: `https://docs.google.com/spreadsheets/d/{SPREADSHEET_ID}/edit`.
/// The title will be displayed as the sheet tab name. Optionally, you can
/// specify custom dimensions (`row_count`, `column_count`) and the position
/// where the sheet should be inserted (index).
///
/// Default dimensions are 1000 rows × 26 columns (A-Z). The index is 0-based (0
/// = first position).
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - spreadsheets
/// - google-sheets
/// - management
///
/// # Errors
///
/// Returns an error if:
/// - The `spreadsheet_id` is empty
/// - The `title` is empty
/// - The Google Sheets credential is not found or invalid
/// - The API request fails (network errors, authentication failures, rate
///   limits)
/// - The response doesn't contain the added sheet information
#[tool]
pub async fn create_sheet(ctx: Context, input: CreateSheetInput) -> Result<CreateSheetOutput> {
    ensure!(
        !input.spreadsheet_id.trim().is_empty(),
        "spreadsheet_id must not be empty"
    );
    ensure!(!input.title.trim().is_empty(), "title must not be empty");

    let client = SheetsClient::from_ctx(&ctx)?;

    // Default to 1000 rows and 26 columns like Google Sheets
    let row_count = input.row_count.unwrap_or(1000);
    let column_count = input.column_count.unwrap_or(26);

    let grid_properties = if row_count != 1000 || column_count != 26 {
        Some(GridProperties {
            row_count,
            column_count,
        })
    } else {
        None
    };

    let request_body = AddSheetRequest {
        requests: vec![types::Request {
            add_sheet: AddSheet {
                properties: SheetProperties {
                    title: input.title.clone(),
                    grid_properties,
                },
            },
        }],
    };

    let response: BatchUpdateResponse = client
        .post_json(
            client.url_with_segments(&["spreadsheets", &input.spreadsheet_id, ":batchUpdate"])?,
            &request_body,
            &[],
        )
        .await?;

    let added_sheet = response
        .replies
        .into_iter()
        .find_map(|r| r.add_sheet)
        .ok_or_else(|| operai::anyhow::anyhow!("No sheet was added in the response"))?;

    Ok(CreateSheetOutput {
        spreadsheet_id: response.spreadsheet_id,
        sheet_id: u64::from(added_sheet.properties.sheet_id),
        title: added_sheet.properties.title,
        index: added_sheet.properties.index,
        row_count,
        column_count,
    })
}

// ============================================================================
// Evaluate Formula Tool
// ============================================================================

/// Input for evaluating a formula.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EvaluateFormulaInput {
    /// The ID of the spreadsheet (for context and named ranges).
    pub spreadsheet_id: String,
    /// The formula to evaluate (e.g., "=SUM(1, 2, 3)" or "=A1+B1").
    pub formula: String,
    /// Optional sheet name for context when evaluating formulas with cell
    /// references.
    #[serde(default)]
    pub sheet_name: Option<String>,
}

/// Output from evaluating a formula.
#[derive(Debug, Serialize, JsonSchema)]
pub struct EvaluateFormulaOutput {
    /// The original formula.
    pub formula: String,
    /// The computed result.
    pub result: CellValue,
    /// Whether the formula was successfully evaluated.
    pub success: bool,
    /// Error message if evaluation failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// # Evaluate Google Sheets Formula
///
/// Evaluates a basic Google Sheets formula and returns the computed result.
/// Use this tool when the user wants to perform simple calculations or test
/// formulas without writing them to a spreadsheet.
///
/// **IMPORTANT**: This is a simplified implementation for demonstration
/// purposes only. It currently supports a very limited subset of formulas:
/// - `=SUM(x, y, z, ...)` - Sums comma-separated numeric values (e.g., `=SUM(1,
///   2, 3)`)
/// - `=x + y` - Basic addition of two numbers (e.g., `=10 + 5`)
/// - `=x * y` - Basic multiplication of two numbers (e.g., `=3 * 4`)
///
/// Cell references (e.g., `=A1+B1`), complex functions, and most Google Sheets
/// formulas are **NOT supported**. This tool is intended for basic arithmetic
/// demonstrations.
///
/// For full formula evaluation, use the Google Sheets API directly by writing
/// formulas to cells and reading the computed results.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - spreadsheets
/// - google-sheets
/// - formulas
///
/// # Errors
///
/// This function currently never returns an error. Formula evaluation failures
/// are reported in the `error` field of the returned `EvaluateFormulaOutput`.
#[tool]
pub async fn evaluate_formula(
    _ctx: Context,
    input: EvaluateFormulaInput,
) -> Result<EvaluateFormulaOutput> {
    // Simple formula evaluation for demonstration
    let formula = input.formula.trim();

    // Handle basic SUM formulas
    if formula.to_uppercase().starts_with("=SUM(") && formula.ends_with(')') {
        let inner = &formula[5..formula.len() - 1];
        let result: Result<f64, _> = inner
            .split(',')
            .map(|s| s.trim().parse::<f64>())
            .try_fold(0.0, |acc, r| r.map(|v| acc + v));

        match result {
            Ok(sum) => {
                return Ok(EvaluateFormulaOutput {
                    formula: input.formula,
                    result: CellValue::Number(sum),
                    success: true,
                    error: None,
                });
            }
            Err(_) => {
                return Ok(EvaluateFormulaOutput {
                    formula: input.formula,
                    result: CellValue::Null,
                    success: false,
                    error: Some("Invalid number in SUM formula".to_string()),
                });
            }
        }
    }

    // Handle basic arithmetic
    if let Some(expr) = formula.strip_prefix('=') {
        // Very basic: try to parse as a simple addition
        if let Some((a, b)) = expr.split_once('+')
            && let (Ok(a), Ok(b)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>())
        {
            return Ok(EvaluateFormulaOutput {
                formula: input.formula,
                result: CellValue::Number(a + b),
                success: true,
                error: None,
            });
        }
        if let Some((a, b)) = expr.split_once('*')
            && let (Ok(a), Ok(b)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>())
        {
            return Ok(EvaluateFormulaOutput {
                formula: input.formula,
                result: CellValue::Number(a * b),
                success: true,
                error: None,
            });
        }
    }

    // Unsupported formula
    Ok(EvaluateFormulaOutput {
        formula: input.formula,
        result: CellValue::Null,
        success: false,
        error: Some("Formula not supported or contains cell references".to_string()),
    })
}

// ============================================================================
// HTTP Client Implementation
// ============================================================================

#[derive(Debug, Clone)]
struct SheetsClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl SheetsClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = GoogleSheetsCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_SHEETS_ENDPOINT))?;

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

        self.handle_response(response).await
    }

    async fn put_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
        query: &[(&str, String)],
    ) -> Result<TRes> {
        let response = self
            .http
            .put(url)
            .query(query)
            .json(body)
            .bearer_auth(&self.access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        self.handle_response(response).await
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
        query: &[(&str, String)],
    ) -> Result<TRes> {
        let response = self
            .http
            .post(url)
            .query(query)
            .json(body)
            .bearer_auth(&self.access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        self.handle_response(response).await
    }

    async fn handle_response<T: for<'de> Deserialize<'de>>(
        &self,
        response: reqwest::Response,
    ) -> Result<T> {
        let status = response.status();
        if status.is_success() {
            Ok(response.json::<T>().await?)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Google Sheets API request failed ({status}): {body}"
            ))
        }
    }
}

fn normalize_base_url(endpoint: &str) -> Result<String> {
    let trimmed = endpoint.trim();
    ensure!(!trimmed.is_empty(), "endpoint must not be empty");
    Ok(trimmed.trim_end_matches('/').to_string())
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Convert a 1-indexed column number to a letter (1 = A, 26 = Z, 27 = AA).
    fn column_letter(col: usize) -> String {
        if col == 0 {
            return "A".to_string();
        }
        let mut result = String::new();
        let mut n = col;
        while n > 0 {
            let remainder = (n - 1) % 26;
            let remainder_u8 = u8::try_from(remainder).expect("remainder should fit within u8");
            result.insert(0, (b'A' + remainder_u8) as char);
            n = (n - 1) / 26;
        }
        result
    }

    // ========================================================================
    // Read Range Tests
    // ========================================================================

    #[tokio::test]
    async fn test_read_range_returns_values_and_counts() {
        let ctx = Context::empty();
        let input = ReadRangeInput {
            spreadsheet_id: "abc123".to_string(),
            range: "Sheet1!A1:B3".to_string(),
            value_render_option: None,
        };

        let result = read_range(ctx, input).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("credential 'google_sheets' not found")
        );
    }

    #[tokio::test]
    async fn test_read_range_with_formatted_value_option() {
        let ctx = Context::empty();
        let input = ReadRangeInput {
            spreadsheet_id: "abc123".to_string(),
            range: "Data!A1:Z100".to_string(),
            value_render_option: Some(ValueRenderOption::FormattedValue),
        };

        let result = read_range(ctx, input).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("credential 'google_sheets' not found")
        );
    }

    #[test]
    fn test_read_range_input_deserializes_minimal() {
        let json = r#"{"spreadsheet_id": "123", "range": "Sheet1!A1:B2"}"#;
        let input: ReadRangeInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.spreadsheet_id, "123");
        assert_eq!(input.range, "Sheet1!A1:B2");
        assert!(input.value_render_option.is_none());
    }

    #[test]
    fn test_read_range_output_serializes_correctly() {
        let output = ReadRangeOutput {
            range: "Sheet1!A1:B2".to_string(),
            values: vec![vec![
                CellValue::String("A".to_string()),
                CellValue::Number(1.0),
            ]],
            row_count: 1,
            column_count: 2,
        };

        let json = serde_json::to_value(output).unwrap();
        assert_eq!(json["range"], "Sheet1!A1:B2");
        assert_eq!(json["row_count"], 1);
        assert_eq!(json["column_count"], 2);
    }

    // ========================================================================
    // Write Range Tests
    // ========================================================================

    #[tokio::test]
    async fn test_write_range_returns_update_counts() {
        let ctx = Context::empty();
        let input = WriteRangeInput {
            spreadsheet_id: "abc123".to_string(),
            range: "Sheet1!A1:C2".to_string(),
            values: vec![
                vec![
                    CellValue::String("A".to_string()),
                    CellValue::String("B".to_string()),
                    CellValue::String("C".to_string()),
                ],
                vec![
                    CellValue::Number(1.0),
                    CellValue::Number(2.0),
                    CellValue::Number(3.0),
                ],
            ],
            value_input_option: None,
        };

        let result = write_range(ctx, input).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("credential 'google_sheets' not found")
        );
    }

    #[tokio::test]
    async fn test_write_range_with_user_entered_option() {
        let ctx = Context::empty();
        let input = WriteRangeInput {
            spreadsheet_id: "xyz789".to_string(),
            range: "Formulas!A1:A1".to_string(),
            values: vec![vec![CellValue::String("=SUM(1,2,3)".to_string())]],
            value_input_option: Some(ValueInputOption::UserEntered),
        };

        let result = write_range(ctx, input).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("credential 'google_sheets' not found")
        );
    }

    #[test]
    fn test_write_range_input_deserializes_with_values() {
        let json = r#"{
            "spreadsheet_id": "123",
            "range": "Sheet1!A1",
            "values": [["hello", 42, true, null]]
        }"#;
        let input: WriteRangeInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.values.len(), 1);
        assert_eq!(input.values[0].len(), 4);
        assert_eq!(input.values[0][0], CellValue::String("hello".to_string()));
        assert_eq!(input.values[0][1], CellValue::Number(42.0));
        assert_eq!(input.values[0][2], CellValue::Boolean(true));
        assert_eq!(input.values[0][3], CellValue::Null);
    }

    #[test]
    fn test_write_range_input_missing_required_field_fails() {
        let json = r#"{"spreadsheet_id": "123", "range": "A1"}"#;
        let err = serde_json::from_str::<WriteRangeInput>(json).unwrap_err();
        assert!(err.to_string().contains("missing field `values`"));
    }

    // ========================================================================
    // Append Row Tests
    // ========================================================================

    #[tokio::test]
    async fn test_append_row_returns_appended_location() {
        let ctx = Context::empty();
        let input = AppendRowInput {
            spreadsheet_id: "abc123".to_string(),
            range: "Sheet1".to_string(),
            values: vec![
                CellValue::String("Name".to_string()),
                CellValue::String("Email".to_string()),
                CellValue::Number(100.0),
            ],
            value_input_option: None,
            insert_data_option: None,
        };

        let result = append_row(ctx, input).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("credential 'google_sheets' not found")
        );
    }

    #[tokio::test]
    async fn test_append_row_with_insert_rows_option() {
        let ctx = Context::empty();
        let input = AppendRowInput {
            spreadsheet_id: "abc123".to_string(),
            range: "Data!A:E".to_string(),
            values: vec![CellValue::Number(1.0)],
            value_input_option: Some(ValueInputOption::Raw),
            insert_data_option: Some(InsertDataOption::InsertRows),
        };

        let result = append_row(ctx, input).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("credential 'google_sheets' not found")
        );
    }

    #[test]
    fn test_append_row_input_deserializes_correctly() {
        let json = r#"{
            "spreadsheet_id": "123",
            "range": "Sheet1",
            "values": ["a", "b", "c"]
        }"#;
        let input: AppendRowInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.spreadsheet_id, "123");
        assert_eq!(input.range, "Sheet1");
        assert_eq!(input.values.len(), 3);
    }

    #[test]
    fn test_column_letter_conversion() {
        assert_eq!(column_letter(1), "A");
        assert_eq!(column_letter(26), "Z");
        assert_eq!(column_letter(27), "AA");
        assert_eq!(column_letter(52), "AZ");
        assert_eq!(column_letter(53), "BA");
    }

    // ========================================================================
    // Create Sheet Tests
    // ========================================================================

    #[tokio::test]
    async fn test_create_sheet_returns_sheet_info() {
        let ctx = Context::empty();
        let input = CreateSheetInput {
            spreadsheet_id: "abc123".to_string(),
            title: "New Sheet".to_string(),
            row_count: None,
            column_count: None,
            index: None,
        };

        let result = create_sheet(ctx, input).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("credential 'google_sheets' not found")
        );
    }

    #[tokio::test]
    async fn test_create_sheet_with_custom_dimensions() {
        let ctx = Context::empty();
        let input = CreateSheetInput {
            spreadsheet_id: "abc123".to_string(),
            title: "Custom Sheet".to_string(),
            row_count: Some(500),
            column_count: Some(10),
            index: Some(2),
        };

        let result = create_sheet(ctx, input).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("credential 'google_sheets' not found")
        );
    }

    #[test]
    fn test_create_sheet_input_deserializes_minimal() {
        let json = r#"{"spreadsheet_id": "123", "title": "Test"}"#;
        let input: CreateSheetInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.spreadsheet_id, "123");
        assert_eq!(input.title, "Test");
        assert!(input.row_count.is_none());
    }

    #[test]
    fn test_create_sheet_output_serializes_correctly() {
        let output = CreateSheetOutput {
            spreadsheet_id: "abc".to_string(),
            sheet_id: 12345,
            title: "Test".to_string(),
            index: 0,
            row_count: 1000,
            column_count: 26,
        };

        let json = serde_json::to_value(output).unwrap();
        assert_eq!(json["sheet_id"], 12345);
        assert_eq!(json["title"], "Test");
    }

    // ========================================================================
    // Evaluate Formula Tests
    // ========================================================================

    #[tokio::test]
    async fn test_evaluate_formula_sum() {
        let ctx = Context::empty();
        let input = EvaluateFormulaInput {
            spreadsheet_id: "abc123".to_string(),
            formula: "=SUM(1, 2, 3)".to_string(),
            sheet_name: None,
        };

        let output = evaluate_formula(ctx, input).await.unwrap();

        assert!(output.success);
        assert_eq!(output.result, CellValue::Number(6.0));
        assert!(output.error.is_none());
    }

    #[tokio::test]
    async fn test_evaluate_formula_addition() {
        let ctx = Context::empty();
        let input = EvaluateFormulaInput {
            spreadsheet_id: "abc123".to_string(),
            formula: "=10 + 5".to_string(),
            sheet_name: None,
        };

        let output = evaluate_formula(ctx, input).await.unwrap();

        assert!(output.success);
        assert_eq!(output.result, CellValue::Number(15.0));
    }

    #[tokio::test]
    async fn test_evaluate_formula_multiplication() {
        let ctx = Context::empty();
        let input = EvaluateFormulaInput {
            spreadsheet_id: "abc123".to_string(),
            formula: "=3 * 4".to_string(),
            sheet_name: None,
        };

        let output = evaluate_formula(ctx, input).await.unwrap();

        assert!(output.success);
        assert_eq!(output.result, CellValue::Number(12.0));
    }

    #[tokio::test]
    async fn test_evaluate_formula_unsupported() {
        let ctx = Context::empty();
        let input = EvaluateFormulaInput {
            spreadsheet_id: "abc123".to_string(),
            formula: "=VLOOKUP(A1, B:C, 2)".to_string(),
            sheet_name: None,
        };

        let output = evaluate_formula(ctx, input).await.unwrap();

        assert!(!output.success);
        assert!(output.error.is_some());
    }

    #[test]
    fn test_evaluate_formula_input_deserializes() {
        let json = r#"{
            "spreadsheet_id": "123",
            "formula": "=SUM(1,2)",
            "sheet_name": "Data"
        }"#;
        let input: EvaluateFormulaInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.formula, "=SUM(1,2)");
        assert_eq!(input.sheet_name, Some("Data".to_string()));
    }

    #[test]
    fn test_evaluate_formula_output_omits_error_when_none() {
        let output = EvaluateFormulaOutput {
            formula: "=1+1".to_string(),
            result: CellValue::Number(2.0),
            success: true,
            error: None,
        };

        let json = serde_json::to_value(output).unwrap();
        assert!(!json.as_object().unwrap().contains_key("error"));
    }

    // ========================================================================
    // Cell Value Tests
    // ========================================================================

    #[test]
    fn test_cell_value_deserializes_all_types() {
        let string_json = r#""hello""#;
        let number_json = "42.5";
        let bool_json = "true";
        let null_json = "null";

        assert_eq!(
            serde_json::from_str::<CellValue>(string_json).unwrap(),
            CellValue::String("hello".to_string())
        );
        assert_eq!(
            serde_json::from_str::<CellValue>(number_json).unwrap(),
            CellValue::Number(42.5)
        );
        assert_eq!(
            serde_json::from_str::<CellValue>(bool_json).unwrap(),
            CellValue::Boolean(true)
        );
        assert_eq!(
            serde_json::from_str::<CellValue>(null_json).unwrap(),
            CellValue::Null
        );
    }

    #[test]
    fn test_cell_value_serializes_correctly() {
        assert_eq!(
            serde_json::to_value(CellValue::String("test".to_string())).unwrap(),
            json!("test")
        );
        assert_eq!(
            serde_json::to_value(CellValue::Number(std::f64::consts::PI)).unwrap(),
            json!(std::f64::consts::PI)
        );
        assert_eq!(
            serde_json::to_value(CellValue::Boolean(false)).unwrap(),
            json!(false)
        );
        assert_eq!(serde_json::to_value(CellValue::Null).unwrap(), json!(null));
    }

    // ========================================================================
    // Credential Tests
    // ========================================================================

    #[test]
    fn test_google_sheets_credential_deserializes_with_required_field() {
        let json = r#"{"access_token": "ya29.xyz"}"#;
        let cred: GoogleSheetsCredential = serde_json::from_str(json).unwrap();

        assert_eq!(cred.access_token, "ya29.xyz");
        assert!(cred.endpoint.is_none());
    }

    #[test]
    fn test_google_sheets_credential_deserializes_with_optional_field() {
        let json = r#"{"access_token": "ya29.xyz", "endpoint": "https://custom.googleapis.com"}"#;
        let cred: GoogleSheetsCredential = serde_json::from_str(json).unwrap();

        assert_eq!(cred.access_token, "ya29.xyz");
        assert_eq!(
            cred.endpoint,
            Some("https://custom.googleapis.com".to_string())
        );
    }

    #[test]
    fn test_google_sheets_credential_missing_access_token_fails() {
        let json = r#"{"endpoint": "https://custom.googleapis.com"}"#;
        let err = serde_json::from_str::<GoogleSheetsCredential>(json).unwrap_err();
        assert!(err.to_string().contains("missing field `access_token`"));
    }

    // ========================================================================
    // Value Option Enum Tests
    // ========================================================================

    #[test]
    fn test_value_render_option_deserializes() {
        let formatted: ValueRenderOption = serde_json::from_str(r#""FORMATTED_VALUE""#).unwrap();
        let unformatted: ValueRenderOption =
            serde_json::from_str(r#""UNFORMATTED_VALUE""#).unwrap();
        let formula: ValueRenderOption = serde_json::from_str(r#""FORMULA""#).unwrap();

        assert!(matches!(formatted, ValueRenderOption::FormattedValue));
        assert!(matches!(unformatted, ValueRenderOption::UnformattedValue));
        assert!(matches!(formula, ValueRenderOption::Formula));
    }

    #[test]
    fn test_value_input_option_deserializes() {
        let raw: ValueInputOption = serde_json::from_str(r#""RAW""#).unwrap();
        let user_entered: ValueInputOption = serde_json::from_str(r#""USER_ENTERED""#).unwrap();

        assert!(matches!(raw, ValueInputOption::Raw));
        assert!(matches!(user_entered, ValueInputOption::UserEntered));
    }

    #[test]
    fn test_insert_data_option_deserializes() {
        let insert: InsertDataOption = serde_json::from_str(r#""INSERT_ROWS""#).unwrap();
        let overwrite: InsertDataOption = serde_json::from_str(r#""OVERWRITE""#).unwrap();

        assert!(matches!(insert, InsertDataOption::InsertRows));
        assert!(matches!(overwrite, InsertDataOption::Overwrite));
    }
}
