//! docs-notes/onenote integration for Operai Toolbox.

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

mod types;
pub use types::*;

define_user_credential! {
    OneNoteCredential("onenote") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_GRAPH_ENDPOINT: &str = "https://graph.microsoft.com/v1.0";

#[init]
async fn setup() -> Result<()> {
    info!("OneNote integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("OneNote integration shutting down");
}

// ============================================================================
// Tool: list_notebooks
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListNotebooksInput {
    /// Maximum number of notebooks to return (1-100). Defaults to 20.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListNotebooksOutput {
    pub notebooks: Vec<Notebook>,
}

/// # List OneNote Notebooks
///
/// Lists all OneNote notebooks for the authenticated user via the Microsoft
/// Graph API. Use this tool when the user wants to browse their notebook
/// collection or find a specific notebook by name or ID. Returns metadata
/// including display name, creation/modification timestamps, and whether the
/// notebook is the default or shared.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - docs
/// - onenote
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The limit parameter is not between 1 and 100
/// - No valid OneNote credentials are configured in the context
/// - The configured endpoint URL is invalid
/// - The Microsoft Graph API request fails
#[tool]
pub async fn list_notebooks(
    ctx: Context,
    input: ListNotebooksInput,
) -> Result<ListNotebooksOutput> {
    let limit = input.limit.unwrap_or(20);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let query = [("$top", limit.to_string())];

    let response: GraphListResponse<Notebook> = client
        .get_json(
            client.url_with_segments(&["me", "onenote", "notebooks"])?,
            &query,
            &[],
        )
        .await?;

    Ok(ListNotebooksOutput {
        notebooks: response.value,
    })
}

// ============================================================================
// Tool: get_page
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetPageInput {
    /// OneNote page ID.
    pub page_id: String,
    /// When true, include the full HTML content of the page.
    #[serde(default)]
    pub include_content: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetPageOutput {
    pub page: Page,
}

/// # Get OneNote Page
///
/// Retrieves a specific OneNote page by ID via the Microsoft Graph API,
/// optionally including its full HTML content. Use this tool when the user
/// wants to read the content of a specific page or retrieve page metadata
/// (title, creation/modification timestamps, content URL, level). The page must
/// be retrieved by ID; use the search tool first if the user only knows
/// keywords or a title.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - docs
/// - onenote
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The `page_id` is empty or contains only whitespace
/// - No valid OneNote credentials are configured in the context
/// - The configured endpoint URL is invalid
/// - The Microsoft Graph API request fails to retrieve the page or its content
#[tool]
pub async fn get_page(ctx: Context, input: GetPageInput) -> Result<GetPageOutput> {
    ensure!(
        !input.page_id.trim().is_empty(),
        "page_id must not be empty"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let select = "id,title,createdDateTime,lastModifiedDateTime,contentUrl,level";

    let query = [("$select", select.to_string())];

    let mut page: Page = client
        .get_json(
            client.url_with_segments(&["me", "onenote", "pages", input.page_id.as_str()])?,
            &query,
            &[],
        )
        .await?;

    // If content is requested, fetch it separately (it's returned as HTML)
    if input.include_content {
        let content = client
            .get_html(client.url_with_segments(&[
                "me",
                "onenote",
                "pages",
                input.page_id.as_str(),
                "content",
            ])?)
            .await?;
        page.content = Some(content);
    }

    Ok(GetPageOutput { page })
}

// ============================================================================
// Tool: create_page
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreatePageInput {
    /// Title of the new page.
    pub title: String,
    /// HTML content for the page body.
    pub content: String,
    /// Optional section ID where the page should be created.
    /// If omitted, the page is created in the default notebook's default
    /// section.
    #[serde(default)]
    pub section_id: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreatePageOutput {
    pub page: Page,
}

/// # Create OneNote Page
///
/// Creates a new OneNote page with a title and HTML content via the Microsoft
/// Graph API. Use this tool when the user wants to add a new page to a
/// OneNote notebook or section. The page can be created in a specific section
/// (if `section_id` is provided) or in the default notebook's default section
/// (if `section_id` is omitted). Content must be provided as valid HTML; the
/// title is automatically escaped and embedded in the HTML structure.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - docs
/// - onenote
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The title is empty or contains only whitespace
/// - The content is empty or contains only whitespace
/// - The ``section_id`` is provided but empty or contains only whitespace
/// - No valid OneNote credentials are configured in the context
/// - The configured endpoint URL is invalid
/// - The Microsoft Graph API request fails to create the page
#[tool]
pub async fn create_page(ctx: Context, input: CreatePageInput) -> Result<CreatePageOutput> {
    ensure!(!input.title.trim().is_empty(), "title must not be empty");
    ensure!(
        !input.content.trim().is_empty(),
        "content must not be empty"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    // Build the HTML body for the page
    let html_body = format!(
        r"<!DOCTYPE html>
<html>
  <head>
    <title>{}</title>
  </head>
  <body>
    {}
  </body>
</html>",
        html_escape(&input.title),
        input.content
    );

    let url = if let Some(section_id) = &input.section_id {
        ensure!(
            !section_id.trim().is_empty(),
            "section_id must not be empty"
        );
        client.url_with_segments(&["me", "onenote", "sections", section_id.as_str(), "pages"])?
    } else {
        client.url_with_segments(&["me", "onenote", "pages"])?
    };

    let page: Page = client.post_html(url, &html_body, &[]).await?;

    Ok(CreatePageOutput { page })
}

// ============================================================================
// Tool: append_content
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AppendContentInput {
    /// OneNote page ID to update.
    pub page_id: String,
    /// HTML content to append to the page.
    pub content: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AppendContentOutput {
    pub updated: bool,
}

/// # Append Content to OneNote Page
///
/// Appends HTML content to an existing OneNote page via the Microsoft Graph
/// API. Use this tool when the user wants to add new content to the bottom of
/// an existing page without replacing the existing content. The content is
/// appended to the page body and must be provided as valid HTML. This operation
/// preserves all existing content on the page.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - docs
/// - onenote
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The `page_id` is empty or contains only whitespace
/// - The content is empty or contains only whitespace
/// - No valid OneNote credentials are configured in the context
/// - The configured endpoint URL is invalid
/// - The Microsoft Graph API request fails to update the page
#[tool]
pub async fn append_content(
    ctx: Context,
    input: AppendContentInput,
) -> Result<AppendContentOutput> {
    ensure!(
        !input.page_id.trim().is_empty(),
        "page_id must not be empty"
    );
    ensure!(
        !input.content.trim().is_empty(),
        "content must not be empty"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let operations = vec![PatchOperation {
        action: PatchAction::Append,
        target: "body".to_string(),
        content: input.content,
        position: None,
    }];

    client
        .patch_json(
            client.url_with_segments(&[
                "me",
                "onenote",
                "pages",
                input.page_id.as_str(),
                "content",
            ])?,
            &operations,
            &[],
        )
        .await?;

    Ok(AppendContentOutput { updated: true })
}

// ============================================================================
// Tool: search
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchInput {
    /// Search query string.
    pub query: String,
    /// Maximum number of results (1-100). Defaults to 20.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchOutput {
    pub pages: Vec<PageSummary>,
}

/// # Search OneNote Pages
///
/// Searches for OneNote pages by keyword or phrase via the Microsoft Graph
/// API. Use this tool when the user wants to find pages containing specific
/// text, keywords, or content across all their notebooks. The search performs a
/// full-text search across page titles and content, returning page summaries
/// with metadata. This is the primary discovery tool when users don't know the
/// specific page ID.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - docs
/// - onenote
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The query is empty or contains only whitespace
/// - The limit parameter is not between 1 and 100
/// - No valid OneNote credentials are configured in the context
/// - The configured endpoint URL is invalid
/// - The Microsoft Graph API request fails
#[tool]
pub async fn search(ctx: Context, input: SearchInput) -> Result<SearchOutput> {
    ensure!(!input.query.trim().is_empty(), "query must not be empty");
    let limit = input.limit.unwrap_or(20);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    // Microsoft Graph search for OneNote uses $search parameter
    let search_value = format!("\"{}\"", input.query);
    let query = [
        ("$search", search_value),
        ("$top", limit.to_string()),
        (
            "$select",
            "id,title,createdDateTime,lastModifiedDateTime,contentUrl".to_string(),
        ),
    ];

    let response: GraphListResponse<PageSummary> = client
        .get_json(
            client.url_with_segments(&["me", "onenote", "pages"])?,
            &query,
            &[("ConsistencyLevel", "eventual")],
        )
        .await?;

    Ok(SearchOutput {
        pages: response.value,
    })
}

// ============================================================================
// Internal Graph API client
// ============================================================================

#[derive(Debug, Deserialize)]
struct GraphListResponse<T> {
    value: Vec<T>,
}

#[derive(Debug, Clone)]
struct GraphClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl GraphClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = OneNoteCredential::get(ctx)?;
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
        extra_headers: &[(&str, &str)],
    ) -> Result<T> {
        let mut request = self.http.get(url).query(query);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        let response = self.send_request(request, "application/json").await?;
        Ok(response.json::<T>().await?)
    }

    async fn get_html(&self, url: reqwest::Url) -> Result<String> {
        let request = self.http.get(url);
        let response = self.send_request(request, "text/html").await?;
        Ok(response.text().await?)
    }

    async fn post_html<T: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        html_body: &str,
        extra_headers: &[(&str, &str)],
    ) -> Result<T> {
        let mut request = self
            .http
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "text/html")
            .body(html_body.to_string());
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        let response = self.send_request(request, "application/json").await?;
        Ok(response.json::<T>().await?)
    }

    async fn patch_json<TReq: Serialize>(
        &self,
        url: reqwest::Url,
        body: &TReq,
        extra_headers: &[(&str, &str)],
    ) -> Result<()> {
        let mut request = self.http.patch(url).json(body);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        self.send_request(request, "application/json").await?;
        Ok(())
    }

    async fn send_request(
        &self,
        request: reqwest::RequestBuilder,
        accept: &str,
    ) -> Result<reqwest::Response> {
        let response = request
            .bearer_auth(&self.access_token)
            .header(reqwest::header::ACCEPT, accept)
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

fn normalize_base_url(endpoint: &str) -> Result<String> {
    let trimmed = endpoint.trim();
    ensure!(!trimmed.is_empty(), "endpoint must not be empty");
    Ok(trimmed.trim_end_matches('/').to_string())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
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
        let mut onenote_values = HashMap::new();
        onenote_values.insert("access_token".to_string(), "test-token".to_string());
        onenote_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("onenote", onenote_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        format!("{}/v1.0", server.uri())
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_patch_action_serialization_roundtrip() {
        for variant in [
            PatchAction::Append,
            PatchAction::Insert,
            PatchAction::Prepend,
            PatchAction::Replace,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: PatchAction = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_notebook_serialization_roundtrip() {
        let notebook = Notebook {
            id: "nb-1".to_string(),
            display_name: Some("My Notebook".to_string()),
            created_date_time: Some("2024-01-01T00:00:00Z".to_string()),
            last_modified_date_time: Some("2024-01-02T00:00:00Z".to_string()),
            is_default: Some(true),
            is_shared: Some(false),
        };
        let json = serde_json::to_string(&notebook).unwrap();
        let parsed: Notebook = serde_json::from_str(&json).unwrap();
        assert_eq!(notebook.id, parsed.id);
        assert_eq!(notebook.display_name, parsed.display_name);
    }

    #[test]
    fn test_page_serialization_roundtrip() {
        let page = Page {
            id: "page-1".to_string(),
            title: Some("My Page".to_string()),
            created_date_time: Some("2024-01-01T00:00:00Z".to_string()),
            last_modified_date_time: Some("2024-01-02T00:00:00Z".to_string()),
            content_url: Some("https://example.com/content".to_string()),
            content: Some("<p>Content</p>".to_string()),
            level: Some(1),
        };
        let json = serde_json::to_string(&page).unwrap();
        let parsed: Page = serde_json::from_str(&json).unwrap();
        assert_eq!(page.id, parsed.id);
        assert_eq!(page.title, parsed.title);
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

    // --- html_escape tests ---

    #[test]
    fn test_html_escape_replaces_special_chars() {
        assert_eq!(html_escape("&"), "&amp;");
        assert_eq!(html_escape("<"), "&lt;");
        assert_eq!(html_escape(">"), "&gt;");
        assert_eq!(html_escape("\""), "&quot;");
        assert_eq!(html_escape("'"), "&#39;");
        assert_eq!(html_escape("<div>&</div>"), "&lt;div&gt;&amp;&lt;/div&gt;");
    }

    // --- Input validation tests ---

    #[tokio::test]
    async fn test_list_notebooks_limit_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_notebooks(ctx, ListNotebooksInput { limit: Some(0) }).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("limit must be between 1 and 100")
        );
    }

    #[tokio::test]
    async fn test_list_notebooks_limit_exceeds_max_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_notebooks(ctx, ListNotebooksInput { limit: Some(101) }).await;

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
                include_content: false,
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
    async fn test_create_page_empty_title_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = create_page(
            ctx,
            CreatePageInput {
                title: "  ".to_string(),
                content: "<p>Content</p>".to_string(),
                section_id: None,
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
    async fn test_create_page_empty_content_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = create_page(
            ctx,
            CreatePageInput {
                title: "My Page".to_string(),
                content: "  ".to_string(),
                section_id: None,
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

    #[tokio::test]
    async fn test_append_content_empty_page_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = append_content(
            ctx,
            AppendContentInput {
                page_id: "  ".to_string(),
                content: "<p>More content</p>".to_string(),
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
    async fn test_append_content_empty_content_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = append_content(
            ctx,
            AppendContentInput {
                page_id: "page-1".to_string(),
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

    #[tokio::test]
    async fn test_search_empty_query_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = search(
            ctx,
            SearchInput {
                query: "  ".to_string(),
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

    // --- Integration tests ---

    #[tokio::test]
    async fn test_list_notebooks_success_returns_notebooks() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "value": [
            {
              "id": "nb-1",
              "displayName": "My Notebook",
              "isDefault": true,
              "isShared": false,
              "createdDateTime": "2024-01-01T00:00:00Z",
              "lastModifiedDateTime": "2024-01-02T00:00:00Z"
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1.0/me/onenote/notebooks"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("$top", "5"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_notebooks(ctx, ListNotebooksInput { limit: Some(5) })
            .await
            .unwrap();

        assert_eq!(output.notebooks.len(), 1);
        assert_eq!(output.notebooks[0].id, "nb-1");
        assert_eq!(
            output.notebooks[0].display_name.as_deref(),
            Some("My Notebook")
        );
        assert_eq!(output.notebooks[0].is_default, Some(true));
    }

    #[tokio::test]
    async fn test_get_page_success_returns_page() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "page-1",
          "title": "My Page",
          "createdDateTime": "2024-01-01T00:00:00Z",
          "lastModifiedDateTime": "2024-01-02T00:00:00Z",
          "contentUrl": "https://example.com/content",
          "level": 1
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1.0/me/onenote/pages/page-1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = get_page(
            ctx,
            GetPageInput {
                page_id: "page-1".to_string(),
                include_content: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.page.id, "page-1");
        assert_eq!(output.page.title.as_deref(), Some("My Page"));
        assert_eq!(output.page.level, Some(1));
    }

    #[tokio::test]
    async fn test_create_page_success_returns_page() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "page-new",
          "title": "New Page",
          "createdDateTime": "2024-01-01T00:00:00Z",
          "lastModifiedDateTime": "2024-01-01T00:00:00Z"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/v1.0/me/onenote/pages"))
            .and(header("authorization", "Bearer test-token"))
            .and(header("content-type", "text/html"))
            .and(body_string_contains("<title>New Page</title>"))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = create_page(
            ctx,
            CreatePageInput {
                title: "New Page".to_string(),
                content: "<p>Page content</p>".to_string(),
                section_id: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.page.id, "page-new");
        assert_eq!(output.page.title.as_deref(), Some("New Page"));
    }

    #[tokio::test]
    async fn test_append_content_success_returns_updated() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("PATCH"))
            .and(path("/v1.0/me/onenote/pages/page-1/content"))
            .and(body_string_contains("\"action\":\"append\""))
            .and(body_string_contains("\"target\":\"body\""))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = append_content(
            ctx,
            AppendContentInput {
                page_id: "page-1".to_string(),
                content: "<p>Additional content</p>".to_string(),
            },
        )
        .await
        .unwrap();

        assert!(output.updated);
    }

    #[tokio::test]
    async fn test_search_success_returns_pages() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "value": [
            {
              "id": "page-1",
              "title": "Search Result",
              "createdDateTime": "2024-01-01T00:00:00Z",
              "lastModifiedDateTime": "2024-01-02T00:00:00Z",
              "contentUrl": "https://example.com/content"
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1.0/me/onenote/pages"))
            .and(header("consistencylevel", "eventual"))
            .and(query_param("$search", "\"test\""))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = search(
            ctx,
            SearchInput {
                query: "test".to_string(),
                limit: Some(10),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.pages.len(), 1);
        assert_eq!(output.pages[0].id, "page-1");
        assert_eq!(output.pages[0].title.as_deref(), Some("Search Result"));
    }
}
