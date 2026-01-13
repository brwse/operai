//! Mattermost integration for Operai Toolbox.
//!
//! Provides tools for interacting with Mattermost team chat:
//! - List channels
//! - Post messages
//! - Reply to messages
//! - Read messages
//! - Upload files

use base64::prelude::*;
use operai::{
    Context, JsonSchema, Result, define_system_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

mod types;
use types::{
    CreatePostRequest, MattermostChannel, MattermostFileInfo, MattermostPost,
    MattermostPostsResponse, MattermostUser,
};

// Define credentials for Mattermost API authentication
define_system_credential! {
    MattermostCredential("mattermost") {
        /// Personal access token or bot token for authentication.
        access_token: String,
        /// Mattermost server URL (e.g., "https://mattermost.example.com").
        server_url: String,
    }
}

const API_VERSION: &str = "/api/v4";

/// Initialize the Mattermost integration.
#[init]
async fn setup() -> Result<()> {
    info!("Mattermost integration initialized");
    Ok(())
}

/// Clean up resources when the integration is unloaded.
#[shutdown]
fn cleanup() {
    info!("Mattermost integration shutting down");
}

// =============================================================================
// List Channels Tool
// =============================================================================

/// Input for the `list_channels` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListChannelsInput {
    /// Team ID to list channels for. If not specified, lists channels for all
    /// teams.
    #[serde(default)]
    pub team_id: Option<String>,
    /// Filter by channel type: "public", "private", or "direct".
    #[serde(default)]
    pub channel_type: Option<ChannelTypeFilter>,
    /// Maximum number of channels to return (default: 50, max: 200).
    #[serde(default)]
    pub limit: Option<u32>,
    /// Page number for pagination (0-indexed).
    #[serde(default)]
    pub page: Option<u32>,
}

/// Filter for channel types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChannelTypeFilter {
    /// Public channels visible to all team members.
    Public,
    /// Private channels with restricted membership.
    Private,
    /// Direct messages between users.
    Direct,
}

/// A Mattermost channel.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Channel {
    /// Unique channel identifier.
    pub id: String,
    /// Channel display name.
    pub display_name: String,
    /// Channel name (used in URLs).
    pub name: String,
    /// Channel type: "O" (open/public), "P" (private), "D" (direct), "G"
    /// (group).
    pub channel_type: String,
    /// Team ID this channel belongs to (empty for direct messages).
    #[serde(default)]
    pub team_id: Option<String>,
    /// Channel header/topic.
    #[serde(default)]
    pub header: Option<String>,
    /// Channel purpose description.
    #[serde(default)]
    pub purpose: Option<String>,
    /// Total message count in the channel.
    pub total_msg_count: u64,
}

/// Output from the `list_channels` tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ListChannelsOutput {
    /// List of channels matching the filter criteria.
    pub channels: Vec<Channel>,
    /// Total number of channels available (for pagination).
    pub total_count: u32,
    /// Whether there are more channels available.
    pub has_more: bool,
}

/// # List Mattermost Channels
///
/// Retrieves a list of Mattermost channels that the authenticated user has
/// access to. This tool is essential for discovering available channels before
/// posting messages or reading content. Use this tool when a user wants to see
/// what channels are available, find a specific channel, or browse channels by
/// type (public, private, or direct messages).
///
/// The results can be filtered by team ID and channel type to narrow down the
/// list. Supports pagination for teams with many channels.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - chat
/// - mattermost
///
/// # Errors
///
/// Returns an error if:
/// - Mattermost credentials are not configured or invalid
/// - The API request fails
/// - The response cannot be deserialized
#[tool]
pub async fn list_channels(ctx: Context, input: ListChannelsInput) -> Result<ListChannelsOutput> {
    let limit = input.limit.unwrap_or(50).min(200);
    let page = input.page.unwrap_or(0);

    let client = MattermostClient::from_ctx(&ctx)?;

    let channels_response: Vec<MattermostChannel> = client
        .get_json(
            client.url_with_segments(&["users", "me", "channels"])?,
            &[("per_page", limit.to_string()), ("page", page.to_string())],
            &[],
        )
        .await?;

    // Apply channel type filter if specified
    let filtered_channels: Vec<Channel> = channels_response
        .into_iter()
        .filter(|c| match input.channel_type {
            Some(ChannelTypeFilter::Public) => c.channel_type == "O",
            Some(ChannelTypeFilter::Private) => c.channel_type == "P",
            Some(ChannelTypeFilter::Direct) => c.channel_type == "D" || c.channel_type == "G",
            None => true,
        })
        .filter(|c| {
            if let Some(ref team_id) = input.team_id {
                &c.team_id == team_id
            } else {
                true
            }
        })
        .map(map_channel)
        .collect();

    let total_count = u32::try_from(filtered_channels.len()).unwrap_or(u32::MAX);
    let has_more = filtered_channels.len() == limit as usize;

    Ok(ListChannelsOutput {
        channels: filtered_channels,
        total_count,
        has_more,
    })
}

// =============================================================================
// Post Message Tool
// =============================================================================

/// Input for the post tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PostInput {
    /// Channel ID to post the message to.
    pub channel_id: String,
    /// Message content (supports Markdown).
    pub message: String,
    /// Optional list of file IDs to attach (from previous upload).
    #[serde(default)]
    pub file_ids: Option<Vec<String>>,
}

/// A posted message.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Message {
    /// Unique message identifier.
    pub id: String,
    /// Channel ID the message was posted to.
    pub channel_id: String,
    /// User ID of the message author.
    pub user_id: String,
    /// Message content.
    pub message: String,
    /// Unix timestamp when the message was created (milliseconds).
    pub create_at: u64,
    /// Unix timestamp when the message was last updated (milliseconds).
    pub update_at: u64,
    /// Root post ID if this is a reply, empty otherwise.
    #[serde(default)]
    pub root_id: Option<String>,
    /// List of attached file IDs.
    #[serde(default)]
    pub file_ids: Vec<String>,
}

/// Output from the post tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct PostOutput {
    /// The posted message.
    pub message: Message,
    /// Permalink to the message.
    pub permalink: String,
}

/// # Post Mattermost Message
///
/// Posts a new message to a Mattermost channel. This is the primary tool for
/// sending messages to channels. Use this tool when a user wants to send a
/// message to a channel, announce something, or start a new conversation
/// thread.
///
/// The message content supports Markdown formatting for rich text. Files can be
/// attached by providing file IDs from previous uploads (see the upload tool).
///
/// Requires a valid `channel_id`, which can be obtained using the
/// `list_channels` tool.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - chat
/// - mattermost
///
/// # Errors
///
/// Returns an error if:
/// - `channel_id` is empty
/// - `message` is empty
/// - Mattermost credentials are not configured or invalid
/// - The API request fails
/// - The response cannot be deserialized
#[tool]
pub async fn post(ctx: Context, input: PostInput) -> Result<PostOutput> {
    ensure!(
        !input.channel_id.trim().is_empty(),
        "channel_id must not be empty"
    );
    ensure!(
        !input.message.trim().is_empty(),
        "message must not be empty"
    );

    let client = MattermostClient::from_ctx(&ctx)?;

    let request = CreatePostRequest {
        channel_id: input.channel_id.clone(),
        message: input.message,
        root_id: String::new(),
        file_ids: input.file_ids.unwrap_or_default(),
    };

    let post: MattermostPost = client
        .post_json(client.url_with_segments(&["posts"])?, &request, &[])
        .await?;

    let permalink = format!("{}/_redirect/pl/{}", client.base_url, post.id);

    Ok(PostOutput {
        message: map_message(post),
        permalink,
    })
}

// =============================================================================
// Reply Tool
// =============================================================================

/// Input for the reply tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplyInput {
    /// ID of the root post to reply to.
    pub post_id: String,
    /// Reply message content (supports Markdown).
    pub message: String,
    /// Optional list of file IDs to attach.
    #[serde(default)]
    pub file_ids: Option<Vec<String>>,
}

/// Output from the reply tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ReplyOutput {
    /// The reply message that was posted.
    pub message: Message,
    /// Permalink to the reply.
    pub permalink: String,
    /// ID of the thread (same as the root post ID).
    pub thread_id: String,
}

/// # Reply to Mattermost Message
///
/// Replies to an existing message in a Mattermost thread. Use this tool when a
/// user wants to respond to a specific message, continue a conversation thread,
/// or provide a follow-up to a previous post.
///
/// This tool creates a threaded reply that is linked to the root post. It
/// automatically determines the correct channel from the parent post and
/// creates a proper thread structure in Mattermost.
///
/// Requires the `post_id` of the message being replied to. The message content
/// supports Markdown formatting. Files can be attached by providing file IDs
/// from previous uploads.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - chat
/// - mattermost
///
/// # Errors
///
/// Returns an error if:
/// - `post_id` is empty
/// - `message` is empty
/// - Mattermost credentials are not configured or invalid
/// - The root post cannot be found
/// - The API request fails
/// - The response cannot be deserialized
#[tool]
pub async fn reply(ctx: Context, input: ReplyInput) -> Result<ReplyOutput> {
    ensure!(
        !input.post_id.trim().is_empty(),
        "post_id must not be empty"
    );
    ensure!(
        !input.message.trim().is_empty(),
        "message must not be empty"
    );

    let client = MattermostClient::from_ctx(&ctx)?;

    // First, get the root post to find the channel_id
    let root_post: MattermostPost = client
        .get_json(
            client.url_with_segments(&["posts", input.post_id.as_str()])?,
            &[],
            &[],
        )
        .await?;

    let request = CreatePostRequest {
        channel_id: root_post.channel_id.clone(),
        message: input.message,
        root_id: input.post_id.clone(),
        file_ids: input.file_ids.unwrap_or_default(),
    };

    let post: MattermostPost = client
        .post_json(client.url_with_segments(&["posts"])?, &request, &[])
        .await?;

    let permalink = format!("{}/_redirect/pl/{}", client.base_url, post.id);

    Ok(ReplyOutput {
        message: map_message(post),
        permalink,
        thread_id: input.post_id,
    })
}

// =============================================================================
// Read Messages Tool
// =============================================================================

/// Input for the read tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadInput {
    /// Channel ID to read messages from.
    pub channel_id: String,
    /// Maximum number of messages to return (default: 30, max: 200).
    #[serde(default)]
    pub limit: Option<u32>,
    /// Return messages before this post ID (for pagination).
    #[serde(default)]
    pub before: Option<String>,
    /// Return messages after this post ID (for pagination).
    #[serde(default)]
    pub after: Option<String>,
}

/// A message with optional thread information.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct MessageWithThread {
    /// The message.
    #[serde(flatten)]
    pub message: Message,
    /// Number of replies if this is a root post.
    #[serde(default)]
    pub reply_count: Option<u32>,
    /// Username of the message author.
    pub username: String,
}

/// Output from the read tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ReadOutput {
    /// List of messages in the channel.
    pub messages: Vec<MessageWithThread>,
    /// Channel information.
    pub channel: Channel,
    /// Whether there are more messages available.
    pub has_more: bool,
}

/// # Read Mattermost Messages
///
/// Retrieves recent messages from a Mattermost channel. Use this tool when a
/// user wants to catch up on conversations, review channel history, or see what
/// has been discussed recently.
///
/// This tool fetches messages with author usernames for context, making it easy
/// to understand who said what. Supports pagination with before/after
/// parameters to navigate through message history efficiently.
///
/// Returns both the messages and channel information, providing full context
/// for the conversation.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - chat
/// - mattermost
///
/// # Errors
///
/// Returns an error if:
/// - `channel_id` is empty
/// - Mattermost credentials are not configured or invalid
/// - The API request fails
/// - The response cannot be deserialized
#[tool]
pub async fn read(ctx: Context, input: ReadInput) -> Result<ReadOutput> {
    ensure!(
        !input.channel_id.trim().is_empty(),
        "channel_id must not be empty"
    );

    let limit = input.limit.unwrap_or(30).min(200);

    let client = MattermostClient::from_ctx(&ctx)?;

    let mut query_params = vec![("per_page", limit.to_string())];
    if let Some(ref before) = input.before {
        query_params.push(("before", before.clone()));
    }
    if let Some(ref after) = input.after {
        query_params.push(("after", after.clone()));
    }

    let posts_response: MattermostPostsResponse = client
        .get_json(
            client.url_with_segments(&["channels", input.channel_id.as_str(), "posts"])?,
            &query_params,
            &[],
        )
        .await?;

    // Get channel info
    let channel: MattermostChannel = client
        .get_json(
            client.url_with_segments(&["channels", input.channel_id.as_str()])?,
            &[],
            &[],
        )
        .await?;

    // Get user info for each unique user_id
    let mut user_cache: std::collections::HashMap<String, MattermostUser> =
        std::collections::HashMap::new();
    let unique_user_ids: Vec<String> = posts_response
        .order
        .iter()
        .filter_map(|id| posts_response.posts.get(id))
        .map(|p| p.user_id.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    for user_id in unique_user_ids {
        if let Ok(user) = client
            .get_json::<MattermostUser>(
                client.url_with_segments(&["users", user_id.as_str()])?,
                &[],
                &[],
            )
            .await
        {
            user_cache.insert(user_id, user);
        }
    }

    let messages: Vec<MessageWithThread> = posts_response
        .order
        .iter()
        .filter_map(|id| posts_response.posts.get(id))
        .map(|post| {
            let username = user_cache
                .get(&post.user_id)
                .map_or_else(|| "unknown".to_string(), |u| u.username.clone());

            MessageWithThread {
                message: map_message(post.clone()),
                reply_count: None,
                username,
            }
        })
        .collect();

    let has_more = messages.len() == limit as usize;

    Ok(ReadOutput {
        messages,
        channel: map_channel(channel),
        has_more,
    })
}

// =============================================================================
// Upload File Tool
// =============================================================================

/// Input for the upload tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadInput {
    /// Channel ID to upload the file to.
    pub channel_id: String,
    /// File name.
    pub filename: String,
    /// File content as base64-encoded string.
    pub content_base64: String,
    /// Optional MIME type (auto-detected if not provided).
    #[serde(default)]
    pub mime_type: Option<String>,
}

/// Information about an uploaded file.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct FileInfo {
    /// Unique file identifier.
    pub id: String,
    /// Original file name.
    pub name: String,
    /// File extension.
    pub extension: String,
    /// File size in bytes.
    pub size: u64,
    /// MIME type.
    pub mime_type: String,
    /// Whether the file has a preview available.
    pub has_preview: bool,
    /// URL to download the file.
    pub download_url: String,
}

/// Output from the upload tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct UploadOutput {
    /// Information about the uploaded file.
    pub file: FileInfo,
    /// File ID to use when attaching to a message.
    pub file_id: String,
}

/// # Upload Mattermost File
///
/// Uploads a file to a Mattermost channel for later attachment to messages. Use
/// this tool when a user wants to share images, documents, or other files in a
/// channel.
///
/// Files must be uploaded before they can be attached to messages. This tool
/// returns a `file_id` that can be passed to the post or reply tools to attach
/// the file to a message.
///
/// The file content must be provided as a base64-encoded string. The MIME type
/// can be specified for proper handling; if not provided, it will be
/// auto-detected when possible.
///
/// Common use cases: sharing screenshots, attaching documents, uploading images
/// for discussion.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - chat
/// - mattermost
/// - files
///
/// # Errors
///
/// Returns an error if:
/// - `channel_id` is empty
/// - `filename` is empty
/// - `content_base64` is empty or not valid base64
/// - Mattermost credentials are not configured or invalid
/// - The API request fails
/// - The response cannot be deserialized
#[tool]
pub async fn upload(ctx: Context, input: UploadInput) -> Result<UploadOutput> {
    ensure!(
        !input.channel_id.trim().is_empty(),
        "channel_id must not be empty"
    );
    ensure!(
        !input.filename.trim().is_empty(),
        "filename must not be empty"
    );
    ensure!(
        !input.content_base64.trim().is_empty(),
        "content_base64 must not be empty"
    );

    let client = MattermostClient::from_ctx(&ctx)?;

    // Decode base64 content
    let content_bytes = BASE64_STANDARD.decode(&input.content_base64)?;

    // Build multipart form
    let part = reqwest::multipart::Part::bytes(content_bytes)
        .file_name(input.filename.clone())
        .mime_str(
            input
                .mime_type
                .as_deref()
                .unwrap_or("application/octet-stream"),
        )?;

    let form = reqwest::multipart::Form::new()
        .text("channel_id", input.channel_id.clone())
        .part("files", part);

    let response = client
        .http
        .post(client.url_with_segments(&["files"])?)
        .bearer_auth(&client.access_token)
        .multipart(form)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(operai::anyhow::anyhow!(
            "Mattermost file upload failed ({status}): {body}"
        ));
    }

    let file_infos: Vec<MattermostFileInfo> = response.json().await?;
    let file_info = file_infos
        .into_iter()
        .next()
        .ok_or_else(|| operai::anyhow::anyhow!("No file info returned"))?;

    let file = FileInfo {
        id: file_info.id.clone(),
        name: file_info.name,
        extension: file_info.extension,
        size: u64::try_from(file_info.size).unwrap_or(0),
        mime_type: file_info.mime_type,
        has_preview: file_info.has_preview_image,
        download_url: format!("{}{}/files/{}", client.base_url, API_VERSION, file_info.id),
    };

    Ok(UploadOutput {
        file_id: file.id.clone(),
        file,
    })
}

// =============================================================================
// HTTP Client
// =============================================================================

#[derive(Debug, Clone)]
struct MattermostClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl MattermostClient {
    /// Creates a new `MattermostClient` from the context.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Credentials cannot be retrieved from the context
    /// - The `access_token` is empty
    /// - The `server_url` is empty
    /// - The `server_url` cannot be normalized
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = MattermostCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );
        ensure!(
            !cred.server_url.trim().is_empty(),
            "server_url must not be empty"
        );

        let base_url = normalize_base_url(&cred.server_url)?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            access_token: cred.access_token,
        })
    }

    /// Constructs a URL by appending path segments to the API base URL.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The base URL combined with API version cannot be parsed as a URL
    /// - The base URL is not an absolute URL (cannot be a base for path
    ///   segments)
    fn url_with_segments(&self, segments: &[&str]) -> Result<reqwest::Url> {
        let mut url = reqwest::Url::parse(&format!("{}{}", self.base_url, API_VERSION))?;
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

    /// Sends a GET request and deserializes the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails
    /// - The response status is not successful
    /// - The response body cannot be deserialized to the target type
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

    /// Sends a POST request with JSON body and deserializes the JSON response.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails
    /// - The response status is not successful
    /// - The response body cannot be deserialized to the target type
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

    /// Sends an HTTP request with authentication and returns the response or an
    /// error.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails to send
    /// - The response status indicates an error (non-2xx)
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
                "Mattermost API request failed ({status}): {body}"
            ))
        }
    }
}

/// Normalizes a Mattermost server URL by trimming whitespace and trailing
/// slashes.
///
/// # Errors
///
/// Returns an error if the provided URL is empty after trimming whitespace.
fn normalize_base_url(url: &str) -> Result<String> {
    let trimmed = url.trim();
    ensure!(!trimmed.is_empty(), "server_url must not be empty");
    Ok(trimmed.trim_end_matches('/').to_string())
}

fn map_channel(channel: MattermostChannel) -> Channel {
    Channel {
        id: channel.id,
        display_name: channel.display_name,
        name: channel.name,
        channel_type: channel.channel_type,
        team_id: if channel.team_id.is_empty() {
            None
        } else {
            Some(channel.team_id)
        },
        header: if channel.header.is_empty() {
            None
        } else {
            Some(channel.header)
        },
        purpose: if channel.purpose.is_empty() {
            None
        } else {
            Some(channel.purpose)
        },
        total_msg_count: channel.total_msg_count,
    }
}

fn map_message(post: MattermostPost) -> Message {
    Message {
        id: post.id,
        channel_id: post.channel_id,
        user_id: post.user_id,
        message: post.message,
        create_at: u64::try_from(post.create_at).unwrap_or(0),
        update_at: u64::try_from(post.update_at).unwrap_or(0),
        root_id: if post.root_id.is_empty() {
            None
        } else {
            Some(post.root_id)
        },
        file_ids: post.file_ids,
    }
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

    fn test_ctx(server_url: &str) -> Context {
        let mut mattermost_values = HashMap::new();
        mattermost_values.insert("access_token".to_string(), "test-token".to_string());
        mattermost_values.insert("server_url".to_string(), server_url.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_system_credential("mattermost", mattermost_values)
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://mattermost.example.com/").unwrap();
        assert_eq!(result, "https://mattermost.example.com");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://mattermost.example.com  ").unwrap();
        assert_eq!(result, "https://mattermost.example.com");
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

    // --- list_channels tests ---

    #[tokio::test]
    async fn test_list_channels_success() {
        let server = MockServer::start().await;

        let response_body = r#"[
            {
                "id": "channel_001",
                "name": "town-square",
                "display_name": "Town Square",
                "type": "O",
                "team_id": "team_001",
                "header": "Welcome!",
                "purpose": "General discussions",
                "total_msg_count": 1523
            },
            {
                "id": "channel_002",
                "name": "off-topic",
                "display_name": "Off-Topic",
                "type": "O",
                "team_id": "team_001",
                "header": "",
                "purpose": "",
                "total_msg_count": 847
            }
        ]"#;

        Mock::given(method("GET"))
            .and(path("/api/v4/users/me/channels"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("per_page", "50"))
            .and(query_param("page", "0"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = list_channels(
            ctx,
            ListChannelsInput {
                team_id: None,
                channel_type: None,
                limit: None,
                page: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.channels.len(), 2);
        assert_eq!(output.channels[0].id, "channel_001");
        assert_eq!(output.channels[0].name, "town-square");
        assert_eq!(output.channels[1].id, "channel_002");
    }

    #[tokio::test]
    async fn test_list_channels_with_filter() {
        let server = MockServer::start().await;

        let response_body = r#"[
            {
                "id": "channel_001",
                "name": "town-square",
                "display_name": "Town Square",
                "type": "O",
                "team_id": "team_001",
                "header": "",
                "purpose": "",
                "total_msg_count": 1523
            },
            {
                "id": "channel_002",
                "name": "private-channel",
                "display_name": "Private Channel",
                "type": "P",
                "team_id": "team_001",
                "header": "",
                "purpose": "",
                "total_msg_count": 100
            }
        ]"#;

        Mock::given(method("GET"))
            .and(path("/api/v4/users/me/channels"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = list_channels(
            ctx,
            ListChannelsInput {
                team_id: None,
                channel_type: Some(ChannelTypeFilter::Public),
                limit: None,
                page: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.channels.len(), 1);
        assert_eq!(output.channels[0].channel_type, "O");
    }

    // --- post tests ---

    #[tokio::test]
    async fn test_post_success() {
        let server = MockServer::start().await;

        let response_body = r#"{
            "id": "post_123",
            "channel_id": "channel_001",
            "user_id": "user_001",
            "message": "Hello, team!",
            "create_at": 1609459200000,
            "update_at": 1609459200000,
            "root_id": "",
            "file_ids": []
        }"#;

        Mock::given(method("POST"))
            .and(path("/api/v4/posts"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_partial_json(serde_json::json!({
                "channel_id": "channel_001",
                "message": "Hello, team!"
            })))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = post(
            ctx,
            PostInput {
                channel_id: "channel_001".to_string(),
                message: "Hello, team!".to_string(),
                file_ids: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.message.id, "post_123");
        assert_eq!(output.message.channel_id, "channel_001");
        assert_eq!(output.message.message, "Hello, team!");
        assert!(output.permalink.contains("post_123"));
    }

    #[tokio::test]
    async fn test_post_empty_channel_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = post(
            ctx,
            PostInput {
                channel_id: "  ".to_string(),
                message: "Hello".to_string(),
                file_ids: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("channel_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_post_empty_message_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = post(
            ctx,
            PostInput {
                channel_id: "channel_001".to_string(),
                message: "  ".to_string(),
                file_ids: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("message must not be empty")
        );
    }

    // --- reply tests ---

    #[tokio::test]
    async fn test_reply_success() {
        let server = MockServer::start().await;

        // Mock getting the root post
        let root_post_response = r#"{
            "id": "post_001",
            "channel_id": "channel_001",
            "user_id": "user_001",
            "message": "Original post",
            "create_at": 1609459200000,
            "update_at": 1609459200000,
            "root_id": "",
            "file_ids": []
        }"#;

        Mock::given(method("GET"))
            .and(path("/api/v4/posts/post_001"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(root_post_response, "application/json"),
            )
            .mount(&server)
            .await;

        // Mock creating the reply
        let reply_response = r#"{
            "id": "post_002",
            "channel_id": "channel_001",
            "user_id": "user_002",
            "message": "Great idea!",
            "create_at": 1609459260000,
            "update_at": 1609459260000,
            "root_id": "post_001",
            "file_ids": []
        }"#;

        Mock::given(method("POST"))
            .and(path("/api/v4/posts"))
            .and(body_partial_json(serde_json::json!({
                "root_id": "post_001",
                "message": "Great idea!"
            })))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(reply_response, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = reply(
            ctx,
            ReplyInput {
                post_id: "post_001".to_string(),
                message: "Great idea!".to_string(),
                file_ids: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.message.id, "post_002");
        assert_eq!(output.message.root_id.as_deref(), Some("post_001"));
        assert_eq!(output.thread_id, "post_001");
        assert_eq!(output.message.message, "Great idea!");
    }

    #[tokio::test]
    async fn test_reply_empty_post_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = reply(
            ctx,
            ReplyInput {
                post_id: "  ".to_string(),
                message: "Reply".to_string(),
                file_ids: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("post_id must not be empty")
        );
    }

    // --- read tests ---

    #[tokio::test]
    async fn test_read_success() {
        let server = MockServer::start().await;

        // Mock channel info
        let channel_response = r#"{
            "id": "channel_001",
            "name": "town-square",
            "display_name": "Town Square",
            "type": "O",
            "team_id": "team_001",
            "header": "Welcome!",
            "purpose": "",
            "total_msg_count": 1523
        }"#;

        Mock::given(method("GET"))
            .and(path("/api/v4/channels/channel_001"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(channel_response, "application/json"),
            )
            .mount(&server)
            .await;

        // Mock posts
        let posts_response = r#"{
            "order": ["post_002", "post_001"],
            "posts": {
                "post_001": {
                    "id": "post_001",
                    "channel_id": "channel_001",
                    "user_id": "user_alice",
                    "message": "Hello everyone!",
                    "create_at": 1609459200000,
                    "update_at": 1609459200000,
                    "root_id": "",
                    "file_ids": []
                },
                "post_002": {
                    "id": "post_002",
                    "channel_id": "channel_001",
                    "user_id": "user_bob",
                    "message": "Hi Alice!",
                    "create_at": 1609459260000,
                    "update_at": 1609459260000,
                    "root_id": "post_001",
                    "file_ids": []
                }
            }
        }"#;

        Mock::given(method("GET"))
            .and(path("/api/v4/channels/channel_001/posts"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(posts_response, "application/json"),
            )
            .mount(&server)
            .await;

        // Mock user info for alice
        let alice_response = r#"{
            "id": "user_alice",
            "username": "alice",
            "email": "alice@example.com",
            "first_name": "Alice",
            "last_name": "Smith"
        }"#;

        Mock::given(method("GET"))
            .and(path("/api/v4/users/user_alice"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(alice_response, "application/json"),
            )
            .mount(&server)
            .await;

        // Mock user info for bob
        let bob_response = r#"{
            "id": "user_bob",
            "username": "bob",
            "email": "bob@example.com",
            "first_name": "Bob",
            "last_name": "Jones"
        }"#;

        Mock::given(method("GET"))
            .and(path("/api/v4/users/user_bob"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(bob_response, "application/json"))
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = read(
            ctx,
            ReadInput {
                channel_id: "channel_001".to_string(),
                limit: None,
                before: None,
                after: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.channel.id, "channel_001");
        assert_eq!(output.messages.len(), 2);
        assert_eq!(output.messages[0].username, "bob");
        assert_eq!(output.messages[1].username, "alice");
    }

    #[tokio::test]
    async fn test_read_empty_channel_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = read(
            ctx,
            ReadInput {
                channel_id: "  ".to_string(),
                limit: None,
                before: None,
                after: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("channel_id must not be empty")
        );
    }

    // --- upload tests ---

    #[tokio::test]
    async fn test_upload_validates_channel_id() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = upload(
            ctx,
            UploadInput {
                channel_id: "  ".to_string(),
                filename: "test.txt".to_string(),
                content_base64: "SGVsbG8=".to_string(),
                mime_type: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("channel_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_upload_validates_filename() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = upload(
            ctx,
            UploadInput {
                channel_id: "channel_001".to_string(),
                filename: "  ".to_string(),
                content_base64: "SGVsbG8=".to_string(),
                mime_type: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("filename must not be empty")
        );
    }

    #[tokio::test]
    async fn test_upload_validates_content() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = upload(
            ctx,
            UploadInput {
                channel_id: "channel_001".to_string(),
                filename: "test.txt".to_string(),
                content_base64: "  ".to_string(),
                mime_type: None,
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
    async fn test_upload_success() {
        let server = MockServer::start().await;

        let response_body = r#"[
            {
                "id": "file_123",
                "name": "test.txt",
                "extension": "txt",
                "size": 5,
                "mime_type": "text/plain",
                "has_preview_image": false
            }
        ]"#;

        Mock::given(method("POST"))
            .and(path("/api/v4/files"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(201).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = upload(
            ctx,
            UploadInput {
                channel_id: "channel_001".to_string(),
                filename: "test.txt".to_string(),
                content_base64: "SGVsbG8=".to_string(), // "Hello" in base64
                mime_type: Some("text/plain".to_string()),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.file_id, "file_123");
        assert_eq!(output.file.name, "test.txt");
        assert_eq!(output.file.extension, "txt");
        assert_eq!(output.file.size, 5);
        assert_eq!(output.file.mime_type, "text/plain");
        assert!(!output.file.has_preview);
        assert!(output.file.download_url.contains("file_123"));
    }

    #[tokio::test]
    async fn test_upload_invalid_base64_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = upload(
            ctx,
            UploadInput {
                channel_id: "channel_001".to_string(),
                filename: "test.txt".to_string(),
                content_base64: "not-valid-base64!!!".to_string(),
                mime_type: None,
            },
        )
        .await;

        assert!(result.is_err());
    }

    // --- ChannelTypeFilter tests ---

    #[test]
    fn test_channel_type_filter_serialization_roundtrip() {
        for variant in [
            ChannelTypeFilter::Public,
            ChannelTypeFilter::Private,
            ChannelTypeFilter::Direct,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: ChannelTypeFilter = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }
}
