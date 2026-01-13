//! team-chat/rocket-chat integration for Operai Toolbox.
use base64::Engine as _;
use operai::{
    Context, JsonSchema, Result, bail, define_user_credential, ensure, info, init, schemars,
    shutdown, tool,
};
use reqwest::{Url, multipart};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

define_user_credential! {
    RocketChatCredential("rocket-chat") {
        /// Rocket.Chat REST API auth token (sent as `X-Auth-Token`).
        auth_token: String,
        /// Rocket.Chat user ID (sent as `X-User-Id`).
        user_id: String,
        /// Base URL for your Rocket.Chat instance (e.g., `https://chat.example.com`).
        #[optional]
        endpoint: Option<String>,
    }
}

#[init]
async fn setup() -> Result<()> {
    info!("team-chat/rocket-chat integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("team-chat/rocket-chat integration shutting down");
}

struct RocketChatHttp {
    client: reqwest::Client,
    base_url: Url,
    auth_token: String,
    user_id: String,
}

impl RocketChatHttp {
    fn new(cred: &RocketChatCredential) -> Result<Self> {
        let endpoint = cred.endpoint.as_deref().unwrap_or_default().trim();
        ensure!(
            !endpoint.is_empty(),
            "rocket-chat credential `endpoint` is required (e.g., https://chat.example.com)"
        );

        let mut base_url = Url::parse(endpoint)?;
        if !base_url.as_str().ends_with('/') {
            base_url = Url::parse(&format!("{endpoint}/"))?;
        }

        Ok(Self {
            client: reqwest::Client::new(),
            base_url,
            auth_token: cred.auth_token.clone(),
            user_id: cred.user_id.clone(),
        })
    }

    fn url(&self, path: &str) -> Result<Url> {
        self.base_url.join(path).map_err(Into::into)
    }

    fn authed(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        req.header("X-Auth-Token", &self.auth_token)
            .header("X-User-Id", &self.user_id)
    }

    async fn get_json<T: DeserializeOwned, Q: Serialize + ?Sized>(
        &self,
        path: &str,
        query: Option<&Q>,
    ) -> Result<T> {
        let url = self.url(path)?;
        let mut req = self.authed(self.client.get(url));
        if let Some(query) = query {
            req = req.query(query);
        }
        execute_json("Rocket.Chat API GET", req).await
    }

    async fn post_json<T: DeserializeOwned, B: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = self.url(path)?;
        let req = self.authed(self.client.post(url)).json(body);
        execute_json("Rocket.Chat API POST", req).await
    }

    async fn post_multipart<T: DeserializeOwned>(
        &self,
        path: &str,
        form: multipart::Form,
    ) -> Result<T> {
        let url = self.url(path)?;
        let req = self.authed(self.client.post(url)).multipart(form);
        execute_json("Rocket.Chat API multipart POST", req).await
    }
}

async fn execute_json<T: DeserializeOwned>(label: &str, req: reqwest::RequestBuilder) -> Result<T> {
    let resp = req.send().await?;
    let status = resp.status();

    if !status.is_success() {
        let error_body = match resp.text().await {
            Ok(body) if !body.is_empty() => body,
            Ok(_) => "<empty body>".to_string(),
            Err(e) => format!("<failed to read body: {e}>"),
        };
        bail!("{label} failed (HTTP {}): {error_body}", status.as_u16());
    }

    Ok(resp.json::<T>().await?)
}

fn validate_nonempty_no_newlines<'a>(value: &'a str, field: &str) -> Result<&'a str> {
    let trimmed = value.trim();
    ensure!(!trimmed.is_empty(), "{field} must not be empty");
    ensure!(
        !trimmed.contains(['\n', '\r']),
        "{field} must not contain newlines"
    );
    Ok(trimmed)
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct Channel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub room_type: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct User {
    pub id: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct Message {
    pub id: String,
    pub room_id: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub user: Option<User>,
}

#[derive(Debug, Deserialize)]
struct ApiChannel {
    #[serde(rename = "_id")]
    id: String,
    name: String,
    #[serde(default)]
    fname: Option<String>,
    #[serde(default)]
    t: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiUser {
    #[serde(rename = "_id")]
    id: String,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiAttachment {
    #[serde(default)]
    title_link: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiMessage {
    #[serde(rename = "_id")]
    id: String,
    #[serde(rename = "rid")]
    room_id: String,
    #[serde(default, rename = "msg")]
    text: Option<String>,
    #[serde(default, rename = "ts")]
    timestamp: Option<String>,
    #[serde(default)]
    tmid: Option<String>,
    #[serde(default)]
    u: Option<ApiUser>,
    #[serde(default)]
    attachments: Vec<ApiAttachment>,
}

#[derive(Debug, Deserialize)]
struct ListChannelsResponse {
    success: bool,
    #[serde(default)]
    channels: Vec<ApiChannel>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChannelHistoryResponse {
    success: bool,
    #[serde(default)]
    messages: Vec<ApiMessage>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PostMessageResponse {
    success: bool,
    #[serde(default)]
    message: Option<ApiMessage>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListChannelsInput {
    /// Number of channels to skip for pagination (>= 0).
    #[serde(default)]
    pub offset: Option<u32>,
    /// Maximum number of channels to return (1-100). Defaults to 100.
    #[serde(default)]
    pub count: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[must_use]
pub struct ListChannelsOutput {
    pub channels: Vec<Channel>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListChannelsQuery {
    offset: u32,
    count: u32,
}

/// # List Rocket.Chat Channels
///
/// Lists all Rocket.Chat channels that the authenticated user has joined.
///
/// Use this tool when a user wants to:
/// - Browse available channels in their Rocket.Chat workspace
/// - Find channel IDs or names for further operations (posting messages,
///   reading history, etc.)
/// - Explore the team chat structure and see what channels are accessible
///
/// The response includes channel metadata such as:
/// - `id`: The unique channel identifier (required for message operations)
/// - `name`: The channel name (e.g., "general", "random")
/// - `display_name`: Optional human-readable display name
/// - `room_type`: The type of room (e.g., "c" for channel)
///
/// Pagination is supported via `offset` and `count` parameters to retrieve
/// channels in batches when dealing with workspaces containing many channels.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - team-chat
/// - rocket-chat
///
/// # Errors
///
/// Returns an error if:
/// - The Rocket.Chat credential is not configured or is missing required fields
/// - The credential endpoint is empty or contains only whitespace
/// - The `count` parameter is outside the valid range (1-100)
/// - The HTTP request to Rocket.Chat API fails
/// - The Rocket.Chat API returns a non-success response (e.g., authentication
///   failure, unauthorized access)
/// - The response JSON cannot be parsed
#[tool]
pub async fn list_channels(ctx: Context, input: ListChannelsInput) -> Result<ListChannelsOutput> {
    let cred = RocketChatCredential::get(&ctx)?;
    let http = RocketChatHttp::new(&cred)?;

    let count = input.count.unwrap_or(100);
    ensure!(
        (1..=100).contains(&count),
        "count must be between 1 and 100"
    );
    let query = ListChannelsQuery {
        offset: input.offset.unwrap_or(0),
        count,
    };

    let resp: ListChannelsResponse = http
        .get_json("api/v1/channels.listJoined", Some(&query))
        .await?;
    ensure!(
        resp.success,
        "Rocket.Chat list channels failed: {}",
        resp.error.unwrap_or_else(|| "unknown error".to_string())
    );

    Ok(ListChannelsOutput {
        channels: resp
            .channels
            .into_iter()
            .map(|channel| Channel {
                id: channel.id,
                name: channel.name,
                display_name: channel.fname,
                room_type: channel.t,
            })
            .collect(),
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PostInput {
    /// Rocket.Chat room ID to post into (preferred).
    #[serde(default)]
    pub room_id: Option<String>,
    /// Rocket.Chat channel name (with or without `#`). Alternative to
    /// `room_id`.
    #[serde(default)]
    pub channel: Option<String>,
    /// Message text.
    pub text: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[must_use]
pub struct PostOutput {
    pub message: Message,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PostMessageRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    room_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel: Option<&'a str>,
    text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    tmid: Option<&'a str>,
}

fn normalize_channel_name(input: &str) -> Result<String> {
    let trimmed = validate_nonempty_no_newlines(input, "channel")?;
    if trimmed.starts_with('#') || trimmed.starts_with('@') {
        return Ok(trimmed.to_string());
    }
    Ok(format!("#{trimmed}"))
}

fn map_message(api: ApiMessage, base_url: &Url) -> Message {
    let user = api.u.map(|u| User {
        id: u.id,
        username: u.username,
        name: u.name,
    });

    let text = api.text.or_else(|| {
        api.attachments
            .first()
            .and_then(|a| a.title_link.as_ref())
            .and_then(|title_link| base_url.join(title_link).ok())
            .map(|u| u.to_string())
    });

    Message {
        id: api.id,
        room_id: api.room_id,
        text,
        timestamp: api.timestamp,
        thread_id: api.tmid,
        user,
    }
}

/// # Send Rocket.Chat Message
///
/// Sends a message to a Rocket.Chat channel or room.
///
/// Use this tool when a user wants to:
/// - Send a new message to a Rocket.Chat channel
/// - Post a notification or update to a team chat
/// - Communicate with team members through Rocket.Chat
///
/// This tool supports two ways to specify the destination:
/// - **By room ID** (preferred): Use the unique `room_id` identifier obtained
///   from `list_channels`
/// - **By channel name**: Use the channel name (e.g., "general" or "#general")
///
/// The tool automatically normalizes channel names by adding a `#` prefix if
/// not present. Direct messages can be addressed using the `@` prefix (e.g.,
/// "@username").
///
/// **Important notes:**
/// - Message text cannot contain newline characters (use `reply` for multi-line
///   or thread replies)
/// - The message will be sent as the authenticated user
/// - The returned message object includes the server-assigned message ID and
///   timestamp
/// - For thread replies, use the `reply` tool instead with a `thread_id`
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - team-chat
/// - rocket-chat
///
/// # Errors
///
/// Returns an error if:
/// - The Rocket.Chat credential is not configured or is missing required fields
/// - The credential endpoint is empty or contains only whitespace
/// - Neither `room_id` nor `channel` is provided
/// - The `text` field is empty or contains only whitespace
/// - The `text`, `room_id`, or `channel` fields contain newline characters
/// - The HTTP request to Rocket.Chat API fails
/// - The Rocket.Chat API returns a non-success response (e.g., room not found,
///   unauthorized access)
/// - The response JSON cannot be parsed or does not contain a message
#[tool]
pub async fn post(ctx: Context, input: PostInput) -> Result<PostOutput> {
    let cred = RocketChatCredential::get(&ctx)?;
    let http = RocketChatHttp::new(&cred)?;

    let text = validate_nonempty_no_newlines(&input.text, "text")?;
    ensure!(
        input.room_id.as_deref().is_some() || input.channel.as_deref().is_some(),
        "must provide either room_id or channel"
    );

    let room_id = input
        .room_id
        .as_deref()
        .map(|v| validate_nonempty_no_newlines(v, "room_id"))
        .transpose()?;

    let channel = input
        .channel
        .as_deref()
        .map(normalize_channel_name)
        .transpose()?;

    let req = PostMessageRequest {
        room_id,
        channel: channel.as_deref(),
        text,
        tmid: None,
    };

    let resp: PostMessageResponse = http.post_json("api/v1/chat.postMessage", &req).await?;
    ensure!(
        resp.success,
        "Rocket.Chat post message failed: {}",
        resp.error.unwrap_or_else(|| "unknown error".to_string())
    );

    let message = resp
        .message
        .ok_or_else(|| operai::anyhow::anyhow!("Rocket.Chat post message returned no message"))?;

    Ok(PostOutput {
        message: map_message(message, &http.base_url),
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplyInput {
    /// Rocket.Chat room ID containing the thread.
    pub room_id: String,
    /// Message ID to reply to (sent as `tmid`).
    pub thread_id: String,
    /// Reply text.
    pub text: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[must_use]
pub struct ReplyOutput {
    pub message: Message,
}

/// # Reply Rocket.Chat Thread Message
///
/// Sends a reply to an existing message thread in a Rocket.Chat room.
///
/// Use this tool when a user wants to:
/// - Respond to a specific message in a thread conversation
/// - Continue a discussion that has been organized into a thread
/// - Reply to a particular message while maintaining thread context
///
/// This tool is specifically for **threaded replies**. Unlike `post`, which
/// sends a new top-level message to a channel, `reply` adds a message to an
/// existing thread identified by `thread_id` (the `tmid` field in Rocket.Chat's
/// API).
///
/// **Key inputs required:**
/// - `room_id`: The ID of the room/channel containing the thread
/// - `thread_id`: The ID of the parent message being replied to
/// - `text`: The reply message content
///
/// **Thread IDs** can be obtained from:
/// - Previous message objects (check the `thread_id` field in message
///   responses)
/// - Reading channel history with the `read` tool
/// - Existing messages that have been threaded in Rocket.Chat
///
/// **Important notes:**
/// - Reply text cannot contain newline characters
/// - The reply will be associated with the parent message and appear in the
///   thread view
/// - If the thread doesn't exist, the reply will still be posted and linked to
///   the parent
/// - Use `post` instead for new top-level messages that aren't replies
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - team-chat
/// - rocket-chat
///
/// # Errors
///
/// Returns an error if:
/// - The Rocket.Chat credential is not configured or is missing required fields
/// - The credential endpoint is empty or contains only whitespace
/// - The `room_id`, `thread_id`, or `text` fields are empty or contain only
///   whitespace
/// - The `room_id`, `thread_id`, or `text` fields contain newline characters
/// - The HTTP request to Rocket.Chat API fails
/// - The Rocket.Chat API returns a non-success response (e.g., room not found,
///   thread not found, unauthorized access)
/// - The response JSON cannot be parsed or does not contain a message
#[tool]
pub async fn reply(ctx: Context, input: ReplyInput) -> Result<ReplyOutput> {
    let cred = RocketChatCredential::get(&ctx)?;
    let http = RocketChatHttp::new(&cred)?;

    let room_id = validate_nonempty_no_newlines(&input.room_id, "room_id")?;
    let thread_id = validate_nonempty_no_newlines(&input.thread_id, "thread_id")?;
    let text = validate_nonempty_no_newlines(&input.text, "text")?;

    let req = PostMessageRequest {
        room_id: Some(room_id),
        channel: None,
        text,
        tmid: Some(thread_id),
    };

    let resp: PostMessageResponse = http.post_json("api/v1/chat.postMessage", &req).await?;
    ensure!(
        resp.success,
        "Rocket.Chat reply failed: {}",
        resp.error.unwrap_or_else(|| "unknown error".to_string())
    );

    let message = resp
        .message
        .ok_or_else(|| operai::anyhow::anyhow!("Rocket.Chat reply returned no message"))?;

    Ok(ReplyOutput {
        message: map_message(message, &http.base_url),
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadInput {
    /// Rocket.Chat room ID to read from.
    pub room_id: String,
    /// Number of messages to return (1-100). Defaults to 20.
    #[serde(default)]
    pub count: Option<u32>,
    /// Number of messages to skip for pagination (>= 0).
    #[serde(default)]
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[must_use]
pub struct ReadOutput {
    pub messages: Vec<Message>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelHistoryQuery<'a> {
    room_id: &'a str,
    count: u32,
    offset: u32,
}

/// # Read Rocket.Chat Messages
///
/// Retrieves recent messages from a Rocket.Chat channel or room.
///
/// Use this tool when a user wants to:
/// - View recent conversation history in a channel
/// - Catch up on messages that were sent while away
/// - Retrieve context before posting a reply
/// - Monitor activity in a specific channel
/// - Analyze or summarize channel discussions
///
/// This tool fetches message history from a specified room, returning messages
/// in chronological order (oldest to newest). Each message includes:
/// - `id`: Unique message identifier
/// - `room_id`: The room where the message was sent
/// - `text`: The message content (or file attachment URL for file uploads)
/// - `timestamp`: When the message was sent
/// - `thread_id`: Present if the message is part of a thread
/// - `user`: Information about the message author
///
/// **Pagination options:**
/// - `count`: Number of messages to retrieve (1-100, defaults to 20)
/// - `offset`: Number of messages to skip for pagination (defaults to 0)
///
/// **Use cases:**
/// - To get the latest 20 messages: Use default parameters
/// - To get more history: Increase `count` up to 100
/// - To paginate through history: Use `offset` to skip already-fetched messages
/// - To monitor for new messages: Repeatedly call with the same parameters and
///   check for new message IDs
///
/// **Important notes:**
/// - Requires a `room_id` (obtainable from `list_channels`)
/// - Returns messages in reverse chronological order from the API
/// - Threaded messages include `thread_id` for use with the `reply` tool
/// - File uploads show attachment URLs instead of text content
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - team-chat
/// - rocket-chat
///
/// # Errors
///
/// Returns an error if:
/// - The Rocket.Chat credential is not configured or is missing required fields
/// - The credential endpoint is empty or contains only whitespace
/// - The `room_id` field is empty or contains only whitespace
/// - The `room_id` field contains newline characters
/// - The `count` parameter is outside the valid range (1-100)
/// - The HTTP request to Rocket.Chat API fails
/// - The Rocket.Chat API returns a non-success response (e.g., room not found,
///   unauthorized access)
/// - The response JSON cannot be parsed
#[tool]
pub async fn read(ctx: Context, input: ReadInput) -> Result<ReadOutput> {
    let cred = RocketChatCredential::get(&ctx)?;
    let http = RocketChatHttp::new(&cred)?;

    let room_id = validate_nonempty_no_newlines(&input.room_id, "room_id")?;
    let count = input.count.unwrap_or(20);
    ensure!(
        (1..=100).contains(&count),
        "count must be between 1 and 100"
    );
    let query = ChannelHistoryQuery {
        room_id,
        count,
        offset: input.offset.unwrap_or(0),
    };

    let resp: ChannelHistoryResponse = http
        .get_json("api/v1/channels.history", Some(&query))
        .await?;
    ensure!(
        resp.success,
        "Rocket.Chat read messages failed: {}",
        resp.error.unwrap_or_else(|| "unknown error".to_string())
    );

    Ok(ReadOutput {
        messages: resp
            .messages
            .into_iter()
            .map(|m| map_message(m, &http.base_url))
            .collect(),
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadInput {
    /// Rocket.Chat room ID to upload into.
    pub room_id: String,
    /// File name to use for the upload.
    pub file_name: String,
    /// File contents as base64.
    pub file_base64: String,
    /// Optional message to accompany the file (sent as `msg`).
    #[serde(default)]
    pub message: Option<String>,
    /// Optional description for the uploaded file.
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[must_use]
pub struct UploadOutput {
    pub message: Message,
}

fn decode_base64(data: &str) -> Result<Vec<u8>> {
    let cleaned: String = data.chars().filter(|c| !c.is_whitespace()).collect();
    base64::engine::general_purpose::STANDARD
        .decode(cleaned.as_bytes())
        .map_err(Into::into)
}

/// # Upload Rocket.Chat File
///
/// Uploads a file to a Rocket.Chat room as a message attachment.
///
/// Use this tool when a user wants to:
/// - Share documents, images, or other files in a channel
/// - Upload files that are too large to embed as text
/// - Send binary content (images, PDFs, archives, etc.) to team members
/// - Attach files to a conversation for reference or collaboration
///
/// This tool handles file uploads using multipart/form-data encoding. The file
/// content must be provided as a base64-encoded string, which will be decoded
/// and uploaded to the Rocket.Chat server.
///
/// **Key inputs required:**
/// - `room_id`: The ID of the room/channel to upload the file to
/// - `file_name`: The name to assign to the uploaded file (e.g., "report.pdf",
///   "image.png")
/// - `file_base64`: Base64-encoded file contents (whitespace is tolerated)
///
/// **Optional inputs:**
/// - `message`: A text message to accompany the file upload (appears as the
///   message text)
/// - `description`: A description for the uploaded file metadata
///
/// **How base64 encoding works:**
/// - The file content must be base64-encoded before being passed to this tool
/// - Most programming languages provide built-in base64 encoding functions
/// - Example in Python: `base64.b64encode(file_bytes).decode('utf-8')`
/// - The tool automatically strips whitespace from the base64 string before
///   decoding
///
/// **Important notes:**
/// - The file will appear as a message in the room with a download link
/// - File size limits are enforced by the Rocket.Chat server (typically
///   10-200MB depending on configuration)
/// - The returned message object includes the attachment URL
/// - Upload failures may occur due to file size restrictions or server storage
///   limits
/// - The file content is validated to ensure it decodes to non-empty bytes
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - team-chat
/// - rocket-chat
///
/// # Errors
///
/// Returns an error if:
/// - The Rocket.Chat credential is not configured or is missing required fields
/// - The credential endpoint is empty or contains only whitespace
/// - The `room_id` or `file_name` fields are empty or contain only whitespace
/// - The `room_id`, `file_name`, `message`, or `description` fields contain
///   newline characters
/// - The `file_base64` data is invalid base64 or decodes to empty bytes
/// - The HTTP request to Rocket.Chat API fails
/// - The Rocket.Chat API returns a non-success response (e.g., room not found,
///   file too large, unauthorized access)
/// - The response JSON cannot be parsed or does not contain a message
#[tool]
pub async fn upload(ctx: Context, input: UploadInput) -> Result<UploadOutput> {
    let cred = RocketChatCredential::get(&ctx)?;
    let http = RocketChatHttp::new(&cred)?;

    let room_id = validate_nonempty_no_newlines(&input.room_id, "room_id")?;
    let file_name = validate_nonempty_no_newlines(&input.file_name, "file_name")?;
    let bytes = decode_base64(&input.file_base64)?;
    ensure!(
        !bytes.is_empty(),
        "file_base64 must decode to non-empty bytes"
    );

    let file_part = multipart::Part::bytes(bytes).file_name(file_name.to_string());
    let mut form = multipart::Form::new().part("file", file_part);

    if let Some(message) = input.message.as_deref() {
        form = form.text(
            "msg",
            validate_nonempty_no_newlines(message, "message")?.to_string(),
        );
    }
    if let Some(description) = input.description.as_deref() {
        form = form.text(
            "description",
            validate_nonempty_no_newlines(description, "description")?.to_string(),
        );
    }

    let path = format!("api/v1/rooms.media/{room_id}");
    let resp: PostMessageResponse = http.post_multipart(&path, form).await?;
    ensure!(
        resp.success,
        "Rocket.Chat upload failed: {}",
        resp.error.unwrap_or_else(|| "unknown error".to_string())
    );

    let message = resp
        .message
        .ok_or_else(|| operai::anyhow::anyhow!("Rocket.Chat upload returned no message"))?;

    Ok(UploadOutput {
        message: map_message(message, &http.base_url),
    })
}

operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_json, header, method, path, query_param},
    };

    use super::*;

    // =========================================================================
    // Serialization roundtrip tests
    // =========================================================================

    #[test]
    fn test_channel_serialization_roundtrip() {
        let channel = Channel {
            id: "c1".to_string(),
            name: "general".to_string(),
            display_name: Some("General Chat".to_string()),
            room_type: Some("c".to_string()),
        };

        let json = serde_json::to_string(&channel).unwrap();
        let parsed: Channel = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, channel.id);
        assert_eq!(parsed.name, channel.name);
        assert_eq!(parsed.display_name, channel.display_name);
        assert_eq!(parsed.room_type, channel.room_type);
    }

    #[test]
    fn test_channel_optional_fields_default_to_none() {
        let json = r#"{"id":"c1","name":"test"}"#;
        let channel: Channel = serde_json::from_str(json).unwrap();

        assert_eq!(channel.id, "c1");
        assert_eq!(channel.name, "test");
        assert_eq!(channel.display_name, None);
        assert_eq!(channel.room_type, None);
    }

    #[test]
    fn test_user_serialization_roundtrip() {
        let user = User {
            id: "u1".to_string(),
            username: Some("alice".to_string()),
            name: Some("Alice Smith".to_string()),
        };

        let json = serde_json::to_string(&user).unwrap();
        let parsed: User = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, user.id);
        assert_eq!(parsed.username, user.username);
        assert_eq!(parsed.name, user.name);
    }

    #[test]
    fn test_message_serialization_roundtrip() {
        let message = Message {
            id: "m1".to_string(),
            room_id: "r1".to_string(),
            text: Some("Hello".to_string()),
            timestamp: Some("2024-01-01T00:00:00.000Z".to_string()),
            thread_id: Some("t1".to_string()),
            user: Some(User {
                id: "u1".to_string(),
                username: Some("bob".to_string()),
                name: None,
            }),
        };

        let json = serde_json::to_string(&message).unwrap();
        let parsed: Message = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, message.id);
        assert_eq!(parsed.room_id, message.room_id);
        assert_eq!(parsed.text, message.text);
        assert_eq!(parsed.thread_id, message.thread_id);
        assert!(parsed.user.is_some());
    }

    #[test]
    fn test_list_channels_input_default_values() {
        let json = r"{}";
        let input: ListChannelsInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.offset, None);
        assert_eq!(input.count, None);
    }

    // =========================================================================
    // Validation helper tests
    // =========================================================================

    #[test]
    fn test_validate_nonempty_no_newlines_trims_whitespace() {
        let result = validate_nonempty_no_newlines("  hello  ", "field").unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_validate_nonempty_no_newlines_rejects_empty_string() {
        let err = validate_nonempty_no_newlines("", "myfield").unwrap_err();
        assert!(err.to_string().contains("myfield must not be empty"));
    }

    #[test]
    fn test_validate_nonempty_no_newlines_rejects_whitespace_only() {
        let err = validate_nonempty_no_newlines("   ", "myfield").unwrap_err();
        assert!(err.to_string().contains("myfield must not be empty"));
    }

    #[test]
    fn test_validate_nonempty_no_newlines_rejects_newline() {
        let err = validate_nonempty_no_newlines("hello\nworld", "myfield").unwrap_err();
        assert!(
            err.to_string()
                .contains("myfield must not contain newlines")
        );
    }

    #[test]
    fn test_validate_nonempty_no_newlines_rejects_carriage_return() {
        let err = validate_nonempty_no_newlines("hello\rworld", "myfield").unwrap_err();
        assert!(
            err.to_string()
                .contains("myfield must not contain newlines")
        );
    }

    // =========================================================================
    // Channel name normalization tests
    // =========================================================================

    #[test]
    fn test_normalize_channel_name_adds_hash_prefix() {
        let result = normalize_channel_name("general").unwrap();
        assert_eq!(result, "#general");
    }

    #[test]
    fn test_normalize_channel_name_preserves_existing_hash() {
        let result = normalize_channel_name("#general").unwrap();
        assert_eq!(result, "#general");
    }

    #[test]
    fn test_normalize_channel_name_preserves_at_prefix() {
        let result = normalize_channel_name("@alice").unwrap();
        assert_eq!(result, "@alice");
    }

    #[test]
    fn test_normalize_channel_name_trims_whitespace() {
        let result = normalize_channel_name("  general  ").unwrap();
        assert_eq!(result, "#general");
    }

    #[test]
    fn test_normalize_channel_name_rejects_empty() {
        let err = normalize_channel_name("").unwrap_err();
        assert!(err.to_string().contains("channel must not be empty"));
    }

    // =========================================================================
    // Base64 decoding tests
    // =========================================================================

    #[test]
    fn test_decode_base64_valid_input() {
        let result = decode_base64("SGVsbG8=").unwrap();
        assert_eq!(result, b"Hello");
    }

    #[test]
    fn test_decode_base64_strips_whitespace() {
        let result = decode_base64("SGVs\n bG8=").unwrap();
        assert_eq!(result, b"Hello");
    }

    #[test]
    fn test_decode_base64_invalid_input() {
        let err = decode_base64("!!!invalid!!!").unwrap_err();
        assert!(err.to_string().to_lowercase().contains("invalid"));
    }

    // =========================================================================
    // RocketChatHttp validation tests
    // =========================================================================

    #[test]
    fn test_rocket_chat_http_rejects_empty_endpoint() {
        let cred = RocketChatCredential {
            auth_token: "token".to_string(),
            user_id: "user".to_string(),
            endpoint: None,
        };

        match RocketChatHttp::new(&cred) {
            Err(err) => assert!(err.to_string().contains("endpoint")),
            Ok(_) => panic!("expected error for empty endpoint"),
        }
    }

    #[test]
    fn test_rocket_chat_http_rejects_whitespace_endpoint() {
        let cred = RocketChatCredential {
            auth_token: "token".to_string(),
            user_id: "user".to_string(),
            endpoint: Some("   ".to_string()),
        };

        match RocketChatHttp::new(&cred) {
            Err(err) => assert!(err.to_string().contains("endpoint")),
            Ok(_) => panic!("expected error for whitespace endpoint"),
        }
    }

    #[test]
    fn test_rocket_chat_http_accepts_valid_endpoint() {
        let cred = RocketChatCredential {
            auth_token: "token".to_string(),
            user_id: "user".to_string(),
            endpoint: Some("https://chat.example.com".to_string()),
        };

        let http = RocketChatHttp::new(&cred).unwrap();
        assert!(
            http.base_url
                .as_str()
                .starts_with("https://chat.example.com")
        );
    }

    #[test]
    fn test_rocket_chat_http_appends_trailing_slash() {
        let cred = RocketChatCredential {
            auth_token: "token".to_string(),
            user_id: "user".to_string(),
            endpoint: Some("https://chat.example.com".to_string()),
        };

        let http = RocketChatHttp::new(&cred).unwrap();
        assert!(http.base_url.as_str().ends_with('/'));
    }

    // =========================================================================
    // map_message helper tests
    // =========================================================================

    #[test]
    fn test_map_message_with_text() {
        let base_url = Url::parse("https://chat.example.com/").unwrap();
        let api_message = ApiMessage {
            id: "m1".to_string(),
            room_id: "r1".to_string(),
            text: Some("Hello".to_string()),
            timestamp: Some("2024-01-01T00:00:00.000Z".to_string()),
            tmid: Some("t1".to_string()),
            u: Some(ApiUser {
                id: "u1".to_string(),
                username: Some("alice".to_string()),
                name: Some("Alice".to_string()),
            }),
            attachments: vec![],
        };

        let message = map_message(api_message, &base_url);

        assert_eq!(message.id, "m1");
        assert_eq!(message.room_id, "r1");
        assert_eq!(message.text, Some("Hello".to_string()));
        assert_eq!(message.thread_id, Some("t1".to_string()));
        assert!(message.user.is_some());
        let user = message.user.unwrap();
        assert_eq!(user.id, "u1");
        assert_eq!(user.username, Some("alice".to_string()));
    }

    #[test]
    fn test_map_message_falls_back_to_attachment_link() {
        let base_url = Url::parse("https://chat.example.com/").unwrap();
        let api_message = ApiMessage {
            id: "m1".to_string(),
            room_id: "r1".to_string(),
            text: None,
            timestamp: None,
            tmid: None,
            u: None,
            attachments: vec![ApiAttachment {
                title_link: Some("/file-upload/abc.pdf".to_string()),
            }],
        };

        let message = map_message(api_message, &base_url);

        assert_eq!(
            message.text,
            Some("https://chat.example.com/file-upload/abc.pdf".to_string())
        );
    }

    #[test]
    fn test_map_message_no_text_no_attachments() {
        let base_url = Url::parse("https://chat.example.com/").unwrap();
        let api_message = ApiMessage {
            id: "m1".to_string(),
            room_id: "r1".to_string(),
            text: None,
            timestamp: None,
            tmid: None,
            u: None,
            attachments: vec![],
        };

        let message = map_message(api_message, &base_url);

        assert_eq!(message.text, None);
    }

    // =========================================================================
    // Input validation boundary tests
    // =========================================================================

    #[tokio::test]
    async fn test_list_channels_rejects_count_zero() {
        let server = MockServer::start().await;
        let ctx = test_context(&server.uri());

        let err = list_channels(
            ctx,
            ListChannelsInput {
                offset: None,
                count: Some(0),
            },
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("count must be between 1 and 100"));
    }

    #[tokio::test]
    async fn test_list_channels_rejects_count_over_100() {
        let server = MockServer::start().await;
        let ctx = test_context(&server.uri());

        let err = list_channels(
            ctx,
            ListChannelsInput {
                offset: None,
                count: Some(101),
            },
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("count must be between 1 and 100"));
    }

    #[tokio::test]
    async fn test_post_requires_room_id_or_channel() {
        let server = MockServer::start().await;
        let ctx = test_context(&server.uri());

        let err = post(
            ctx,
            PostInput {
                room_id: None,
                channel: None,
                text: "hello".to_string(),
            },
        )
        .await
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("must provide either room_id or channel")
        );
    }

    #[tokio::test]
    async fn test_post_rejects_empty_text() {
        let server = MockServer::start().await;
        let ctx = test_context(&server.uri());

        let err = post(
            ctx,
            PostInput {
                room_id: Some("r1".to_string()),
                channel: None,
                text: "   ".to_string(),
            },
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("text must not be empty"));
    }

    #[tokio::test]
    async fn test_read_rejects_count_zero() {
        let server = MockServer::start().await;
        let ctx = test_context(&server.uri());

        let err = read(
            ctx,
            ReadInput {
                room_id: "r1".to_string(),
                count: Some(0),
                offset: None,
            },
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("count must be between 1 and 100"));
    }

    #[tokio::test]
    async fn test_upload_rejects_empty_file() {
        let server = MockServer::start().await;
        let ctx = test_context(&server.uri());

        let err = upload(
            ctx,
            UploadInput {
                room_id: "r1".to_string(),
                file_name: "test.txt".to_string(),
                file_base64: String::new(), // Empty base64 decodes to empty bytes
                message: None,
                description: None,
            },
        )
        .await
        .unwrap_err();

        // Empty string is invalid base64, but if it were valid empty it would fail on
        // empty bytes check
        assert!(
            err.to_string().to_lowercase().contains("invalid") || err.to_string().contains("empty")
        );
    }

    // =========================================================================
    // Original integration tests
    // =========================================================================

    fn test_context(endpoint: &str) -> Context {
        let mut cred_fields = HashMap::new();
        cred_fields.insert("auth_token".to_string(), "test-token".to_string());
        cred_fields.insert("user_id".to_string(), "test-user-id".to_string());
        cred_fields.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-1", "sess-1", "user-1")
            .with_user_credential("rocket-chat", cred_fields)
    }

    #[tokio::test]
    async fn list_channels_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/channels.listJoined"))
            .and(query_param("count", "100"))
            .and(query_param("offset", "0"))
            .and(header("x-auth-token", "test-token"))
            .and(header("x-user-id", "test-user-id"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"success":true,"channels":[{"_id":"c1","name":"general","fname":"General","t":"c"}]}"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_context(&server.uri());
        let out = list_channels(
            ctx,
            ListChannelsInput {
                offset: None,
                count: None,
            },
        )
        .await
        .expect("list_channels");

        assert_eq!(out.channels.len(), 1);
        assert_eq!(out.channels[0].id, "c1");
        assert_eq!(out.channels[0].name, "general");
        assert_eq!(out.channels[0].display_name.as_deref(), Some("General"));
        assert_eq!(out.channels[0].room_type.as_deref(), Some("c"));
    }

    #[tokio::test]
    async fn list_channels_success_false_returns_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/channels.listJoined"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"success":false,"error":"unauthorized"}"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_context(&server.uri());
        let err = list_channels(
            ctx,
            ListChannelsInput {
                offset: None,
                count: None,
            },
        )
        .await
        .expect_err("should error");
        assert!(err.to_string().contains("unauthorized"));
    }

    #[tokio::test]
    async fn post_success() {
        let server = MockServer::start().await;

        let expected_body = PostMessageRequest {
            room_id: Some("r1"),
            channel: None,
            text: "hello",
            tmid: None,
        };

        Mock::given(method("POST"))
            .and(path("/api/v1/chat.postMessage"))
            .and(header("x-auth-token", "test-token"))
            .and(header("x-user-id", "test-user-id"))
            .and(body_json(expected_body))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"success":true,"message":{"_id":"m1","rid":"r1","msg":"hello","ts":"2024-01-01T00:00:00.000Z","u":{"_id":"u1","username":"alice"}}}"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_context(&server.uri());
        let out = post(
            ctx,
            PostInput {
                room_id: Some("r1".to_string()),
                channel: None,
                text: "hello".to_string(),
            },
        )
        .await
        .expect("post");

        assert_eq!(out.message.id, "m1");
        assert_eq!(out.message.room_id, "r1");
        assert_eq!(out.message.text.as_deref(), Some("hello"));
        assert_eq!(
            out.message
                .user
                .as_ref()
                .and_then(|u| u.username.as_deref()),
            Some("alice")
        );
    }

    #[tokio::test]
    async fn post_http_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/chat.postMessage"))
            .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
            .mount(&server)
            .await;

        let ctx = test_context(&server.uri());
        let err = post(
            ctx,
            PostInput {
                room_id: Some("r1".to_string()),
                channel: None,
                text: "hello".to_string(),
            },
        )
        .await
        .expect_err("should error");
        assert!(err.to_string().contains("HTTP 401"));
    }

    #[tokio::test]
    async fn reply_success() {
        let server = MockServer::start().await;

        let expected_body = PostMessageRequest {
            room_id: Some("r1"),
            channel: None,
            text: "reply",
            tmid: Some("parent1"),
        };

        Mock::given(method("POST"))
            .and(path("/api/v1/chat.postMessage"))
            .and(body_json(expected_body))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"success":true,"message":{"_id":"m2","rid":"r1","msg":"reply","tmid":"parent1","ts":"2024-01-01T00:00:01.000Z"}}"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_context(&server.uri());
        let out = reply(
            ctx,
            ReplyInput {
                room_id: "r1".to_string(),
                thread_id: "parent1".to_string(),
                text: "reply".to_string(),
            },
        )
        .await
        .expect("reply");

        assert_eq!(out.message.id, "m2");
        assert_eq!(out.message.thread_id.as_deref(), Some("parent1"));
        assert_eq!(out.message.text.as_deref(), Some("reply"));
    }

    #[tokio::test]
    async fn reply_success_false_returns_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/chat.postMessage"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"success":false,"error":"error-room-not-found"}"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_context(&server.uri());
        let err = reply(
            ctx,
            ReplyInput {
                room_id: "r1".to_string(),
                thread_id: "parent1".to_string(),
                text: "reply".to_string(),
            },
        )
        .await
        .expect_err("should error");
        assert!(err.to_string().contains("error-room-not-found"));
    }

    #[tokio::test]
    async fn read_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/channels.history"))
            .and(query_param("roomId", "r1"))
            .and(query_param("count", "2"))
            .and(query_param("offset", "0"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"success":true,"messages":[{"_id":"m1","rid":"r1","msg":"one","ts":"2024-01-01T00:00:00.000Z"},{"_id":"m2","rid":"r1","msg":"two","ts":"2024-01-01T00:00:01.000Z"}]}"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_context(&server.uri());
        let out = read(
            ctx,
            ReadInput {
                room_id: "r1".to_string(),
                count: Some(2),
                offset: None,
            },
        )
        .await
        .expect("read");

        assert_eq!(out.messages.len(), 2);
        assert_eq!(out.messages[0].id, "m1");
        assert_eq!(out.messages[1].id, "m2");
    }

    #[tokio::test]
    async fn read_http_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/channels.history"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server)
            .await;

        let ctx = test_context(&server.uri());
        let err = read(
            ctx,
            ReadInput {
                room_id: "r1".to_string(),
                count: Some(1),
                offset: None,
            },
        )
        .await
        .expect_err("should error");
        assert!(err.to_string().contains("HTTP 500"));
    }

    #[tokio::test]
    async fn upload_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/rooms.media/r1"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"success":true,"message":{"_id":"m3","rid":"r1","attachments":[{"title_link":"/file-upload/file1.txt"}]}}"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_context(&server.uri());
        let out = upload(
            ctx,
            UploadInput {
                room_id: "r1".to_string(),
                file_name: "file1.txt".to_string(),
                file_base64: "SGVsbG8=".to_string(),
                message: Some("hi".to_string()),
                description: None,
            },
        )
        .await
        .expect("upload");

        assert_eq!(out.message.id, "m3");
        let expected_url = format!("{}/file-upload/file1.txt", server.uri());
        assert_eq!(out.message.text.as_deref(), Some(expected_url.as_str()));

        let requests = server.received_requests().await.expect("requests");
        assert_eq!(requests.len(), 1);
        let body_str = String::from_utf8_lossy(&requests[0].body);
        assert!(body_str.contains("Hello"));
        assert!(body_str.contains("file1.txt"));
    }

    #[tokio::test]
    async fn upload_http_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/rooms.media/r1"))
            .respond_with(ResponseTemplate::new(413).set_body_string("too big"))
            .mount(&server)
            .await;

        let ctx = test_context(&server.uri());
        let err = upload(
            ctx,
            UploadInput {
                room_id: "r1".to_string(),
                file_name: "file1.txt".to_string(),
                file_base64: "SGVsbG8=".to_string(),
                message: None,
                description: None,
            },
        )
        .await
        .expect_err("should error");
        assert!(err.to_string().contains("HTTP 413"));
    }
}
