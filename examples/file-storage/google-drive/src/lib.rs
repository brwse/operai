//! file-storage/google-drive integration for Operai Toolbox.

mod types;

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
use types::{DriveFile, FileListResponse, Permission, PermissionRole, PermissionType};

define_user_credential! {
    GoogleDriveCredential("google_drive") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_DRIVE_API_ENDPOINT: &str = "https://www.googleapis.com/drive/v3";

#[init]
async fn setup() -> Result<()> {
    info!("Google Drive integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Google Drive integration shutting down");
}

// ============================================================================
// search_files - Search for files in Google Drive
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchFilesInput {
    /// Search query using Google Drive search syntax.
    /// Examples: "name contains 'report'", "mimeType = 'application/pdf'"
    pub query: String,
    /// Maximum number of results (1-100). Defaults to 10.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Fields to include in response. Defaults to common fields.
    #[serde(default)]
    pub fields: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchFilesOutput {
    pub files: Vec<DriveFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// # Search Google Drive Files
///
/// Searches for files in Google Drive using the Drive API v3 search syntax.
/// Use this tool when the user wants to find files by name, type, date, or
/// other Google Drive search criteria.
///
/// The search query supports Google Drive's search operators including:
/// - `name contains 'term'` - search by file name
/// - `mimeType = 'type'` - search by MIME type (e.g., 'application/pdf')
/// - `modifiedTime > 'date'` - files modified after a date
/// - `starred = true` - starred files
/// - Combine operators with `and` / `or`
///
/// Returns a list of files with metadata including ID, name, MIME type,
/// timestamps, size, and view links. Supports pagination via `next_page_token`.
///
/// # Errors
///
/// Returns an error if:
/// - The `query` string is empty or contains only whitespace
/// - The `limit` is not between 1 and 100
/// - No valid Google Drive credentials are configured
/// - The `access_token` in credentials is empty
/// - The Google Drive API request fails (network error, authentication failure,
///   rate limit, etc.)
/// - The API response is malformed or cannot be parsed
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - file-storage
/// - google-drive
/// - search
#[tool]
pub async fn search_files(ctx: Context, input: SearchFilesInput) -> Result<SearchFilesOutput> {
    ensure!(!input.query.trim().is_empty(), "query must not be empty");
    let limit = input.limit.unwrap_or(10);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let client = DriveClient::from_ctx(&ctx)?;

    let fields = input.fields.unwrap_or_else(|| {
        "files(id,name,mimeType,description,createdTime,modifiedTime,size,webViewLink,\
         webContentLink,parents,shared,ownedByMe)"
            .to_string()
    });

    let query = [
        ("q", input.query),
        ("pageSize", limit.to_string()),
        ("fields", fields),
    ];

    let response: FileListResponse = client.get_json("files", &query).await?;

    Ok(SearchFilesOutput {
        files: response.files,
        next_page_token: response.next_page_token,
    })
}

// ============================================================================
// download_file - Download a file from Google Drive
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DownloadFileInput {
    /// File ID to download.
    pub file_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DownloadFileOutput {
    /// Base64-encoded file content.
    pub content: String,
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: usize,
}

/// # Download Google Drive File
///
/// Downloads a file from Google Drive by its file ID and returns the content
/// as base64-encoded data. Use this tool when the user wants to retrieve the
/// actual file contents from Google Drive (not just metadata).
///
/// This tool performs two operations:
/// 1. Fetches file metadata to determine the file name and MIME type
/// 2. Downloads the raw file content using the alt=media endpoint
///
/// The output includes base64-encoded content, file name, MIME type, and size.
/// The user will need to decode the base64 content to get the actual file data.
///
/// Requires a valid `file_id` which can be obtained from search results or
/// other Google Drive operations.
///
/// # Errors
///
/// Returns an error if:
/// - The `file_id` is empty or contains only whitespace
/// - No valid Google Drive credentials are configured
/// - The `access_token` in credentials is empty
/// - The Google Drive API request fails (network error, authentication failure,
///   file not found, etc.)
/// - The API response is malformed or cannot be parsed
/// - The file content cannot be downloaded or read
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - file-storage
/// - google-drive
/// - download
#[tool]
pub async fn download_file(ctx: Context, input: DownloadFileInput) -> Result<DownloadFileOutput> {
    ensure!(
        !input.file_id.trim().is_empty(),
        "file_id must not be empty"
    );

    let client = DriveClient::from_ctx(&ctx)?;

    // First get file metadata
    let file: DriveFile = client
        .get_json(
            &format!("files/{}", input.file_id),
            &[("fields", "name,mimeType".to_string())],
        )
        .await?;

    // Download file content
    let content_bytes = client.download_file_content(&input.file_id).await?;

    Ok(DownloadFileOutput {
        content: base64_encode(&content_bytes),
        file_name: file.name,
        mime_type: file
            .mime_type
            .unwrap_or_else(|| "application/octet-stream".to_string()),
        size_bytes: content_bytes.len(),
    })
}

// ============================================================================
// upload_file - Upload a file to Google Drive
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadFileInput {
    /// File name.
    pub name: String,
    /// Base64-encoded file content.
    pub content: String,
    /// MIME type of the file.
    #[serde(default)]
    pub mime_type: Option<String>,
    /// Parent folder IDs.
    #[serde(default)]
    pub parents: Vec<String>,
    /// File description.
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UploadFileOutput {
    pub file_id: String,
    pub name: String,
    pub web_view_link: Option<String>,
}

/// # Upload Google Drive File
///
/// Uploads a new file to Google Drive using multipart upload. Use this tool
/// when the user wants to create or add a file to their Google Drive.
///
/// This tool accepts base64-encoded file content and uploads it to Google Drive
/// with the specified metadata (name, MIME type, description). The file can be
/// placed in specific folders by providing parent folder IDs.
///
/// Key inputs:
/// - `name`: The display name for the file in Drive
/// - `content`: Base64-encoded file data (must encode the actual file bytes)
/// - `mime_type`: Optional file type (defaults to 'application/octet-stream')
/// - `parents`: Optional list of folder IDs to place the file in
/// - `description`: Optional file description
///
/// Returns the created file ID, name, and web view link for accessing the file.
///
/// # Errors
///
/// Returns an error if:
/// - The `name` is empty or contains only whitespace
/// - The `content` string is empty or contains only whitespace
/// - The `content` is not valid base64 encoding
/// - No valid Google Drive credentials are configured
/// - The `access_token` in credentials is empty
/// - The Google Drive API request fails (network error, authentication failure,
///   insufficient quota, etc.)
/// - The API response is malformed or cannot be parsed
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - file-storage
/// - google-drive
/// - upload
#[tool]
pub async fn upload_file(ctx: Context, input: UploadFileInput) -> Result<UploadFileOutput> {
    ensure!(!input.name.trim().is_empty(), "name must not be empty");
    ensure!(
        !input.content.trim().is_empty(),
        "content must not be empty"
    );

    let content_bytes = base64_decode(&input.content)?;
    let mime_type = input
        .mime_type
        .unwrap_or_else(|| "application/octet-stream".to_string());

    let client = DriveClient::from_ctx(&ctx)?;

    let file = client
        .upload_file(
            &input.name,
            &mime_type,
            &input.parents,
            input.description.as_deref(),
            &content_bytes,
        )
        .await?;

    Ok(UploadFileOutput {
        file_id: file.id,
        name: file.name,
        web_view_link: file.web_view_link,
    })
}

// ============================================================================
// share_file - Create a sharing link for a file
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ShareFileInput {
    /// File ID to share.
    pub file_id: String,
    /// Permission role (reader, writer, commenter).
    pub role: PermissionRole,
    /// Permission type (user, group, domain, anyone).
    #[serde(rename = "type")]
    pub permission_type: PermissionType,
    /// Email address for user/group permissions.
    #[serde(default)]
    pub email_address: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ShareFileOutput {
    pub permission_id: String,
    pub web_view_link: Option<String>,
}

/// # Share Google Drive File
///
/// Creates a sharing permission for a Google Drive file, allowing it to be
/// accessed by specific users, groups, domains, or anyone with the link.
/// Use this tool when the user wants to share a file they own or have edit
/// permissions for.
///
/// This tool controls who can access the file and what they can do:
/// - `role`: Access level - 'reader' (view only), 'writer' (edit), or
///   'commenter'
/// - `permission_type`: Who gets access - 'user', 'group', 'domain', or
///   'anyone'
/// - `email_address`: Required when type is 'user' or 'group'
///
/// Common scenarios:
/// - Share with specific person: type='user', email='person@example.com',
///   role='reader'
/// - Share with link only: type='anyone', role='reader' (creates view link)
/// - Share for collaboration: type='user', email='colleague@example.com',
///   role='writer'
///
/// Returns the permission ID and the web view link for easy sharing.
///
/// # Errors
///
/// Returns an error if:
/// - The `file_id` is empty or contains only whitespace
/// - The `permission_type` is `User` or `Group` but no `email_address` is
///   provided
/// - No valid Google Drive credentials are configured
/// - The `access_token` in credentials is empty
/// - The Google Drive API request fails (network error, authentication failure,
///   insufficient permissions, etc.)
/// - The API response is malformed or cannot be parsed
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - file-storage
/// - google-drive
/// - share
#[tool]
pub async fn share_file(ctx: Context, input: ShareFileInput) -> Result<ShareFileOutput> {
    ensure!(
        !input.file_id.trim().is_empty(),
        "file_id must not be empty"
    );

    if matches!(
        input.permission_type,
        PermissionType::User | PermissionType::Group
    ) {
        ensure!(
            input.email_address.is_some(),
            "email_address is required for user/group permissions"
        );
    }

    let client = DriveClient::from_ctx(&ctx)?;

    let permission = client
        .create_permission(
            &input.file_id,
            input.permission_type,
            input.role,
            input.email_address.as_deref(),
        )
        .await?;

    // Get updated file metadata with web link
    let file: DriveFile = client
        .get_json(
            &format!("files/{}", input.file_id),
            &[("fields", "webViewLink".to_string())],
        )
        .await?;

    Ok(ShareFileOutput {
        permission_id: permission.id,
        web_view_link: file.web_view_link,
    })
}

// ============================================================================
// move_file - Move a file to a different folder
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MoveFileInput {
    /// File ID to move.
    pub file_id: String,
    /// Destination folder ID.
    pub destination_folder_id: String,
    /// Remove from all current parent folders. Defaults to true.
    #[serde(default)]
    pub remove_from_parents: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MoveFileOutput {
    pub file_id: String,
    pub parents: Vec<String>,
}

/// # Move Google Drive File
///
/// Moves a file from its current location(s) to a different folder in Google
/// Drive. Use this tool when the user wants to organize or relocate files
/// within their Google Drive folder structure.
///
/// This tool updates the parent folder(s) of a file. By default, it removes the
/// file from all current parent folders before adding it to the destination.
/// Set `remove_from_parents` to false to add the file to the destination while
/// keeping it in its current locations (useful for files in multiple folders).
///
/// Key inputs:
/// - `file_id`: The ID of the file to move (obtainable from search results)
/// - `destination_folder_id`: The target folder ID
/// - `remove_from_parents`: Whether to remove from current folders (default:
///   true)
///
/// Returns the updated file ID and its new parent folder list.
///
/// # Errors
///
/// Returns an error if:
/// - The `file_id` is empty or contains only whitespace
/// - The `destination_folder_id` is empty or contains only whitespace
/// - No valid Google Drive credentials are configured
/// - The `access_token` in credentials is empty
/// - The Google Drive API request fails (network error, authentication failure,
///   file/folder not found, etc.)
/// - The API response is malformed or cannot be parsed
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - file-storage
/// - google-drive
/// - move
#[tool]
pub async fn move_file(ctx: Context, input: MoveFileInput) -> Result<MoveFileOutput> {
    ensure!(
        !input.file_id.trim().is_empty(),
        "file_id must not be empty"
    );
    ensure!(
        !input.destination_folder_id.trim().is_empty(),
        "destination_folder_id must not be empty"
    );

    let client = DriveClient::from_ctx(&ctx)?;

    // Get current parents if we need to remove them
    let remove_from_parents = input.remove_from_parents.unwrap_or(true);
    let current_parents = if remove_from_parents {
        let file: DriveFile = client
            .get_json(
                &format!("files/{}", input.file_id),
                &[("fields", "parents".to_string())],
            )
            .await?;
        file.parents
    } else {
        vec![]
    };

    // Build query parameters
    let mut query = vec![("addParents", input.destination_folder_id.clone())];
    if !current_parents.is_empty() {
        query.push(("removeParents", current_parents.join(",")));
    }
    query.push(("fields", "id,parents".to_string()));

    let updated_file: DriveFile = client
        .patch_json(
            &format!("files/{}", input.file_id),
            &query,
            &serde_json::json!({}),
        )
        .await?;

    Ok(MoveFileOutput {
        file_id: updated_file.id,
        parents: updated_file.parents,
    })
}

// ============================================================================
// rename_file - Rename a file in Google Drive
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RenameFileInput {
    /// File ID to rename.
    pub file_id: String,
    /// New file name.
    pub new_name: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RenameFileOutput {
    pub file_id: String,
    pub name: String,
}

/// # Rename Google Drive File
///
/// Renames a file in Google Drive without changing its content or location.
/// Use this tool when the user wants to change the display name of an existing
/// file while keeping everything else the same.
///
/// This is a metadata-only operation that updates the file's name property.
/// The file ID remains unchanged, and all sharing permissions and parent
/// folder associations are preserved.
///
/// Key inputs:
/// - `file_id`: The ID of the file to rename (obtainable from search results)
/// - `new_name`: The new display name for the file
///
/// Returns the file ID and its new name.
///
/// # Errors
///
/// Returns an error if:
/// - The `file_id` is empty or contains only whitespace
/// - The `new_name` is empty or contains only whitespace
/// - No valid Google Drive credentials are configured
/// - The `access_token` in credentials is empty
/// - The Google Drive API request fails (network error, authentication failure,
///   file not found, etc.)
/// - The API response is malformed or cannot be parsed
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - file-storage
/// - google-drive
/// - rename
#[tool]
pub async fn rename_file(ctx: Context, input: RenameFileInput) -> Result<RenameFileOutput> {
    ensure!(
        !input.file_id.trim().is_empty(),
        "file_id must not be empty"
    );
    ensure!(
        !input.new_name.trim().is_empty(),
        "new_name must not be empty"
    );

    let client = DriveClient::from_ctx(&ctx)?;

    let body = serde_json::json!({
        "name": input.new_name
    });

    let updated_file: DriveFile = client
        .patch_json(
            &format!("files/{}", input.file_id),
            &[("fields", "id,name".to_string())],
            &body,
        )
        .await?;

    Ok(RenameFileOutput {
        file_id: updated_file.id,
        name: updated_file.name,
    })
}

// ============================================================================
// Helper Client Implementation
// ============================================================================

#[derive(Debug, Clone)]
struct DriveClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl DriveClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = GoogleDriveCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url = normalize_base_url(
            cred.endpoint
                .as_deref()
                .unwrap_or(DEFAULT_DRIVE_API_ENDPOINT),
        )?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            access_token: cred.access_token,
        })
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T> {
        let url = format!("{}/{}", self.base_url, path);
        let response = self.send_request(self.http.get(&url).query(query)).await?;
        Ok(response.json::<T>().await?)
    }

    async fn patch_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        query: &[(&str, String)],
        body: &serde_json::Value,
    ) -> Result<T> {
        let url = format!("{}/{}", self.base_url, path);
        let response = self
            .send_request(self.http.patch(&url).query(query).json(body))
            .await?;
        Ok(response.json::<T>().await?)
    }

    async fn download_file_content(&self, file_id: &str) -> Result<Vec<u8>> {
        let url = format!("{}/files/{}?alt=media", self.base_url, file_id);
        let response = self.send_request(self.http.get(&url)).await?;
        Ok(response.bytes().await?.to_vec())
    }

    async fn upload_file(
        &self,
        name: &str,
        mime_type: &str,
        parents: &[String],
        description: Option<&str>,
        content: &[u8],
    ) -> Result<DriveFile> {
        // Use multipart upload
        let metadata = serde_json::json!({
            "name": name,
            "mimeType": mime_type,
            "parents": parents,
            "description": description,
        });

        let boundary = "===============brwse_boundary===============";
        let mut body = Vec::new();

        // Metadata part
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Type: application/json; charset=UTF-8\r\n\r\n");
        body.extend_from_slice(metadata.to_string().as_bytes());
        body.extend_from_slice(b"\r\n");

        // File content part
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(format!("Content-Type: {mime_type}\r\n\r\n").as_bytes());
        body.extend_from_slice(content);
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(format!("--{boundary}--").as_bytes());

        // Transform base URL to upload endpoint
        // e.g., https://www.googleapis.com/drive/v3 -> https://www.googleapis.com/upload/drive/v3
        let upload_url = self.base_url.replace("/drive/v3", "/upload/drive/v3");
        let url = format!("{upload_url}?uploadType=multipart&fields=id,name,webViewLink");
        let response = self
            .send_request(
                self.http
                    .post(&url)
                    .header(
                        "Content-Type",
                        format!("multipart/related; boundary={boundary}"),
                    )
                    .body(body),
            )
            .await?;

        Ok(response.json::<DriveFile>().await?)
    }

    async fn create_permission(
        &self,
        file_id: &str,
        permission_type: PermissionType,
        role: PermissionRole,
        email_address: Option<&str>,
    ) -> Result<Permission> {
        let mut body = serde_json::json!({
            "type": permission_type,
            "role": role,
        });

        if let Some(email) = email_address {
            body["emailAddress"] = serde_json::json!(email);
        }

        let url = format!("{}/files/{}/permissions", self.base_url, file_id);
        let response = self.send_request(self.http.post(&url).json(&body)).await?;

        Ok(response.json::<Permission>().await?)
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
                "Google Drive API request failed ({status}): {body}"
            ))
        }
    }
}

fn normalize_base_url(endpoint: &str) -> Result<String> {
    let trimmed = endpoint.trim();
    ensure!(!trimmed.is_empty(), "endpoint must not be empty");
    Ok(trimmed.trim_end_matches('/').to_string())
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn base64_decode(data: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| operai::anyhow::anyhow!("Failed to decode base64: {e}"))
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path, query_param},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut drive_values = HashMap::new();
        drive_values.insert("access_token".to_string(), "test-token".to_string());
        drive_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("google_drive", drive_values)
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_permission_role_serialization_roundtrip() {
        for variant in [
            PermissionRole::Owner,
            PermissionRole::Reader,
            PermissionRole::Writer,
            PermissionRole::Commenter,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: PermissionRole = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_permission_type_serialization_roundtrip() {
        for variant in [
            PermissionType::User,
            PermissionType::Group,
            PermissionType::Domain,
            PermissionType::Anyone,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: PermissionType = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://www.googleapis.com/drive/v3/").unwrap();
        assert_eq!(result, "https://www.googleapis.com/drive/v3");
    }

    #[test]
    fn test_normalize_base_url_empty_returns_error() {
        let result = normalize_base_url("");
        assert!(result.is_err());
    }

    // --- Input validation tests ---

    #[tokio::test]
    async fn test_search_files_empty_query_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = search_files(
            ctx,
            SearchFilesInput {
                query: "   ".to_string(),
                limit: None,
                fields: None,
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
    async fn test_search_files_limit_exceeds_max_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = search_files(
            ctx,
            SearchFilesInput {
                query: "test".to_string(),
                limit: Some(101),
                fields: None,
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
    async fn test_download_file_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = download_file(
            ctx,
            DownloadFileInput {
                file_id: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("file_id must not be empty")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_search_files_success_returns_files() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "files": [
            {
              "id": "file-1",
              "name": "Test Document.pdf",
              "mimeType": "application/pdf",
              "size": "1024",
              "webViewLink": "https://drive.google.com/file/d/file-1/view"
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/files"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("q", "name contains 'test'"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = search_files(
            ctx,
            SearchFilesInput {
                query: "name contains 'test'".to_string(),
                limit: Some(10),
                fields: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.files.len(), 1);
        assert_eq!(output.files[0].id, "file-1");
        assert_eq!(output.files[0].name, "Test Document.pdf");
    }

    #[tokio::test]
    async fn test_rename_file_success() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "id": "file-1",
          "name": "New Name.pdf"
        }
        "#;

        Mock::given(method("PATCH"))
            .and(path("/files/file-1"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = rename_file(
            ctx,
            RenameFileInput {
                file_id: "file-1".to_string(),
                new_name: "New Name.pdf".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.file_id, "file-1");
        assert_eq!(output.name, "New Name.pdf");
    }

    #[tokio::test]
    async fn test_download_file_success() {
        let server = MockServer::start().await;

        // Mock metadata request
        let metadata_body = r#"
        {
          "id": "file-123",
          "name": "test.txt",
          "mimeType": "text/plain"
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/files/file-123"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("fields", "name,mimeType"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(metadata_body, "application/json"),
            )
            .mount(&server)
            .await;

        // Mock content download
        let content_bytes = b"Hello, World!";
        Mock::given(method("GET"))
            .and(path("/files/file-123"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("alt", "media"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(*content_bytes))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = download_file(
            ctx,
            DownloadFileInput {
                file_id: "file-123".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.file_name, "test.txt");
        assert_eq!(output.mime_type, "text/plain");
        assert_eq!(output.size_bytes, 13);

        // Verify base64 encoding
        let decoded = base64_decode(&output.content).unwrap();
        assert_eq!(decoded, content_bytes);
    }

    #[tokio::test]
    async fn test_upload_file_success() {
        let server = MockServer::start().await;

        let response_body = r#"
        {
          "id": "new-file-456",
          "name": "uploaded.txt",
          "mimeType": "text/plain",
          "webViewLink": "https://drive.google.com/file/d/new-file-456/view"
        }
        "#;

        // Match any path for upload since the mock server URL format is different
        Mock::given(method("POST"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let content = base64_encode(b"Test file content");

        let output = upload_file(
            ctx,
            UploadFileInput {
                name: "uploaded.txt".to_string(),
                content,
                mime_type: Some("text/plain".to_string()),
                parents: vec![],
                description: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.file_id, "new-file-456");
        assert_eq!(output.name, "uploaded.txt");
        assert_eq!(
            output.web_view_link,
            Some("https://drive.google.com/file/d/new-file-456/view".to_string())
        );
    }

    #[tokio::test]
    async fn test_share_file_success() {
        let server = MockServer::start().await;

        // Mock permission creation
        let permission_body = r#"
        {
          "id": "perm-789",
          "type": "user",
          "role": "writer",
          "emailAddress": "user@example.com"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/files/file-123/permissions"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(permission_body, "application/json"),
            )
            .mount(&server)
            .await;

        // Mock file metadata retrieval for web link
        let file_body = r#"
        {
          "id": "file-123",
          "name": "Test File.pdf",
          "webViewLink": "https://drive.google.com/file/d/file-123/view"
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/files/file-123"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("fields", "webViewLink"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(file_body, "application/json"))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = share_file(
            ctx,
            ShareFileInput {
                file_id: "file-123".to_string(),
                role: PermissionRole::Writer,
                permission_type: PermissionType::User,
                email_address: Some("user@example.com".to_string()),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.permission_id, "perm-789");
        assert_eq!(
            output.web_view_link,
            Some("https://drive.google.com/file/d/file-123/view".to_string())
        );
    }

    #[tokio::test]
    async fn test_move_file_success() {
        let server = MockServer::start().await;

        // Mock get current parents
        let current_file_body = r#"
        {
          "id": "file-123",
          "name": "test.txt",
          "parents": ["folder-old"]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/files/file-123"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("fields", "parents"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(current_file_body, "application/json"),
            )
            .mount(&server)
            .await;

        // Mock update parents
        let updated_file_body = r#"
        {
          "id": "file-123",
          "name": "test.txt",
          "parents": ["folder-new"]
        }
        "#;

        Mock::given(method("PATCH"))
            .and(path("/files/file-123"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(updated_file_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = move_file(
            ctx,
            MoveFileInput {
                file_id: "file-123".to_string(),
                destination_folder_id: "folder-new".to_string(),
                remove_from_parents: Some(true),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.file_id, "file-123");
        assert_eq!(output.parents, vec!["folder-new"]);
    }

    #[tokio::test]
    async fn test_share_file_requires_email_for_user_type() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = share_file(
            ctx,
            ShareFileInput {
                file_id: "file-123".to_string(),
                role: PermissionRole::Reader,
                permission_type: PermissionType::User,
                email_address: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("email_address is required")
        );
    }

    #[tokio::test]
    async fn test_upload_file_invalid_base64_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = upload_file(
            ctx,
            UploadFileInput {
                name: "test.txt".to_string(),
                content: "not valid base64!!!".to_string(),
                mime_type: Some("text/plain".to_string()),
                parents: vec![],
                description: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Failed to decode base64")
        );
    }
}
