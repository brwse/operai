//! docs-notes/notion integration for Operai Toolbox.

mod types;

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
use types::{DatabaseSummary, ObjectType, PageDetail, PageSummary, Parent, RichTextContent};

define_user_credential! {
    NotionCredential("notion") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_NOTION_API_ENDPOINT: &str = "https://api.notion.com/v1";
const NOTION_VERSION: &str = "2022-06-28"; // Notion API version (stable version)

#[init]
async fn setup() -> Result<()> {
    info!("Notion integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Notion integration shutting down");
}

// ============================================================================
// Search pages/db
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchInput {
    /// Search query string to filter by title.
    pub query: String,
    /// Filter by object type (page or database).
    #[serde(default)]
    pub filter: Option<ObjectType>,
    /// Maximum number of results (1-100). Defaults to 10.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchOutput {
    pub results: Vec<SearchResult>,
    pub has_more: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum SearchResult {
    Page(PageSummary),
    Database(DatabaseSummary),
}

/// # Search Notion Pages and Databases
///
/// Searches for pages and databases in Notion using a text query.
/// Use this tool when the user wants to find specific content, pages, or
/// databases in their Notion workspace.
///
/// The search functionality:
/// - Searches across all pages and databases shared with the integration
/// - Filters results by object type (page, database, or both)
/// - Returns paginated results with metadata (ID, title, timestamps)
/// - Supports customizable result limits (1-100, default: 10)
///
/// Key inputs:
/// - `query`: The search string to match against page/database titles
/// - `filter`: Optional filter to search only pages or only databases
/// - `limit`: Maximum number of results to return (1-100)
///
/// Returns a list of matching pages and databases with their IDs, titles, and
/// metadata. Use the returned IDs with other tools like `get_page` or
/// `update_properties`.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - docs
/// - notion
/// - search
///
/// # Errors
///
/// Returns an error if:
/// - The query string is empty or contains only whitespace
/// - The limit is not between 1 and 100
/// - The Notion credentials are missing or invalid
/// - The API request fails due to network or authentication issues
#[tool]
pub async fn search_pages_db(ctx: Context, input: SearchInput) -> Result<SearchOutput> {
    ensure!(!input.query.trim().is_empty(), "query must not be empty");
    let limit = input.limit.unwrap_or(10);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let client = NotionClient::from_ctx(&ctx)?;

    let mut body = serde_json::json!({
        "query": input.query,
        "page_size": limit,
    });

    if let Some(filter) = input.filter {
        body["filter"] = serde_json::json!({
            "value": match filter {
                ObjectType::Page => "page",
                ObjectType::Database => "database",
            },
            "property": "object",
        });
    }

    let response: NotionSearchResponse = client.post_json("/search", &body).await?;

    let results = response
        .results
        .into_iter()
        .map(|result| {
            if result.object == "database" {
                SearchResult::Database(DatabaseSummary {
                    object: result.object,
                    id: result.id,
                    created_time: result.created_time,
                    last_edited_time: result.last_edited_time,
                    archived: result.archived,
                    title: result.title.unwrap_or_default(),
                })
            } else {
                SearchResult::Page(PageSummary {
                    object: result.object,
                    id: result.id,
                    created_time: result.created_time,
                    last_edited_time: result.last_edited_time,
                    archived: result.archived,
                    parent: result.parent,
                })
            }
        })
        .collect();

    Ok(SearchOutput {
        results,
        has_more: response.has_more,
    })
}

// ============================================================================
// Get page
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetPageInput {
    /// Notion page ID.
    pub page_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetPageOutput {
    pub page: PageDetail,
}

/// # Get Notion Page Details
///
/// Retrieves detailed information about a specific Notion page, including all
/// its properties and metadata. Use this tool when the user wants to view the
/// full details of a page, such as after finding it via search.
///
/// The retrieved page includes:
/// - Page metadata (ID, creation time, last edited time, archived status)
/// - Parent information (workspace, page, or database)
/// - All page properties and their current values
///
/// Prerequisites:
/// - You need a valid `page_id`, typically obtained from the `search_pages_db`
///   tool
/// - The page must be shared with the integration
///
/// Common use cases:
/// - Reading page content after finding it via search
/// - Getting current property values before updating them
/// - Verifying a page exists and is accessible
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - docs
/// - notion
/// - page
///
/// # Errors
///
/// Returns an error if:
/// - The `page_id` is empty or contains only whitespace
/// - The Notion credentials are missing or invalid
/// - The API request fails due to network or authentication issues
/// - The page with the given ID does not exist or is not accessible
#[tool]
pub async fn get_page(ctx: Context, input: GetPageInput) -> Result<GetPageOutput> {
    ensure!(
        !input.page_id.trim().is_empty(),
        "page_id must not be empty"
    );

    let client = NotionClient::from_ctx(&ctx)?;
    let page: PageDetail = client
        .get_json(&format!("/pages/{}", input.page_id))
        .await?;

    Ok(GetPageOutput { page })
}

// ============================================================================
// Create page
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreatePageInput {
    /// Parent page or database ID.
    pub parent_id: String,
    /// Whether parent is a database (true) or page (false).
    #[serde(default)]
    pub parent_is_database: bool,
    /// Page title.
    pub title: String,
    /// Optional properties (JSON object for database pages).
    #[serde(default)]
    pub properties: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreatePageOutput {
    pub page_id: String,
    pub url: String,
}

/// # Create Notion Page
///
/// Creates a new page in Notion, either as a child of an existing page or in a
/// database. Use this tool when the user wants to create a new page in their
/// Notion workspace.
///
/// The page can be created in two ways:
/// 1. **As a child of a page**: Set `parent_is_database=false` and provide the
///    parent page ID
/// 2. **In a database**: Set `parent_is_database=true` and provide the database
///    ID
///
/// Key inputs:
/// - `parent_id`: ID of the parent page or database
/// - `parent_is_database`: Set to `true` if parent is a database, `false` if
///   it's a page
/// - `title`: Title for the new page (required)
/// - `properties`: Optional JSON object with additional properties for database
///   pages
///
/// For database pages, you can optionally provide custom properties that match
/// the database schema. The properties parameter should be a JSON object with
/// property names and values following Notion's property structure.
///
/// Returns the newly created page ID and URL, which can be used for further
/// operations.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - docs
/// - notion
/// - page
///
/// # Errors
///
/// Returns an error if:
/// - The `parent_id` is empty or contains only whitespace
/// - The title is empty or contains only whitespace
/// - The Notion credentials are missing or invalid
/// - The API request fails due to network or authentication issues
/// - The parent page or database does not exist or is not accessible
#[tool]
pub async fn create_page(ctx: Context, input: CreatePageInput) -> Result<CreatePageOutput> {
    ensure!(
        !input.parent_id.trim().is_empty(),
        "parent_id must not be empty"
    );
    ensure!(!input.title.trim().is_empty(), "title must not be empty");

    let client = NotionClient::from_ctx(&ctx)?;

    let parent = if input.parent_is_database {
        serde_json::json!({
            "database_id": input.parent_id,
        })
    } else {
        serde_json::json!({
            "page_id": input.parent_id,
        })
    };

    let mut body = serde_json::json!({
        "parent": parent,
        "properties": {
            "title": {
                "title": [{
                    "text": {
                        "content": input.title,
                    }
                }]
            }
        }
    });

    if let Some(props) = input.properties
        && let Some(obj) = body.get_mut("properties")
        && let Some(obj_map) = obj.as_object_mut()
        && let Some(props_map) = props.as_object()
    {
        for (key, value) in props_map {
            obj_map.insert(key.clone(), value.clone());
        }
    }

    let response: NotionPageResponse = client.post_json("/pages", &body).await?;

    Ok(CreatePageOutput {
        page_id: response.id,
        url: response.url,
    })
}

// ============================================================================
// Update properties
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdatePropertiesInput {
    /// Notion page ID.
    pub page_id: String,
    /// Properties to update (JSON object).
    pub properties: serde_json::Value,
    /// Whether to archive the page.
    #[serde(default)]
    pub archived: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdatePropertiesOutput {
    pub updated: bool,
}

/// # Update Notion Page Properties
///
/// Updates properties on an existing Notion page, such as status, tags, dates,
/// select fields, and other metadata. Use this tool when the user wants to
/// modify page properties without changing the page content/blocks.
///
/// This tool modifies page metadata and properties, NOT the page content
/// (text/blocks). To add content to a page, use the `append_blocks` tool
/// instead.
///
/// Key inputs:
/// - `page_id`: ID of the page to update (obtained from search or creation)
/// - `properties`: JSON object with property names and values following
///   Notion's property structure
/// - `archived`: Optional flag to archive/unarchive the page
///
/// Common properties to update:
/// - Status: `{"Status": {"select": {"name": "In Progress"}}}`
/// - Select: `{"Priority": {"select": {"name": "High"}}}`
/// - Multi-select: `{"Tags": {"multi_select": [{"name": "urgent"}]}}`
/// - Date: `{"Due Date": {"date": {"start": "2024-01-15"}}}`
/// - Checkbox: `{"Done": {"checkbox": true}}`
///
/// The properties JSON must match the page's property schema. Invalid
/// properties will cause an error.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - docs
/// - notion
/// - page
///
/// # Errors
///
/// Returns an error if:
/// - The `page_id` is empty or contains only whitespace
/// - The Notion credentials are missing or invalid
/// - The API request fails due to network or authentication issues
/// - The page with the given ID does not exist or is not accessible
/// - The properties JSON is invalid or does not match the page's schema
#[tool]
pub async fn update_properties(
    ctx: Context,
    input: UpdatePropertiesInput,
) -> Result<UpdatePropertiesOutput> {
    ensure!(
        !input.page_id.trim().is_empty(),
        "page_id must not be empty"
    );

    let client = NotionClient::from_ctx(&ctx)?;

    let mut body = serde_json::json!({
        "properties": input.properties,
    });

    if let Some(archived) = input.archived {
        body["archived"] = serde_json::json!(archived);
    }

    client
        .patch_empty(&format!("/pages/{}", input.page_id), &body)
        .await?;

    Ok(UpdatePropertiesOutput { updated: true })
}

// ============================================================================
// Append blocks
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AppendBlocksInput {
    /// Block ID (page or block) to append children to.
    pub block_id: String,
    /// Array of block objects to append.
    pub children: Vec<BlockInput>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BlockInput {
    /// Block type (paragraph, `heading_1`, etc).
    pub block_type: String,
    /// Text content for the block.
    pub content: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AppendBlocksOutput {
    pub appended: bool,
    pub block_count: usize,
}

/// # Append Notion Content Blocks
///
/// Appends content blocks (paragraphs, headings, lists, etc.) to a Notion page
/// or existing block. Use this tool when the user wants to add content to a
/// page, such as writing text, adding headings, or creating structured content.
///
/// This tool adds content to a page, not properties. To update page properties
/// like status or tags, use `update_properties` instead.
///
/// Supported block types include:
/// - `paragraph`: Regular text paragraphs
/// - `heading_1`, `heading_2`, `heading_3`: Headings of different levels
/// - `bulleted_list_item`, `numbered_list_item`: List items
/// - `to_do`: Checkbox items
/// - `quote`: Block quotes
/// - `code`: Code blocks
/// - `divider`: Horizontal rules
///
/// Key inputs:
/// - `block_id`: ID of the page or block to append content to (typically a page
///   ID)
/// - `children`: Array of block objects, each with `block_type` and `content`
///
/// Example usage:
/// ```json
/// {
///   "block_id": "page-123",
///   "children": [
///     {"block_type": "heading_1", "content": "Introduction"},
///     {"block_type": "paragraph", "content": "This is the content."},
///     {"block_type": "to_do", "content": "Remember to follow up"}
///   ]
/// }
/// ```
///
/// The blocks are appended to the end of the existing content. Returns the
/// count of blocks added.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - docs
/// - notion
/// - blocks
///
/// # Errors
///
/// Returns an error if:
/// - The `block_id` is empty or contains only whitespace
/// - The children array is empty
/// - The Notion credentials are missing or invalid
/// - The API request fails due to network or authentication issues
/// - The parent block or page does not exist or is not accessible
#[tool]
pub async fn append_blocks(ctx: Context, input: AppendBlocksInput) -> Result<AppendBlocksOutput> {
    ensure!(
        !input.block_id.trim().is_empty(),
        "block_id must not be empty"
    );
    ensure!(!input.children.is_empty(), "children must not be empty");

    let client = NotionClient::from_ctx(&ctx)?;

    let children: Vec<serde_json::Value> = input
        .children
        .iter()
        .map(|block| {
            let mut block_obj = serde_json::json!({
                "object": "block",
                "type": block.block_type,
            });
            let type_content = serde_json::json!({
                "rich_text": [{
                    "type": "text",
                    "text": {
                        "content": block.content,
                    }
                }]
            });
            block_obj[block.block_type.as_str()] = type_content;
            block_obj
        })
        .collect();

    let body = serde_json::json!({
        "children": children,
    });

    let block_count = children.len();

    client
        .patch_empty(&format!("/blocks/{}/children", input.block_id), &body)
        .await?;

    Ok(AppendBlocksOutput {
        appended: true,
        block_count,
    })
}

// ============================================================================
// Internal HTTP client
// ============================================================================

#[derive(Debug, Deserialize)]
struct NotionSearchResponse {
    results: Vec<NotionSearchResultRaw>,
    has_more: bool,
}

#[derive(Debug, Deserialize)]
struct NotionSearchResultRaw {
    object: String,
    id: String,
    created_time: String,
    last_edited_time: String,
    #[serde(default)]
    archived: bool,
    parent: Parent,
    #[serde(default)]
    title: Option<Vec<RichTextContent>>,
}

#[derive(Debug, Deserialize)]
struct NotionPageResponse {
    id: String,
    url: String,
}

#[derive(Debug, Clone)]
struct NotionClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl NotionClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = NotionCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url = normalize_base_url(
            cred.endpoint
                .as_deref()
                .unwrap_or(DEFAULT_NOTION_API_ENDPOINT),
        )?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            access_token: cred.access_token,
        })
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self.send_request(self.http.get(&url)).await?;
        Ok(response.json::<T>().await?)
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &TReq,
    ) -> Result<TRes> {
        let url = format!("{}{}", self.base_url, path);
        let response = self.send_request(self.http.post(&url).json(body)).await?;
        Ok(response.json::<TRes>().await?)
    }

    async fn patch_empty<TReq: Serialize>(&self, path: &str, body: &TReq) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        self.send_request(self.http.patch(&url).json(body)).await?;
        Ok(())
    }

    async fn send_request(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let response = request
            .bearer_auth(&self.access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header("Notion-Version", NOTION_VERSION)
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Notion API request failed ({status}): {body}"
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
        matchers::{header, method, path},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut notion_values = HashMap::new();
        notion_values.insert("access_token".to_string(), "test-token".to_string());
        notion_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("notion", notion_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        format!("{}/v1", server.uri())
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_object_type_serialization_roundtrip() {
        for variant in [ObjectType::Page, ObjectType::Database] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: ObjectType = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://api.notion.com/v1/").unwrap();
        assert_eq!(result, "https://api.notion.com/v1");
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
    async fn test_search_empty_query_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = search_pages_db(
            ctx,
            SearchInput {
                query: "   ".to_string(),
                filter: None,
                limit: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("query must not be empty")
        );
    }

    #[tokio::test]
    async fn test_search_limit_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = search_pages_db(
            ctx,
            SearchInput {
                query: "test".to_string(),
                filter: None,
                limit: Some(0),
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
    async fn test_get_page_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = get_page(
            ctx,
            GetPageInput {
                page_id: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("page_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_create_page_empty_parent_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = create_page(
            ctx,
            CreatePageInput {
                parent_id: "  ".to_string(),
                parent_is_database: false,
                title: "Test".to_string(),
                properties: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("parent_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_create_page_empty_title_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = create_page(
            ctx,
            CreatePageInput {
                parent_id: "page-123".to_string(),
                parent_is_database: false,
                title: "  ".to_string(),
                properties: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("title must not be empty")
        );
    }

    #[tokio::test]
    async fn test_append_blocks_empty_block_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = append_blocks(
            ctx,
            AppendBlocksInput {
                block_id: "  ".to_string(),
                children: vec![BlockInput {
                    block_type: "paragraph".to_string(),
                    content: "Test".to_string(),
                }],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("block_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_append_blocks_empty_children_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = append_blocks(
            ctx,
            AppendBlocksInput {
                block_id: "page-123".to_string(),
                children: vec![],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("children must not be empty")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_search_pages_success_returns_results() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "results": [
            {
              "object": "page",
              "id": "page-1",
              "created_time": "2024-01-01T00:00:00.000Z",
              "last_edited_time": "2024-01-02T00:00:00.000Z",
              "archived": false,
              "parent": {
                "type": "workspace",
                "workspace": true
              }
            }
          ],
          "has_more": false,
          "next_cursor": null
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/v1/search"))
            .and(header("authorization", "Bearer test-token"))
            .and(header("notion-version", NOTION_VERSION))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = search_pages_db(
            ctx,
            SearchInput {
                query: "test".to_string(),
                filter: None,
                limit: Some(10),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.results.len(), 1);
        assert!(!output.has_more);
    }

    #[tokio::test]
    async fn test_get_page_success_returns_page() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "object": "page",
          "id": "page-123",
          "created_time": "2024-01-01T00:00:00.000Z",
          "last_edited_time": "2024-01-02T00:00:00.000Z",
          "archived": false,
          "parent": {
            "type": "workspace",
            "workspace": true
          },
          "properties": {}
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1/pages/page-123"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = get_page(
            ctx,
            GetPageInput {
                page_id: "page-123".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.page.id, "page-123");
    }

    #[tokio::test]
    async fn test_create_page_success_returns_page_id() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "new-page-123",
          "url": "https://notion.so/new-page-123"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/v1/pages"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = create_page(
            ctx,
            CreatePageInput {
                parent_id: "parent-123".to_string(),
                parent_is_database: false,
                title: "New Page".to_string(),
                properties: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.page_id, "new-page-123");
        assert_eq!(output.url, "https://notion.so/new-page-123");
    }

    #[tokio::test]
    async fn test_update_properties_success_returns_updated() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("PATCH"))
            .and(path("/v1/pages/page-123"))
            .respond_with(ResponseTemplate::new(200).set_body_raw("{}", "application/json"))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = update_properties(
            ctx,
            UpdatePropertiesInput {
                page_id: "page-123".to_string(),
                properties: serde_json::json!({}),
                archived: None,
            },
        )
        .await
        .unwrap();

        assert!(output.updated);
    }

    #[tokio::test]
    async fn test_append_blocks_success_returns_appended() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("PATCH"))
            .and(path("/v1/blocks/page-123/children"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(r#"{"results": []}"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = append_blocks(
            ctx,
            AppendBlocksInput {
                block_id: "page-123".to_string(),
                children: vec![BlockInput {
                    block_type: "paragraph".to_string(),
                    content: "Hello world".to_string(),
                }],
            },
        )
        .await
        .unwrap();

        assert!(output.appended);
        assert_eq!(output.block_count, 1);
    }
}
