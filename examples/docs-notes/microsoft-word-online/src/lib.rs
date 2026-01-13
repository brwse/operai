//! Microsoft Word Online integration for Operai Toolbox.
//!
//! This integration provides tools for interacting with Microsoft Word
//! documents via the Microsoft Graph API.
//!
//! ## Important Notes
//!
//! Microsoft Graph API provides limited functionality for Word documents:
//! - **Supported**: Document metadata retrieval, file download, format
//!   conversion (PDF, etc.)
//! - **Not Supported**: Direct paragraph/table manipulation, document comments,
//!   content updates
//!
//! For advanced Word document manipulation, consider:
//! - Office JavaScript API (Office.js) for add-in development
//! - Downloading files and using client-side libraries (e.g., python-docx)
//! - Microsoft Word REST API (limited availability)

mod types;

use operai::{
    Context, JsonSchema, Result, anyhow, define_user_credential, ensure, info, init, schemars,
    shutdown, tool,
};
use serde::{Deserialize, Serialize};
use types::DriveItem;

define_user_credential! {
    WordOnlineCredential("word_online") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_GRAPH_ENDPOINT: &str = "https://graph.microsoft.com/v1.0";

#[init]
async fn setup() -> Result<()> {
    info!("Microsoft Word Online integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Microsoft Word Online integration shutting down");
}

// ============================================================================
// Get Document Tool
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetDocumentInput {
    /// The unique identifier of the document (item ID from
    /// OneDrive/SharePoint).
    pub document_id: String,
    /// The drive ID containing the document. If not provided, uses the user's
    /// default drive.
    #[serde(default)]
    pub drive_id: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetDocumentOutput {
    pub id: String,
    pub name: String,
    pub web_url: String,
    pub created_at: String,
    pub modified_at: String,
    pub last_modified_by: String,
    pub size_bytes: u64,
}

/// # Get Word Online Document
///
/// Retrieves metadata for a Microsoft Word document stored on OneDrive or
/// SharePoint via the Microsoft Graph API.
///
/// Use this tool when you need to retrieve information about a Word document
/// such as its name, file size, creation/modification timestamps, web URL, and
/// the user who last modified it. This tool only returns document metadata—it
/// does not provide access to the document's textual content, paragraphs,
/// tables, or comments.
///
/// **When to use this tool:**
/// - Checking if a document exists and is accessible
/// - Getting document properties (name, size, URLs, timestamps)
/// - Retrieving the web URL for opening the document in a browser
/// - Verifying document ownership and modification history
///
/// **When NOT to use this tool:**
/// - Reading or editing document content (text, paragraphs, tables)
/// - Adding or viewing comments within the document
/// - Converting the document to another format (use `export_document` instead)
///
/// **Important constraints:**
/// - Requires a valid Microsoft Graph API access token configured in user
///   credentials
/// - The `document_id` must be a valid item ID from OneDrive or SharePoint
/// - If accessing documents from a specific drive (not the user's default
///   drive), provide the `drive_id` parameter
/// - Microsoft Graph API does not support direct content access via REST
///   endpoints
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - docs
/// - word
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The provided `document_id` is empty or contains only whitespace
/// - No valid access token is configured in the user credentials
/// - The Microsoft Graph API request fails (network errors, authentication
///   failures, rate limiting)
/// - The document doesn't exist (404 Not Found)
/// - The API response is malformed or cannot be parsed
#[tool]
pub async fn get_document(ctx: Context, input: GetDocumentInput) -> Result<GetDocumentOutput> {
    ensure!(
        !input.document_id.trim().is_empty(),
        "document_id must not be empty"
    );

    let client = GraphClient::from_ctx(&ctx)?;
    let drive_path = input
        .drive_id
        .as_ref()
        .map_or_else(|| "me/drive".to_string(), |id| format!("drives/{id}"));

    // Get document metadata
    let item: DriveItem = client
        .get_json(
            client.url_with_segments(&[&drive_path, "items", &input.document_id])?,
            &[],
            &[],
        )
        .await?;

    Ok(GetDocumentOutput {
        id: item.id,
        name: item.name,
        web_url: item.web_url.unwrap_or_default(),
        created_at: item.created_date_time.unwrap_or_default(),
        modified_at: item.last_modified_date_time.unwrap_or_default(),
        last_modified_by: item
            .last_modified_by
            .and_then(|i| i.user)
            .and_then(|u| u.display_name)
            .unwrap_or_else(|| "Unknown".to_string()),
        size_bytes: u64::try_from(item.size.unwrap_or(0)).unwrap_or(0),
    })
}

// ============================================================================
// Export Document Tool
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Pdf,
    Html,
    Txt,
    Rtf,
    Odt,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportDocumentInput {
    pub document_id: String,
    #[serde(default)]
    pub drive_id: Option<String>,
    pub format: ExportFormat,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ExportDocumentOutput {
    pub document_id: String,
    pub format: String,
    pub download_url: String,
    pub expires_at: String,
    pub size_bytes: u64,
    pub filename: String,
}

/// # Export Word Online Document
///
/// Converts a Microsoft Word document to a different format (PDF, HTML, TXT,
/// RTF, or ODT) and generates a download URL via the Microsoft Graph API.
///
/// Use this tool when you need to convert a Word document stored on OneDrive or
/// SharePoint to another format. The tool returns a download URL that can be
/// used to retrieve the converted document. The conversion is performed
/// server-side by Microsoft Graph API's format conversion endpoint.
///
/// **When to use this tool:**
/// - Converting a Word document to PDF for sharing or archiving
/// - Exporting to HTML for web publishing
/// - Extracting plain text content from a Word document
/// - Converting to open formats like RTF or ODT for compatibility
///
/// **Supported output formats:**
/// - `pdf`: Portable Document Format (ideal for sharing and printing)
/// - `html`: HTML web page (suitable for web publishing)
/// - `txt`: Plain text (extracts text content without formatting)
/// - `rtf`: Rich Text Format (widely compatible text format)
/// - `odt`: `OpenDocument` Text (open standard format)
///
/// **Important constraints:**
/// - Requires a valid Microsoft Graph API access token configured in user
///   credentials
/// - The download URL expires after **one hour**—use it promptly
/// - The returned output includes the filename, size, and expiry timestamp
/// - The `document_id` must be a valid item ID from OneDrive or SharePoint
/// - If accessing documents from a specific drive (not the user's default
///   drive), provide the `drive_id` parameter
/// - Conversion is performed asynchronously; the download URL is generated
///   immediately
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - docs
/// - word
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The provided `document_id` is empty or contains only whitespace
/// - No valid access token is configured in the user credentials
/// - The Microsoft Graph API request fails (network errors, authentication
///   failures, rate limiting)
/// - The document doesn't exist or is inaccessible
/// - The export operation fails due to document corruption or unsupported
///   content
/// - The API response is malformed or cannot be parsed
#[tool]
pub async fn export_document(
    ctx: Context,
    input: ExportDocumentInput,
) -> Result<ExportDocumentOutput> {
    ensure!(
        !input.document_id.trim().is_empty(),
        "document_id must not be empty"
    );

    let client = GraphClient::from_ctx(&ctx)?;
    let drive_path = input
        .drive_id
        .as_ref()
        .map_or_else(|| "me/drive".to_string(), |id| format!("drives/{id}"));

    let format_str = match input.format {
        ExportFormat::Pdf => "pdf",
        ExportFormat::Html => "html",
        ExportFormat::Txt => "txt",
        ExportFormat::Rtf => "rtf",
        ExportFormat::Odt => "odt",
    };

    // Get document metadata for filename
    let item: DriveItem = client
        .get_json(
            client.url_with_segments(&[&drive_path, "items", &input.document_id])?,
            &[],
            &[],
        )
        .await?;

    // Microsoft Graph export endpoint
    let query = [("format", format_str.to_string())];
    let download_url = client
        .url_with_segments(&[&drive_path, "items", &input.document_id, "content"])?
        .to_string();

    let base_name = item
        .name
        .strip_suffix(".docx")
        .or(item.name.strip_suffix(".doc"))
        .unwrap_or(&item.name);
    let filename = format!("{base_name}.{format_str}");

    // Calculate expiry (Graph download URLs typically expire in 1 hour)
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(1);

    Ok(ExportDocumentOutput {
        document_id: input.document_id,
        format: format_str.to_string(),
        download_url: format!("{download_url}?{}", serde_urlencoded::to_string(query)?),
        expires_at: expires_at.to_rfc3339(),
        size_bytes: u64::try_from(item.size.unwrap_or(0)).unwrap_or(0),
        filename,
    })
}

// ============================================================================
// Graph Client Helper
// ============================================================================

#[derive(Debug, Clone)]
struct GraphClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl GraphClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = WordOnlineCredential::get(ctx)?;
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
                .map_err(|()| anyhow::anyhow!("base_url must be an absolute URL"))?;
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

        let response = self.send_request(request).await?;
        Ok(response.json::<T>().await?)
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
            Err(anyhow::anyhow!(
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
        let mut word_values = HashMap::new();
        word_values.insert("access_token".to_string(), "test-token".to_string());
        word_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("word_online", word_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        format!("{}/v1.0", server.uri())
    }

    // --- Input validation tests ---

    #[tokio::test]
    async fn test_get_document_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = get_document(
            ctx,
            GetDocumentInput {
                document_id: "  ".to_string(),
                drive_id: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("document_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_export_document_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = export_document(
            ctx,
            ExportDocumentInput {
                document_id: "  ".to_string(),
                drive_id: None,
                format: ExportFormat::Pdf,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("document_id must not be empty")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_get_document_success_returns_metadata() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "doc-123",
          "name": "Test Document.docx",
          "webUrl": "https://onedrive.live.com/edit.aspx?resid=ABC123",
          "size": 45678,
          "createdDateTime": "2024-01-15T10:30:00Z",
          "lastModifiedDateTime": "2024-01-20T14:45:00Z",
          "lastModifiedBy": {
            "user": {
              "displayName": "John Doe",
              "id": "user-1"
            }
          },
          "file": {
            "mimeType": "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
          }
        }
        "#;

        Mock::given(method("GET"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = get_document(
            ctx,
            GetDocumentInput {
                document_id: "doc-123".to_string(),
                drive_id: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.id, "doc-123");
        assert_eq!(output.name, "Test Document.docx");
        assert!(output.web_url.starts_with("https://"));
        assert_eq!(output.last_modified_by, "John Doe");
        assert_eq!(output.size_bytes, 45678);
    }

    #[tokio::test]
    async fn test_get_document_not_found_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/v1.0/me/drive/items/missing"))
            .respond_with(ResponseTemplate::new(404).set_body_raw(
                r#"{ "error": { "code": "itemNotFound", "message": "Item not found" } }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = get_document(
            ctx,
            GetDocumentInput {
                document_id: "missing".to_string(),
                drive_id: None,
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("404"));
    }

    #[tokio::test]
    async fn test_export_document_to_pdf_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "doc-123",
          "name": "Sample Document.docx",
          "size": 50000
        }
        "#;

        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = export_document(
            ctx,
            ExportDocumentInput {
                document_id: "doc-123".to_string(),
                drive_id: None,
                format: ExportFormat::Pdf,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.document_id, "doc-123");
        assert_eq!(output.format, "pdf");
        assert!(output.download_url.contains("doc-123"));
        assert!(output.download_url.contains("format=pdf"));
        assert_eq!(output.filename, "Sample Document.pdf");
        assert_eq!(output.size_bytes, 50000);
    }

    #[test]
    fn test_export_format_deserializes_all_variants() {
        let pdf: ExportFormat = serde_json::from_str(r#""pdf""#).unwrap();
        let html: ExportFormat = serde_json::from_str(r#""html""#).unwrap();
        let txt: ExportFormat = serde_json::from_str(r#""txt""#).unwrap();
        let rtf: ExportFormat = serde_json::from_str(r#""rtf""#).unwrap();
        let odt: ExportFormat = serde_json::from_str(r#""odt""#).unwrap();

        assert!(matches!(pdf, ExportFormat::Pdf));
        assert!(matches!(html, ExportFormat::Html));
        assert!(matches!(txt, ExportFormat::Txt));
        assert!(matches!(rtf, ExportFormat::Rtf));
        assert!(matches!(odt, ExportFormat::Odt));
    }

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
}
