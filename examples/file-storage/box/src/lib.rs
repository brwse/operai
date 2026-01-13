//! file-storage/box integration for Operai Toolbox.
use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

mod types;
use types::{
    AccessibleBy, BoxFile, BoxFolder, BoxItem, Collaboration, CollaborationItem,
    CollaborationRequest, CreateFolderRequest, ParentReference, PathCollection, SearchResults,
    SharedLink, SharedLinkAccess, SharedLinkRequest,
};

define_user_credential! {
    BoxCredential("box") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_BOX_ENDPOINT: &str = "https://api.box.com/2.0";
const UPLOAD_ENDPOINT: &str = "https://upload.box.com/api/2.0";

#[init]
async fn setup() -> Result<()> {
    info!("Box integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Box integration shutting down");
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchInput {
    /// Search query string to find files and folders.
    pub query: String,
    /// Maximum number of results (1-200). Defaults to 30.
    #[serde(default)]
    pub limit: Option<u32>,
    /// File extensions to filter by (e.g., "pdf", "docx").
    #[serde(default)]
    pub file_extensions: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchOutput {
    pub items: Vec<SearchItem>,
    pub total_count: u32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SearchItem {
    pub id: String,
    pub item_type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// # Search Box Files
///
/// Searches for files and folders in Box using the Box API.
///
/// Use this tool when a user wants to find files or folders in their Box
/// account by name, content, or other search criteria. This is ideal for
/// locating specific documents, folders, or resources when the user knows part
/// of the name or wants to filter by file type.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - file-storage
/// - box
/// - search
///
/// # Errors
///
/// Returns an error if:
/// - The query string is empty or contains only whitespace
/// - The limit is not between 1 and 200
/// - The Box credentials are missing or invalid
/// - The Box API request fails (network error, authentication error, etc.)
/// - The API response cannot be parsed
#[tool]
pub async fn search(ctx: Context, input: SearchInput) -> Result<SearchOutput> {
    ensure!(!input.query.trim().is_empty(), "query must not be empty");
    let limit = input.limit.unwrap_or(30);
    ensure!(
        (1..=200).contains(&limit),
        "limit must be between 1 and 200"
    );

    let client = BoxClient::from_ctx(&ctx)?;

    let mut query_params = vec![("query", input.query), ("limit", limit.to_string())];

    if !input.file_extensions.is_empty() {
        query_params.push(("file_extensions", input.file_extensions.join(",")));
    }

    let results: SearchResults = client
        .get_json(client.url_with_path("/search")?, &query_params)
        .await?;

    let items = results
        .entries
        .into_iter()
        .map(|item| match item {
            BoxItem::File(f) => SearchItem {
                id: f.id,
                item_type: "file".to_string(),
                name: f.name,
                size: f.size,
                path: f.path_collection.map(|p| format_path(&p)),
            },
            BoxItem::Folder(f) => SearchItem {
                id: f.id,
                item_type: "folder".to_string(),
                name: f.name,
                size: None,
                path: f.path_collection.map(|p| format_path(&p)),
            },
        })
        .collect();

    Ok(SearchOutput {
        items,
        total_count: results.total_count,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DownloadInput {
    /// Box file ID to download.
    pub file_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DownloadOutput {
    pub download_url: String,
    pub file_name: String,
}

/// # Download Box File
///
/// Retrieves a download URL for a Box file using the file ID.
///
/// Use this tool when a user wants to download or access a file from their Box
/// account. This tool returns a direct download URL that can be used to
/// retrieve the file content. The user must provide a valid Box file ID, which
/// can be obtained through search or other Box API operations. Note that this
/// returns a URL, not the actual file content.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - file-storage
/// - box
/// - download
///
/// # Errors
///
/// Returns an error if:
/// - The `file_id` is empty or contains only whitespace
/// - The Box credentials are missing or invalid
/// - The Box API request fails (file not found, authentication error, etc.)
/// - The API response cannot be parsed
#[tool]
pub async fn download(ctx: Context, input: DownloadInput) -> Result<DownloadOutput> {
    ensure!(
        !input.file_id.trim().is_empty(),
        "file_id must not be empty"
    );

    let client = BoxClient::from_ctx(&ctx)?;

    let file: BoxFile = client
        .get_json(
            client.url_with_path(&format!("/files/{}", input.file_id))?,
            &[],
        )
        .await?;

    let download_url = format!("{}/files/{}/content", client.base_url, input.file_id);

    Ok(DownloadOutput {
        download_url,
        file_name: file.name,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadInput {
    /// Parent folder ID where the file will be uploaded. Use "0" for root.
    pub parent_folder_id: String,
    /// Name of the file to create.
    pub file_name: String,
    /// Base64-encoded file content.
    pub content_base64: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UploadOutput {
    pub file_id: String,
    pub file_name: String,
}

/// # Upload Box File
///
/// Uploads a new file to Box using base64-encoded content.
///
/// Use this tool when a user wants to upload a file from their local system to
/// their Box account. The file content must be provided as a base64-encoded
/// string. The user can specify the destination folder ID (use "0" for the root
/// folder) and the desired file name. This tool handles the multipart upload
/// process and returns the newly created file ID and name.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - file-storage
/// - box
/// - upload
///
/// # Errors
///
/// Returns an error if:
/// - The `parent_folder_id` is empty or contains only whitespace
/// - The `file_name` is empty or contains only whitespace
/// - The `content_base64` is empty or contains only whitespace
/// - The `content_base64` is not valid base64 encoding
/// - The Box credentials are missing or invalid
/// - The Box API request fails (network error, authentication error, etc.)
/// - The API response cannot be parsed or contains no files
#[tool]
pub async fn upload(ctx: Context, input: UploadInput) -> Result<UploadOutput> {
    ensure!(
        !input.parent_folder_id.trim().is_empty(),
        "parent_folder_id must not be empty"
    );
    ensure!(
        !input.file_name.trim().is_empty(),
        "file_name must not be empty"
    );
    ensure!(
        !input.content_base64.trim().is_empty(),
        "content_base64 must not be empty"
    );

    let content = base64_decode(&input.content_base64)?;

    let client = BoxClient::from_ctx(&ctx)?;

    let attributes_json = format!(
        r#"{{"name":"{}","parent":{{"id":"{}"}}}}"#,
        input.file_name.replace('"', "\\\""),
        input.parent_folder_id
    );

    let form = reqwest::multipart::Form::new()
        .text("attributes", attributes_json)
        .part(
            "file",
            reqwest::multipart::Part::bytes(content).file_name(input.file_name.clone()),
        );

    let response: UploadResponse = client
        .post_multipart(format!("{UPLOAD_ENDPOINT}/files/content"), form)
        .await?;

    let file = response
        .entries
        .into_iter()
        .next()
        .ok_or_else(|| operai::anyhow::anyhow!("Upload returned no files"))?;

    Ok(UploadOutput {
        file_id: file.id,
        file_name: file.name,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateFolderInput {
    /// Name of the folder to create.
    pub name: String,
    /// Parent folder ID. Use "0" for root.
    pub parent_folder_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateFolderOutput {
    pub folder_id: String,
    pub name: String,
}

/// # Create Box Folder
///
/// Creates a new folder in Box at the specified parent location.
///
/// Use this tool when a user wants to create a new folder in their Box account
/// to organize files and other folders. The user must provide a folder name and
/// the parent folder ID where the new folder should be created (use "0" for the
/// root folder). This tool returns the newly created folder ID and name, which
/// can be used for subsequent operations.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - file-storage
/// - box
/// - folder
///
/// # Errors
///
/// Returns an error if:
/// - The `name` is empty or contains only whitespace
/// - The `parent_folder_id` is empty or contains only whitespace
/// - The Box credentials are missing or invalid
/// - The Box API request fails (network error, authentication error, etc.)
/// - The API response cannot be parsed
#[tool]
pub async fn create_folder(ctx: Context, input: CreateFolderInput) -> Result<CreateFolderOutput> {
    ensure!(!input.name.trim().is_empty(), "name must not be empty");
    ensure!(
        !input.parent_folder_id.trim().is_empty(),
        "parent_folder_id must not be empty"
    );

    let client = BoxClient::from_ctx(&ctx)?;

    let request = CreateFolderRequest {
        name: input.name,
        parent: ParentReference {
            id: input.parent_folder_id,
        },
    };

    let folder: BoxFolder = client
        .post_json(client.url_with_path("/folders")?, &request)
        .await?;

    Ok(CreateFolderOutput {
        folder_id: folder.id,
        name: folder.name,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ShareLinkInput {
    /// Item ID (file or folder) to share.
    pub item_id: String,
    /// Item type: "file" or "folder".
    pub item_type: String,
    /// Access level: "open", "company", "collaborators".
    pub access: String,
    /// Optional password for the shared link.
    #[serde(default)]
    pub password: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ShareLinkOutput {
    pub shared_link_url: String,
    pub access: String,
}

/// # Create Box Shared Link
///
/// Creates a shared link for a Box file or folder with configurable access
/// permissions.
///
/// Use this tool when a user wants to share a file or folder from their Box
/// account with others via a URL. The user can specify the access level (open
/// to anyone, within the company, or restricted to collaborators) and
/// optionally set a password for additional security. This tool returns the
/// shared link URL that can be distributed to grant others access to the item
/// according to the specified permissions.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - file-storage
/// - box
/// - share
///
/// # Errors
///
/// Returns an error if:
/// - The `item_id` is empty or contains only whitespace
/// - The `item_type` is not "file" or "folder"
/// - The access level is not "open", "company", or "collaborators"
/// - The Box credentials are missing or invalid
/// - The Box API request fails (network error, authentication error, etc.)
/// - The API response cannot be parsed
#[tool]
pub async fn share_link(ctx: Context, input: ShareLinkInput) -> Result<ShareLinkOutput> {
    ensure!(
        !input.item_id.trim().is_empty(),
        "item_id must not be empty"
    );
    ensure!(
        input.item_type == "file" || input.item_type == "folder",
        "item_type must be 'file' or 'folder'"
    );
    ensure!(
        matches!(input.access.as_str(), "open" | "company" | "collaborators"),
        "access must be 'open', 'company', or 'collaborators'"
    );

    let client = BoxClient::from_ctx(&ctx)?;

    let request = SharedLinkRequest {
        shared_link: SharedLinkAccess {
            access: input.access.clone(),
            password: input.password,
        },
    };

    let path = match input.item_type.as_str() {
        "file" => format!("/files/{}?fields=shared_link", input.item_id),
        "folder" => format!("/folders/{}?fields=shared_link", input.item_id),
        _ => unreachable!(),
    };

    let response: ShareLinkResponse = client
        .put_json(client.url_with_path(&path)?, &request)
        .await?;

    Ok(ShareLinkOutput {
        shared_link_url: response.shared_link.url,
        access: input.access,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetPermissionsInput {
    /// Item ID (file or folder) to set permissions on.
    pub item_id: String,
    /// Item type: "file" or "folder".
    pub item_type: String,
    /// Email address of the user to grant access.
    pub user_email: String,
    /// Role: "editor", "viewer", "previewer", "uploader",
    /// "`previewer_uploader`", "`viewer_uploader`", "co-owner".
    pub role: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SetPermissionsOutput {
    pub collaboration_id: String,
    pub role: String,
}

/// # Set Box File Permissions
///
/// Adds a collaborator to a Box file or folder with specific permission roles.
///
/// Use this tool when a user wants to grant another person access to a file or
/// folder in their Box account. The user must specify the collaborator's email
/// address and the desired permission role (e.g., editor, viewer, uploader,
/// co-owner, etc.). This tool creates a collaboration that allows the specified
/// user to access the item according to the assigned role. The collaboration ID
/// and role are returned for reference.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - file-storage
/// - box
/// - permissions
///
/// # Errors
///
/// Returns an error if:
/// - The `item_id` is empty or contains only whitespace
/// - The `item_type` is not "file" or "folder"
/// - The `user_email` is empty or contains only whitespace
/// - The `role` is not one of the valid roles
/// - The Box credentials are missing or invalid
/// - The Box API request fails (network error, authentication error, etc.)
/// - The API response cannot be parsed
#[tool]
pub async fn set_permissions(
    ctx: Context,
    input: SetPermissionsInput,
) -> Result<SetPermissionsOutput> {
    ensure!(
        !input.item_id.trim().is_empty(),
        "item_id must not be empty"
    );
    ensure!(
        input.item_type == "file" || input.item_type == "folder",
        "item_type must be 'file' or 'folder'"
    );
    ensure!(
        !input.user_email.trim().is_empty(),
        "user_email must not be empty"
    );
    ensure!(
        matches!(
            input.role.as_str(),
            "editor"
                | "viewer"
                | "previewer"
                | "uploader"
                | "previewer_uploader"
                | "viewer_uploader"
                | "co-owner"
        ),
        "role must be one of: editor, viewer, previewer, uploader, previewer_uploader, \
         viewer_uploader, co-owner"
    );

    let client = BoxClient::from_ctx(&ctx)?;

    let request = CollaborationRequest {
        item: CollaborationItem {
            item_type: input.item_type,
            id: input.item_id,
        },
        accessible_by: AccessibleBy {
            user_type: "user".to_string(),
            id: None,
            login: Some(input.user_email),
        },
        role: input.role.clone(),
    };

    let collab: Collaboration = client
        .post_json(client.url_with_path("/collaborations")?, &request)
        .await?;

    Ok(SetPermissionsOutput {
        collaboration_id: collab.id,
        role: input.role,
    })
}

fn format_path(path_collection: &PathCollection) -> String {
    path_collection
        .entries
        .iter()
        .map(|e| e.name.as_str())
        .collect::<Vec<_>>()
        .join("/")
}

/// Decodes a base64-encoded string.
///
/// # Errors
///
/// Returns an error if the input is not valid base64 encoding.
fn base64_decode(input: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(input)
        .map_err(|e| operai::anyhow::anyhow!("Failed to decode base64: {e}"))
}

#[derive(Deserialize)]
struct UploadResponse {
    entries: Vec<BoxFile>,
}

#[derive(Deserialize)]
struct ShareLinkResponse {
    shared_link: SharedLink,
}

#[derive(Debug, Clone)]
struct BoxClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl BoxClient {
    /// Creates a new `BoxClient` from the provided context.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Box credentials are not found in the context
    /// - The `access_token` is empty or contains only whitespace
    /// - The endpoint URL is invalid
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = BoxCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_BOX_ENDPOINT))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            access_token: cred.access_token,
        })
    }

    /// Constructs a full URL by combining the base URL with the provided path.
    ///
    /// # Errors
    ///
    /// Returns an error if the resulting URL is invalid.
    fn url_with_path(&self, path: &str) -> Result<reqwest::Url> {
        let url_str = format!("{}{}", self.base_url, path);
        reqwest::Url::parse(&url_str).map_err(|e| operai::anyhow::anyhow!("Invalid URL: {e}"))
    }

    /// Sends a GET request and parses the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails (network error, connection timeout, etc.)
    /// - The response status indicates an error
    /// - The response body cannot be parsed as JSON
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

    /// Sends a POST request with a JSON body and parses the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails (network error, connection timeout, etc.)
    /// - The response status indicates an error
    /// - The response body cannot be parsed as JSON
    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
    ) -> Result<TRes> {
        let response = self
            .http
            .post(url)
            .json(body)
            .bearer_auth(&self.access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Sends a POST request with multipart/form-data and parses the JSON
    /// response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails (network error, connection timeout, etc.)
    /// - The response status indicates an error
    /// - The response body cannot be parsed as JSON
    async fn post_multipart<T: for<'de> Deserialize<'de>>(
        &self,
        url: String,
        form: reqwest::multipart::Form,
    ) -> Result<T> {
        let response = self
            .http
            .post(&url)
            .multipart(form)
            .bearer_auth(&self.access_token)
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Sends a PUT request with a JSON body and parses the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails (network error, connection timeout, etc.)
    /// - The response status indicates an error
    /// - The response body cannot be parsed as JSON
    async fn put_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
    ) -> Result<TRes> {
        let response = self
            .http
            .put(url)
            .json(body)
            .bearer_auth(&self.access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Handles an HTTP response, parsing successful responses or returning an
    /// error for failures.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The response status indicates an error (non-2xx)
    /// - The response body cannot be parsed as JSON (for successful responses)
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
                "Box API request failed ({status}): {body}"
            ))
        }
    }
}

/// Normalizes a base URL by trimming whitespace and removing trailing slashes.
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

    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_partial_json, header, method, path, query_param},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut box_values = HashMap::new();
        box_values.insert("access_token".to_string(), "test-token".to_string());
        box_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("box", box_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        server.uri()
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_search_item_serialization_roundtrip() {
        let item = SearchItem {
            id: "123".to_string(),
            item_type: "file".to_string(),
            name: "test.pdf".to_string(),
            size: Some(1024),
            path: Some("/Documents".to_string()),
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: SearchItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item.id, parsed.id);
        assert_eq!(item.name, parsed.name);
    }

    #[test]
    fn test_box_file_deserializes_with_optional_fields() {
        let json = r#"{"id":"123","type":"file","name":"test.pdf"}"#;
        let parsed: BoxFile = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.id, "123");
        assert_eq!(parsed.name, "test.pdf");
        assert!(parsed.size.is_none());
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://api.box.com/2.0/").unwrap();
        assert_eq!(result, "https://api.box.com/2.0");
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

        let result = search(
            ctx,
            SearchInput {
                query: "   ".to_string(),
                limit: None,
                file_extensions: vec![],
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

        let result = search(
            ctx,
            SearchInput {
                query: "test".to_string(),
                limit: Some(0),
                file_extensions: vec![],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("limit must be between 1 and 200")
        );
    }

    #[tokio::test]
    async fn test_download_empty_file_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = download(
            ctx,
            DownloadInput {
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

    #[tokio::test]
    async fn test_upload_empty_parent_folder_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = upload(
            ctx,
            UploadInput {
                parent_folder_id: "  ".to_string(),
                file_name: "test.txt".to_string(),
                content_base64: "SGVsbG8=".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("parent_folder_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_create_folder_empty_name_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = create_folder(
            ctx,
            CreateFolderInput {
                name: "  ".to_string(),
                parent_folder_id: "0".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("name must not be empty")
        );
    }

    #[tokio::test]
    async fn test_share_link_invalid_item_type_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = share_link(
            ctx,
            ShareLinkInput {
                item_id: "123".to_string(),
                item_type: "invalid".to_string(),
                access: "open".to_string(),
                password: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("item_type must be")
        );
    }

    #[tokio::test]
    async fn test_share_link_invalid_access_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = share_link(
            ctx,
            ShareLinkInput {
                item_id: "123".to_string(),
                item_type: "file".to_string(),
                access: "invalid".to_string(),
                password: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("access must be"));
    }

    #[tokio::test]
    async fn test_set_permissions_invalid_role_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = set_permissions(
            ctx,
            SetPermissionsInput {
                item_id: "123".to_string(),
                item_type: "file".to_string(),
                user_email: "test@example.com".to_string(),
                role: "invalid".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("role must be one of")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_search_success_returns_items() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "total_count": 1,
          "entries": [
            {
              "type": "file",
              "id": "file-123",
              "name": "document.pdf",
              "size": 2048
            }
          ],
          "offset": 0,
          "limit": 30
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/search"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("query", "document"))
            .and(query_param("limit", "30"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = search(
            ctx,
            SearchInput {
                query: "document".to_string(),
                limit: None,
                file_extensions: vec![],
            },
        )
        .await
        .unwrap();

        assert_eq!(output.total_count, 1);
        assert_eq!(output.items.len(), 1);
        assert_eq!(output.items[0].id, "file-123");
        assert_eq!(output.items[0].name, "document.pdf");
        assert_eq!(output.items[0].size, Some(2048));
    }

    #[tokio::test]
    async fn test_download_success_returns_url() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "type": "file",
          "id": "file-123",
          "name": "test.pdf",
          "size": 1024
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/files/file-123"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = download(
            ctx,
            DownloadInput {
                file_id: "file-123".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.file_name, "test.pdf");
        assert!(output.download_url.contains("file-123"));
    }

    #[tokio::test]
    async fn test_create_folder_success_returns_folder_id() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "type": "folder",
          "id": "folder-456",
          "name": "New Folder"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/folders"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(serde_json::json!({
                "name": "New Folder",
                "parent": { "id": "0" }
            })))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = create_folder(
            ctx,
            CreateFolderInput {
                name: "New Folder".to_string(),
                parent_folder_id: "0".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.folder_id, "folder-456");
        assert_eq!(output.name, "New Folder");
    }

    #[tokio::test]
    async fn test_share_link_success_returns_url() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "type": "file",
          "id": "file-123",
          "shared_link": {
            "url": "https://box.com/s/abc123",
            "access": "open"
          }
        }
        "#;

        Mock::given(method("PUT"))
            .and(path("/files/file-123"))
            .and(query_param("fields", "shared_link"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = share_link(
            ctx,
            ShareLinkInput {
                item_id: "file-123".to_string(),
                item_type: "file".to_string(),
                access: "open".to_string(),
                password: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.shared_link_url, "https://box.com/s/abc123");
        assert_eq!(output.access, "open");
    }

    #[tokio::test]
    async fn test_share_link_folder_success_returns_url() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "type": "folder",
          "id": "folder-456",
          "shared_link": {
            "url": "https://box.com/s/def456",
            "access": "company"
          }
        }
        "#;

        Mock::given(method("PUT"))
            .and(path("/folders/folder-456"))
            .and(query_param("fields", "shared_link"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = share_link(
            ctx,
            ShareLinkInput {
                item_id: "folder-456".to_string(),
                item_type: "folder".to_string(),
                access: "company".to_string(),
                password: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.shared_link_url, "https://box.com/s/def456");
        assert_eq!(output.access, "company");
    }

    #[tokio::test]
    async fn test_set_permissions_success_returns_collaboration_id() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "type": "collaboration",
          "id": "collab-789",
          "role": "editor"
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/collaborations"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = set_permissions(
            ctx,
            SetPermissionsInput {
                item_id: "file-123".to_string(),
                item_type: "file".to_string(),
                user_email: "user@example.com".to_string(),
                role: "editor".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.collaboration_id, "collab-789");
        assert_eq!(output.role, "editor");
    }

    #[tokio::test]
    async fn test_search_error_response_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(401).set_body_raw(
                r#"{"type":"error","status":401,"code":"unauthorized","message":"Invalid token"}"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = search(
            ctx,
            SearchInput {
                query: "test".to_string(),
                limit: None,
                file_extensions: vec![],
            },
        )
        .await;

        let message = result.unwrap_err().to_string();
        assert!(message.contains("401"));
    }
}
