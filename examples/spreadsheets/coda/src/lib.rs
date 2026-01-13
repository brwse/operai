//! spreadsheets/coda integration for Operai Toolbox.

mod types;

use operai::{
    Context, JsonSchema, Result, define_system_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
use types::{
    CellValue, CodaComment, CodaDoc, CodaListResponse, CodaRow, CreateCommentRequest, UpsertRow,
    UpsertRowsRequest, UpsertRowsResponse,
};

define_system_credential! {
    CodaCredential("coda") {
        api_key: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_API_ENDPOINT: &str = "https://coda.io/apis/v1";

#[init]
async fn setup() -> Result<()> {
    info!("Coda integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Coda integration shutting down");
}

// ========== List Docs ==========

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListDocsInput {
    /// Maximum number of docs to return (1-100). Defaults to 25.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListDocsOutput {
    pub docs: Vec<DocSummary>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DocSummary {
    pub id: String,
    pub name: String,
    pub browser_link: String,
    #[serde(default)]
    pub owner_name: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// # List Coda Docs
///
/// Lists all Coda documents accessible to the authenticated user, providing
/// essential metadata for each document including ID, name, browser link,
/// owner, and timestamps.
///
/// Use this tool when:
/// - A user wants to see all available Coda documents
/// - You need to find a document ID to perform operations on a specific doc
/// - A user asks to browse, search, or discover their Coda documents
/// - You need to display a list of documents for the user to select from
///
/// The results are paginated and can be controlled using the `limit` parameter
/// (1-100, default 25). Each returned document includes a `browser_link` that
/// can be used to open the document directly in a web browser.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - coda
/// - spreadsheet
/// - docs
#[tool]
/// # Errors
///
/// Returns an error if:
/// - The provided `limit` is not between 1 and 100
/// - Credentials are not configured or are invalid
/// - The API endpoint URL is malformed
/// - The HTTP request to the Coda API fails
/// - The API response cannot be parsed as JSON
pub async fn list_docs(ctx: Context, input: ListDocsInput) -> Result<ListDocsOutput> {
    let limit = input.limit.unwrap_or(25);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let client = CodaClient::from_ctx(&ctx)?;
    let query = [("limit", limit.to_string())];

    let response: CodaListResponse<CodaDoc> = client
        .get_json(client.url_with_path("/docs")?, &query)
        .await?;

    Ok(ListDocsOutput {
        docs: response
            .items
            .into_iter()
            .map(|doc| DocSummary {
                id: doc.id,
                name: doc.name,
                browser_link: doc.browser_link,
                owner_name: doc.owner_name,
                created_at: doc.created_at,
                updated_at: doc.updated_at,
            })
            .collect(),
    })
}

// ========== Query Table ==========

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryTableInput {
    /// Document ID.
    pub doc_id: String,
    /// Table ID or name.
    pub table_id_or_name: String,
    /// Optional query string to filter rows.
    #[serde(default)]
    pub query: Option<String>,
    /// Maximum number of rows to return (1-500). Defaults to 100.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct QueryTableOutput {
    pub rows: Vec<RowData>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RowData {
    pub id: String,
    pub name: String,
    pub index: i32,
    pub values: serde_json::Map<String, serde_json::Value>,
    pub browser_link: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// # Query Coda Table
///
/// Retrieves rows from a specific Coda table within a document, with optional
/// filtering using Coda's query syntax. Returns detailed row data including
/// cell values, metadata, and direct links.
///
/// Use this tool when:
/// - A user wants to read or retrieve data from a Coda table
/// - A user needs to search or filter rows within a table using specific
///   criteria
/// - A user asks to "get rows", "read data", or "query a table" in Coda
/// - You need to display the contents of a table to the user
/// - A user wants to export or analyze table data
///
/// Key inputs:
/// - `doc_id`: The unique identifier of the Coda document (use `list_docs` to
///   find it)
/// - `table_id_or_name`: Can be either the table's ID or its visible name in
///   the document
/// - `query`: Optional filter string using Coda's query syntax to match
///   specific rows
/// - `limit`: Maximum rows to return (1-500, default 100)
///
/// Each returned row contains all cell values as a JSON map, along with
/// metadata like creation/modification times and a browser link for direct
/// access to the row in the web interface.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - coda
/// - spreadsheet
/// - table
/// - query
#[tool]
/// # Errors
///
/// Returns an error if:
/// - `doc_id` is empty or contains only whitespace
/// - `table_id_or_name` is empty or contains only whitespace
/// - The provided `limit` is not between 1 and 500
/// - Credentials are not configured or are invalid
/// - The API endpoint URL is malformed
/// - The HTTP request to the Coda API fails
/// - The API response cannot be parsed as JSON
pub async fn query_table(ctx: Context, input: QueryTableInput) -> Result<QueryTableOutput> {
    ensure!(!input.doc_id.trim().is_empty(), "doc_id must not be empty");
    ensure!(
        !input.table_id_or_name.trim().is_empty(),
        "table_id_or_name must not be empty"
    );

    let limit = input.limit.unwrap_or(100);
    ensure!(
        (1..=500).contains(&limit),
        "limit must be between 1 and 500"
    );

    let client = CodaClient::from_ctx(&ctx)?;
    let path = format!(
        "/docs/{}/tables/{}/rows",
        input.doc_id, input.table_id_or_name
    );

    let mut query = vec![("limit", limit.to_string())];
    if let Some(q) = &input.query
        && !q.trim().is_empty()
    {
        query.push(("query", q.clone()));
    }

    let response: CodaListResponse<CodaRow> = client
        .get_json(client.url_with_path(&path)?, &query)
        .await?;

    Ok(QueryTableOutput {
        rows: response
            .items
            .into_iter()
            .map(|row| RowData {
                id: row.id,
                name: row.name,
                index: row.index,
                values: row.values,
                browser_link: row.browser_link,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
            .collect(),
    })
}

// ========== Upsert Row ==========

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpsertRowInput {
    /// Document ID.
    pub doc_id: String,
    /// Table ID or name.
    pub table_id_or_name: String,
    /// Rows to insert or update.
    pub rows: Vec<RowValues>,
    /// Optional column names to use as unique keys for upserts.
    #[serde(default)]
    pub key_columns: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RowValues {
    /// Map of column names to values.
    pub cells: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UpsertRowOutput {
    pub request_id: String,
    pub added_row_ids: Vec<String>,
}

/// # Upsert Coda Row
///
/// Inserts new rows or updates existing rows in a Coda table based on key
/// column matching. "Upsert" means "update if exists, insert if not found" -
/// this tool handles both operations intelligently.
///
/// Use this tool when:
/// - A user wants to add new data to a Coda table
/// - A user wants to update existing rows in a Coda table
/// - A user needs to synchronize or import data into Coda
/// - A user asks to "add rows", "update rows", "insert data", or "modify cells"
///   in Coda
/// - You need to ensure data is written to a table without creating duplicates
///
/// Key inputs:
/// - `doc_id`: The unique identifier of the Coda document (use `list_docs` to
///   find it)
/// - `table_id_or_name`: Can be either the table's ID or its visible name in
///   the document
/// - `rows`: Array of rows to insert/update, where each row is a map of column
///   names to values
/// - `key_columns`: Optional list of column names used to uniquely identify
///   rows for updates. If provided, existing rows matching these key values
///   will be updated instead of creating new rows. If omitted, all rows are
///   treated as new inserts.
///
/// How upsert logic works:
/// - If `key_columns` is specified: The tool searches for rows where the key
///   column values match. Matching rows are updated; non-matching rows are
///   inserted as new.
/// - If `key_columns` is empty: All rows are inserted as new entries, even if
///   duplicate data exists.
///
/// Returns the request ID and list of newly created row IDs (updated rows are
/// not included in the response).
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - coda
/// - spreadsheet
/// - table
/// - upsert
#[tool]
/// # Errors
///
/// Returns an error if:
/// - `doc_id` is empty or contains only whitespace
/// - `table_id_or_name` is empty or contains only whitespace
/// - `rows` is empty
/// - Credentials are not configured or are invalid
/// - The API endpoint URL is malformed
/// - The HTTP request to the Coda API fails
/// - The API response cannot be parsed as JSON
pub async fn upsert_row(ctx: Context, input: UpsertRowInput) -> Result<UpsertRowOutput> {
    ensure!(!input.doc_id.trim().is_empty(), "doc_id must not be empty");
    ensure!(
        !input.table_id_or_name.trim().is_empty(),
        "table_id_or_name must not be empty"
    );
    ensure!(!input.rows.is_empty(), "rows must not be empty");

    let client = CodaClient::from_ctx(&ctx)?;
    let path = format!(
        "/docs/{}/tables/{}/rows",
        input.doc_id, input.table_id_or_name
    );

    let request = UpsertRowsRequest {
        rows: input
            .rows
            .into_iter()
            .map(|row| UpsertRow {
                cells: row
                    .cells
                    .into_iter()
                    .map(|(col, val)| CellValue {
                        column: col,
                        value: val,
                    })
                    .collect(),
            })
            .collect(),
        key_columns: if input.key_columns.is_empty() {
            None
        } else {
            Some(input.key_columns)
        },
    };

    let response: UpsertRowsResponse = client
        .post_json(client.url_with_path(&path)?, &request)
        .await?;

    Ok(UpsertRowOutput {
        request_id: response.request_id,
        added_row_ids: response.added_row_ids,
    })
}

// ========== Add Comment ==========

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddCommentInput {
    /// Document ID.
    pub doc_id: String,
    /// Row ID to comment on.
    pub row_id: String,
    /// Table ID or name.
    pub table_id_or_name: String,
    /// Comment content.
    pub content: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AddCommentOutput {
    pub comment_id: String,
    pub created_at: String,
}

/// # Add Coda Comment
///
/// Adds a comment to a specific row within a Coda table, enabling collaboration
/// and communication around data entries. Comments are associated with both the
/// table and a specific row.
///
/// Use this tool when:
/// - A user wants to add a comment or note to a specific table row
/// - A user needs to provide feedback or context about a row's data
/// - A user asks to "comment on a row", "add a note", or "leave feedback" in
///   Coda
/// - Collaborative discussion is needed around specific data entries
/// - A user wants to annotate or document issues with a particular row
///
/// Key inputs:
/// - `doc_id`: The unique identifier of the Coda document (use `list_docs` to
///   find it)
/// - `table_id_or_name`: Can be either the table's ID or its visible name in
///   the document
/// - `row_id`: The unique identifier of the specific row to comment on
///   (obtained from `query_table`)
/// - `content`: The text content of the comment to be added
///
/// The comment will be visible to users with access to the document in the Coda
/// web interface and will include the timestamp of creation. Returns the
/// comment ID and creation timestamp.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - coda
/// - spreadsheet
/// - comment
#[tool]
/// # Errors
///
/// Returns an error if:
/// - `doc_id` is empty or contains only whitespace
/// - `table_id_or_name` is empty or contains only whitespace
/// - `row_id` is empty or contains only whitespace
/// - `content` is empty or contains only whitespace
/// - Credentials are not configured or are invalid
/// - The API endpoint URL is malformed
/// - The HTTP request to the Coda API fails
/// - The API response cannot be parsed as JSON
pub async fn add_comment(ctx: Context, input: AddCommentInput) -> Result<AddCommentOutput> {
    ensure!(!input.doc_id.trim().is_empty(), "doc_id must not be empty");
    ensure!(
        !input.table_id_or_name.trim().is_empty(),
        "table_id_or_name must not be empty"
    );
    ensure!(!input.row_id.trim().is_empty(), "row_id must not be empty");
    ensure!(
        !input.content.trim().is_empty(),
        "content must not be empty"
    );

    let client = CodaClient::from_ctx(&ctx)?;
    let path = format!(
        "/docs/{}/tables/{}/rows/{}/comments",
        input.doc_id, input.table_id_or_name, input.row_id
    );

    let request = CreateCommentRequest {
        content: input.content,
    };

    let response: CodaComment = client
        .post_json(client.url_with_path(&path)?, &request)
        .await?;

    Ok(AddCommentOutput {
        comment_id: response.id,
        created_at: response.created_at,
    })
}

// ========== HTTP Client ==========

#[derive(Debug, Clone)]
struct CodaClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl CodaClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = CodaCredential::get(ctx)?;
        ensure!(!cred.api_key.trim().is_empty(), "api_key must not be empty");

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_API_ENDPOINT))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            api_key: cred.api_key,
        })
    }

    fn url_with_path(&self, path: &str) -> Result<reqwest::Url> {
        let url_str = format!("{}{}", self.base_url, path);
        Ok(reqwest::Url::parse(&url_str)?)
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
            .bearer_auth(&self.api_key)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.json::<T>().await?)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Coda API request failed ({status}): {body}"
            ))
        }
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
    ) -> Result<TRes> {
        let response = self
            .http
            .post(url)
            .json(body)
            .bearer_auth(&self.api_key)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.json::<TRes>().await?)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Coda API request failed ({status}): {body}"
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
    use std::collections::HashMap;

    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_string_contains, header, method, path, query_param},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut coda_values = HashMap::new();
        coda_values.insert("api_key".to_string(), "test-api-key".to_string());
        coda_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_system_credential("coda", coda_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        server.uri()
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_doc_summary_serialization_roundtrip() {
        let doc = DocSummary {
            id: "doc-123".to_string(),
            name: "Test Doc".to_string(),
            browser_link: "https://coda.io/d/doc-123".to_string(),
            owner_name: Some("Alice".to_string()),
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: Some("2024-01-02T00:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&doc).unwrap();
        let parsed: DocSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(doc.id, parsed.id);
        assert_eq!(doc.name, parsed.name);
    }

    #[test]
    fn test_row_data_serialization_roundtrip() {
        let mut values = serde_json::Map::new();
        values.insert(
            "col1".to_string(),
            serde_json::Value::String("val1".to_string()),
        );

        let row = RowData {
            id: "row-123".to_string(),
            name: "Row 1".to_string(),
            index: 0,
            values,
            browser_link: "https://coda.io/d/doc/table/row".to_string(),
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            updated_at: Some("2024-01-02T00:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&row).unwrap();
        let parsed: RowData = serde_json::from_str(&json).unwrap();
        assert_eq!(row.id, parsed.id);
        assert_eq!(row.name, parsed.name);
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://coda.io/apis/v1/").unwrap();
        assert_eq!(result, "https://coda.io/apis/v1");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://coda.io/apis/v1  ").unwrap();
        assert_eq!(result, "https://coda.io/apis/v1");
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
    async fn test_list_docs_limit_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_docs(ctx, ListDocsInput { limit: Some(0) }).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("limit must be between 1 and 100")
        );
    }

    #[tokio::test]
    async fn test_list_docs_limit_exceeds_max_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_docs(ctx, ListDocsInput { limit: Some(101) }).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("limit must be between 1 and 100")
        );
    }

    #[tokio::test]
    async fn test_query_table_empty_doc_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = query_table(
            ctx,
            QueryTableInput {
                doc_id: "  ".to_string(),
                table_id_or_name: "table-1".to_string(),
                query: None,
                limit: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("doc_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_query_table_empty_table_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = query_table(
            ctx,
            QueryTableInput {
                doc_id: "doc-1".to_string(),
                table_id_or_name: "  ".to_string(),
                query: None,
                limit: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("table_id_or_name must not be empty")
        );
    }

    #[tokio::test]
    async fn test_upsert_row_empty_doc_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let mut cells = serde_json::Map::new();
        cells.insert(
            "col1".to_string(),
            serde_json::Value::String("val1".to_string()),
        );

        let result = upsert_row(
            ctx,
            UpsertRowInput {
                doc_id: "  ".to_string(),
                table_id_or_name: "table-1".to_string(),
                rows: vec![RowValues { cells }],
                key_columns: vec![],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("doc_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_upsert_row_empty_rows_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = upsert_row(
            ctx,
            UpsertRowInput {
                doc_id: "doc-1".to_string(),
                table_id_or_name: "table-1".to_string(),
                rows: vec![],
                key_columns: vec![],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("rows must not be empty")
        );
    }

    #[tokio::test]
    async fn test_add_comment_empty_content_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = add_comment(
            ctx,
            AddCommentInput {
                doc_id: "doc-1".to_string(),
                table_id_or_name: "table-1".to_string(),
                row_id: "row-1".to_string(),
                content: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("content must not be empty")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_list_docs_success_returns_docs() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "items": [
            {
              "id": "doc-1",
              "type": "doc",
              "href": "https://coda.io/apis/v1/docs/doc-1",
              "browserLink": "https://coda.io/d/doc-1",
              "name": "My Doc",
              "ownerName": "Alice",
              "createdAt": "2024-01-01T00:00:00Z",
              "updatedAt": "2024-01-02T00:00:00Z"
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/docs"))
            .and(header("authorization", "Bearer test-api-key"))
            .and(query_param("limit", "25"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_docs(ctx, ListDocsInput { limit: None }).await.unwrap();

        assert_eq!(output.docs.len(), 1);
        assert_eq!(output.docs[0].id, "doc-1");
        assert_eq!(output.docs[0].name, "My Doc");
    }

    #[tokio::test]
    async fn test_query_table_success_returns_rows() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "items": [
            {
              "id": "row-1",
              "type": "row",
              "href": "https://coda.io/apis/v1/docs/doc-1/tables/table-1/rows/row-1",
              "name": "Row 1",
              "index": 0,
              "browserLink": "https://coda.io/d/doc-1#table-1/row-1",
              "createdAt": "2024-01-01T00:00:00Z",
              "updatedAt": "2024-01-02T00:00:00Z",
              "values": { "col1": "val1" }
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/docs/doc-1/tables/table-1/rows"))
            .and(query_param("limit", "100"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = query_table(
            ctx,
            QueryTableInput {
                doc_id: "doc-1".to_string(),
                table_id_or_name: "table-1".to_string(),
                query: None,
                limit: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.rows.len(), 1);
        assert_eq!(output.rows[0].id, "row-1");
        assert_eq!(output.rows[0].name, "Row 1");
    }

    #[tokio::test]
    async fn test_upsert_row_success_returns_request_id() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "requestId": "req-123",
          "addedRowIds": ["row-1"]
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/docs/doc-1/tables/table-1/rows"))
            .and(body_string_contains("\"cells\":["))
            .respond_with(
                ResponseTemplate::new(202).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);

        let mut cells = serde_json::Map::new();
        cells.insert(
            "col1".to_string(),
            serde_json::Value::String("val1".to_string()),
        );

        let output = upsert_row(
            ctx,
            UpsertRowInput {
                doc_id: "doc-1".to_string(),
                table_id_or_name: "table-1".to_string(),
                rows: vec![RowValues { cells }],
                key_columns: vec![],
            },
        )
        .await
        .unwrap();

        assert_eq!(output.request_id, "req-123");
        assert_eq!(output.added_row_ids, vec!["row-1"]);
    }

    #[tokio::test]
    async fn test_add_comment_success_returns_comment_id() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "comment-1",
          "type": "comment",
          "href": "https://coda.io/apis/v1/docs/doc-1/comments/comment-1",
          "createdAt": "2024-01-01T00:00:00Z",
          "modifiedAt": "2024-01-01T00:00:00Z",
          "content": "Great work!"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/docs/doc-1/tables/table-1/rows/row-1/comments"))
            .and(body_string_contains("\"content\":\"Great work!\""))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = add_comment(
            ctx,
            AddCommentInput {
                doc_id: "doc-1".to_string(),
                table_id_or_name: "table-1".to_string(),
                row_id: "row-1".to_string(),
                content: "Great work!".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.comment_id, "comment-1");
        assert_eq!(output.created_at, "2024-01-01T00:00:00Z");
    }

    #[tokio::test]
    async fn test_list_docs_error_response_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/docs"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_raw(r#"{ "error": "Unauthorized" }"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = list_docs(ctx, ListDocsInput { limit: None }).await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("401"));
    }
}
