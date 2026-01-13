//! file-storage/onedrive integration for Operai Toolbox.

use base64::Engine;
use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

mod types;
use types::{DriveItem, ItemReference, Permission, SharingLink};

define_user_credential! {
    OneDriveCredential("onedrive") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_GRAPH_ENDPOINT: &str = "https://graph.microsoft.com/v1.0";

#[init]
async fn setup() -> Result<()> {
    info!("OneDrive integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("OneDrive integration shutting down");
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchFilesInput {
    /// Search query string (searches file names and content).
    pub query: String,
    /// Maximum number of results (1-50). Defaults to 10.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchFilesOutput {
    pub items: Vec<DriveItem>,
}

/// # Search OneDrive Files
///
/// Searches for files and folders in the user's OneDrive using the Microsoft
/// Graph API. This tool searches across both file names and file contents,
/// making it ideal for finding specific documents even if you don't know their
/// exact location.
///
/// Use this tool when a user wants to:
/// - Find files or folders by name or content
/// - Locate documents containing specific text
/// - Search within their OneDrive storage
///
/// Key constraints:
/// - Query must not be empty or contain only whitespace
/// - Limit must be between 1 and 50 (defaults to 10 if not specified)
/// - The search uses Microsoft Graph's search API which requires the
///   `ConsistencyLevel` header
/// - Returns only files/folders the user has access to
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - files
/// - onedrive
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The query string is empty or contains only whitespace
/// - The limit is not between 1 and 50
/// - The OneDrive credential is missing or invalid
/// - The access token is missing or empty
/// - The HTTP request to Microsoft Graph API fails
/// - The response cannot be parsed as JSON
#[tool]
pub async fn search_files(ctx: Context, input: SearchFilesInput) -> Result<SearchFilesOutput> {
    ensure!(!input.query.trim().is_empty(), "query must not be empty");
    let limit = input.limit.unwrap_or(10);
    ensure!((1..=50).contains(&limit), "limit must be between 1 and 50");

    let client = GraphClient::from_ctx(&ctx)?;

    // Microsoft Graph search API uses path parameter: search(q='search-term')
    // We need to URL-encode the search term and put it in the path
    let search_param = format!("search(q='{}')", input.query);
    let query = [("$top", limit.to_string())];

    let response: GraphListResponse<DriveItem> = client
        .get_json(
            client.url_with_segments(&["me", "drive", "root", &search_param])?,
            &query,
            &[("ConsistencyLevel", "eventual")],
        )
        .await?;

    Ok(SearchFilesOutput {
        items: response.value,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DownloadInput {
    /// File ID or path (e.g., "/Documents/file.txt").
    pub item_id_or_path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DownloadOutput {
    pub download_url: String,
    pub item: DriveItem,
}

/// # Download OneDrive File
///
/// Gets a download URL for a file stored in OneDrive using the Microsoft
/// Graph API. This tool provides a direct download link that can be used to
/// retrieve the file's contents.
///
/// Use this tool when a user wants to:
/// - Download a file from their OneDrive
/// - Get a shareable download link for a file
/// - Access the contents of a specific document
///
/// The tool accepts either:
/// - A file ID (e.g., "01VAN3P2ZH7JMUBHC4N3ZB4JWG2XZD5WHG")
/// - A file path (e.g., "/Documents/report.pdf")
///
/// Key constraints:
/// - `item_id_or_path` must not be empty or contain only whitespace
/// - The item must exist and be accessible to the user
/// - Returns a download URL that can be used directly, along with file metadata
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - files
/// - onedrive
/// - microsoft-graph
/// - download
///
/// # Errors
///
/// Returns an error if:
/// - The `item_id_or_path` is empty or contains only whitespace
/// - The OneDrive credential is missing or invalid
/// - The access token is missing or empty
/// - The HTTP request to Microsoft Graph API fails
/// - The response cannot be parsed as JSON
/// - The item cannot be found
#[tool]
pub async fn download(ctx: Context, input: DownloadInput) -> Result<DownloadOutput> {
    ensure!(
        !input.item_id_or_path.trim().is_empty(),
        "item_id_or_path must not be empty"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let path_segments = if input.item_id_or_path.starts_with('/') {
        vec!["me", "drive", "root:", &input.item_id_or_path]
    } else {
        vec!["me", "drive", "items", &input.item_id_or_path]
    };

    let item: DriveItem = client
        .get_json(client.url_with_segments(&path_segments)?, &[], &[])
        .await?;

    let download_url = item.download_url.clone().unwrap_or_else(|| {
        // Construct download URL if not provided
        format!("{}/me/drive/items/{}/content", client.base_url, item.id)
    });

    Ok(DownloadOutput { download_url, item })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadInput {
    /// Parent folder ID or path. Use "/" for root.
    pub parent_folder_path: String,
    /// Name of the file to create.
    pub file_name: String,
    /// Base64-encoded file content.
    pub content_base64: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UploadOutput {
    pub item: DriveItem,
}

/// # Upload OneDrive File
///
/// Uploads a file to the user's OneDrive using the Microsoft Graph API's
/// simple upload endpoint. This tool is ideal for uploading files up to 250MB
/// in size directly to OneDrive.
///
/// Use this tool when a user wants to:
/// - Upload a new file to their OneDrive
/// - Save a document, image, or any file to cloud storage
/// - Create a file in a specific folder
///
/// The tool supports both path-based and ID-based folder targeting:
/// - Path: Use "/" for root folder or "/Documents" for a subfolder
/// - ID: Use the folder's ID directly (e.g.,
///   "01VAN3P2ZH7JMUBHC4N3ZB4JWG2XZD5WHG")
///
/// Key constraints:
/// - `parent_folder_path` must not be empty (use "/" for root)
/// - `file_name` must not be empty
/// - `content_base64` must be valid base64-encoded file content
/// - Maximum file size is 250MB (for larger files, use the upload session API
///   instead)
/// - Content must be base64-encoded before passing to this tool
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - files
/// - onedrive
/// - microsoft-graph
/// - upload
///
/// # Errors
///
/// Returns an error if:
/// - The `parent_folder_path` is empty or contains only whitespace
/// - The `file_name` is empty or contains only whitespace
/// - The ``content_base64`` is empty or contains only whitespace
/// - The ``content_base64`` is not valid base64
/// - The decoded content exceeds 250MB limit
/// - The OneDrive credential is missing or invalid
/// - The access token is missing or empty
/// - The HTTP request to Microsoft Graph API fails
/// - The response cannot be parsed as JSON
#[tool]
pub async fn upload(ctx: Context, input: UploadInput) -> Result<UploadOutput> {
    ensure!(
        !input.parent_folder_path.trim().is_empty(),
        "parent_folder_path must not be empty"
    );
    ensure!(
        !input.file_name.trim().is_empty(),
        "file_name must not be empty"
    );
    ensure!(
        !input.content_base64.trim().is_empty(),
        "`content_base64` must not be empty"
    );

    let content = base64::engine::general_purpose::STANDARD
        .decode(&input.content_base64)
        .map_err(|e| operai::anyhow::anyhow!("Invalid base64 content: {e}"))?;

    ensure!(
        content.len() <= 250 * 1024 * 1024,
        "content size exceeds 250MB limit for simple upload"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let parent_path = if input.parent_folder_path == "/" {
        String::new()
    } else {
        input.parent_folder_path.trim_end_matches('/').to_string()
    };

    // Determine the URL based on whether parent_folder_path is a path or ID
    let url = if parent_path.is_empty() || parent_path.starts_with('/') {
        // Path-based: /me/drive/root:/path/to/folder/file.txt:/content
        let full_path = if parent_path.is_empty() {
            format!("/{}:/content", input.file_name)
        } else {
            format!("{}:/{}:/content", parent_path, input.file_name)
        };
        format!("{}/me/drive/root:{}", client.base_url, full_path)
    } else {
        // ID-based: /me/drive/items/{parent-id}:/{filename}:/content
        format!(
            "{}/me/drive/items/{}:/{}:/content",
            client.base_url, parent_path, input.file_name
        )
    };

    let item: DriveItem = client
        .put_json(reqwest::Url::parse(&url)?, &content, &[])
        .await?;

    Ok(UploadOutput { item })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ShareLinkInput {
    /// File or folder ID.
    pub item_id: String,
    /// Link type: "view" or "edit".
    pub link_type: String,
    /// Link scope: "anonymous" or "organization". Defaults to "anonymous".
    #[serde(default)]
    pub scope: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ShareLinkOutput {
    pub link: SharingLink,
}

/// # Create OneDrive Sharing Link
///
/// Creates a sharing link for a file or folder in OneDrive using the
/// Microsoft Graph API. This tool generates shareable links that allow others
/// to access or collaborate on files.
///
/// Use this tool when a user wants to:
/// - Share a file or folder with others
/// - Create a view-only link for a document
/// - Create an edit link to allow collaboration
/// - Generate a shareable URL for a OneDrive item
///
/// Link types:
/// - "view": Recipients can view the file but not edit it
/// - "edit": Recipients can edit the file
///
/// Scopes:
/// - "anonymous": Anyone with the link can access (default)
/// - "organization": Only people in the user's organization can access
///
/// Key constraints:
/// - `item_id` must not be empty and must reference an existing file/folder
/// - `link_type` must be either "view" or "edit"
/// - scope must be either "anonymous" or "organization" (defaults to
///   "anonymous")
/// - Returns a sharing link with web URL that can be shared with others
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - files
/// - onedrive
/// - microsoft-graph
/// - sharing
///
/// # Errors
///
/// Returns an error if:
/// - The ``item_id`` is empty or contains only whitespace
/// - The ``link_type`` is empty or not "view" or "edit"
/// - The `scope` is not "anonymous" or "organization"
/// - The OneDrive credential is missing or invalid
/// - The access token is missing or empty
/// - The HTTP request to Microsoft Graph API fails
/// - The response cannot be parsed as JSON
/// - The response does not contain a sharing link
#[tool]
pub async fn share_link(ctx: Context, input: ShareLinkInput) -> Result<ShareLinkOutput> {
    ensure!(
        !input.item_id.trim().is_empty(),
        "`item_id` must not be empty"
    );
    ensure!(
        !input.link_type.trim().is_empty(),
        "`link_type` must not be empty"
    );
    ensure!(
        matches!(input.link_type.as_str(), "view" | "edit"),
        "`link_type` must be 'view' or 'edit'"
    );

    let scope = input.scope.unwrap_or_else(|| "anonymous".to_string());
    ensure!(
        matches!(scope.as_str(), "anonymous" | "organization"),
        "scope must be 'anonymous' or 'organization'"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let request = GraphCreateLinkRequest {
        link_type: input.link_type,
        scope,
    };

    let permission: Permission = client
        .post_json(
            client.url_with_segments(&["me", "drive", "items", &input.item_id, "createLink"])?,
            &request,
            &[],
        )
        .await?;

    let link = permission.link.ok_or_else(|| {
        operai::anyhow::anyhow!("No sharing link returned in permission response")
    })?;

    Ok(ShareLinkOutput { link })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MoveInput {
    /// File or folder ID to move.
    pub item_id: String,
    /// Destination folder ID.
    pub destination_folder_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MoveOutput {
    pub item: DriveItem,
}

/// # Move OneDrive File or Folder
///
/// Moves a file or folder to a different location in OneDrive using the
/// Microsoft Graph API. This tool relocates items within the user's OneDrive
/// without changing their content.
///
/// Use this tool when a user wants to:
/// - Move a file to a different folder
/// - Reorganize their OneDrive folder structure
/// - Relocate documents to a more appropriate location
///
/// Key constraints:
/// - `item_id` must not be empty and must reference an existing file or folder
/// - `destination_folder_id` must not be empty and must reference an existing
///   folder
/// - Both the source item and destination folder must be accessible to the user
/// - The move operation preserves all file metadata and versions
/// - Cannot move items between different drives (only within the same OneDrive)
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - files
/// - onedrive
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The ``item_id`` is empty or contains only whitespace
/// - The ``destination_folder_id`` is empty or contains only whitespace
/// - The OneDrive credential is missing or invalid
/// - The access token is missing or empty
/// - The HTTP request to Microsoft Graph API fails
/// - The response cannot be parsed as JSON
#[tool]
pub async fn move_item(ctx: Context, input: MoveInput) -> Result<MoveOutput> {
    ensure!(
        !input.item_id.trim().is_empty(),
        "`item_id` must not be empty"
    );
    ensure!(
        !input.destination_folder_id.trim().is_empty(),
        "`destination_folder_id` must not be empty"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let request = GraphMoveRequest {
        parent_reference: ItemReference {
            id: Some(input.destination_folder_id),
            path: None,
            drive_id: None,
        },
    };

    let item: DriveItem = client
        .patch_json(
            client.url_with_segments(&["me", "drive", "items", &input.item_id])?,
            &request,
            &[],
        )
        .await?;

    Ok(MoveOutput { item })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RenameInput {
    /// File or folder ID to rename.
    pub item_id: String,
    /// New name for the item.
    pub new_name: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RenameOutput {
    pub item: DriveItem,
}

/// # Rename OneDrive File or Folder
///
/// Renames a file or folder in OneDrive using the Microsoft Graph API.
/// This tool changes the name of an item without modifying its content or
/// location.
///
/// Use this tool when a user wants to:
/// - Rename a file to a more descriptive name
/// - Fix a typo in a file or folder name
/// - Update the name of a document to better reflect its contents
/// - Standardize naming conventions for files
///
/// Key constraints:
/// - `item_id` must not be empty and must reference an existing file or folder
/// - `new_name` must not be empty or contain only whitespace
/// - The new name must be unique within the parent folder
/// - The rename operation preserves all file metadata, versions, and sharing
///   links
/// - File extensions should be preserved when renaming files to maintain type
///   associations
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - files
/// - onedrive
/// - microsoft-graph
///
/// # Errors
///
/// Returns an error if:
/// - The ``item_id`` is empty or contains only whitespace
/// - The ``new_name`` is empty or contains only whitespace
/// - The OneDrive credential is missing or invalid
/// - The access token is missing or empty
/// - The HTTP request to Microsoft Graph API fails
/// - The response cannot be parsed as JSON
#[tool]
pub async fn rename(ctx: Context, input: RenameInput) -> Result<RenameOutput> {
    ensure!(
        !input.item_id.trim().is_empty(),
        "`item_id` must not be empty"
    );
    ensure!(
        !input.new_name.trim().is_empty(),
        "`new_name` must not be empty"
    );

    let client = GraphClient::from_ctx(&ctx)?;

    let request = GraphRenameRequest {
        name: input.new_name,
    };

    let item: DriveItem = client
        .patch_json(
            client.url_with_segments(&["me", "drive", "items", &input.item_id])?,
            &request,
            &[],
        )
        .await?;

    Ok(RenameOutput { item })
}

// --- Internal Graph API client and request/response types ---

#[derive(Debug, Deserialize)]
struct GraphListResponse<T> {
    value: Vec<T>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphCreateLinkRequest {
    #[serde(rename = "type")]
    link_type: String,
    scope: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphMoveRequest {
    parent_reference: ItemReference,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphRenameRequest {
    name: String,
}

#[derive(Debug, Clone)]
struct GraphClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl GraphClient {
    /// Creates a new `GraphClient` from the tool context.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The OneDrive credential is not found in the context
    /// - The access token is empty or contains only whitespace
    /// - The endpoint URL is invalid
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = OneDriveCredential::get(ctx)?;
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

    /// Constructs a URL by appending path segments to the base URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the base URL cannot be used as a base for URL
    /// construction.
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

    /// Sends a GET request and parses the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails
    /// - The response status indicates an error
    /// - The response body cannot be parsed as JSON
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

    /// Sends a POST request with JSON body and parses the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails
    /// - The response status indicates an error
    /// - The response body cannot be parsed as JSON
    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
        extra_headers: &[(&str, &str)],
    ) -> Result<TRes> {
        let mut request = self.http.post(url).json(body);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        let response = self.send_request(request).await?;
        Ok(response.json::<TRes>().await?)
    }

    /// Sends a PUT request with binary body and parses the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails
    /// - The response status indicates an error
    /// - The response body cannot be parsed as JSON
    async fn put_json<TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &[u8],
        extra_headers: &[(&str, &str)],
    ) -> Result<TRes> {
        let mut request = self.http.put(url).body(body.to_vec());
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        let response = self.send_request(request).await?;
        Ok(response.json::<TRes>().await?)
    }

    /// Sends a PATCH request with JSON body and parses the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails
    /// - The response status indicates an error
    /// - The response body cannot be parsed as JSON
    async fn patch_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
        extra_headers: &[(&str, &str)],
    ) -> Result<TRes> {
        let mut request = self.http.patch(url).json(body);
        for (key, value) in extra_headers {
            request = request.header(*key, *value);
        }

        let response = self.send_request(request).await?;
        Ok(response.json::<TRes>().await?)
    }

    /// Sends an HTTP request with authentication and headers.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails
    /// - The response status indicates an error
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

/// Normalizes a base URL by trimming whitespace and trailing slashes.
///
/// # Errors
///
/// Returns an error if the endpoint is empty or contains only whitespace.
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

    use types::FileFacet;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path, query_param},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut onedrive_values = HashMap::new();
        onedrive_values.insert("access_token".to_string(), "test-token".to_string());
        onedrive_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("onedrive", onedrive_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        format!("{}/v1.0", server.uri())
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_drive_item_serialization_roundtrip() {
        let item = DriveItem {
            id: "item-123".to_string(),
            name: "test.txt".to_string(),
            size: Some(1024),
            created_date_time: Some("2024-01-01T00:00:00Z".to_string()),
            last_modified_date_time: Some("2024-01-02T00:00:00Z".to_string()),
            web_url: Some("https://example.com".to_string()),
            folder: None,
            file: Some(FileFacet {
                mime_type: Some("text/plain".to_string()),
                hashes: None,
            }),
            download_url: Some("https://example.com/download".to_string()),
            parent_reference: None,
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: DriveItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item.id, parsed.id);
        assert_eq!(item.name, parsed.name);
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
    async fn test_search_files_empty_query_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = search_files(
            ctx,
            SearchFilesInput {
                query: "   ".to_string(),
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
    async fn test_search_files_limit_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = search_files(
            ctx,
            SearchFilesInput {
                query: "test".to_string(),
                limit: Some(0),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("limit must be between 1 and 50")
        );
    }

    #[tokio::test]
    async fn test_download_empty_item_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = download(
            ctx,
            DownloadInput {
                item_id_or_path: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("item_id_or_path must not be empty")
        );
    }

    #[tokio::test]
    async fn test_upload_empty_file_name_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = upload(
            ctx,
            UploadInput {
                parent_folder_path: "/".to_string(),
                file_name: "  ".to_string(),
                content_base64: "SGVsbG8=".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("file_name must not be empty")
        );
    }

    #[tokio::test]
    async fn test_share_link_invalid_type_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = share_link(
            ctx,
            ShareLinkInput {
                item_id: "item-123".to_string(),
                link_type: "invalid".to_string(),
                scope: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("`link_type` must be 'view' or 'edit'")
        );
    }

    #[tokio::test]
    async fn test_move_empty_item_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = move_item(
            ctx,
            MoveInput {
                item_id: "  ".to_string(),
                destination_folder_id: "folder-123".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("`item_id` must not be empty")
        );
    }

    #[tokio::test]
    async fn test_rename_empty_new_name_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = rename(
            ctx,
            RenameInput {
                item_id: "item-123".to_string(),
                new_name: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("`new_name` must not be empty")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_search_files_success_returns_items() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "value": [
            {
              "id": "item-1",
              "name": "test.txt",
              "size": 1024,
              "createdDateTime": "2024-01-01T00:00:00Z",
              "lastModifiedDateTime": "2024-01-02T00:00:00Z",
              "webUrl": "https://example.com/test.txt",
              "file": { "mimeType": "text/plain" }
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1.0/me/drive/root/search(q='document')"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("$top", "5"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = search_files(
            ctx,
            SearchFilesInput {
                query: "document".to_string(),
                limit: Some(5),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.items.len(), 1);
        assert_eq!(output.items[0].id, "item-1");
        assert_eq!(output.items[0].name, "test.txt");
    }

    #[tokio::test]
    async fn test_download_success_returns_url() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "item-123",
          "name": "file.pdf",
          "size": 2048,
          "@microsoft.graph.downloadUrl": "https://download.example.com/file.pdf"
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/v1.0/me/drive/items/item-123"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = download(
            ctx,
            DownloadInput {
                item_id_or_path: "item-123".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.item.id, "item-123");
        assert_eq!(output.download_url, "https://download.example.com/file.pdf");
    }

    #[tokio::test]
    async fn test_share_link_success_returns_link() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "perm-123",
          "link": {
            "type": "view",
            "scope": "anonymous",
            "webUrl": "https://1drv.ms/xyz"
          }
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/v1.0/me/drive/items/item-123/createLink"))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = share_link(
            ctx,
            ShareLinkInput {
                item_id: "item-123".to_string(),
                link_type: "view".to_string(),
                scope: Some("anonymous".to_string()),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.link.web_url, "https://1drv.ms/xyz");
        assert_eq!(output.link.link_type, "view");
    }

    #[tokio::test]
    async fn test_rename_success_returns_updated_item() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "item-123",
          "name": "renamed.txt"
        }
        "#;

        Mock::given(method("PATCH"))
            .and(path("/v1.0/me/drive/items/item-123"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = rename(
            ctx,
            RenameInput {
                item_id: "item-123".to_string(),
                new_name: "renamed.txt".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.item.id, "item-123");
        assert_eq!(output.item.name, "renamed.txt");
    }

    #[tokio::test]
    async fn test_upload_success_returns_item() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "item-456",
          "name": "test.txt",
          "size": 12
        }
        "#;

        Mock::given(method("PUT"))
            .and(path("/v1.0/me/drive/root:/test.txt:/content"))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = upload(
            ctx,
            UploadInput {
                parent_folder_path: "/".to_string(),
                file_name: "test.txt".to_string(),
                content_base64: "SGVsbG8gV29ybGQ=".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.item.id, "item-456");
        assert_eq!(output.item.name, "test.txt");
        assert_eq!(output.item.size, Some(12));
    }

    #[tokio::test]
    async fn test_upload_to_subfolder_success_returns_item() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "item-789",
          "name": "document.pdf",
          "size": 1024
        }
        "#;

        Mock::given(method("PUT"))
            .and(path(
                "/v1.0/me/drive/root:/Documents:/document.pdf:/content",
            ))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = upload(
            ctx,
            UploadInput {
                parent_folder_path: "/Documents".to_string(),
                file_name: "document.pdf".to_string(),
                content_base64: "VGVzdCBjb250ZW50".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.item.id, "item-789");
        assert_eq!(output.item.name, "document.pdf");
        assert_eq!(output.item.size, Some(1024));
    }

    #[tokio::test]
    async fn test_upload_by_folder_id_success_returns_item() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "item-999",
          "name": "file.docx",
          "size": 2048
        }
        "#;

        Mock::given(method("PUT"))
            .and(path("/v1.0/me/drive/items/folder-123:/file.docx:/content"))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = upload(
            ctx,
            UploadInput {
                parent_folder_path: "folder-123".to_string(),
                file_name: "file.docx".to_string(),
                content_base64: "VGVzdCBjb250ZW50".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.item.id, "item-999");
        assert_eq!(output.item.name, "file.docx");
        assert_eq!(output.item.size, Some(2048));
    }

    #[tokio::test]
    async fn test_move_success_returns_updated_item() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "id": "item-456",
          "name": "file.txt",
          "parentReference": {
            "id": "folder-789"
          }
        }
        "#;

        Mock::given(method("PATCH"))
            .and(path("/v1.0/me/drive/items/item-456"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = move_item(
            ctx,
            MoveInput {
                item_id: "item-456".to_string(),
                destination_folder_id: "folder-789".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.item.id, "item-456");
        assert_eq!(
            output
                .item
                .parent_reference
                .as_ref()
                .unwrap()
                .id
                .as_ref()
                .unwrap(),
            "folder-789"
        );
    }
}
