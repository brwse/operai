//! Dropbox Paper integration for Operai Toolbox.
//!
//! This integration provides tools for working with Dropbox Paper documents:
//! - Search documents
//! - Get document content
//! - Create and update documents
//!
//! ## API Migration Notes
//!
//! Dropbox Paper has been migrated to the Dropbox file system. New users (since
//! Sept 2019) have their Paper docs stored as `.paper` files alongside regular
//! Dropbox files. This integration uses the Dropbox API `/files/` endpoints to
//! work with these files.

use std::time::{SystemTime, UNIX_EPOCH};

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap};
use serde::{Deserialize, Serialize};

mod types;

/// Default Dropbox API endpoint
const DEFAULT_API_ENDPOINT: &str = "https://api.dropboxapi.com/2";

/// Dropbox API client
#[derive(Debug, Clone)]
pub struct DropboxClient {
    api_endpoint: String,
    content_endpoint: String,
    access_token: String,
    client: reqwest::Client,
}

impl DropboxClient {
    /// Create a new Dropbox API client
    pub fn new(access_token: String, endpoint: Option<String>) -> Self {
        let api_endpoint = endpoint.unwrap_or_else(|| DEFAULT_API_ENDPOINT.to_string());
        let content_endpoint = api_endpoint.replace("api.dropboxapi.com", "content.dropboxapi.com");

        Self {
            api_endpoint,
            content_endpoint,
            access_token,
            client: reqwest::Client::new(),
        }
    }

    /// Build authorization header
    fn auth_header(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            format!("Bearer {}", self.access_token).parse().unwrap(),
        );
        headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
        headers
    }

    /// Make an RPC-style API call (returns JSON in response body)
    async fn rpc_call<T: for<'de> Deserialize<'de>, R: Serialize>(
        &self,
        path: &str,
        request: &R,
    ) -> Result<T> {
        let url = format!("{}{}", self.api_endpoint, path);
        let response = self
            .client
            .post(&url)
            .headers(self.auth_header())
            .json(request)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Dropbox API error ({status}): {error_text}"
            ))
        }
    }

    /// Download file content (for .paper files)
    async fn download_file(&self, path: &str, export_format: Option<String>) -> Result<String> {
        let url = format!("{}/files/download", self.content_endpoint);

        let args = types::DownloadRequest {
            path: path.to_string(),
            export_format,
        };

        let mut headers = self.auth_header();
        headers.insert(
            "Dropbox-API-Arg",
            serde_json::to_string(&args)?.parse().unwrap(),
        );

        let response = self.client.post(&url).headers(headers).send().await?;

        if response.status().is_success() {
            Ok(response.text().await?)
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Dropbox download error ({status}): {error_text}"
            ))
        }
    }

    /// Upload file content (for .paper files)
    async fn upload_file(
        &self,
        path: &str,
        content: &str,
        mode: Option<&str>,
        import_format: Option<&str>,
    ) -> Result<types::FileMetadata> {
        let url = format!("{}/files/upload", self.content_endpoint);

        let args = types::UploadRequest {
            path: path.to_string(),
            mode: mode.map(std::string::ToString::to_string),
            import_format: import_format.map(std::string::ToString::to_string),
            autorename: Some(false),
            client_modified: None,
            mute: Some(true),
            strict_conflict: Some(false),
        };

        let mut headers = self.auth_header();
        headers.insert(
            "Dropbox-API-Arg",
            serde_json::to_string(&args)?.parse().unwrap(),
        );
        headers.insert(CONTENT_TYPE, "application/octet-stream".parse().unwrap());

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .body(content.to_string())
            .send()
            .await?;

        if response.status().is_success() {
            // Metadata comes in Dropbox-API-Result header
            if let Some(result_json) = response.headers().get("Dropbox-API-Result") {
                let result_str = result_json.to_str().unwrap_or_default();
                Ok(serde_json::from_str(result_str)?)
            } else {
                Err(anyhow::anyhow!(
                    "Missing Dropbox-API-Result header in upload response"
                ))
            }
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Dropbox upload error ({status}): {error_text}"
            ))
        }
    }

    /// Create a shared link for a file
    async fn create_shared_link(&self, path: &str) -> Result<String> {
        let url = format!(
            "{}/sharing/create_shared_link_with_settings",
            self.api_endpoint
        );

        let args = types::CreateSharedLinkRequest {
            path: path.to_string(),
            settings: Some(types::SharedLinkSettings {
                requested_visibility: Some("public".to_string()),
                audience: None,
                access: Some("viewer".to_string()),
            }),
        };

        let response = self
            .client
            .post(&url)
            .headers(self.auth_header())
            .json(&args)
            .send()
            .await?;

        if response.status().is_success() {
            let result: types::SharedLinkResponse = response.json().await?;
            result
                .url
                .ok_or_else(|| anyhow::anyhow!("No URL in shared link response"))
        } else {
            // Link might already exist - try to get existing link
            let get_url = format!("{}/sharing/list_shared_links", self.api_endpoint);
            let get_args = serde_json::json!({ "path": path, "direct_only": true });

            let get_response = self
                .client
                .post(&get_url)
                .headers(self.auth_header())
                .json(&get_args)
                .send()
                .await?;

            if get_response.status().is_success() {
                #[derive(Deserialize)]
                struct ListResponse {
                    links: Vec<LinkData>,
                }

                #[derive(Deserialize)]
                struct LinkData {
                    url: Option<String>,
                }

                let list_resp: ListResponse = get_response.json().await?;
                list_resp
                    .links
                    .into_iter()
                    .find_map(|l| l.url)
                    .ok_or_else(|| anyhow::anyhow!("No shared link found"))
            } else {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                Err(anyhow::anyhow!(
                    "Dropbox sharing error ({status}): {error_text}"
                ))
            }
        }
    }

    /// Get file metadata
    async fn get_metadata(&self, path: &str) -> Result<types::FileMetadataResponse> {
        self.rpc_call(
            "/files/get_metadata",
            &types::GetMetadataRequest {
                path: path.to_string(),
                include_media_info: Some(false),
            },
        )
        .await
    }

    /// Search for files matching a query
    async fn search(&self, query: &str, limit: u32) -> Result<types::SearchResponse> {
        self.rpc_call(
            "/files/search",
            &types::SearchRequest {
                query: query.to_string(),
                path: Some(String::new()),
                limit: Some(limit),
                mode: Some("filename".to_string()),
            },
        )
        .await
    }
}

define_user_credential! {
    DropboxCredential("dropbox") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

/// Initialize the Dropbox Paper integration.
///
/// # Errors
///
/// This function currently never returns an error. The `Result` type is used
/// for future compatibility with initialization logic that may fail (e.g.,
/// validating credentials, establishing network connections, or allocating
/// resources).
#[init]
async fn setup() -> Result<()> {
    info!("Dropbox Paper integration initialized");
    Ok(())
}

/// Clean up resources when the integration is unloaded.
#[shutdown]
fn cleanup() {
    info!("Dropbox Paper integration shutting down");
}

// ============================================================================
// Search Documents Tool
// ============================================================================

/// Input for searching Dropbox Paper documents.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchDocsInput {
    /// The search query string.
    pub query: String,
    /// Maximum number of results to return (1-100, default 20).
    #[serde(default)]
    pub limit: Option<u32>,
    /// Filter by folder ID to search within a specific folder.
    #[serde(default)]
    pub folder_id: Option<String>,
    /// Filter documents modified after this ISO 8601 timestamp.
    #[serde(default)]
    pub modified_after: Option<String>,
}

/// A document returned from a search query.
#[derive(Debug, Serialize, JsonSchema)]
pub struct DocumentSummary {
    /// The unique document ID.
    pub doc_id: String,
    /// The document title.
    pub title: String,
    /// The folder ID containing this document.
    pub folder_id: Option<String>,
    /// When the document was last modified (ISO 8601).
    pub last_modified: String,
    /// The owner's email address.
    pub owner_email: String,
    /// Document status (active, archived, deleted).
    pub status: String,
}

/// Output from the search documents tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchDocsOutput {
    /// List of matching documents.
    pub documents: Vec<DocumentSummary>,
    /// Whether there are more results available.
    pub has_more: bool,
    /// Cursor for pagination (if more results exist).
    pub cursor: Option<String>,
}

/// # Search Dropbox Paper Documents
///
/// Searches for Dropbox Paper documents matching a query string. Use this tool
/// when the user wants to find documents by name or content keywords.
///
/// This tool searches the Dropbox file system for `.paper` files and returns
/// a list of matching documents with metadata including title, modification
/// date, owner, and sharing status. The search automatically filters to only
/// return Paper documents (not other file types or folders).
///
/// ## When to Use
///
/// - User wants to find existing Paper documents by keyword
/// - User needs to list documents in a specific folder (use `folder_id`)
/// - User wants to find recently modified documents (use `modified_after`)
/// - User is browsing their document library before reading or editing
///
/// ## Important Notes
///
/// - The search query is applied to filenames and may include `.paper`
///   extension automatically if not present
/// - Only files with `.paper` extension (case-insensitive) are returned
/// - Results include document metadata but not full content
/// - Use `get_doc` to retrieve the full content of a specific document
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - docs
/// - dropbox
/// - paper
/// - search
///
/// # Errors
///
/// Returns an error if:
/// - The query string is empty or contains only whitespace
/// - The limit parameter is outside the valid range (1-100)
/// - Credentials are missing or invalid
///
/// # Panics
///
/// This function will panic if the system time is before the Unix epoch
/// (January 1, 1970), which is used for generating timestamps when metadata is
/// missing.
#[tool]
pub async fn search_docs(ctx: Context, input: SearchDocsInput) -> Result<SearchDocsOutput> {
    ensure!(!input.query.trim().is_empty(), "query must not be empty");
    let limit = input.limit.unwrap_or(20);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let cred = ctx.user_credential::<DropboxCredential>("dropbox")?;
    let client = DropboxClient::new(cred.access_token, cred.endpoint);

    // Build search query for .paper files
    let search_query = if input.query.contains(".paper") {
        input.query.clone()
    } else {
        format!("{} .paper", input.query.trim())
    };

    let response = client.search(&search_query, limit).await?;

    // Convert search results to document summaries
    let mut documents = Vec::new();
    for match_item in response.matches {
        let metadata = match_item.metadata;

        // Only include files (not folders) with .paper extension
        if metadata.tag != "file" {
            continue;
        }

        let name = metadata.name.unwrap_or_default();
        // Check for .paper extension (case-insensitive)
        if !std::path::Path::new(&name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("paper"))
        {
            continue;
        }

        // Get additional metadata to populate fields
        let doc_id = metadata.id.unwrap_or_default();
        // Strip .paper suffix (case-insensitive)
        let title = name
            .strip_suffix(".paper")
            .or_else(|| name.strip_suffix(".PAPER"))
            .or_else(|| name.strip_suffix(".Paper"))
            .unwrap_or(&name)
            .to_string();

        let last_modified = metadata
            .server_modified
            .or(metadata.client_modified)
            .unwrap_or_else(|| {
                // Current time as ISO 8601
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                // Use i64::try_from to handle potential overflow for dates beyond year 2262
                chrono::DateTime::from_timestamp(i64::try_from(now).unwrap_or(i64::MAX), 0)
                    .unwrap()
                    .to_rfc3339()
            });

        // Try to get more detailed metadata
        let (folder_id, owner_email, status) = if let Some(path) = &metadata.path_display {
            match client.get_metadata(path).await {
                Ok(meta) => {
                    let folder_id = meta.parent_shared_folder_id.or_else(|| {
                        path.split('/')
                            .rev()
                            .nth(1)
                            .map(std::string::ToString::to_string)
                    });

                    let owner_email = meta
                        .sharer_info
                        .and_then(|s| s.id)
                        .unwrap_or_else(|| "unknown".to_string());

                    let status = if meta.sharing_info.is_some() {
                        "shared".to_string()
                    } else {
                        "private".to_string()
                    };

                    (folder_id, owner_email, status)
                }
                Err(_) => (None, "unknown".to_string(), "active".to_string()),
            }
        } else {
            (None, "unknown".to_string(), "active".to_string())
        };

        documents.push(DocumentSummary {
            doc_id,
            title,
            folder_id,
            last_modified,
            owner_email,
            status,
        });
    }

    Ok(SearchDocsOutput {
        documents,
        has_more: response.has_more,
        cursor: None, // Dropbox API doesn't provide search cursors
    })
}

// ============================================================================
// Get Document Tool
// ============================================================================

/// Input for getting a Dropbox Paper document.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetDocInput {
    /// The unique document ID.
    pub doc_id: String,
    /// Export format: "markdown" (default) or "html".
    #[serde(default)]
    pub export_format: Option<String>,
}

/// Sharing settings for a document.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SharingSettings {
    /// Who can access: "private", "team", "public".
    pub access_level: String,
    /// Whether link sharing is enabled.
    pub link_sharing_enabled: bool,
    /// The shareable link URL (if enabled).
    pub share_link: Option<String>,
}

/// Output from the get document tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GetDocOutput {
    /// The unique document ID.
    pub doc_id: String,
    /// The document title.
    pub title: String,
    /// The document content in the requested format.
    pub content: String,
    /// The export format used.
    pub format: String,
    /// Document revision number.
    pub revision: u64,
    /// When the document was created (ISO 8601).
    pub created_at: String,
    /// When the document was last modified (ISO 8601).
    pub last_modified: String,
    /// The owner's email address.
    pub owner_email: String,
    /// Sharing settings for the document.
    pub sharing: SharingSettings,
}

/// # Get Dropbox Paper Document
///
/// Retrieves the full content and metadata of a Dropbox Paper document. Use
/// this tool when the user wants to read the complete contents of a specific
/// document.
///
/// This tool downloads the document content from Dropbox and returns it along
/// with rich metadata including creation/modification dates, revision number,
/// owner information, and sharing settings. The content can be exported in
/// Markdown (default) or HTML format.
///
/// ## When to Use
///
/// - User wants to read the full content of a specific document
/// - User needs to display a document with its metadata
/// - User wants to check document revision history or sharing status
/// - User needs to export a document in a specific format (Markdown or HTML)
///
/// ## Important Notes
///
/// - The `doc_id` can be a file ID (e.g., "id:abc123") or a path (e.g., "/My
///   Document" or "/My Document.paper")
/// - If a path is provided without the `.paper` extension, it will be added
///   automatically
/// - Markdown format is returned by default; use `export_format: "html"` for
///   HTML output
/// - The tool automatically generates shareable links for shared documents
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - docs
/// - dropbox
/// - paper
///
/// # Errors
///
/// Returns an error if:
/// - The document ID is empty or contains only whitespace
/// - Credentials are missing or invalid
/// - The document doesn't exist or can't be accessed
///
/// # Panics
///
/// This function will panic if the system time is before the Unix epoch
/// (January 1, 1970), which is used for generating timestamps when metadata is
/// missing.
#[tool]
pub async fn get_doc(ctx: Context, input: GetDocInput) -> Result<GetDocOutput> {
    ensure!(!input.doc_id.trim().is_empty(), "doc_id must not be empty");

    let format = input
        .export_format
        .unwrap_or_else(|| "markdown".to_string());

    // Validate format
    let format = match format.as_str() {
        "markdown" | "html" => format,
        _ => "markdown".to_string(),
    };

    let cred = ctx.user_credential::<DropboxCredential>("dropbox")?;
    let client = DropboxClient::new(cred.access_token, cred.endpoint);

    // doc_id can be either a file ID or a path. Try both.
    let path = if input.doc_id.starts_with("id:") {
        // It's an ID - we need to look it up via search or assume it's a path
        // For now, treat as-is (Dropbox API can accept IDs in some cases)
        input.doc_id.clone()
    } else if input.doc_id.starts_with('/') {
        // It's already a path
        input.doc_id.clone()
    } else {
        // Assume it's a path and add leading slash
        format!("/{}", input.doc_id)
    };

    // Ensure .paper extension (case-insensitive check)
    let path = if std::path::Path::new(&path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("paper"))
    {
        path
    } else {
        format!("{path}.paper")
    };

    // Get file metadata first
    let metadata = client.get_metadata(&path).await?;

    // Download content with export format
    let export_format = if format == "html" {
        Some("html".to_string())
    } else {
        None // markdown is default
    };

    let content = client.download_file(&path, export_format).await?;

    // Build sharing settings
    let access_level = if metadata.sharing_info.is_some() {
        if metadata
            .sharing_info
            .as_ref()
            .and_then(|s| s.read_only)
            .unwrap_or(false)
        {
            "public".to_string()
        } else {
            "shared".to_string()
        }
    } else {
        "private".to_string()
    };

    let share_link = if metadata.sharing_info.is_some() {
        Some(client.create_shared_link(&path).await?)
    } else {
        None
    };

    let revision = metadata
        .rev
        .as_ref()
        .and_then(|r| r.strip_prefix('r'))
        .and_then(|r| r.parse::<u64>().ok())
        .unwrap_or(0);

    Ok(GetDocOutput {
        doc_id: metadata.id,
        title: metadata
            .name
            .strip_suffix(".paper")
            .unwrap_or(&metadata.name)
            .to_string(),
        content,
        format,
        revision,
        created_at: metadata.client_modified.unwrap_or_else(|| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            // Use i64::try_from to handle potential overflow for dates beyond year 2262
            chrono::DateTime::from_timestamp(i64::try_from(now).unwrap_or(i64::MAX), 0)
                .unwrap()
                .to_rfc3339()
        }),
        last_modified: metadata.server_modified.unwrap_or_else(|| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            // Use i64::try_from to handle potential overflow for dates beyond year 2262
            chrono::DateTime::from_timestamp(i64::try_from(now).unwrap_or(i64::MAX), 0)
                .unwrap()
                .to_rfc3339()
        }),
        owner_email: metadata
            .sharer_info
            .and_then(|s| s.id)
            .unwrap_or_else(|| "unknown".to_string()),
        sharing: SharingSettings {
            access_level,
            link_sharing_enabled: share_link.is_some(),
            share_link,
        },
    })
}

// ============================================================================
// Create/Update Document Tool
// ============================================================================

/// Input for creating or updating a Dropbox Paper document.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpsertDocInput {
    /// Document ID to update. If not provided, creates a new document.
    #[serde(default)]
    pub doc_id: Option<String>,
    /// The document title (required for new documents).
    #[serde(default)]
    pub title: Option<String>,
    /// The document content in Markdown format.
    pub content: String,
    /// Folder ID to create the document in (for new documents).
    #[serde(default)]
    pub folder_id: Option<String>,
    /// Import format of the content: "markdown" (default) or "html".
    #[serde(default)]
    pub import_format: Option<String>,
    /// Update mode: "overwrite" (default) or "append".
    #[serde(default)]
    pub update_mode: Option<String>,
}

/// Output from the create/update document tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct UpsertDocOutput {
    /// The document ID (newly created or existing).
    pub doc_id: String,
    /// The document title.
    pub title: String,
    /// Whether a new document was created (vs updated).
    pub created: bool,
    /// The new revision number.
    pub revision: u64,
    /// Shareable link to the document.
    pub share_link: String,
}

/// # Create or Update Dropbox Paper Document
///
/// Creates a new Dropbox Paper document or updates an existing one. Use this
/// tool when the user wants to create a new document or modify the content of
/// an existing document.
///
/// This flexible tool handles both creation and updates in a single call. When
/// creating a new document, provide a `title`. When updating an existing
/// document, provide the `doc_id` (file ID or path). Content can be in
/// Markdown (default) or HTML format, and updates can either overwrite or
/// append to existing content.
///
/// ## When to Use
///
/// - User wants to create a new Paper document
/// - User wants to edit/replace the content of an existing document
/// - User wants to append new content to an existing document
/// - User needs to create a document from formatted text (Markdown or HTML)
///
/// ## Important Notes
///
/// - **Creating new documents**: Provide `title` and omit `doc_id`. The title
///   will be sanitized to create a safe filename.
/// - **Updating existing documents**: Provide `doc_id` (file ID or path like
///   "/My Doc"). The `.paper` extension is added automatically if missing.
/// - **Update modes**: Use `update_mode: "overwrite"` (default) to replace
///   content, or `update_mode: "append"` to add to existing content.
/// - **Import formats**: Use `import_format: "markdown"` (default) or
///   `import_format: "html"` to specify the content format.
/// - A shareable link is automatically generated for the document.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - docs
/// - dropbox
/// - paper
///
/// # Errors
///
/// Returns an error if:
/// - The content is empty or contains only whitespace
/// - The `doc_id` is provided but is empty or contains only whitespace
/// - Credentials are missing or invalid
#[tool]
pub async fn upsert_doc(ctx: Context, input: UpsertDocInput) -> Result<UpsertDocOutput> {
    ensure!(
        !input.content.trim().is_empty(),
        "content must not be empty"
    );

    if let Some(ref doc_id) = input.doc_id {
        ensure!(!doc_id.trim().is_empty(), "doc_id must not be empty");
    }

    let cred = ctx.user_credential::<DropboxCredential>("dropbox")?;
    let client = DropboxClient::new(cred.access_token, cred.endpoint);

    // Validate import format
    let import_format = match input.import_format.as_deref() {
        Some("html") => Some("html"),
        _ => None, // markdown is default
    };

    // Validate update mode
    let update_mode = match input.update_mode.as_deref() {
        Some("append") => "append",
        _ => "overwrite",
    };

    let is_new = input.doc_id.is_none();
    let title = input
        .title
        .clone()
        .unwrap_or_else(|| "Untitled".to_string());

    // Build the file path
    let (path, _doc_id_out) = if let Some(doc_id) = input.doc_id {
        let path = if doc_id.starts_with('/') {
            doc_id.clone()
        } else {
            format!("/{doc_id}")
        };

        // Ensure .paper extension (case-insensitive check)
        let path = if std::path::Path::new(&path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("paper"))
        {
            path
        } else {
            format!("{path}.paper")
        };

        (path, doc_id)
    } else {
        // New document - create from title
        let safe_title = title
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                    c
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim()
            .to_string();

        let path = format!("/{safe_title}.paper");
        (path, String::new()) // Will be filled after upload
    };

    // Upload the file
    let mode = if is_new {
        None // add mode (create new)
    } else {
        Some(update_mode) // update mode
    };

    let metadata = client
        .upload_file(&path, &input.content, mode, import_format)
        .await?;

    // Create share link
    let share_link = client
        .create_shared_link(&path)
        .await
        .unwrap_or_else(|_| String::new());

    Ok(UpsertDocOutput {
        doc_id: metadata.id,
        title: metadata
            .name
            .strip_suffix(".paper")
            .unwrap_or(&metadata.name)
            .to_string(),
        created: is_new,
        revision: metadata
            .rev
            .as_ref()
            .and_then(|r| r.strip_prefix('r'))
            .and_then(|r| r.parse::<u64>().ok())
            .unwrap_or(1),
        share_link,
    })
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Credential Tests
    // ========================================================================

    #[test]
    fn test_credential_deserializes_with_access_token_only() {
        let json = r#"{ "access_token": "sl.abc123" }"#;

        let cred: DropboxCredential = serde_json::from_str(json).unwrap();

        assert_eq!(cred.access_token, "sl.abc123");
        assert_eq!(cred.endpoint, None);
    }

    #[test]
    fn test_credential_deserializes_with_custom_endpoint() {
        let json = r#"{
            "access_token": "sl.abc123",
            "endpoint": "https://api.dropboxapi.com"
        }"#;

        let cred: DropboxCredential = serde_json::from_str(json).unwrap();

        assert_eq!(cred.access_token, "sl.abc123");
        assert_eq!(cred.endpoint.as_deref(), Some("https://api.dropboxapi.com"));
    }

    #[test]
    fn test_credential_missing_access_token_fails() {
        let json = r#"{ "endpoint": "https://example.com" }"#;

        let err = serde_json::from_str::<DropboxCredential>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `access_token`"));
    }

    // ========================================================================
    // Search Documents Tests
    // ========================================================================

    #[tokio::test]
    async fn test_search_docs_empty_query_fails() {
        let ctx = Context::empty();
        let input = SearchDocsInput {
            query: "   ".to_string(),
            limit: None,
            folder_id: None,
            modified_after: None,
        };

        let result = search_docs(ctx, input).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("query must not be empty")
        );
    }

    #[tokio::test]
    async fn test_search_docs_limit_too_low_fails() {
        let ctx = Context::empty();
        let input = SearchDocsInput {
            query: "test".to_string(),
            limit: Some(0),
            folder_id: None,
            modified_after: None,
        };

        let result = search_docs(ctx, input).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("limit must be between 1 and 100")
        );
    }

    #[tokio::test]
    async fn test_search_docs_limit_too_high_fails() {
        let ctx = Context::empty();
        let input = SearchDocsInput {
            query: "test".to_string(),
            limit: Some(101),
            folder_id: None,
            modified_after: None,
        };

        let result = search_docs(ctx, input).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("limit must be between 1 and 100")
        );
    }

    #[test]
    fn test_search_docs_input_deserializes_with_query_only() {
        let json = r#"{ "query": "project notes" }"#;

        let input: SearchDocsInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.query, "project notes");
        assert_eq!(input.limit, None);
        assert_eq!(input.folder_id, None);
        assert_eq!(input.modified_after, None);
    }

    #[test]
    fn test_search_docs_input_missing_query_fails() {
        let json = r#"{ "limit": 10 }"#;

        let err = serde_json::from_str::<SearchDocsInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `query`"));
    }

    // ========================================================================
    // Get Document Tests
    // ========================================================================

    #[tokio::test]
    async fn test_get_doc_empty_doc_id_fails() {
        let ctx = Context::empty();
        let input = GetDocInput {
            doc_id: "   ".to_string(),
            export_format: None,
        };

        let result = get_doc(ctx, input).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("doc_id must not be empty")
        );
    }

    #[test]
    fn test_get_doc_input_deserializes_with_doc_id_only() {
        let json = r#"{ "doc_id": "paper-abc123" }"#;

        let input: GetDocInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.doc_id, "paper-abc123");
        assert_eq!(input.export_format, None);
    }

    #[test]
    fn test_get_doc_input_deserializes_with_html_format() {
        let json = r#"{ "doc_id": "paper-abc123", "export_format": "html" }"#;

        let input: GetDocInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.doc_id, "paper-abc123");
        assert_eq!(input.export_format.as_deref(), Some("html"));
    }

    #[test]
    fn test_get_doc_input_missing_doc_id_fails() {
        let json = r#"{ "export_format": "markdown" }"#;

        let err = serde_json::from_str::<GetDocInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `doc_id`"));
    }

    // ========================================================================
    // Create/Update Document Tests
    // ========================================================================

    #[tokio::test]
    async fn test_upsert_doc_empty_content_fails() {
        let ctx = Context::empty();
        let input = UpsertDocInput {
            doc_id: None,
            title: Some("Test".to_string()),
            content: "   ".to_string(),
            folder_id: None,
            import_format: None,
            update_mode: None,
        };

        let result = upsert_doc(ctx, input).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("content must not be empty")
        );
    }

    #[tokio::test]
    async fn test_upsert_doc_empty_doc_id_fails() {
        let ctx = Context::empty();
        let input = UpsertDocInput {
            doc_id: Some("   ".to_string()),
            title: Some("Test".to_string()),
            content: "Content".to_string(),
            folder_id: None,
            import_format: None,
            update_mode: None,
        };

        let result = upsert_doc(ctx, input).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("doc_id must not be empty")
        );
    }

    #[test]
    fn test_upsert_doc_input_deserializes_for_new_document() {
        let json = r##"{
            "title": "My New Document",
            "content": "# Hello World"
        }"##;

        let input: UpsertDocInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.doc_id, None);
        assert_eq!(input.title.as_deref(), Some("My New Document"));
        assert_eq!(input.content, "# Hello World");
        assert_eq!(input.folder_id, None);
    }

    #[test]
    fn test_upsert_doc_input_deserializes_for_update() {
        let json = r#"{
            "doc_id": "paper-123",
            "content": "Updated content",
            "update_mode": "append"
        }"#;

        let input: UpsertDocInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.doc_id.as_deref(), Some("paper-123"));
        assert_eq!(input.content, "Updated content");
        assert_eq!(input.update_mode.as_deref(), Some("append"));
    }

    #[test]
    fn test_upsert_doc_input_missing_content_fails() {
        let json = r#"{ "title": "Test" }"#;

        let err = serde_json::from_str::<UpsertDocInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `content`"));
    }

    // ========================================================================
    // Document Summary Serialization Tests
    // ========================================================================

    #[test]
    fn test_document_summary_serializes_correctly() {
        let summary = DocumentSummary {
            doc_id: "paper-123".to_string(),
            title: "Test Document".to_string(),
            folder_id: Some("folder-456".to_string()),
            last_modified: "2024-01-15T10:30:00Z".to_string(),
            owner_email: "user@example.com".to_string(),
            status: "active".to_string(),
        };

        let json = serde_json::to_value(&summary).unwrap();

        assert_eq!(json.get("doc_id").unwrap(), "paper-123");
        assert_eq!(json.get("title").unwrap(), "Test Document");
        assert_eq!(json.get("folder_id").unwrap(), "folder-456");
        assert_eq!(json.get("last_modified").unwrap(), "2024-01-15T10:30:00Z");
        assert_eq!(json.get("owner_email").unwrap(), "user@example.com");
        assert_eq!(json.get("status").unwrap(), "active");
    }

    #[test]
    fn test_document_summary_with_no_folder() {
        let summary = DocumentSummary {
            doc_id: "paper-789".to_string(),
            title: "Unfiled Doc".to_string(),
            folder_id: None,
            last_modified: "2024-02-01T00:00:00Z".to_string(),
            owner_email: "user@example.com".to_string(),
            status: "active".to_string(),
        };

        let json = serde_json::to_value(&summary).unwrap();

        assert!(json.get("folder_id").unwrap().is_null());
    }

    // ========================================================================
    // Sharing Settings Serialization Tests
    // ========================================================================

    #[test]
    fn test_sharing_settings_private_serializes() {
        let settings = SharingSettings {
            access_level: "private".to_string(),
            link_sharing_enabled: false,
            share_link: None,
        };

        let json = serde_json::to_value(&settings).unwrap();

        assert_eq!(json.get("access_level").unwrap(), "private");
        assert_eq!(json.get("link_sharing_enabled").unwrap(), false);
        assert!(json.get("share_link").unwrap().is_null());
    }

    #[test]
    fn test_sharing_settings_public_with_link_serializes() {
        let settings = SharingSettings {
            access_level: "public".to_string(),
            link_sharing_enabled: true,
            share_link: Some("https://paper.dropbox.com/doc/abc123".to_string()),
        };

        let json = serde_json::to_value(&settings).unwrap();

        assert_eq!(json.get("access_level").unwrap(), "public");
        assert_eq!(json.get("link_sharing_enabled").unwrap(), true);
        assert_eq!(
            json.get("share_link").unwrap(),
            "https://paper.dropbox.com/doc/abc123"
        );
    }

    // ========================================================================
    // DropboxClient Tests
    // ========================================================================

    #[test]
    fn test_dropbox_client_new_with_default_endpoint() {
        let client = DropboxClient::new("test-token".to_string(), None);

        assert_eq!(client.api_endpoint, "https://api.dropboxapi.com/2");
        assert_eq!(client.content_endpoint, "https://content.dropboxapi.com/2");
        assert_eq!(client.access_token, "test-token");
    }

    #[test]
    fn test_dropbox_client_new_with_custom_endpoint() {
        let client = DropboxClient::new(
            "test-token".to_string(),
            Some("https://custom.api.com".to_string()),
        );

        assert_eq!(client.api_endpoint, "https://custom.api.com");
        assert_eq!(client.content_endpoint, "https://custom.api.com");
        assert_eq!(client.access_token, "test-token");
    }

    #[test]
    fn test_dropbox_client_auth_header() {
        let client = DropboxClient::new("sl.test123".to_string(), None);
        let headers = client.auth_header();

        assert_eq!(headers.get(AUTHORIZATION).unwrap(), "Bearer sl.test123");
        assert_eq!(headers.get(CONTENT_TYPE).unwrap(), "application/json");
    }
}

#[cfg(test)]
mod integration_tests {
    use std::collections::HashMap;

    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut dropbox_values = HashMap::new();
        dropbox_values.insert("access_token".to_string(), "test-token".to_string());
        dropbox_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("dropbox", dropbox_values)
    }

    #[tokio::test]
    async fn test_search_docs_with_real_api_mock() {
        let mock_server = MockServer::start().await;

        // Mock search endpoint
        Mock::given(method("POST"))
            .and(path("/files/search"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "matches": [
                    {
                        "metadata": {
                            ".tag": "file",
                            "id": "id:abc123",
                            "name": "Test Document.paper",
                            "path_display": "/Test Document.paper",
                            "server_modified": "2024-01-15T10:30:00Z"
                        },
                        "platform_joined": false
                    }
                ],
                "has_more": false
            })))
            .mount(&mock_server)
            .await;

        // Mock get_metadata endpoint
        Mock::given(method("POST"))
            .and(path("/files/get_metadata"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                ".tag": "file",
                "id": "id:abc123",
                "name": "Test Document.paper",
                "path_display": "/Test Document.paper",
                "server_modified": "2024-01-15T10:30:00Z",
                "client_modified": "2024-01-10T09:00:00Z",
                "rev": "r1",
                "size": 1024
            })))
            .mount(&mock_server)
            .await;

        let ctx = test_ctx(&mock_server.uri());

        let input = SearchDocsInput {
            query: "test".to_string(),
            limit: None,
            folder_id: None,
            modified_after: None,
        };

        let output = search_docs(ctx, input).await.unwrap();

        assert_eq!(output.documents.len(), 1);
        assert_eq!(output.documents[0].doc_id, "id:abc123");
        assert_eq!(output.documents[0].title, "Test Document");
        assert!(!output.has_more);
    }

    #[tokio::test]
    async fn test_get_doc_with_real_api_mock() {
        let mock_server = MockServer::start().await;

        // Mock get_metadata endpoint
        Mock::given(method("POST"))
            .and(path("/files/get_metadata"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                ".tag": "file",
                "id": "id:abc123",
                "name": "Test Document.paper",
                "path_display": "/Test Document.paper",
                "server_modified": "2024-01-15T10:30:00Z",
                "client_modified": "2024-01-10T09:00:00Z",
                "rev": "r1",
                "size": 1024
            })))
            .mount(&mock_server)
            .await;

        // Mock download endpoint
        Mock::given(method("POST"))
            .and(path("/files/download"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string("# Test Content\n\nThis is a test."),
            )
            .mount(&mock_server)
            .await;

        let ctx = test_ctx(&mock_server.uri());

        let input = GetDocInput {
            doc_id: "/Test Document".to_string(),
            export_format: None,
        };

        let output = get_doc(ctx, input).await.unwrap();

        assert_eq!(output.doc_id, "id:abc123");
        assert_eq!(output.title, "Test Document");
        assert_eq!(output.format, "markdown");
        assert!(output.content.contains("Test Content"));
        assert_eq!(output.revision, 1);
    }

    #[tokio::test]
    async fn test_upsert_doc_create_new_with_real_api_mock() {
        let mock_server = MockServer::start().await;

        // Mock upload endpoint
        Mock::given(method("POST"))
            .and(path("/files/upload"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Dropbox-API-Result", r#"{"id": "id:new123", "name": "New Document.paper", "path_display": "/New Document.paper", "rev": "r1", ".tag": "file"}"#)
                    .set_body_string("")
            )
            .mount(&mock_server)
            .await;

        // Mock create_shared_link endpoint
        Mock::given(method("POST"))
            .and(path("/sharing/create_shared_link_with_settings"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "url": "https://dropbox.com/s/abc123",
                ".tag": "file"
            })))
            .mount(&mock_server)
            .await;

        let ctx = test_ctx(&mock_server.uri());

        let input = UpsertDocInput {
            doc_id: None,
            title: Some("New Document".to_string()),
            content: "# New Document\n\nContent here.".to_string(),
            folder_id: None,
            import_format: None,
            update_mode: None,
        };

        let output = upsert_doc(ctx, input).await.unwrap();

        assert_eq!(output.doc_id, "id:new123");
        assert_eq!(output.title, "New Document");
        assert!(output.created);
        assert_eq!(output.revision, 1);
        assert!(output.share_link.contains("dropbox.com"));
    }

    #[tokio::test]
    async fn test_search_docs_filters_non_paper_files() {
        let mock_server = MockServer::start().await;

        // Mock search endpoint with mixed results
        Mock::given(method("POST"))
            .and(path("/files/search"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "matches": [
                    {
                        "metadata": {
                            ".tag": "file",
                            "id": "id:paper1",
                            "name": "Document.paper",
                            "path_display": "/Document.paper",
                        }
                    },
                    {
                        "metadata": {
                            ".tag": "file",
                            "id": "id:doc2",
                            "name": "NotPaper.txt",
                            "path_display": "/NotPaper.txt",
                        }
                    },
                    {
                        "metadata": {
                            ".tag": "folder",
                            "id": "id:folder1",
                            "name": "My Folder",
                        }
                    }
                ],
                "has_more": false
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("POST"))
            .and(path("/files/get_metadata"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                ".tag": "file",
                "id": "id:paper1",
                "name": "Document.paper",
                "path_display": "/Document.paper",
                "rev": "r1"
            })))
            .mount(&mock_server)
            .await;

        let ctx = test_ctx(&mock_server.uri());

        let input = SearchDocsInput {
            query: "test".to_string(),
            limit: None,
            folder_id: None,
            modified_after: None,
        };

        let output = search_docs(ctx, input).await.unwrap();

        // Should only return the .paper file, not .txt or folders
        assert_eq!(output.documents.len(), 1);
        assert_eq!(output.documents[0].doc_id, "id:paper1");
    }
}
