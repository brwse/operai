//! Dropbox integration for Operai Toolbox.
//!
//! This integration provides tools for interacting with Dropbox file storage:
//! - Search for files and folders
//! - Download files
//! - Upload files
//! - Create shared links
//! - Move and rename files/folders

use base64::Engine;
use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
mod types;
use types::{
    DropboxDownloadMetadata, DropboxFileMetadata, MoveResponse, MovedMetadata, SearchMetadata,
    SearchResponse, SharedLinkResponse,
};

// Dropbox uses OAuth2 bearer tokens which are ephemeral and may vary
// per-request. Therefore we use define_user_credential! instead of
// define_system_credential!
define_user_credential! {
    DropboxCredential("dropbox") {
        access_token: String,
        #[optional]
        api_base_url: Option<String>,
        #[optional]
        content_base_url: Option<String>,
    }
}

#[derive(Clone)]
struct DropboxClient {
    http: reqwest::Client,
    api_base: reqwest::Url,
    content_base: reqwest::Url,
    access_token: String,
}

impl DropboxClient {
    /// Creates a new Dropbox client from the given credentials.
    ///
    /// # Errors
    ///
    /// Returns an error if the API base URL or content base URL cannot be
    /// parsed as valid URLs.
    fn new(cred: DropboxCredential) -> Result<Self> {
        let http = reqwest::Client::new();
        let api_base = reqwest::Url::parse(
            cred.api_base_url
                .as_deref()
                .unwrap_or("https://api.dropboxapi.com"),
        )?;
        let content_base = reqwest::Url::parse(
            cred.content_base_url
                .as_deref()
                .unwrap_or("https://content.dropboxapi.com"),
        )?;

        Ok(Self {
            http,
            api_base,
            content_base,
            access_token: cred.access_token,
        })
    }

    fn authed(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        req.bearer_auth(&self.access_token)
    }

    /// Constructs a full API URL for the given path.
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be joined to the base URL.
    fn api_url(&self, path: &str) -> Result<reqwest::Url> {
        Ok(self.api_base.join(path)?)
    }

    /// Constructs a full content URL for the given path.
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be joined to the base URL.
    fn content_url(&self, path: &str) -> Result<reqwest::Url> {
        Ok(self.content_base.join(path)?)
    }
}

/// Executes an HTTP request and parses the JSON response.
///
/// # Errors
///
/// Returns an error if:
/// - The HTTP request fails to send or receive a response
/// - The response status code indicates an error (non-2xx)
/// - The response body cannot be parsed as JSON
async fn execute_json<T: serde::de::DeserializeOwned>(
    operation: &str,
    req: reqwest::RequestBuilder,
) -> Result<T> {
    let response = req.send().await?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(operai::anyhow::anyhow!(
            "Dropbox {operation} failed: HTTP {status} {body}"
        ));
    }
    Ok(response.json::<T>().await?)
}

/// Executes an HTTP request and returns the raw response.
///
/// # Errors
///
/// Returns an error if:
/// - The HTTP request fails to send or receive a response
/// - The response status code indicates an error (non-2xx)
async fn execute_bytes(operation: &str, req: reqwest::RequestBuilder) -> Result<reqwest::Response> {
    let response = req.send().await?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(operai::anyhow::anyhow!(
            "Dropbox {operation} failed: HTTP {status} {body}"
        ));
    }
    Ok(response)
}

/// Initialize the Dropbox tool library.
///
/// # Errors
///
/// This function currently never returns an error, but the signature allows for
/// future initialization logic.
#[init]
async fn setup() -> Result<()> {
    info!("Dropbox integration initialized");
    Ok(())
}

/// Clean up resources when the library is unloaded.
#[shutdown]
fn cleanup() {
    info!("Dropbox integration shutting down");
}

// ============================================================================
// Search Tool
// ============================================================================

/// Input for searching files and folders in Dropbox.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchInput {
    /// The search query string (e.g., filename, extension, or content).
    pub query: String,
    /// The path to search within (defaults to root if not specified).
    #[serde(default)]
    pub path: Option<String>,
    /// Maximum number of results to return (1-1000, defaults to 100).
    #[serde(default)]
    pub max_results: Option<u32>,
    /// Filter by file category: "image", "document", "pdf", "spreadsheet",
    /// "audio", "video", "folder", "other".
    #[serde(default)]
    pub file_category: Option<String>,
}

/// Metadata for a file or folder returned from search.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FileMetadata {
    /// The name of the file or folder.
    pub name: String,
    /// The full path in Dropbox.
    pub path_display: String,
    /// The lowercased full path (for comparison).
    pub path_lower: String,
    /// The unique identifier for this file/folder.
    pub id: String,
    /// Whether this is a folder.
    pub is_folder: bool,
    /// File size in bytes (None for folders).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// Last modified time in ISO 8601 format (None for folders).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_modified: Option<String>,
    /// Content hash for the file (None for folders).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
}

/// Output from the search tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchOutput {
    /// List of matching files and folders.
    pub matches: Vec<FileMetadata>,
    /// Whether there are more results available.
    pub has_more: bool,
    /// Cursor for pagination (use in subsequent requests).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// # Search Dropbox Files and Folders
///
/// Searches for files and folders in a Dropbox account by filename, extension,
/// or file content. Use this tool when the user wants to find specific files or
/// folders in their Dropbox.
///
/// The search supports:
/// - **Filename search**: Matches against file and folder names
/// - **Extension search**: Filter by file type (e.g., ".pdf", ".jpg")
/// - **Content search**: Searches within file contents (for supported file
///   types)
/// - **Category filtering**: Restrict results to specific file types
///   (documents, images, PDFs, spreadsheets, audio, video, folders)
/// - **Path scoping**: Limit search to a specific folder hierarchy
///
/// Returns matching files/folders with metadata including path, size,
/// modification date, and content hash. Supports pagination for large result
/// sets (max 1000 results per request).
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - file-storage
/// - dropbox
/// - search
///
/// # Errors
///
/// Returns an error if:
/// - The query string is empty or contains only whitespace
/// - User credentials are not configured
/// - The API base URL or content base URL cannot be parsed
/// - The HTTP request fails or returns a non-success status
/// - The response body cannot be parsed as JSON
#[tool]
pub async fn search(ctx: Context, input: SearchInput) -> Result<SearchOutput> {
    ensure!(!input.query.trim().is_empty(), "query must not be empty");

    let cred = DropboxCredential::get(&ctx)?;
    let client = DropboxClient::new(cred)?;

    let max_results = input.max_results.unwrap_or(100).clamp(1, 1000);

    let mut body = serde_json::json!({
        "query": input.query,
        "options": {
            "path": input.path,
            "max_results": max_results,
        }
    });

    if let Some(category) = input.file_category {
        body["options"]["file_categories"] = serde_json::json!([category]);
    }

    let url = client.api_url("/2/files/search_v2")?;
    let response: SearchResponse = execute_json(
        "files/search_v2",
        client
            .authed(client.http.post(url))
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(&body),
    )
    .await?;

    let matches = response
        .matches
        .into_iter()
        .filter_map(|m| match m.metadata {
            SearchMetadata::File {
                name,
                path_display,
                path_lower,
                id,
                size,
                server_modified,
                content_hash,
            } => Some(FileMetadata {
                name,
                path_display,
                path_lower,
                id,
                is_folder: false,
                size,
                server_modified,
                content_hash,
            }),
            SearchMetadata::Folder {
                name,
                path_display,
                path_lower,
                id,
            } => Some(FileMetadata {
                name,
                path_display,
                path_lower,
                id,
                is_folder: true,
                size: None,
                server_modified: None,
                content_hash: None,
            }),
            SearchMetadata::Other => None,
        })
        .collect();

    Ok(SearchOutput {
        matches,
        has_more: response.has_more,
        cursor: response.cursor,
    })
}

// ============================================================================
// Download Tool
// ============================================================================

/// Input for downloading a file from Dropbox.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DownloadInput {
    /// The path to the file to download (e.g., "/Documents/report.pdf").
    pub path: String,
}

/// Output from the download tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct DownloadOutput {
    /// The file metadata.
    pub metadata: FileMetadata,
    /// Base64-encoded file content.
    pub content_base64: String,
    /// The content type/MIME type of the file.
    pub content_type: String,
}

/// # Download Dropbox File
///
/// Downloads a file from Dropbox and returns its content as a base64-encoded
/// string. Use this tool when the user wants to retrieve the actual content of
/// a file from their Dropbox.
///
/// This tool returns the complete file content (binary or text) encoded in
/// base64 format, along with comprehensive file metadata including filename,
/// path, size, modification date, and content hash. The content type/MIME type
/// is also provided for proper file handling.
///
/// **Important**: This tool downloads the entire file content. For large files,
/// consider the file size before downloading. The output includes the file size
/// in metadata for this purpose.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - file-storage
/// - dropbox
/// - download
///
/// # Errors
///
/// Returns an error if:
/// - The `path` is empty or contains only whitespace
/// - User credentials are not configured
/// - The API base URL or content base URL cannot be parsed
/// - The HTTP request fails or returns a non-success status
/// - The Dropbox-API-Result header is missing
/// - The metadata header cannot be parsed as JSON
/// - The response body cannot be read
#[tool]
pub async fn download(ctx: Context, input: DownloadInput) -> Result<DownloadOutput> {
    ensure!(!input.path.trim().is_empty(), "path must not be empty");

    let cred = DropboxCredential::get(&ctx)?;
    let client = DropboxClient::new(cred)?;

    let url = client.content_url("/2/files/download")?;

    let arg = serde_json::json!({"path": input.path});
    let response = execute_bytes(
        "files/download",
        client
            .authed(client.http.post(url))
            .header("Dropbox-API-Arg", arg.to_string()),
    )
    .await?;

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let metadata_header = response
        .headers()
        .get("Dropbox-API-Result")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            operai::anyhow::anyhow!("Dropbox download missing Dropbox-API-Result header")
        })?;

    let metadata: DropboxDownloadMetadata = serde_json::from_str(metadata_header)?;

    let bytes = response.bytes().await?;
    let content_base64 = base64::engine::general_purpose::STANDARD.encode(bytes);

    Ok(DownloadOutput {
        metadata: FileMetadata {
            name: metadata.name,
            path_display: metadata.path_display,
            path_lower: metadata.path_lower,
            id: metadata.id,
            is_folder: false,
            size: metadata.size,
            server_modified: metadata.server_modified,
            content_hash: metadata.content_hash,
        },
        content_base64,
        content_type,
    })
}

// ============================================================================
// Upload Tool
// ============================================================================

/// Write mode for upload conflicts.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum WriteMode {
    /// Never overwrite existing files.
    #[default]
    Add,
    /// Overwrite existing files.
    Overwrite,
    /// Update only if the given revision matches the latest.
    Update {
        /// The revision to update from.
        rev: String,
    },
}

/// Input for uploading a file to Dropbox.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadInput {
    /// The destination path in Dropbox (e.g., "/Documents/report.pdf").
    pub path: String,
    /// Base64-encoded file content.
    pub content_base64: String,
    /// How to handle conflicts with existing files.
    #[serde(default)]
    pub mode: WriteMode,
    /// If true, files won't trigger desktop notifications.
    #[serde(default)]
    pub mute: bool,
}

/// Output from the upload tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct UploadOutput {
    /// The metadata of the uploaded file.
    pub metadata: FileMetadata,
    /// The revision identifier for this version.
    pub rev: String,
}

/// # Upload Dropbox File
///
/// Uploads a file to Dropbox from base64-encoded content.
/// Use this tool when the user wants to create a new file or update an existing
/// file in their Dropbox.
///
/// This tool accepts file content as a base64-encoded string (supporting both
/// text and binary files) and uploads it to the specified path in Dropbox.
/// Returns the uploaded file's metadata and revision identifier.
///
/// **Upload modes**:
/// - **Add** (default): Never overwrite existing files; will fail if a file
///   already exists at the path
/// - **Overwrite**: Replace any existing file at the destination path
/// - **Update**: Only upload if the file's current revision matches the
///   provided revision (prevents race conditions)
///
/// The `mute` option controls whether desktop notifications are triggered by
/// the upload.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - file-storage
/// - dropbox
/// - upload
///
/// # Errors
///
/// Returns an error if:
/// - The `path` is empty or contains only whitespace
/// - The `content_base64` is empty or contains only whitespace
/// - User credentials are not configured
/// - The API base URL or content base URL cannot be parsed
/// - The base64 content cannot be decoded
/// - The HTTP request fails or returns a non-success status
/// - The response metadata cannot be parsed as JSON
#[tool]
pub async fn upload(ctx: Context, input: UploadInput) -> Result<UploadOutput> {
    ensure!(!input.path.trim().is_empty(), "path must not be empty");
    ensure!(
        !input.content_base64.trim().is_empty(),
        "content_base64 must not be empty"
    );

    let cred = DropboxCredential::get(&ctx)?;
    let client = DropboxClient::new(cred)?;

    let content =
        base64::engine::general_purpose::STANDARD.decode(input.content_base64.as_bytes())?;

    let url = client.content_url("/2/files/upload")?;

    let mode = match input.mode {
        WriteMode::Add => serde_json::json!("add"),
        WriteMode::Overwrite => serde_json::json!("overwrite"),
        WriteMode::Update { rev } => serde_json::json!({"update": rev}),
    };

    let arg = serde_json::json!({
        "path": input.path,
        "mode": mode,
        "mute": input.mute,
    });

    let response: DropboxFileMetadata = execute_json(
        "files/upload",
        client
            .authed(client.http.post(url))
            .header("Dropbox-API-Arg", arg.to_string())
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(content),
    )
    .await?;

    Ok(UploadOutput {
        metadata: FileMetadata {
            name: response.name,
            path_display: response.path_display,
            path_lower: response.path_lower,
            id: response.id,
            is_folder: false,
            size: response.size,
            server_modified: response.server_modified,
            content_hash: response.content_hash,
        },
        rev: response.rev,
    })
}

// ============================================================================
// Share Link Tool
// ============================================================================

/// Visibility settings for a shared link.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum SharedLinkVisibility {
    /// Anyone with the link can access.
    #[default]
    Public,
    /// Only team members can access.
    TeamOnly,
    /// Only specific users can access (requires password).
    Password,
}

/// Input for creating a shared link.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ShareLinkInput {
    /// The path to the file or folder to share.
    pub path: String,
    /// Link visibility setting.
    #[serde(default)]
    pub visibility: SharedLinkVisibility,
    /// Expiration date in ISO 8601 format (optional).
    #[serde(default)]
    pub expires: Option<String>,
    /// Password for password-protected links (required if visibility is
    /// "password").
    #[serde(default)]
    pub password: Option<String>,
}

/// Output from the share link tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ShareLinkOutput {
    /// The shared link URL.
    pub url: String,
    /// The direct download URL (appends ?dl=1).
    pub direct_url: String,
    /// The visibility of the link.
    pub visibility: SharedLinkVisibility,
    /// The expiration date (if set).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    /// Whether the link is password protected.
    pub is_password_protected: bool,
    /// Metadata of the shared file/folder.
    pub metadata: FileMetadata,
}

/// # Create Dropbox Shared Link
///
/// Creates a shared link for a file or folder in Dropbox, allowing others to
/// access the content. Use this tool when the user wants to share a file or
/// folder with others via a URL.
///
/// This tool generates a shareable link with configurable visibility and
/// security options:
/// - **Public**: Anyone with the link can access the file/folder
/// - **Team only**: Only team members (for Dropbox Business teams) can access
/// - **Password protected**: Requires a password for access (password must be
///   provided when using this mode)
///
/// Additional options:
/// - **Expiration date**: Set an optional expiration date in ISO 8601 format
///   (e.g., "2024-12-31T23:59:59Z")
/// - **Direct download URL**: The output includes both the preview URL and a
///   direct download URL
///
/// Returns the shared link URL, direct download URL, visibility settings, and
/// metadata of the shared item.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - file-storage
/// - dropbox
/// - sharing
///
/// # Errors
///
/// Returns an error if:
/// - The `path` is empty or contains only whitespace
/// - Password visibility is selected but no password is provided
/// - User credentials are not configured
/// - The API base URL or content base URL cannot be parsed
/// - The HTTP request fails or returns a non-success status
/// - The response cannot be parsed as JSON
#[tool]
pub async fn share_link(ctx: Context, input: ShareLinkInput) -> Result<ShareLinkOutput> {
    ensure!(!input.path.trim().is_empty(), "path must not be empty");

    let cred = DropboxCredential::get(&ctx)?;
    let client = DropboxClient::new(cred)?;

    let mut settings = serde_json::json!({});
    match input.visibility {
        SharedLinkVisibility::Public => {
            settings["requested_visibility"] = serde_json::json!("public");
        }
        SharedLinkVisibility::TeamOnly => {
            settings["requested_visibility"] = serde_json::json!("team_only");
        }
        SharedLinkVisibility::Password => {
            settings["requested_visibility"] = serde_json::json!("password");
            let password = input.password.clone().ok_or_else(|| {
                operai::anyhow::anyhow!("password is required when visibility is password")
            })?;
            settings["link_password"] = serde_json::json!(password);
        }
    }

    if let Some(expires) = input.expires.clone() {
        settings["expires"] = serde_json::json!(expires);
    }

    let url = client.api_url("/2/sharing/create_shared_link_with_settings")?;
    let response: SharedLinkResponse = execute_json(
        "sharing/create_shared_link_with_settings",
        client
            .authed(client.http.post(url))
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(&serde_json::json!({
                "path": input.path,
                "settings": settings,
            })),
    )
    .await?;

    // Dropbox shared links are usually dl=0; direct download uses dl=1.
    let direct_url = if response.url.contains("dl=0") {
        response.url.replace("dl=0", "dl=1")
    } else if response.url.contains('?') {
        format!("{}&dl=1", response.url)
    } else {
        format!("{}?dl=1", response.url)
    };

    // Best-effort metadata using the input path; callers can fetch actual metadata
    // separately.
    let name = input
        .path
        .split('/')
        .next_back()
        .unwrap_or("item")
        .to_string();
    let is_folder = !name.contains('.');

    let is_password_protected = matches!(input.visibility, SharedLinkVisibility::Password);

    Ok(ShareLinkOutput {
        url: response.url,
        direct_url,
        visibility: input.visibility,
        expires: response.expires.or(input.expires),
        is_password_protected,
        metadata: FileMetadata {
            name,
            path_display: input.path.clone(),
            path_lower: input.path.to_lowercase(),
            id: String::new(),
            is_folder,
            size: None,
            server_modified: None,
            content_hash: None,
        },
    })
}

// ============================================================================
// Move/Rename Tool
// ============================================================================

/// Input for moving or renaming a file/folder.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct MoveRenameInput {
    /// The current path of the file or folder.
    pub from_path: String,
    /// The new path (for move) or new name in the same folder (for rename).
    pub to_path: String,
    /// If true, allows overwriting an existing file at the destination.
    #[serde(default)]
    pub allow_overwrite: bool,
    /// If true and moving a folder, move it along with its contents.
    #[serde(default = "default_true")]
    pub autorename: bool,
}

fn default_true() -> bool {
    true
}

/// Output from the move/rename tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct MoveRenameOutput {
    /// The metadata of the file/folder at its new location.
    pub metadata: FileMetadata,
    /// The original path before the move/rename.
    pub from_path: String,
}

/// # Move or Rename Dropbox File/Folder
///
/// Moves a file or folder to a new location in Dropbox, or renames it within
/// the same location. Use this tool when the user wants to reorganize their
/// Dropbox files or rename items.
///
/// This tool handles both moving and renaming operations:
/// - **Rename**: Change a file/folder name by keeping it in the same folder
///   (e.g., "/old.txt" → "/new.txt")
/// - **Move**: Relocate a file/folder to a different folder (e.g.,
///   "/Documents/file.txt" → "/Archive/file.txt")
/// - **Move and rename**: Combine both operations in one call (e.g.,
///   "/Docs/old.txt" → "/Archive/new.txt")
///
/// **Key behaviors**:
/// - By default, enables `autorename` to avoid conflicts (appends "(1)", "(2)",
///   etc. if destination exists)
/// - The `allow_overwrite` option controls whether existing files at the
///   destination can be replaced
/// - When moving folders, the entire folder structure and contents are moved
/// - Preserves all file metadata and content
///
/// Returns the metadata of the file/folder at its new location and the original
/// path.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - file-storage
/// - dropbox
/// - move
/// - rename
///
/// # Errors
///
/// Returns an error if:
/// - The `from_path` is empty or contains only whitespace
/// - The `to_path` is empty or contains only whitespace
/// - User credentials are not configured
/// - The API base URL or content base URL cannot be parsed
/// - The HTTP request fails or returns a non-success status
/// - The response metadata cannot be parsed as JSON
#[tool]
pub async fn move_rename(ctx: Context, input: MoveRenameInput) -> Result<MoveRenameOutput> {
    ensure!(
        !input.from_path.trim().is_empty(),
        "from_path must not be empty"
    );
    ensure!(
        !input.to_path.trim().is_empty(),
        "to_path must not be empty"
    );
    let original_from_path = input.from_path.clone();

    let cred = DropboxCredential::get(&ctx)?;
    let client = DropboxClient::new(cred)?;

    let url = client.api_url("/2/files/move_v2")?;
    let response: MoveResponse = execute_json(
        "files/move_v2",
        client
            .authed(client.http.post(url))
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(&serde_json::json!({
                "from_path": input.from_path,
                "to_path": input.to_path,
                "allow_shared_folder": true,
                "autorename": input.autorename,
                "allow_ownership_transfer": false,
            })),
    )
    .await?;

    let moved: MovedMetadata = serde_json::from_value(response.metadata)?;

    let metadata = match moved {
        MovedMetadata::File {
            name,
            path_display,
            path_lower,
            id,
            size,
            server_modified,
            content_hash,
        } => FileMetadata {
            name,
            path_display,
            path_lower,
            id,
            is_folder: false,
            size,
            server_modified,
            content_hash,
        },
        MovedMetadata::Folder {
            name,
            path_display,
            path_lower,
            id,
        } => FileMetadata {
            name,
            path_display,
            path_lower,
            id,
            is_folder: true,
            size: None,
            server_modified: None,
            content_hash: None,
        },
    };

    Ok(MoveRenameOutput {
        metadata,
        from_path: original_from_path,
    })
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_dropbox_credential_deserializes_with_access_token_and_optional_endpoints() {
        let json = r#"{
            "access_token": "sl.Bxxxxxxxxxxxxxxxxxx",
            "api_base_url": "https://api.dropboxapi.com",
            "content_base_url": "https://content.dropboxapi.com"
        }"#;
        let cred: DropboxCredential = serde_json::from_str(json).unwrap();
        assert_eq!(cred.access_token, "sl.Bxxxxxxxxxxxxxxxxxx");
        assert_eq!(
            cred.api_base_url.as_deref(),
            Some("https://api.dropboxapi.com")
        );
        assert_eq!(
            cred.content_base_url.as_deref(),
            Some("https://content.dropboxapi.com")
        );
    }

    #[test]
    fn test_search_input_deserializes_with_all_fields() {
        let json = r#"{
            "query": "budget",
            "path": "/Finance",
            "max_results": 50,
            "file_category": "spreadsheet"
        }"#;
        let input: SearchInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.query, "budget");
        assert_eq!(input.path.as_deref(), Some("/Finance"));
        assert_eq!(input.max_results, Some(50));
        assert_eq!(input.file_category.as_deref(), Some("spreadsheet"));
    }

    #[test]
    fn test_write_mode_serializes_correctly() {
        let add = serde_json::to_value(WriteMode::Add).unwrap();
        let overwrite = serde_json::to_value(WriteMode::Overwrite).unwrap();
        let update = serde_json::to_value(WriteMode::Update {
            rev: "abc".to_string(),
        })
        .unwrap();

        assert_eq!(add, json!("add"));
        assert_eq!(overwrite, json!("overwrite"));
        assert_eq!(update, json!({"update": {"rev": "abc"}}));
    }

    #[test]
    fn test_shared_link_visibility_serializes_correctly() {
        let public = serde_json::to_value(SharedLinkVisibility::Public).unwrap();
        let team = serde_json::to_value(SharedLinkVisibility::TeamOnly).unwrap();
        let password = serde_json::to_value(SharedLinkVisibility::Password).unwrap();

        assert_eq!(public, json!("public"));
        assert_eq!(team, json!("team_only"));
        assert_eq!(password, json!("password"));
    }

    // --- Input validation tests ---

    fn test_ctx(api_base: &str, content_base: &str) -> Context {
        use std::collections::HashMap;

        let mut dropbox_values = HashMap::new();
        dropbox_values.insert("access_token".to_string(), "test-token".to_string());
        dropbox_values.insert("api_base_url".to_string(), api_base.to_string());
        dropbox_values.insert("content_base_url".to_string(), content_base.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("dropbox", dropbox_values)
    }

    #[tokio::test]
    async fn test_search_empty_query_returns_error() {
        let ctx = test_ctx(
            "https://api.dropboxapi.com",
            "https://content.dropboxapi.com",
        );
        let result = search(
            ctx,
            SearchInput {
                query: "   ".to_string(),
                path: None,
                max_results: None,
                file_category: None,
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
    async fn test_download_empty_path_returns_error() {
        let ctx = test_ctx(
            "https://api.dropboxapi.com",
            "https://content.dropboxapi.com",
        );
        let result = download(
            ctx,
            DownloadInput {
                path: "   ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("path must not be empty")
        );
    }

    #[tokio::test]
    async fn test_upload_empty_path_returns_error() {
        let ctx = test_ctx(
            "https://api.dropboxapi.com",
            "https://content.dropboxapi.com",
        );
        let result = upload(
            ctx,
            UploadInput {
                path: "   ".to_string(),
                content_base64: "aGVsbG8=".to_string(),
                mode: WriteMode::Add,
                mute: false,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("path must not be empty")
        );
    }

    #[tokio::test]
    async fn test_upload_empty_content_returns_error() {
        let ctx = test_ctx(
            "https://api.dropboxapi.com",
            "https://content.dropboxapi.com",
        );
        let result = upload(
            ctx,
            UploadInput {
                path: "/test.txt".to_string(),
                content_base64: "   ".to_string(),
                mode: WriteMode::Add,
                mute: false,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("content_base64 must not be empty")
        );
    }

    #[tokio::test]
    async fn test_share_link_empty_path_returns_error() {
        let ctx = test_ctx(
            "https://api.dropboxapi.com",
            "https://content.dropboxapi.com",
        );
        let result = share_link(
            ctx,
            ShareLinkInput {
                path: "   ".to_string(),
                visibility: SharedLinkVisibility::Public,
                expires: None,
                password: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("path must not be empty")
        );
    }

    #[tokio::test]
    async fn test_move_rename_empty_from_path_returns_error() {
        let ctx = test_ctx(
            "https://api.dropboxapi.com",
            "https://content.dropboxapi.com",
        );
        let result = move_rename(
            ctx,
            MoveRenameInput {
                from_path: "   ".to_string(),
                to_path: "/new.txt".to_string(),
                allow_overwrite: false,
                autorename: true,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("from_path must not be empty")
        );
    }

    #[tokio::test]
    async fn test_move_rename_empty_to_path_returns_error() {
        let ctx = test_ctx(
            "https://api.dropboxapi.com",
            "https://content.dropboxapi.com",
        );
        let result = move_rename(
            ctx,
            MoveRenameInput {
                from_path: "/old.txt".to_string(),
                to_path: "   ".to_string(),
                allow_overwrite: false,
                autorename: true,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("to_path must not be empty")
        );
    }

    // --- Integration tests with wiremock ---

    #[tokio::test]
    async fn test_search_success_returns_matches() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{body_string_contains, header, method, path},
        };

        let server = MockServer::start().await;
        let api_base = server.uri();
        let content_base = server.uri();

        let response_body = r#"{
            "matches": [
                {
                    "metadata": {
                        ".tag": "file",
                        "name": "report.pdf",
                        "id": "id:abc123",
                        "path_display": "/Documents/report.pdf",
                        "path_lower": "/documents/report.pdf",
                        "size": 12345,
                        "server_modified": "2024-01-15T10:00:00Z",
                        "content_hash": "abc123hash"
                    }
                }
            ],
            "has_more": false
        }"#;

        Mock::given(method("POST"))
            .and(path("/2/files/search_v2"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("budget"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&api_base, &content_base);
        let output = search(
            ctx,
            SearchInput {
                query: "budget".to_string(),
                path: None,
                max_results: Some(10),
                file_category: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.matches.len(), 1);
        assert_eq!(output.matches[0].name, "report.pdf");
        assert_eq!(output.matches[0].path_display, "/Documents/report.pdf");
        assert!(!output.has_more);
    }

    #[tokio::test]
    async fn test_search_error_response_returns_error() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{method, path},
        };

        let server = MockServer::start().await;
        let api_base = server.uri();
        let content_base = server.uri();

        Mock::given(method("POST"))
            .and(path("/2/files/search_v2"))
            .respond_with(ResponseTemplate::new(401).set_body_raw(
                r#"{"error_summary": "invalid_access_token"}"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&api_base, &content_base);
        let result = search(
            ctx,
            SearchInput {
                query: "test".to_string(),
                path: None,
                max_results: None,
                file_category: None,
            },
        )
        .await;

        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("401"));
    }

    #[tokio::test]
    async fn test_upload_success_returns_metadata() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{body_string, header, method, path},
        };

        let server = MockServer::start().await;
        let api_base = server.uri();
        let content_base = server.uri();

        let response_body = r#"{
            "name": "test.txt",
            "id": "id:xyz789",
            "path_display": "/test.txt",
            "path_lower": "/test.txt",
            "size": 5,
            "server_modified": "2024-01-15T10:00:00Z",
            "rev": "abc123"
        }"#;

        Mock::given(method("POST"))
            .and(path("/2/files/upload"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string("hello"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&api_base, &content_base);
        let output = upload(
            ctx,
            UploadInput {
                path: "/test.txt".to_string(),
                content_base64: base64::engine::general_purpose::STANDARD.encode("hello"),
                mode: WriteMode::Add,
                mute: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.metadata.name, "test.txt");
        assert_eq!(output.metadata.path_display, "/test.txt");
        assert_eq!(output.rev, "abc123");
    }

    #[tokio::test]
    async fn test_upload_with_update_mode_sends_correct_json() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{body_string_contains, header, method, path},
        };

        let server = MockServer::start().await;
        let api_base = server.uri();
        let content_base = server.uri();

        let response_body = r#"{
            "name": "test.txt",
            "id": "id:xyz789",
            "path_display": "/test.txt",
            "path_lower": "/test.txt",
            "size": 5,
            "server_modified": "2024-01-15T10:00:00Z",
            "rev": "new_rev_456"
        }"#;

        // Verify that the Dropbox-API-Arg header contains the update mode
        Mock::given(method("POST"))
            .and(path("/2/files/upload"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("hello"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&api_base, &content_base);
        let output = upload(
            ctx,
            UploadInput {
                path: "/test.txt".to_string(),
                content_base64: base64::engine::general_purpose::STANDARD.encode("hello"),
                mode: WriteMode::Update {
                    rev: "rev_abc_123".to_string(),
                },
                mute: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.metadata.name, "test.txt");
        assert_eq!(output.rev, "new_rev_456");
    }
}
