//! docs-notes/google-docs integration for Operai Toolbox.
use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

mod types;
pub use types::{Document, Location, Range};

define_user_credential! {
    GoogleDocsCredential("google_docs") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
        #[optional]
        drive_endpoint: Option<String>,
    }
}

const DEFAULT_DOCS_ENDPOINT: &str = "https://docs.googleapis.com/v1";
const DEFAULT_DRIVE_ENDPOINT: &str = "https://www.googleapis.com/drive/v3";

#[init]
#[expect(clippy::unused_async, reason = "required by the #[init] proc macro")]
async fn setup() -> Result<()> {
    info!("Google Docs integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Google Docs integration shutting down");
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetDocInput {
    /// The Google Docs document ID (from the document URL).
    pub document_id: String,
    /// Whether to include the full document body content.
    #[serde(default)]
    pub include_body: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetDocOutput {
    pub document: Document,
}

/// # Get Google Docs Document
///
/// Retrieves a Google Doc by its document ID, including metadata and optionally
/// the full document body content. Use this tool when you need to read the
/// contents or properties of an existing Google Docs document.
///
/// The document ID can be extracted from a Google Docs URL (e.g., from
/// `https://docs.google.com/document/d/DOCUMENT_ID/edit`). This tool returns
/// the complete document structure including title, revision ID, and body
/// content (when `include_body` is true).
///
/// Use this tool when:
/// - A user wants to read the contents of a specific Google Doc
/// - You need to fetch document metadata (title, revision ID, etc.)
/// - Preparing to make edits to a document (get current state first)
/// - Exporting or analyzing document content
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - docs
/// - google
/// - document
///
/// # Errors
///
/// Returns an error if:
/// - The `document_id` is empty or contains only whitespace
/// - No valid Google Docs credential is configured in the context
/// - The access token in the credential is empty
/// - The configured endpoint URL is invalid
/// - The HTTP request to the Google Docs API fails (network errors, timeout,
///   etc.)
/// - The API returns a non-success status code (e.g., 404 for not found, 403
///   for permission denied)
/// - The response body cannot be parsed as a valid `Document` object
#[tool]
pub async fn get_doc(ctx: Context, input: GetDocInput) -> Result<GetDocOutput> {
    ensure!(
        !input.document_id.trim().is_empty(),
        "document_id must not be empty"
    );

    let client = DocsClient::from_ctx(&ctx)?;

    let document: Document = client
        .get_json(
            client.docs_url_with_segments(&["documents", input.document_id.as_str()])?,
            &[],
        )
        .await?;

    Ok(GetDocOutput { document })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InsertTextInput {
    /// The Google Docs document ID.
    pub document_id: String,
    /// The text to insert.
    pub text: String,
    /// The location (index) where text should be inserted. Index 1 is the start
    /// of the body.
    pub location: Location,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct InsertTextOutput {
    pub document_id: String,
    pub revision_id: String,
}

/// # Insert Google Docs Text
///
/// Inserts new text content at a specific location within a Google Doc.
/// Use this tool when you need to add new content to a document without
/// replacing or deleting existing text.
///
/// The location is specified by a character index where index 1 represents
/// the start of the document body. The text will be inserted at the exact
/// position specified, pushing existing content to the right.
///
/// Use this tool when:
/// - Adding new paragraphs, sentences, or words to a document
/// - Appending content to the end of a document
/// - Prepending content to the beginning of a document
/// - Inserting text between existing content segments
///
/// Note: To replace existing text, use the `update_text` tool instead.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - docs
/// - google
/// - edit
///
/// # Errors
///
/// Returns an error if:
/// - The `document_id` is empty or contains only whitespace
/// - The `text` to insert is empty
/// - The `location.index` is less than 1 (index must be >= 1, where 1 is the
///   start of the body)
/// - No valid Google Docs credential is configured in the context
/// - The access token in the credential is empty
/// - The configured endpoint URL is invalid
/// - The HTTP request to the Google Docs API fails (network errors, timeout,
///   etc.)
/// - The API returns a non-success status code (e.g., 404 for not found, 403
///   for permission denied)
/// - The response body cannot be parsed as a valid `BatchUpdateResponse` object
#[tool]
pub async fn insert_text(ctx: Context, input: InsertTextInput) -> Result<InsertTextOutput> {
    ensure!(
        !input.document_id.trim().is_empty(),
        "document_id must not be empty"
    );
    ensure!(!input.text.is_empty(), "text must not be empty");
    ensure!(input.location.index >= 1, "location index must be >= 1");

    let client = DocsClient::from_ctx(&ctx)?;

    let request = types::BatchUpdateRequest {
        requests: vec![types::Request::InsertText(types::InsertTextRequest {
            text: input.text,
            location: input.location,
        })],
    };

    let response: types::BatchUpdateResponse = client
        .post_json(
            client.docs_url_with_segments(&[
                "documents",
                input.document_id.as_str(),
                ":batchUpdate",
            ])?,
            &request,
        )
        .await?;

    Ok(InsertTextOutput {
        document_id: response.document_id,
        revision_id: response.revision_id,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateTextInput {
    /// The Google Docs document ID.
    pub document_id: String,
    /// The range of text to replace.
    pub range: Range,
    /// The new text to insert (or empty string to delete).
    pub new_text: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateTextOutput {
    pub document_id: String,
    pub revision_id: String,
}

/// # Update Google Docs Text
///
/// Replaces or deletes text within a specified range in a Google Doc.
/// Use this tool when you need to modify existing content by replacing it
/// with new text, or delete text entirely by providing an empty string.
///
/// The tool works by first deleting the content in the specified range,
/// then optionally inserting new text at the same position. The range is
/// defined by start and end character indices (1-indexed, where 1 is the
/// start of the document body).
///
/// Use this tool when:
/// - Replacing specific words, phrases, or paragraphs with new content
/// - Deleting text by providing an empty `new_text` string
/// - Making corrections or edits to existing content
/// - Fixing typos or updating information in a document
///
/// Note: To insert new text without deleting existing content, use the
/// `insert_text` tool instead.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - docs
/// - google
/// - edit
///
/// # Errors
///
/// Returns an error if:
/// - The `document_id` is empty or contains only whitespace
/// - The `range.start_index` is less than 1
/// - The `range.end_index` is not greater than `range.start_index`
/// - No valid Google Docs credential is configured in the context
/// - The access token in the credential is empty
/// - The configured endpoint URL is invalid
/// - The HTTP request to the Google Docs API fails (network errors, timeout,
///   etc.)
/// - The API returns a non-success status code (e.g., 404 for not found, 403
///   for permission denied)
/// - The response body cannot be parsed as a valid `BatchUpdateResponse` object
#[tool]
pub async fn update_text(ctx: Context, input: UpdateTextInput) -> Result<UpdateTextOutput> {
    ensure!(
        !input.document_id.trim().is_empty(),
        "document_id must not be empty"
    );
    ensure!(
        input.range.start_index >= 1,
        "range start_index must be >= 1"
    );
    ensure!(
        input.range.end_index > input.range.start_index,
        "range end_index must be greater than start_index"
    );

    let client = DocsClient::from_ctx(&ctx)?;

    let mut requests = vec![types::Request::DeleteContentRange(
        types::DeleteContentRangeRequest {
            range: input.range.clone(),
        },
    )];

    if !input.new_text.is_empty() {
        requests.push(types::Request::InsertText(types::InsertTextRequest {
            text: input.new_text,
            location: Location {
                index: input.range.start_index,
                segment_id: input.range.segment_id,
            },
        }));
    }

    let request = types::BatchUpdateRequest { requests };

    let response: types::BatchUpdateResponse = client
        .post_json(
            client.docs_url_with_segments(&[
                "documents",
                input.document_id.as_str(),
                ":batchUpdate",
            ])?,
            &request,
        )
        .await?;

    Ok(UpdateTextOutput {
        document_id: response.document_id,
        revision_id: response.revision_id,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddCommentInput {
    /// The Google Docs document ID.
    pub document_id: String,
    /// The comment content.
    pub content: String,
    /// The range of text to attach the comment to.
    pub anchor: Range,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AddCommentOutput {
    pub comment_id: String,
    pub document_id: String,
}

/// # Add Google Docs Comment
///
/// Adds a comment to a Google Doc, attached to a specific range of text.
/// Use this tool when a user wants to add feedback, suggestions, or notes
/// to a document without modifying the actual content.
///
/// Comments are attached to specific text ranges and will appear in the
/// document's comment thread, visible to collaborators with access to the
/// document. The comment is anchored to the specified text range, so if the
/// document is edited, the comment may move or become orphaned.
///
/// Use this tool when:
/// - Providing feedback on document content
/// - Asking questions about specific text sections
/// - Adding notes or suggestions for collaborators
/// - Reviewing and annotating documents
/// - Requesting changes or clarifications
///
/// Note: Comments are managed through the Google Drive API, not the Docs API,
/// which means they are separate from document content and have their own
/// lifecycle (can be resolved, replied to, etc.).
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - docs
/// - google
/// - comment
///
/// # Errors
///
/// Returns an error if:
/// - The `document_id` is empty or contains only whitespace
/// - The `content` is empty or contains only whitespace
/// - The `anchor.start_index` is less than 1
/// - The `anchor.end_index` is not greater than `anchor.start_index`
/// - No valid Google Docs credential is configured in the context
/// - The access token in the credential is empty
/// - The configured endpoint URL is invalid
/// - The HTTP request to the Google Drive API fails (network errors, timeout,
///   etc.)
/// - The API returns a non-success status code (e.g., 404 for not found, 403
///   for permission denied)
/// - The response body cannot be parsed as a valid comment object
///
/// Note: Comments are managed through the Drive API, not the Docs API.
#[tool]
pub async fn add_comment(ctx: Context, input: AddCommentInput) -> Result<AddCommentOutput> {
    ensure!(
        !input.document_id.trim().is_empty(),
        "document_id must not be empty"
    );
    ensure!(
        !input.content.trim().is_empty(),
        "content must not be empty"
    );
    ensure!(
        input.anchor.start_index >= 1,
        "anchor start_index must be >= 1"
    );
    ensure!(
        input.anchor.end_index > input.anchor.start_index,
        "anchor end_index must be greater than start_index"
    );

    let client = DocsClient::from_ctx(&ctx)?;

    // Comments API is part of Drive API
    // POST https://www.googleapis.com/drive/v3/files/{fileId}/comments
    let comment_body = serde_json::json!({
        "content": input.content,
        "anchor": {
            "r": input.anchor.start_index,
            "type": "text"
        }
    });

    let response: types::CommentResponse = client
        .post_json(
            client.drive_url_with_segments(&["files", input.document_id.as_str(), "comments"])?,
            &comment_body,
        )
        .await?;

    Ok(AddCommentOutput {
        comment_id: response.id,
        document_id: input.document_id,
    })
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Pdf,
    Txt,
    Html,
    Docx,
    Odt,
    Rtf,
    Epub,
    Md,
}

impl ExportFormat {
    fn mime_type(self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Txt => "text/plain",
            Self::Html => "application/zip",
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Odt => "application/vnd.oasis.opendocument.text",
            Self::Rtf => "application/rtf",
            Self::Epub => "application/epub+zip",
            Self::Md => "text/markdown",
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportDocInput {
    /// The Google Docs document ID.
    pub document_id: String,
    /// Export format (pdf, txt, html, docx, odt, rtf, epub, md). Defaults to
    /// pdf.
    #[serde(default)]
    pub format: Option<ExportFormat>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ExportDocOutput {
    pub document_id: String,
    pub format: ExportFormat,
    pub mime_type: String,
    pub download_url: String,
}

/// # Export Google Docs Document
///
/// Exports a Google Doc to various file formats including PDF, plain text,
/// HTML, Word (.docx), `OpenDocument` (.odt), RTF, EPUB, or Markdown.
/// Use this tool when a user wants to download or convert a Google Doc to
/// a standard file format for offline use, sharing, or further processing.
///
/// This tool leverages the Google Drive API's export functionality and returns
/// a download URL that can be used to retrieve the exported file. By default,
/// the export format is PDF if no format is specified.
///
/// Supported export formats:
/// - PDF: Best for sharing and printing
/// - TXT: Plain text, format-stripped content
/// - HTML: Web-ready format (packaged as ZIP)
/// - DOCX: Microsoft Word compatible format
/// - ODT: `OpenDocument` format for LibreOffice/OpenOffice
/// - RTF: Rich Text Format
/// - EPUB: E-book format
/// - MD: Markdown format for documentation/development
///
/// Use this tool when:
/// - Converting a Google Doc to PDF for sharing or archiving
/// - Exporting to Word/Office formats for external collaborators
/// - Converting to Markdown for documentation workflows
/// - Downloading a local copy of a document
/// - Migrating content from Google Docs to other systems
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - docs
/// - google
/// - export
///
/// # Errors
///
/// Returns an error if:
/// - The `document_id` is empty or contains only whitespace
/// - No valid Google Docs credential is configured in the context
/// - The access token in the credential is empty
/// - The configured Drive endpoint URL is invalid
/// - The HTTP request to construct the export URL fails (URL parsing errors)
/// - The HTTP request to export the document fails (network errors, timeout,
///   etc.)
/// - The API returns a non-success status code (e.g., 404 for not found, 403
///   for permission denied)
#[tool]
pub async fn export_doc(ctx: Context, input: ExportDocInput) -> Result<ExportDocOutput> {
    ensure!(
        !input.document_id.trim().is_empty(),
        "document_id must not be empty"
    );

    let format = input.format.unwrap_or(ExportFormat::Pdf);
    let client = DocsClient::from_ctx(&ctx)?;

    // Call Drive API files.export
    // GET https://www.googleapis.com/drive/v3/files/{fileId}/export?mimeType={mimeType}
    let export_url =
        client.drive_url_with_segments(&["files", input.document_id.as_str(), "export"])?;

    // Add query parameter for MIME type
    let url_with_params =
        reqwest::Url::parse_with_params(export_url.as_str(), &[("mimeType", format.mime_type())])?;

    // Make the export request
    let response = client
        .http
        .get(url_with_params)
        .bearer_auth(&client.access_token)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(operai::anyhow::anyhow!(
            "Google Drive API export request failed ({status}): {body}"
        ));
    }

    Ok(ExportDocOutput {
        document_id: input.document_id,
        format,
        mime_type: format.mime_type().to_string(),
        download_url: export_url.to_string(),
    })
}

#[derive(Debug, Clone)]
struct DocsClient {
    http: reqwest::Client,
    docs_base_url: String,
    drive_base_url: String,
    access_token: String,
}

impl DocsClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = GoogleDocsCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let docs_base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_DOCS_ENDPOINT))?;
        let drive_base_url = normalize_base_url(
            cred.drive_endpoint
                .as_deref()
                .unwrap_or(DEFAULT_DRIVE_ENDPOINT),
        )?;

        Ok(Self {
            http: reqwest::Client::new(),
            docs_base_url,
            drive_base_url,
            access_token: cred.access_token,
        })
    }

    fn docs_url_with_segments(&self, segments: &[&str]) -> Result<reqwest::Url> {
        Self::url_with_segments(&self.docs_base_url, segments)
    }

    fn drive_url_with_segments(&self, segments: &[&str]) -> Result<reqwest::Url> {
        Self::url_with_segments(&self.drive_base_url, segments)
    }

    fn url_with_segments(base: &str, segments: &[&str]) -> Result<reqwest::Url> {
        let mut url = reqwest::Url::parse(base)?;
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
        let request = self.http.get(url).query(query);
        let response = self.send_request(request).await?;
        Ok(response.json::<T>().await?)
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
    ) -> Result<TRes> {
        let request = self.http.post(url).json(body);
        let response = self.send_request(request).await?;
        Ok(response.json::<TRes>().await?)
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
                "Google Docs API request failed ({status}): {body}"
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

    fn test_ctx(docs_endpoint: &str) -> Context {
        test_ctx_with_endpoints(docs_endpoint, "https://www.googleapis.com/drive/v3")
    }

    fn test_ctx_with_endpoints(docs_endpoint: &str, drive_endpoint: &str) -> Context {
        let mut google_docs_values = HashMap::new();
        google_docs_values.insert("access_token".to_string(), "ya29.test-token".to_string());
        google_docs_values.insert("endpoint".to_string(), docs_endpoint.to_string());
        google_docs_values.insert("drive_endpoint".to_string(), drive_endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("google_docs", google_docs_values)
    }

    fn docs_endpoint_for(server: &MockServer) -> String {
        format!("{}/v1", server.uri())
    }

    fn drive_endpoint_for(server: &MockServer) -> String {
        format!("{}/v3", server.uri())
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_location_serialization_roundtrip() {
        let location = Location {
            index: 42,
            segment_id: None,
        };
        let json = serde_json::to_string(&location).unwrap();
        let parsed: Location = serde_json::from_str(&json).unwrap();
        assert_eq!(location.index, parsed.index);
    }

    #[test]
    fn test_range_serialization_roundtrip() {
        let range = Range {
            start_index: 10,
            end_index: 50,
            segment_id: None,
        };
        let json = serde_json::to_string(&range).unwrap();
        let parsed: Range = serde_json::from_str(&json).unwrap();
        assert_eq!(range.start_index, parsed.start_index);
        assert_eq!(range.end_index, parsed.end_index);
    }

    #[test]
    fn test_export_format_serialization_roundtrip() {
        for format in [
            ExportFormat::Pdf,
            ExportFormat::Txt,
            ExportFormat::Html,
            ExportFormat::Docx,
            ExportFormat::Odt,
            ExportFormat::Rtf,
            ExportFormat::Epub,
            ExportFormat::Md,
        ] {
            let json = serde_json::to_string(&format).unwrap();
            let parsed: ExportFormat = serde_json::from_str(&json).unwrap();
            assert_eq!(format, parsed);
        }
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://docs.googleapis.com/v1/").unwrap();
        assert_eq!(result, "https://docs.googleapis.com/v1");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://docs.googleapis.com/v1  ").unwrap();
        assert_eq!(result, "https://docs.googleapis.com/v1");
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
    async fn test_get_doc_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&docs_endpoint_for(&server));

        let result = get_doc(
            ctx,
            GetDocInput {
                document_id: "  ".to_string(),
                include_body: false,
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
    async fn test_insert_text_empty_document_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&docs_endpoint_for(&server));

        let result = insert_text(
            ctx,
            InsertTextInput {
                document_id: "  ".to_string(),
                text: "Hello".to_string(),
                location: Location {
                    index: 1,
                    segment_id: None,
                },
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
    async fn test_insert_text_empty_text_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&docs_endpoint_for(&server));

        let result = insert_text(
            ctx,
            InsertTextInput {
                document_id: "doc-123".to_string(),
                text: String::new(),
                location: Location {
                    index: 1,
                    segment_id: None,
                },
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("text must not be empty")
        );
    }

    #[tokio::test]
    async fn test_insert_text_invalid_location_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&docs_endpoint_for(&server));

        let result = insert_text(
            ctx,
            InsertTextInput {
                document_id: "doc-123".to_string(),
                text: "Hello".to_string(),
                location: Location {
                    index: 0,
                    segment_id: None,
                },
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("location index must be >= 1")
        );
    }

    #[tokio::test]
    async fn test_update_text_invalid_range_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&docs_endpoint_for(&server));

        let result = update_text(
            ctx,
            UpdateTextInput {
                document_id: "doc-123".to_string(),
                range: Range {
                    start_index: 10,
                    end_index: 5,
                    segment_id: None,
                },
                new_text: "Updated".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("end_index must be greater than start_index")
        );
    }

    #[tokio::test]
    async fn test_add_comment_empty_content_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&docs_endpoint_for(&server));

        let result = add_comment(
            ctx,
            AddCommentInput {
                document_id: "doc-123".to_string(),
                content: "  ".to_string(),
                anchor: Range {
                    start_index: 1,
                    end_index: 10,
                    segment_id: None,
                },
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
    async fn test_export_doc_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&docs_endpoint_for(&server));

        let result = export_doc(
            ctx,
            ExportDocInput {
                document_id: "  ".to_string(),
                format: Some(ExportFormat::Pdf),
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
    async fn test_get_doc_success_returns_document() {
        let server = MockServer::start().await;
        let endpoint = docs_endpoint_for(&server);

        let response_body = r#"
        {
          "documentId": "doc-abc123",
          "title": "Test Document",
          "revisionId": "rev-456"
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1/documents/doc-abc123"))
            .and(header("authorization", "Bearer ya29.test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = get_doc(
            ctx,
            GetDocInput {
                document_id: "doc-abc123".to_string(),
                include_body: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.document.document_id, "doc-abc123");
        assert_eq!(output.document.title, "Test Document");
    }

    #[tokio::test]
    async fn test_get_doc_error_response_returns_error() {
        let server = MockServer::start().await;
        let endpoint = docs_endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/v1/documents/missing"))
            .respond_with(ResponseTemplate::new(404).set_body_raw(
                r#"{ "error": { "code": 404, "message": "Document not found" } }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = get_doc(
            ctx,
            GetDocInput {
                document_id: "missing".to_string(),
                include_body: false,
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("404"));
    }

    #[tokio::test]
    async fn test_insert_text_success_returns_document_id() {
        let server = MockServer::start().await;
        let endpoint = docs_endpoint_for(&server);

        let response_body = r#"
        {
          "documentId": "doc-xyz789",
          "revisionId": "rev-new-123",
          "replies": []
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/v1/documents/doc-xyz789/:batchUpdate"))
            .and(header("authorization", "Bearer ya29.test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = insert_text(
            ctx,
            InsertTextInput {
                document_id: "doc-xyz789".to_string(),
                text: "Hello, World!".to_string(),
                location: Location {
                    index: 1,
                    segment_id: None,
                },
            },
        )
        .await
        .unwrap();

        assert_eq!(output.document_id, "doc-xyz789");
        assert_eq!(output.revision_id, "rev-new-123");
    }

    #[tokio::test]
    async fn test_add_comment_success_returns_comment_id() {
        let server = MockServer::start().await;
        let docs_endpoint = docs_endpoint_for(&server);
        let drive_endpoint = drive_endpoint_for(&server);

        let response_body = r#"
        {
          "id": "comment-abc456",
          "content": "This is a test comment"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/v3/files/doc-comment/comments"))
            .and(header("authorization", "Bearer ya29.test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx_with_endpoints(&docs_endpoint, &drive_endpoint);
        let output = add_comment(
            ctx,
            AddCommentInput {
                document_id: "doc-comment".to_string(),
                content: "This is a test comment".to_string(),
                anchor: Range {
                    start_index: 1,
                    end_index: 10,
                    segment_id: None,
                },
            },
        )
        .await
        .unwrap();

        assert_eq!(output.comment_id, "comment-abc456");
        assert_eq!(output.document_id, "doc-comment");
    }

    #[tokio::test]
    async fn test_export_doc_returns_correct_mime_type() {
        let server = MockServer::start().await;
        let docs_endpoint = docs_endpoint_for(&server);
        let drive_endpoint = drive_endpoint_for(&server);

        // Mock the Drive API export endpoint
        Mock::given(method("GET"))
            .and(path("/v3/files/doc-export/export"))
            .and(header("authorization", "Bearer ya29.test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"test pdf content"))
            .mount(&server)
            .await;

        let ctx = test_ctx_with_endpoints(&docs_endpoint, &drive_endpoint);
        let output = export_doc(
            ctx,
            ExportDocInput {
                document_id: "doc-export".to_string(),
                format: Some(ExportFormat::Pdf),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.document_id, "doc-export");
        assert_eq!(output.format, ExportFormat::Pdf);
        assert_eq!(output.mime_type, "application/pdf");
        assert!(output.download_url.contains("doc-export"));
    }

    #[tokio::test]
    async fn test_export_doc_defaults_to_pdf() {
        let server = MockServer::start().await;
        let docs_endpoint = docs_endpoint_for(&server);
        let drive_endpoint = drive_endpoint_for(&server);

        // Mock the Drive API export endpoint
        Mock::given(method("GET"))
            .and(path("/v3/files/doc-default/export"))
            .and(header("authorization", "Bearer ya29.test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"test content"))
            .mount(&server)
            .await;

        let ctx = test_ctx_with_endpoints(&docs_endpoint, &drive_endpoint);
        let output = export_doc(
            ctx,
            ExportDocInput {
                document_id: "doc-default".to_string(),
                format: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.format, ExportFormat::Pdf);
    }

    #[tokio::test]
    async fn test_export_doc_html_mime_type_is_zip() {
        let server = MockServer::start().await;
        let docs_endpoint = docs_endpoint_for(&server);
        let drive_endpoint = drive_endpoint_for(&server);

        // Mock the Drive API export endpoint
        Mock::given(method("GET"))
            .and(path("/v3/files/doc-html/export"))
            .and(header("authorization", "Bearer ya29.test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"test html zip"))
            .mount(&server)
            .await;

        let ctx = test_ctx_with_endpoints(&docs_endpoint, &drive_endpoint);
        let output = export_doc(
            ctx,
            ExportDocInput {
                document_id: "doc-html".to_string(),
                format: Some(ExportFormat::Html),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.format, ExportFormat::Html);
        // HTML exports are packaged as ZIP files
        assert_eq!(output.mime_type, "application/zip");
    }

    #[tokio::test]
    async fn test_export_doc_markdown_mime_type() {
        let server = MockServer::start().await;
        let docs_endpoint = docs_endpoint_for(&server);
        let drive_endpoint = drive_endpoint_for(&server);

        // Mock the Drive API export endpoint
        Mock::given(method("GET"))
            .and(path("/v3/files/doc-md/export"))
            .and(header("authorization", "Bearer ya29.test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"# Test Markdown"))
            .mount(&server)
            .await;

        let ctx = test_ctx_with_endpoints(&docs_endpoint, &drive_endpoint);
        let output = export_doc(
            ctx,
            ExportDocInput {
                document_id: "doc-md".to_string(),
                format: Some(ExportFormat::Md),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.format, ExportFormat::Md);
        assert_eq!(output.mime_type, "text/markdown");
    }
}
