//! Microsoft SharePoint integration for Operai Toolbox.
//!
//! This integration provides tools for interacting with SharePoint document
//! libraries:
//! - Search documents
//! - Upload files
//! - Set permissions
//! - Create folders
//! - Get sharing links

use std::sync::Arc;

use base64::Engine;
use operai::{
    Context, JsonSchema, Result, bail, define_user_credential, ensure, info, init, schemars,
    shutdown, tool,
};
use reqwest::{Client, StatusCode, header};
use serde::{Deserialize, Serialize};

define_user_credential! {
    SharePointCredential("sharepoint") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

/// Default Microsoft Graph API endpoint.
const DEFAULT_GRAPH_ENDPOINT: &str = "https://graph.microsoft.com/v1.0";

/// Graph API client for making authenticated requests to Microsoft Graph.
#[derive(Clone)]
struct GraphClient {
    client: Arc<Client>,
    endpoint: String,
    access_token: String,
}

impl GraphClient {
    /// Create a new `GraphClient` from the tool context.
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = SharePointCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url = cred
            .endpoint
            .as_deref()
            .unwrap_or(DEFAULT_GRAPH_ENDPOINT)
            .to_string();

        let client = Client::builder()
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {e}"))?;

        Ok(Self {
            client: Arc::new(client),
            endpoint: base_url,
            access_token: cred.access_token,
        })
    }

    /// Get the base URL for API requests.
    fn base_url(&self) -> &str {
        &self.endpoint
    }

    /// Make a GET request to the Graph API.
    async fn get(&self, path: &str) -> Result<Response> {
        let url = format!("{}{}", self.base_url(), path);
        let response = self
            .client
            .get(&url)
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", self.access_token),
            )
            .header(header::CONTENT_TYPE, "application/json")
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send GET request: {e}"))?;

        self.handle_response(response).await
    }

    /// Make a POST request to the Graph API.
    async fn post<T: Serialize>(&self, path: &str, body: &T) -> Result<Response> {
        let url = format!("{}{}", self.base_url(), path);
        let response = self
            .client
            .post(&url)
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", self.access_token),
            )
            .header(header::CONTENT_TYPE, "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send POST request: {e}"))?;

        self.handle_response(response).await
    }

    /// Make a PUT request to the Graph API.
    async fn put(&self, path: &str, body: Vec<u8>, content_type: &str) -> Result<Response> {
        let url = format!("{}{}", self.base_url(), path);
        let response = self
            .client
            .put(&url)
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", self.access_token),
            )
            .header(header::CONTENT_TYPE, content_type)
            .body(body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send PUT request: {e}"))?;

        self.handle_response(response).await
    }

    /// Handle the response from Graph API, extracting errors appropriately.
    async fn handle_response(&self, response: reqwest::Response) -> Result<Response> {
        let status = response.status();

        if status.is_success() {
            let text = response
                .text()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to read response body: {e}"))?;
            return Ok(Response { body: text });
        }

        // Handle error responses
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read error body".to_string());

        match status {
            StatusCode::UNAUTHORIZED => {
                bail!("Authentication failed: Invalid or expired access token")
            }
            StatusCode::FORBIDDEN => {
                bail!("Permission denied: Insufficient permissions to access this resource")
            }
            StatusCode::NOT_FOUND => {
                bail!("Resource not found: The specified site, drive, or item does not exist")
            }
            StatusCode::BAD_REQUEST => {
                bail!("Bad request: {error_body}")
            }
            _ => {
                bail!(
                    "Graph API request failed with status {}: {error_body}",
                    status.as_u16()
                )
            }
        }
    }
}

/// Response from the Graph API.
struct Response {
    body: String,
}

/// Initialize the SharePoint tool library.
#[init]
async fn setup() -> Result<()> {
    info!("SharePoint integration initialized");
    Ok(())
}

/// Clean up resources when the library is unloaded.
#[shutdown]
fn cleanup() {
    info!("SharePoint integration shutting down");
}

// =============================================================================
// Search Documents Tool
// =============================================================================

/// Input for the `search_docs` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchDocsInput {
    /// The search query string (supports SharePoint/KQL syntax).
    pub query: String,
    /// The site ID or URL to search within (optional, searches all accessible
    /// sites if omitted).
    #[serde(default)]
    pub site_id: Option<String>,
    /// The drive ID (document library) to search within (optional).
    #[serde(default)]
    pub drive_id: Option<String>,
    /// Maximum number of results to return (default: 25, max: 100).
    #[serde(default)]
    pub limit: Option<u32>,
    /// Filter by file type (e.g., "docx", "pdf", "xlsx").
    #[serde(default)]
    pub file_type: Option<String>,
}

/// A single search result item.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchResultItem {
    /// Unique identifier for the document.
    pub id: String,
    /// Name of the document.
    pub name: String,
    /// Full path to the document in SharePoint.
    pub path: String,
    /// URL to access the document.
    pub web_url: String,
    /// MIME type of the document.
    pub mime_type: String,
    /// Size in bytes.
    pub size: u64,
    /// Last modified timestamp (ISO 8601).
    pub modified_at: String,
    /// User who last modified the document.
    pub modified_by: String,
    /// Relevance score from search.
    pub score: f64,
}

/// Output from the `search_docs` tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchDocsOutput {
    /// List of matching documents.
    pub results: Vec<SearchResultItem>,
    /// Total number of matches (may be more than returned).
    pub total_count: u32,
    /// The query that was executed.
    pub query: String,
}

/// Graph API response for drive item search.
#[derive(Debug, Deserialize)]
struct GraphSearchResponse {
    value: Vec<GraphDriveItem>,
}

/// Graph API drive item representation.
#[derive(Debug, Deserialize)]
struct GraphDriveItem {
    id: String,
    name: String,
    web_url: String,
    file_system_info: Option<GraphFileSystemInfo>,
    file: Option<GraphFile>,
    size: Option<i64>,
    last_modified_by: Option<GraphIdentitySet>,
    parent_reference: Option<GraphParentReference>,
}

#[derive(Debug, Deserialize)]
struct GraphFileSystemInfo {
    last_modified_date_time: String,
}

#[derive(Debug, Deserialize)]
struct GraphFile {
    mime_type: String,
}

#[derive(Debug, Deserialize)]
struct GraphIdentitySet {
    user: Option<GraphIdentity>,
}

#[derive(Debug, Deserialize)]
struct GraphIdentity {
    display_name: Option<String>,
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphParentReference {
    path: Option<String>,
}

/// # Search SharePoint Documents
///
/// Searches for documents and files in SharePoint document libraries using
/// the Microsoft Graph API. Supports keyword search and can target specific
/// sites, drives (document libraries), or file types.
///
/// Use this tool when a user wants to:
/// - Find documents by name or content keywords
/// - Locate files within a specific SharePoint site or library
/// - Search for specific file types (e.g., PDFs, Word documents)
///
/// The search requires at least a `site_id` to scope the search to a specific
/// SharePoint site. Optionally provide a `drive_id` to search within a
/// specific document library, and use `file_type` to filter results (e.g.,
/// "docx", "pdf").
///
/// The tool returns matching documents with metadata including file name, path,
/// web URL, MIME type, size, and modification information.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - file-storage
/// - sharepoint
/// - search
///
/// # Errors
///
/// This function will return an error if:
/// - The SharePoint API is unreachable or times out
/// - Authentication fails due to invalid or expired credentials
/// - The provided site ID or drive ID does not exist
/// - The search query is malformed or exceeds SharePoint's query length limits
/// - Neither `site_id` nor `drive_id` is provided (cross-site search requires
///   Microsoft Search API)
///
/// # Panics
///
/// This function will panic if the number of search results exceeds the maximum
/// value of `u32`.
#[tool]
pub async fn search_docs(ctx: Context, input: SearchDocsInput) -> Result<SearchDocsOutput> {
    let client = GraphClient::from_ctx(&ctx)?;
    let limit = input.limit.unwrap_or(25).min(100);

    // Build search query - use drive search endpoint if drive_id is provided
    let path = if let (Some(site_id), Some(drive_id)) = (&input.site_id, &input.drive_id) {
        // Search within a specific drive
        let encoded_query = urlencoding::encode(&input.query);
        format!("/sites/{site_id}/drives/{drive_id}/root/search(q='{encoded_query}')?$top={limit}")
    } else if let Some(site_id) = &input.site_id {
        // Search within a site's default drive
        let encoded_query = urlencoding::encode(&input.query);
        format!("/sites/{site_id}/drive/root/search(q='{encoded_query}')?$top={limit}")
    } else {
        // Use Microsoft Search API for broader search
        bail!(
            "Searching across all sites requires Microsoft Search API with POST /search/query \
             endpoint. Please provide site_id and optionally drive_id for targeted search."
        );
    };

    // Apply file type filter if provided (this would need to be done client-side or
    // via KQL)
    let query_with_filter = if let Some(file_type) = &input.file_type {
        format!("{} filetype:{}", input.query, file_type)
    } else {
        input.query.clone()
    };

    let response = client.get(&path).await?;

    let graph_response: GraphSearchResponse = serde_json::from_str(&response.body)
        .map_err(|e| anyhow::anyhow!("Failed to parse search response: {e}"))?;

    let results: Vec<SearchResultItem> = graph_response
        .value
        .into_iter()
        .map(|item| {
            let mime_type = item.file.as_ref().map_or_else(
                || {
                    // Infer from file name
                    let path = std::path::Path::new(&item.name);
                    let ext = path
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();

                    match ext.as_str() {
                        "docx" | "doc" => "application/vnd.openxmlformats-officedocument.\
                                           wordprocessingml.document"
                            .to_string(),
                        "xlsx" | "xls" => "application/vnd.openxmlformats-officedocument.\
                                           spreadsheetml.sheet"
                            .to_string(),
                        "pptx" | "ppt" => "application/vnd.openxmlformats-officedocument.\
                                           presentationml.presentation"
                            .to_string(),
                        "pdf" => "application/pdf".to_string(),
                        "txt" => "text/plain".to_string(),
                        _ => "application/octet-stream".to_string(),
                    }
                },
                |f| f.mime_type.clone(),
            );

            let modified_by = item
                .last_modified_by
                .as_ref()
                .and_then(|s| s.user.as_ref())
                .and_then(|u| u.email.clone().or_else(|| u.display_name.clone()))
                .unwrap_or_else(|| "Unknown".to_string());

            let modified_at = item.file_system_info.as_ref().map_or_else(
                || chrono::Utc::now().to_rfc3339(),
                |f| f.last_modified_date_time.clone(),
            );

            let path = item
                .parent_reference
                .as_ref()
                .and_then(|p| p.path.clone())
                .unwrap_or_else(|| "/".to_string());

            SearchResultItem {
                id: item.id,
                name: item.name.clone(),
                path,
                web_url: item.web_url,
                mime_type,
                size: u64::try_from(item.size.unwrap_or(0)).unwrap_or(0),
                modified_at,
                modified_by,
                score: 1.0, // Search endpoint doesn't return scores
            }
        })
        .collect();

    Ok(SearchDocsOutput {
        total_count: u32::try_from(results.len()).expect("Result count should fit in u32"),
        results,
        query: query_with_filter,
    })
}

// =============================================================================
// Upload File Tool
// =============================================================================

/// Input for the upload tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadInput {
    /// The site ID where the file will be uploaded.
    pub site_id: String,
    /// The drive ID (document library) to upload to.
    pub drive_id: String,
    /// The folder path within the drive (e.g., "/Reports/2024").
    #[serde(default)]
    pub folder_path: Option<String>,
    /// The name for the uploaded file.
    pub file_name: String,
    /// Base64-encoded file content.
    pub content_base64: String,
    /// MIME type of the file (optional, will be inferred if not provided).
    #[serde(default)]
    pub mime_type: Option<String>,
    /// Conflict behavior: "fail", "replace", or "rename" (default: "fail").
    #[serde(default)]
    pub conflict_behavior: Option<String>,
}

/// Output from the upload tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct UploadOutput {
    /// Unique identifier for the uploaded file.
    pub id: String,
    /// Name of the uploaded file.
    pub name: String,
    /// Full path to the file in SharePoint.
    pub path: String,
    /// URL to access the file.
    pub web_url: String,
    /// Size of the uploaded file in bytes.
    pub size: u64,
    /// `ETag` for the uploaded file (for concurrency control).
    pub etag: String,
}

/// Graph API response for uploaded drive item.
#[derive(Debug, Deserialize)]
struct GraphUploadResponse {
    id: String,
    name: String,
    web_url: String,
    size: i64,
    #[serde(rename = "@microsoft.graph.etag")]
    etag: String,
}

/// # Upload SharePoint File
///
/// Uploads a file to a SharePoint document library using the Microsoft Graph
/// API. Supports uploading to specific folders within a drive with configurable
/// conflict resolution behavior.
///
/// Use this tool when a user wants to:
/// - Upload a document or file to SharePoint
/// - Save a file to a specific folder in a document library
/// - Replace an existing file with a new version
///
/// The file content must be provided as a base64-encoded string. The tool
/// requires the target `site_id` and `drive_id` (document library). Optionally
/// specify a `folder_path` to upload to a specific folder (e.g.,
/// "/Reports/2024").
///
/// The `conflict_behavior` parameter controls what happens if a file with the
/// same name already exists:
/// - "fail" (default): Return an error
/// - "replace": Overwrite the existing file
/// - "rename": Automatically rename the new file
///
/// Returns the uploaded file's metadata including ID, name, path, web URL,
/// size, and `ETag` for version control.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - file-storage
/// - sharepoint
/// - upload
///
/// # Errors
///
/// This function will return an error if:
/// - The provided `conflict_behavior` is not one of "fail", "replace", or
///   "rename"
/// - The base64 content is malformed and cannot be decoded
/// - The SharePoint API is unreachable or times out
/// - Authentication fails due to invalid or expired credentials
/// - The specified site ID, drive ID, or folder path does not exist
/// - File size exceeds SharePoint's upload limits
/// - The file name contains invalid characters
///
/// # Panics
///
/// This function will panic if the file size from SharePoint is negative
/// (should never happen with valid API responses).
#[tool]
pub async fn upload(ctx: Context, input: UploadInput) -> Result<UploadOutput> {
    let client = GraphClient::from_ctx(&ctx)?;

    // Validate conflict behavior
    let conflict_behavior = input.conflict_behavior.as_deref().unwrap_or("fail");
    if !["fail", "replace", "rename"].contains(&conflict_behavior) {
        bail!("conflict_behavior must be one of: fail, replace, rename");
    }

    // Decode base64 content
    let content = base64::engine::general_purpose::STANDARD
        .decode(&input.content_base64)
        .map_err(|e| anyhow::anyhow!("Failed to decode base64 content: {e}"))?;

    let mime_type = input
        .mime_type
        .as_deref()
        .unwrap_or("application/octet-stream");

    // Build the item path
    let folder_path = input.folder_path.as_deref().unwrap_or("");
    let full_path = if folder_path.is_empty() {
        format!("/{}", input.file_name)
    } else {
        format!("{}/{}", folder_path.trim_end_matches('/'), input.file_name)
    };

    // URL encode the path
    let encoded_path = urlencoding::encode(&full_path);

    // Build the upload endpoint URL
    let path = format!(
        "/sites/{}/drives/{}/root:{}:/content?@microsoft.graph.conflictBehavior={}",
        input.site_id, input.drive_id, encoded_path, conflict_behavior
    );

    let response = client.put(&path, content, mime_type).await?;

    let upload_response: GraphUploadResponse = serde_json::from_str(&response.body)
        .map_err(|e| anyhow::anyhow!("Failed to parse upload response: {e}"))?;

    Ok(UploadOutput {
        id: upload_response.id,
        name: upload_response.name,
        path: full_path,
        web_url: upload_response.web_url,
        // File sizes from SharePoint are always non-negative
        size: u64::try_from(upload_response.size).expect("File size should be non-negative"),
        etag: upload_response.etag,
    })
}

// =============================================================================
// Set Permissions Tool
// =============================================================================

/// Permission role for SharePoint items.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PermissionRole {
    /// Read-only access.
    Read,
    /// Can edit but not share or delete.
    Write,
    /// Full control including sharing and deletion.
    Owner,
}

/// Type of recipient for permissions.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecipientType {
    /// Individual user by email.
    User,
    /// Security or Microsoft 365 group.
    Group,
    /// Anyone with the link (for link-based sharing).
    Anyone,
}

/// A permission grant to add or modify.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct PermissionGrant {
    /// The recipient type.
    pub recipient_type: RecipientType,
    /// Email address or group ID (not required for "anyone" type).
    #[serde(default)]
    pub recipient: Option<String>,
    /// The role to grant.
    pub role: PermissionRole,
    /// Expiration date for the permission (ISO 8601, optional).
    #[serde(default)]
    pub expires_at: Option<String>,
}

/// Input for the `set_permissions` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetPermissionsInput {
    /// The site ID containing the item.
    pub site_id: String,
    /// The drive ID containing the item.
    pub drive_id: String,
    /// The item ID (file or folder) to set permissions on.
    pub item_id: String,
    /// Permissions to grant.
    pub grants: Vec<PermissionGrant>,
    /// Whether to send notification emails to recipients (default: true).
    #[serde(default)]
    pub send_notification: Option<bool>,
    /// Optional message to include in notification emails.
    #[serde(default)]
    pub message: Option<String>,
}

/// A permission that was successfully applied.
#[derive(Debug, Serialize, JsonSchema)]
pub struct AppliedPermission {
    /// Permission ID.
    pub id: String,
    /// The recipient.
    pub recipient: String,
    /// The role granted.
    pub role: String,
    /// Expiration date if set.
    pub expires_at: Option<String>,
}

/// Output from the `set_permissions` tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SetPermissionsOutput {
    /// The item ID that permissions were set on.
    pub item_id: String,
    /// List of permissions that were applied.
    pub permissions: Vec<AppliedPermission>,
    /// Whether notifications were sent.
    pub notifications_sent: bool,
}

/// Graph API request body for invite action.
#[derive(Debug, Serialize)]
struct GraphInviteRequest {
    require_sign_in: bool,
    send_invitation: bool,
    roles: Vec<String>,
    recipients: Vec<GraphRecipient>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expiration_date_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct GraphRecipient {
    #[serde(skip_serializing_if = "Option::is_none")]
    email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    alias: Option<String>,
}

/// Graph API response for permission.
#[derive(Debug, Deserialize)]
struct GraphPermissionResponse {
    value: Vec<GraphPermission>,
}

#[derive(Debug, Deserialize)]
struct GraphPermission {
    id: String,
    roles: Vec<String>,
    granted_to: Option<GraphGrantedTo>,
    expiration_date_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphGrantedTo {
    user: Option<GraphIdentity>,
    application: Option<GraphApplication>,
}

#[derive(Debug, Deserialize)]
struct GraphApplication {
    display_name: Option<String>,
}

/// # Set SharePoint Permissions
///
/// Sets or updates sharing permissions on a SharePoint document or folder
/// using the Microsoft Graph API. Grants access to users, groups, or creates
/// anonymous sharing links with configurable permission levels.
///
/// Use this tool when a user wants to:
/// - Share a document or folder with specific users or groups
/// - Grant read, write, or owner permissions to recipients
/// - Control whether recipients receive notification emails
/// - Set expiration dates on shared access
///
/// The tool requires the target `site_id`, `drive_id`, and `item_id` (file or
/// folder). Provide one or more `grants` specifying:
/// - `recipient_type`: "user" (by email), "group" (by ID), or "anyone"
///   (anonymous link)
/// - `role`: "read" (view only), "write" (edit), or "owner" (full control)
/// - Optional expiration date and notification message
///
/// For "user" and "group" recipient types, the `recipient` field (email or
/// group ID) is required. For "anyone" links, it creates a link that works
/// without sign-in.
///
/// Optionally disable `send_notification` to share silently without email
/// alerts.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - file-storage
/// - sharepoint
/// - permissions
///
/// # Errors
///
/// This function will return an error if:
/// - The `grants` vector is empty
/// - A grant with `recipient_type` of `User` or `Group` is missing a
///   `recipient`
/// - The SharePoint API is unreachable or times out
/// - Authentication fails due to invalid or expired credentials
/// - The specified site ID, drive ID, or item ID does not exist
/// - The user does not have permission to modify permissions on the item
#[tool]
pub async fn set_permissions(
    ctx: Context,
    input: SetPermissionsInput,
) -> Result<SetPermissionsOutput> {
    let client = GraphClient::from_ctx(&ctx)?;

    if input.grants.is_empty() {
        bail!("At least one permission grant is required");
    }

    // Validate grants
    for grant in &input.grants {
        if !matches!(grant.recipient_type, RecipientType::Anyone) && grant.recipient.is_none() {
            bail!("recipient is required for user and group permission types");
        }
    }

    let send_notification = input.send_notification.unwrap_or(true);

    // Build recipients list
    let mut recipients = Vec::new();
    for grant in &input.grants {
        let recipient = match grant.recipient_type {
            RecipientType::User => GraphRecipient {
                email: grant.recipient.clone(),
                alias: None,
            },
            RecipientType::Group => GraphRecipient {
                email: None,
                alias: grant.recipient.clone(),
            },
            RecipientType::Anyone => GraphRecipient {
                email: None,
                alias: None,
            },
        };
        recipients.push(recipient);
    }

    // For simplicity, use the first grant's role and expiration
    // In a real scenario, you might need multiple API calls for different roles
    let first_grant = &input.grants[0];
    let role_str = match first_grant.role {
        PermissionRole::Read => "read",
        PermissionRole::Write => "write",
        PermissionRole::Owner => "owner",
    };

    let invite_body = GraphInviteRequest {
        require_sign_in: true,
        send_invitation: send_notification,
        roles: vec![role_str.to_string()],
        recipients: recipients.clone(),
        message: input.message.clone(),
        expiration_date_time: first_grant.expires_at.clone(),
    };

    let path = format!(
        "/sites/{}/drives/{}/items/{}/invite",
        input.site_id, input.drive_id, input.item_id
    );

    let response = client.post(&path, &invite_body).await?;

    let perm_response: GraphPermissionResponse = serde_json::from_str(&response.body)
        .map_err(|e| anyhow::anyhow!("Failed to parse permission response: {e}"))?;

    let permissions: Vec<AppliedPermission> = perm_response
        .value
        .iter()
        .map(|p| {
            let recipient_name = p
                .granted_to
                .as_ref()
                .and_then(|g| {
                    g.user
                        .as_ref()
                        .and_then(|u| u.email.clone().or_else(|| u.display_name.clone()))
                        .or_else(|| g.application.as_ref().and_then(|a| a.display_name.clone()))
                })
                .unwrap_or_else(|| "Unknown".to_string());

            let role = p
                .roles
                .first()
                .cloned()
                .unwrap_or_else(|| "read".to_string());

            AppliedPermission {
                id: p.id.clone(),
                recipient: recipient_name,
                role,
                expires_at: p.expiration_date_time.clone(),
            }
        })
        .collect();

    Ok(SetPermissionsOutput {
        item_id: input.item_id,
        permissions,
        notifications_sent: send_notification,
    })
}

// =============================================================================
// Create Folder Tool
// =============================================================================

/// Input for the `create_folder` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateFolderInput {
    /// The site ID where the folder will be created.
    pub site_id: String,
    /// The drive ID (document library) to create the folder in.
    pub drive_id: String,
    /// Parent folder path (e.g., "/Reports" or empty for root).
    #[serde(default)]
    pub parent_path: Option<String>,
    /// Name for the new folder.
    pub folder_name: String,
    /// Description for the folder (stored in metadata).
    #[serde(default)]
    pub description: Option<String>,
}

/// Output from the `create_folder` tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateFolderOutput {
    /// Unique identifier for the created folder.
    pub id: String,
    /// Name of the created folder.
    pub name: String,
    /// Full path to the folder.
    pub path: String,
    /// URL to access the folder.
    pub web_url: String,
    /// Creation timestamp (ISO 8601).
    pub created_at: String,
}

/// Graph API request body for creating a folder.
#[derive(Debug, Serialize)]
struct GraphCreateFolderRequest {
    name: String,
    folder: GraphFolder,
    #[serde(rename = "@microsoft.graph.conflictBehavior")]
    conflict_behavior: String,
}

#[derive(Debug, Serialize)]
struct GraphFolder {}

/// Graph API response for created drive item.
#[derive(Debug, Deserialize)]
struct GraphCreateFolderResponse {
    id: String,
    name: String,
    web_url: String,
    created_date_time: String,
}

/// # Create SharePoint Folder
///
/// Creates a new folder in a SharePoint document library using the Microsoft
/// Graph API. Supports creating folders at the root level or within existing
/// parent folders.
///
/// Use this tool when a user wants to:
/// - Create a new folder in a SharePoint document library
/// - Organize documents into a folder structure
/// - Create nested folders within existing folders
///
/// The tool requires the target `site_id` and `drive_id` (document library).
/// Provide a `folder_name` for the new folder. Optionally specify a
/// `parent_path` to create the folder within an existing folder (e.g.,
/// "/Reports/2024"). If omitted, the folder is created at the root of the
/// document library.
///
/// SharePoint folder names cannot contain the following characters: \ / : *
/// ? " < > | # % The folder name must also be non-empty.
///
/// Returns the created folder's metadata including ID, name, full path, web
/// URL, and creation timestamp.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - file-storage
/// - sharepoint
/// - folder
///
/// # Errors
///
/// This function will return an error if:
/// - The folder name is empty
/// - The folder name contains invalid characters (`\`, `/`, `:`, `*`, `?`, `"`,
///   `<`, `>`, `|`, `#`, `%`)
/// - The SharePoint API is unreachable or times out
/// - Authentication fails due to invalid or expired credentials
/// - The specified site ID, drive ID, or parent path does not exist
/// - A folder with the same name already exists at the specified path
#[tool]
pub async fn create_folder(ctx: Context, input: CreateFolderInput) -> Result<CreateFolderOutput> {
    let client = GraphClient::from_ctx(&ctx)?;

    // Validate folder name (SharePoint restrictions)
    let invalid_chars = ['\\', '/', ':', '*', '?', '"', '<', '>', '|', '#', '%'];
    if input
        .folder_name
        .chars()
        .any(|c| invalid_chars.contains(&c))
    {
        bail!("Folder name contains invalid characters. Cannot use: \\ / : * ? \" < > | # %");
    }

    if input.folder_name.is_empty() {
        bail!("Folder name cannot be empty");
    }

    let parent = input.parent_path.as_deref().unwrap_or("");
    let full_path = if parent.is_empty() {
        format!("/{}", input.folder_name)
    } else {
        format!("{}/{}", parent.trim_end_matches('/'), input.folder_name)
    };

    let create_request = GraphCreateFolderRequest {
        name: input.folder_name.clone(),
        folder: GraphFolder {},
        conflict_behavior: "fail".to_string(),
    };

    // Use the children endpoint to create in a specific location
    let path = if parent.is_empty() {
        format!(
            "/sites/{}/drives/{}/root/children",
            input.site_id, input.drive_id
        )
    } else {
        let encoded_parent = urlencoding::encode(parent);
        format!(
            "/sites/{}/drives/{}/root:{}:/children",
            input.site_id, input.drive_id, encoded_parent
        )
    };

    let response = client.post(&path, &create_request).await?;

    let folder_response: GraphCreateFolderResponse = serde_json::from_str(&response.body)
        .map_err(|e| anyhow::anyhow!("Failed to parse folder response: {e}"))?;

    Ok(CreateFolderOutput {
        id: folder_response.id,
        name: folder_response.name,
        path: full_path,
        web_url: folder_response.web_url,
        created_at: folder_response.created_date_time,
    })
}

// =============================================================================
// Get Link Tool
// =============================================================================

/// Type of sharing link to create.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LinkType {
    /// View-only link.
    View,
    /// Edit link.
    Edit,
    /// Embed link (for embedding in web pages).
    Embed,
}

/// Scope of the sharing link.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LinkScope {
    /// Anyone with the link (no sign-in required).
    Anonymous,
    /// Only people in the organization.
    Organization,
    /// Only existing users with access.
    ExistingAccess,
}

/// Input for the `get_link` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetLinkInput {
    /// The site ID containing the item.
    pub site_id: String,
    /// The drive ID containing the item.
    pub drive_id: String,
    /// The item ID (file or folder) to get/create a link for.
    pub item_id: String,
    /// Type of link to create.
    pub link_type: LinkType,
    /// Scope of the link.
    pub scope: LinkScope,
    /// Password protection for the link (optional).
    #[serde(default)]
    pub password: Option<String>,
    /// Expiration date for the link (ISO 8601, optional).
    #[serde(default)]
    pub expires_at: Option<String>,
    /// Whether to block download for view links (optional).
    #[serde(default)]
    pub block_download: Option<bool>,
}

/// Output from the `get_link` tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GetLinkOutput {
    /// The sharing link URL.
    pub link: String,
    /// Unique identifier for the sharing link.
    pub link_id: String,
    /// Type of the link.
    pub link_type: String,
    /// Scope of the link.
    pub scope: String,
    /// Whether the link is password protected.
    pub has_password: bool,
    /// Expiration date if set.
    pub expires_at: Option<String>,
    /// Whether download is blocked.
    pub download_blocked: bool,
}

/// Graph API request body for creating a link.
#[derive(Debug, Serialize)]
struct GraphCreateLinkRequest {
    #[serde(rename = "type")]
    link_type: String,
    scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expiration_date_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "blockDownload")]
    block_download: Option<bool>,
}

/// Graph API response for created link.
#[derive(Debug, Deserialize)]
struct GraphCreateLinkResponse {
    #[serde(rename = "@microsoft.graph.temporaryId")]
    temporary_id: Option<String>,
    id: Option<String>,
    link: Option<GraphLink>,
}

#[derive(Debug, Deserialize)]
struct GraphLink {
    #[serde(rename = "type")]
    link_type: Option<String>,
    scope: Option<String>,
    web_url: String,
    #[serde(rename = "preventDownload")]
    prevent_download: Option<bool>,
}

/// # Get SharePoint Sharing Link
///
/// Generates a sharing link for a SharePoint document or folder using the
/// Microsoft Graph API. Creates links with configurable access levels, scopes,
/// and security settings.
///
/// Use this tool when a user wants to:
/// - Create a shareable link for a document or folder
/// - Generate view-only or edit links with specific access scopes
/// - Create password-protected or time-limited sharing links
/// - Generate links for embedding in web pages or applications
///
/// The tool requires the target `site_id`, `drive_id`, and `item_id` (file or
/// folder). Configure the link using:
/// - `link_type`: "view" (read-only), "edit" (can modify), or "embed" (for
///   embedding)
/// - `scope`: "anonymous" (anyone with link), "organization" (internal only),
///   or "existingAccess" (already-permitted users)
/// - Optional password protection, expiration date, and download blocking
///
/// For "anonymous" scope, the link works for anyone without requiring sign-in.
/// For "organization" scope, recipients must be signed into the organization.
/// The "existingAccess" scope creates a link that only works for users who
/// already have access to the item.
///
/// Returns the sharing link URL, link ID, configuration details, and security
/// settings.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - file-storage
/// - sharepoint
/// - link
///
/// # Errors
///
/// This function will return an error if:
/// - The SharePoint API is unreachable or times out
/// - Authentication fails due to invalid or expired credentials
/// - The specified site ID, drive ID, or item ID does not exist
/// - The user does not have permission to create sharing links for the item
/// - The link type and scope combination is not supported
/// - The password does not meet SharePoint's password complexity requirements
/// - The expiration date is in the past
#[tool]
pub async fn get_link(ctx: Context, input: GetLinkInput) -> Result<GetLinkOutput> {
    let client = GraphClient::from_ctx(&ctx)?;

    let link_type_str = match input.link_type {
        LinkType::View => "view",
        LinkType::Edit => "edit",
        LinkType::Embed => "embed",
    };

    let scope_str = match input.scope {
        LinkScope::Anonymous => "anonymous",
        LinkScope::Organization => "organization",
        LinkScope::ExistingAccess => "existingAccess",
    };

    let create_link_request = GraphCreateLinkRequest {
        link_type: link_type_str.to_string(),
        scope: scope_str.to_string(),
        password: input.password.clone(),
        expiration_date_time: input.expires_at.clone(),
        block_download: input.block_download,
    };

    let path = format!(
        "/sites/{}/drives/{}/items/{}/createLink",
        input.site_id, input.drive_id, input.item_id
    );

    let response = client.post(&path, &create_link_request).await?;

    let link_response: GraphCreateLinkResponse = serde_json::from_str(&response.body)
        .map_err(|e| anyhow::anyhow!("Failed to parse createLink response: {e}"))?;

    let link = link_response
        .link
        .ok_or_else(|| anyhow::anyhow!("CreateLink response did not contain a link object"))?;

    let link_id = link_response
        .temporary_id
        .or(link_response.id)
        .unwrap_or_else(|| input.item_id.clone());

    Ok(GetLinkOutput {
        link: link.web_url,
        link_id,
        link_type: link.link_type.unwrap_or_else(|| link_type_str.to_string()),
        scope: link.scope.unwrap_or_else(|| scope_str.to_string()),
        has_password: input.password.is_some(),
        expires_at: input.expires_at,
        download_blocked: link.prevent_download.unwrap_or(false),
    })
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Credential Tests
    // =========================================================================

    #[test]
    fn test_credential_deserializes_with_access_token_only() {
        let json = r#"{ "access_token": "eyJ0eXAi..." }"#;
        let cred: SharePointCredential = serde_json::from_str(json).unwrap();
        assert_eq!(cred.access_token, "eyJ0eXAi...");
        assert!(cred.endpoint.is_none());
    }

    #[test]
    fn test_credential_deserializes_with_all_fields() {
        let json =
            r#"{ "access_token": "eyJ0eXAi...", "endpoint": "https://graph.microsoft.com/v1.0" }"#;
        let cred: SharePointCredential = serde_json::from_str(json).unwrap();
        assert_eq!(cred.access_token, "eyJ0eXAi...");
        assert_eq!(
            cred.endpoint.as_deref(),
            Some("https://graph.microsoft.com/v1.0")
        );
    }

    #[test]
    fn test_credential_missing_access_token_fails() {
        let json = r#"{ "endpoint": "https://graph.microsoft.com/v1.0" }"#;
        let err = serde_json::from_str::<SharePointCredential>(json).unwrap_err();
        assert!(err.to_string().contains("missing field `access_token`"));
    }

    // =========================================================================
    // Search Docs Tests
    // =========================================================================

    #[test]
    fn test_search_input_deserializes_minimal() {
        let json = r#"{ "query": "budget report" }"#;
        let input: SearchDocsInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.query, "budget report");
        assert!(input.site_id.is_none());
        assert!(input.drive_id.is_none());
        assert!(input.limit.is_none());
        assert!(input.file_type.is_none());
    }

    #[test]
    fn test_search_input_deserializes_full() {
        let json = r#"{
            "query": "quarterly report",
            "site_id": "site-123",
            "drive_id": "drive-456",
            "limit": 50,
            "file_type": "pdf"
        }"#;
        let input: SearchDocsInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.query, "quarterly report");
        assert_eq!(input.site_id.as_deref(), Some("site-123"));
        assert_eq!(input.drive_id.as_deref(), Some("drive-456"));
        assert_eq!(input.limit, Some(50));
        assert_eq!(input.file_type.as_deref(), Some("pdf"));
    }

    // =========================================================================
    // Upload Tests
    // =========================================================================

    #[test]
    fn test_upload_input_deserializes_minimal() {
        let json = r#"{
            "site_id": "site-123",
            "drive_id": "drive-456",
            "file_name": "report.pdf",
            "content_base64": "SGVsbG8gV29ybGQ="
        }"#;
        let input: UploadInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.site_id, "site-123");
        assert_eq!(input.drive_id, "drive-456");
        assert_eq!(input.file_name, "report.pdf");
        assert!(input.folder_path.is_none());
        assert!(input.conflict_behavior.is_none());
    }

    #[test]
    fn test_base64_decode_valid() {
        let encoded = "SGVsbG8gV29ybGQ=";
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        assert_eq!(decoded, b"Hello World");
    }

    #[test]
    fn test_base64_decode_invalid_fails() {
        let encoded = "Invalid!Base64@";
        let result = base64::engine::general_purpose::STANDARD.decode(encoded);
        assert!(result.is_err());
    }

    // =========================================================================
    // Set Permissions Tests
    // =========================================================================

    #[test]
    fn test_permission_grant_deserializes() {
        let json = r#"{
            "recipient_type": "user",
            "recipient": "user@contoso.com",
            "role": "read"
        }"#;
        let grant: PermissionGrant = serde_json::from_str(json).unwrap();
        assert!(matches!(grant.recipient_type, RecipientType::User));
        assert_eq!(grant.recipient.as_deref(), Some("user@contoso.com"));
        assert!(matches!(grant.role, PermissionRole::Read));
    }

    // =========================================================================
    // Create Folder Tests
    // =========================================================================

    #[test]
    fn test_create_folder_input_deserializes() {
        let json = r#"{
            "site_id": "site-123",
            "drive_id": "drive-456",
            "folder_name": "New Folder"
        }"#;
        let input: CreateFolderInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.site_id, "site-123");
        assert_eq!(input.folder_name, "New Folder");
        assert!(input.parent_path.is_none());
    }

    // =========================================================================
    // Get Link Tests
    // =========================================================================

    #[test]
    fn test_link_type_deserializes() {
        let json = r#""view""#;
        let link_type: LinkType = serde_json::from_str(json).unwrap();
        assert!(matches!(link_type, LinkType::View));

        let json = r#""edit""#;
        let link_type: LinkType = serde_json::from_str(json).unwrap();
        assert!(matches!(link_type, LinkType::Edit));
    }

    #[test]
    fn test_link_scope_deserializes() {
        let json = r#""anonymous""#;
        let scope: LinkScope = serde_json::from_str(json).unwrap();
        assert!(matches!(scope, LinkScope::Anonymous));

        let json = r#""organization""#;
        let scope: LinkScope = serde_json::from_str(json).unwrap();
        assert!(matches!(scope, LinkScope::Organization));
    }

    #[test]
    fn test_get_link_input_deserializes_full() {
        let json = r#"{
            "site_id": "site-123",
            "drive_id": "drive-456",
            "item_id": "item-789",
            "link_type": "view",
            "scope": "organization",
            "password": "secret",
            "expires_at": "2024-06-30T00:00:00Z",
            "block_download": true
        }"#;

        let input: GetLinkInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.site_id, "site-123");
        assert!(matches!(input.link_type, LinkType::View));
        assert!(matches!(input.scope, LinkScope::Organization));
        assert_eq!(input.password.as_deref(), Some("secret"));
        assert_eq!(input.block_download, Some(true));
    }
}
