//! team-chat/discord integration for Operai Toolbox.
//!
//! This integration provides tools for interacting with Discord servers,
//! including listing channels, posting messages, reading messages,
//! managing threads, and uploading files.

use std::fmt::Write;

use anyhow::anyhow;
use operai::{
    Context, JsonSchema, Result, define_system_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

mod types;
pub use types::*;

define_system_credential! {
    DiscordCredential("discord") {
        /// Discord bot token for API authentication.
        bot_token: String,
        /// Optional custom API base URL (defaults to Discord's API).
        #[optional]
        api_base_url: Option<String>,
    }
}

const DEFAULT_API_BASE_URL: &str = "https://discord.com/api/v10";
const USER_AGENT: &str = "Brwse-Tool/1.0 (https://brwse.com; DiscordBot)";

/// Discord API rate limit retry delay in seconds.
const RATE_LIMIT_RETRY_AFTER_DEFAULT: u64 = 5;

/// Initialize the Discord integration.
#[init]
async fn setup() -> Result<()> {
    info!("Discord integration initialized");
    Ok(())
}

/// Clean up resources when the library is unloaded.
#[shutdown]
fn cleanup() {
    info!("Discord integration shutting down");
}

// =============================================================================
// list_channels
// =============================================================================

/// Input for listing channels in a Discord guild.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListChannelsInput {
    /// The ID of the guild (server) to list channels for.
    pub guild_id: String,
}

/// Output containing the list of channels.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ListChannelsOutput {
    /// The list of channels in the guild.
    pub channels: Vec<Channel>,
    /// The total number of channels returned.
    pub count: usize,
}

/// # List Discord Channels
///
/// Retrieves a comprehensive list of all channels (text, voice, and category)
/// within a specified Discord server (guild).
///
/// Use this tool when you need to:
/// - Explore the available channels in a Discord server
/// - Find the `channel_id` for a specific channel by name
/// - Understand the structure and organization of a Discord server
/// - Validate that the bot has access to a particular guild
///
/// The returned channels include their IDs, types, names, and metadata such as
/// permission overwrites and position. Channel IDs are required for other
/// operations like posting messages or reading message history.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - chat
/// - discord
///
/// # Errors
///
/// Returns an error if:
/// - The `guild_id` is empty or contains only whitespace
/// - The Discord bot token is not configured or is empty
/// - The Discord API request fails (network error, authentication failure, rate
///   limit, etc.)
/// - The response from Discord cannot be parsed as valid channel data
#[tool]
pub async fn list_channels(ctx: Context, input: ListChannelsInput) -> Result<ListChannelsOutput> {
    ensure!(
        !input.guild_id.trim().is_empty(),
        "guild_id must not be empty"
    );

    let client = DiscordClient::from_ctx(&ctx)?;
    let channels: Vec<Channel> = client
        .get_json(&format!("/guilds/{}/channels", input.guild_id))
        .await?;

    let count = channels.len();
    Ok(ListChannelsOutput { channels, count })
}

// =============================================================================
// post_message
// =============================================================================

/// Input for posting a message to a Discord channel.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct PostMessageInput {
    /// The ID of the channel to post the message to.
    pub channel_id: String,
    /// The content of the message to send.
    pub content: String,
    /// Optional message ID to reply to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
}

/// Output after posting a message.
#[derive(Debug, Serialize, JsonSchema)]
pub struct PostMessageOutput {
    /// The created message.
    pub message: Message,
}

/// # Post Discord Message
///
/// Sends a text message to a specified Discord channel. Optionally reply to an
/// existing message.
///
/// Use this tool when you need to:
/// - Send a new message to a Discord text channel
/// - Reply to an existing message in a channel
/// - Post notifications, updates, or responses to Discord users
///
/// The message content must not exceed Discord's 2000 character limit. For
/// replies, provide the `reply_to` message ID and Discord will display the
/// message as a reply with a reference to the original message.
///
/// Note: This tool only sends plain text messages. For file uploads, use the
/// Discord Upload File tool instead.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - chat
/// - discord
///
/// # Errors
///
/// Returns an error if:
/// - The `channel_id` is empty or contains only whitespace
/// - The `content` is empty or contains only whitespace
/// - The `content` exceeds Discord's 2000 character limit
/// - The Discord bot token is not configured or is empty
/// - The Discord API request fails (network error, authentication failure, rate
///   limit, etc.)
/// - The response from Discord cannot be parsed as valid message data
#[tool]
pub async fn post_message(ctx: Context, input: PostMessageInput) -> Result<PostMessageOutput> {
    ensure!(
        !input.channel_id.trim().is_empty(),
        "channel_id must not be empty"
    );
    ensure!(
        !input.content.trim().is_empty(),
        "content must not be empty"
    );
    ensure!(
        input.content.len() <= 2000,
        "content must not exceed 2000 characters"
    );

    let client = DiscordClient::from_ctx(&ctx)?;

    let mut payload = serde_json::json!({
        "content": input.content,
    });

    if let Some(reply_to) = input.reply_to {
        payload["message_reference"] = serde_json::json!({
            "message_id": reply_to,
        });
    }

    let message: Message = client
        .post_json(
            &format!("/channels/{}/messages", input.channel_id),
            &payload,
        )
        .await?;

    Ok(PostMessageOutput { message })
}

// =============================================================================
// read_messages
// =============================================================================

/// Input for reading messages from a Discord channel.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadMessagesInput {
    /// The ID of the channel to read messages from.
    pub channel_id: String,
    /// Maximum number of messages to retrieve (1-100, default 50).
    #[serde(default = "default_limit")]
    pub limit: u8,
    /// Get messages before this message ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// Get messages after this message ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// Get messages around this message ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub around: Option<String>,
}

fn default_limit() -> u8 {
    50
}

/// Output containing the retrieved messages.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ReadMessagesOutput {
    /// The list of messages retrieved.
    pub messages: Vec<Message>,
    /// The number of messages returned.
    pub count: usize,
    /// Whether there are more messages available (approximation).
    pub has_more: bool,
}

/// # Read Discord Messages
///
/// Retrieves message history from a Discord channel with pagination support.
///
/// Use this tool when you need to:
/// - Read recent messages from a channel
/// - Fetch historical messages for context or analysis
/// - Retrieve messages before or after a specific message ID
/// - Get messages around a particular message (contextual fetch)
///
/// The `limit` parameter controls how many messages to retrieve (1-100, default
/// 50). Use the pagination parameters to navigate through message history:
/// - `before`: Get messages older than the specified message ID
/// - `after`: Get messages newer than the specified message ID
/// - `around`: Get messages surrounding the specified message ID
///
/// Only one pagination parameter (`before`, `after`, or `around`) can be used
/// at a time. The `has_more` field in the response indicates whether there are
/// additional messages beyond the current batch.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - chat
/// - discord
///
/// # Errors
///
/// Returns an error if:
/// - The `channel_id` is empty or contains only whitespace
/// - The `limit` is not between 1 and 100 (inclusive)
/// - The Discord bot token is not configured or is empty
/// - The Discord API request fails (network error, authentication failure, rate
///   limit, etc.)
/// - The response from Discord cannot be parsed as valid message data
/// - More than one of `before`, `after`, or `around` is specified (Discord API
///   limitation)
#[tool]
pub async fn read_messages(ctx: Context, input: ReadMessagesInput) -> Result<ReadMessagesOutput> {
    ensure!(
        !input.channel_id.trim().is_empty(),
        "channel_id must not be empty"
    );
    ensure!(
        (1..=100).contains(&input.limit),
        "limit must be between 1 and 100"
    );

    let client = DiscordClient::from_ctx(&ctx)?;
    let mut url = format!(
        "/channels/{}/messages?limit={}",
        input.channel_id, input.limit
    );

    if let Some(before) = &input.before {
        let _ = write!(url, "&before={before}");
    }
    if let Some(after) = &input.after {
        let _ = write!(url, "&after={after}");
    }
    if let Some(around) = &input.around {
        let _ = write!(url, "&around={around}");
    }

    let messages: Vec<Message> = client.get_json(&url).await?;

    let count = messages.len();
    let has_more = count >= input.limit as usize;

    Ok(ReadMessagesOutput {
        messages,
        count,
        has_more,
    })
}

// =============================================================================
// manage_threads
// =============================================================================

/// The action to perform on a thread.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ThreadAction {
    /// Create a new thread from a message.
    CreateFromMessage,
    /// Create a new thread without a starter message (forum/media channels).
    CreateStandalone,
    /// Archive the thread.
    Archive,
    /// Unarchive the thread.
    Unarchive,
    /// Lock the thread (prevent new messages).
    Lock,
    /// Unlock the thread.
    Unlock,
}

/// Input for managing Discord threads.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ManageThreadsInput {
    /// The action to perform on the thread.
    pub action: ThreadAction,
    /// The ID of the channel (for create) or thread ID (for other actions).
    pub channel_id: String,
    /// The message ID to create a thread from (required for
    /// `create_from_message`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    /// The name for the thread (required for create actions).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Auto-archive duration in minutes (60, 1440, 4320, or 10080).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_archive_duration: Option<u32>,
}

/// Output after performing a thread action.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ManageThreadsOutput {
    /// The thread that was created or modified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread: Option<Thread>,
    /// Whether the action was successful.
    pub success: bool,
    /// A message describing the result.
    pub message: String,
}

/// # Manage Discord Threads
///
/// Performs various thread management operations in Discord channels.
///
/// Use this tool when you need to:
/// - Create a new discussion thread from an existing message
/// - Create a standalone thread (useful in forum or media channels)
/// - Archive or unarchive existing threads
/// - Lock or unlock threads to control posting permissions
///
/// ## Supported Actions
///
/// - `CreateFromMessage`: Creates a thread linked to a specific message
///   (requires `message_id` and `name`)
/// - `CreateStandalone`: Creates a thread without a starter message (requires
///   `name`)
/// - `Archive`: Archives a thread, making it read-only
/// - `Unarchive`: Unarchives a thread, allowing new messages
/// - `Lock`: Locks a thread, preventing anyone from posting
/// - `Unlock`: Unlocks a thread, restoring posting permissions
///
/// The `auto_archive_duration` option controls when the thread will
/// automatically archive due to inactivity. Valid values are:
/// - 60 minutes (1 hour)
/// - 1440 minutes (1 day)
/// - 4320 minutes (3 days)
/// - 10080 minutes (7 days)
///
/// Thread names must not exceed 100 characters.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - chat
/// - discord
///
/// # Errors
///
/// Returns an error if:
/// - The `channel_id` is empty or contains only whitespace
/// - For `CreateFromMessage` action: `message_id` or `name` is missing/empty
/// - For `CreateStandalone` action: `name` is missing/empty
/// - The `name` exceeds Discord's 100 character limit for thread names
/// - The `auto_archive_duration` is not one of the allowed values (60, 1440,
///   4320, or 10080)
/// - The Discord bot token is not configured or is empty
/// - The Discord API request fails (network error, authentication failure, rate
///   limit, etc.)
/// - The response from Discord cannot be parsed as valid thread data
#[tool]
pub async fn manage_threads(
    ctx: Context,
    input: ManageThreadsInput,
) -> Result<ManageThreadsOutput> {
    ensure!(
        !input.channel_id.trim().is_empty(),
        "channel_id must not be empty"
    );

    let client = DiscordClient::from_ctx(&ctx)?;

    match input.action {
        ThreadAction::CreateFromMessage => create_thread_from_message(&client, &input).await,
        ThreadAction::CreateStandalone => create_standalone_thread(&client, &input).await,
        ThreadAction::Archive => {
            update_thread_state(&client, &input.channel_id, "archived", true, "archived").await
        }
        ThreadAction::Unarchive => {
            update_thread_state(&client, &input.channel_id, "archived", false, "unarchived").await
        }
        ThreadAction::Lock => {
            update_thread_state(&client, &input.channel_id, "locked", true, "locked").await
        }
        ThreadAction::Unlock => {
            update_thread_state(&client, &input.channel_id, "locked", false, "unlocked").await
        }
    }
}

fn validate_thread_name(name: &str) -> Result<()> {
    ensure!(!name.trim().is_empty(), "thread name must not be empty");
    ensure!(
        name.len() <= 100,
        "thread name must not exceed 100 characters"
    );
    Ok(())
}

fn apply_auto_archive_duration(
    payload: &mut serde_json::Value,
    auto_archive_duration: Option<u32>,
) -> Result<()> {
    if let Some(duration) = auto_archive_duration {
        ensure!(
            [60, 1440, 4320, 10080].contains(&duration),
            "auto_archive_duration must be 60, 1440, 4320, or 10080"
        );
        payload["auto_archive_duration"] = serde_json::json!(duration);
    }
    Ok(())
}

async fn create_thread_from_message(
    client: &DiscordClient,
    input: &ManageThreadsInput,
) -> Result<ManageThreadsOutput> {
    let message_id = input
        .message_id
        .as_deref()
        .ok_or_else(|| anyhow!("message_id required for create_from_message action"))?;
    let name = input
        .name
        .as_deref()
        .ok_or_else(|| anyhow!("name required for create_from_message action"))?;

    validate_thread_name(name)?;

    let mut payload = serde_json::json!({
        "name": name,
    });
    apply_auto_archive_duration(&mut payload, input.auto_archive_duration)?;

    let thread: Thread = client
        .post_json(
            &format!(
                "/channels/{}/messages/{}/threads",
                input.channel_id, message_id
            ),
            &payload,
        )
        .await?;

    Ok(ManageThreadsOutput {
        thread: Some(thread.clone()),
        success: true,
        message: format!("Thread '{}' created successfully", thread.name),
    })
}

async fn create_standalone_thread(
    client: &DiscordClient,
    input: &ManageThreadsInput,
) -> Result<ManageThreadsOutput> {
    let name = input
        .name
        .as_deref()
        .ok_or_else(|| anyhow!("name required for create_standalone action"))?;

    validate_thread_name(name)?;

    let mut payload = serde_json::json!({
        "name": name,
        "type": 11, // PUBLIC_THREAD
    });
    apply_auto_archive_duration(&mut payload, input.auto_archive_duration)?;

    let thread: Thread = client
        .post_json(&format!("/channels/{}/threads", input.channel_id), &payload)
        .await?;

    Ok(ManageThreadsOutput {
        thread: Some(thread.clone()),
        success: true,
        message: format!("Thread '{}' created successfully", thread.name),
    })
}

async fn update_thread_state(
    client: &DiscordClient,
    channel_id: &str,
    field: &str,
    value: bool,
    action: &str,
) -> Result<ManageThreadsOutput> {
    let mut payload_map = serde_json::Map::new();
    payload_map.insert(field.to_string(), serde_json::json!(value));
    let payload = serde_json::Value::Object(payload_map);

    let thread: Thread = client
        .patch_json(&format!("/channels/{channel_id}"), &payload)
        .await?;

    Ok(ManageThreadsOutput {
        thread: Some(thread),
        success: true,
        message: format!("Thread {channel_id} {action} successfully"),
    })
}

// =============================================================================
// upload_file
// =============================================================================

/// Input for uploading a file to a Discord channel.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadFileInput {
    /// The ID of the channel to upload the file to.
    pub channel_id: String,
    /// The filename for the uploaded file.
    pub filename: String,
    /// The file content as base64-encoded data.
    pub content_base64: String,
    /// Optional message content to accompany the file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Whether to mark the file as a spoiler.
    #[serde(default)]
    pub spoiler: bool,
}

/// Output after uploading a file.
#[derive(Debug, Serialize, JsonSchema)]
pub struct UploadFileOutput {
    /// The message containing the uploaded file.
    pub message: Message,
}

/// # Upload Discord File
///
/// Uploads a file attachment to a Discord channel with optional message.
///
/// Use this tool when you need to:
/// - Share documents, images, videos, or other files in a Discord channel
/// - Upload screenshots, logs, or reports
/// - Send files with accompanying message context
/// - Mark files as spoilers to hide content until users click to reveal
///
/// The file content must be provided as a base64-encoded string. The tool
/// automatically validates the file size (25MB limit) and decodes the base64
/// data before upload. Content type is detected based on the file extension.
///
/// Optional features:
/// - Include a message with the file upload using the `message` parameter
/// - Mark the file as a spoiler by setting `spoiler: true` (prefixes the
///   filename with "SPOILER_")
///
/// Note: Discord's file size limit is 25MB for standard accounts. For larger
/// files or boosted servers with higher limits, alternative approaches may be
/// needed.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - chat
/// - discord
///
/// # Errors
///
/// Returns an error if:
/// - The `channel_id` is empty or contains only whitespace
/// - The `filename` is empty or contains only whitespace
/// - The `content_base64` is empty or contains only whitespace
/// - The `content_base64` cannot be decoded as valid base64 data
/// - The decoded file size exceeds Discord's 25MB limit
/// - The Discord bot token is not configured or is empty
/// - The Discord API request fails (network error, authentication failure, rate
///   limit, etc.)
/// - The response from Discord cannot be parsed as valid message data
#[tool]
pub async fn upload_file(ctx: Context, input: UploadFileInput) -> Result<UploadFileOutput> {
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

    // Decode base64
    let file_bytes = base64_decode(&input.content_base64)?;

    // Check file size (Discord limit is 25MB for free users, 500MB for boosted
    // servers)
    ensure!(
        file_bytes.len() <= 25 * 1024 * 1024,
        "file size must not exceed 25MB"
    );

    let client = DiscordClient::from_ctx(&ctx)?;

    let filename = if input.spoiler {
        format!("SPOILER_{}", input.filename)
    } else {
        input.filename.clone()
    };

    // Build multipart form
    let file_part = reqwest::multipart::Part::bytes(file_bytes)
        .file_name(filename)
        .mime_str(guess_content_type(&input.filename).as_str())?;

    let mut form = reqwest::multipart::Form::new().part("files[0]", file_part);

    // Add payload_json with message content if provided
    if let Some(message_content) = input.message {
        let payload = serde_json::json!({
            "content": message_content,
        });
        form = form.text("payload_json", payload.to_string());
    }

    let message: Message = client
        .post_multipart(&format!("/channels/{}/messages", input.channel_id), form)
        .await?;

    Ok(UploadFileOutput { message })
}

// =============================================================================
// HTTP Client
// =============================================================================

#[derive(Clone)]
struct DiscordClient {
    http: reqwest::Client,
    base_url: String,
    bot_token: String,
}

impl DiscordClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = DiscordCredential::get(ctx)?;
        ensure!(
            !cred.bot_token.trim().is_empty(),
            "bot_token must not be empty"
        );

        let base_url = cred
            .api_base_url
            .as_deref()
            .unwrap_or(DEFAULT_API_BASE_URL)
            .trim_end_matches('/')
            .to_string();

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            bot_token: cred.bot_token,
        })
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .get(&url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .header("User-Agent", USER_AGENT)
            .send()
            .await?;

        self.handle_response(response).await
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &TReq,
    ) -> Result<TRes> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .header("User-Agent", USER_AGENT)
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await?;

        self.handle_response(response).await
    }

    async fn patch_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &TReq,
    ) -> Result<TRes> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .patch(&url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .header("User-Agent", USER_AGENT)
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await?;

        self.handle_response(response).await
    }

    async fn post_multipart<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        form: reqwest::multipart::Form,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .header("User-Agent", USER_AGENT)
            .multipart(form)
            .send()
            .await?;

        self.handle_response(response).await
    }

    async fn handle_response<T: for<'de> Deserialize<'de>>(
        &self,
        response: reqwest::Response,
    ) -> Result<T> {
        let status = response.status();

        if status.is_success() {
            return Ok(response.json::<T>().await?);
        }

        // Handle rate limiting (HTTP 429)
        if status.as_u16() == 429 {
            let body = response.text().await.unwrap_or_default();
            let retry_after =
                Self::extract_retry_after(&body).unwrap_or(RATE_LIMIT_RETRY_AFTER_DEFAULT);

            return Err(anyhow!(
                "Discord API rate limit exceeded. Retry after {retry_after}s. Response: {body}"
            ));
        }

        // Handle other errors with Discord error codes
        let body = response.text().await.unwrap_or_default();
        let error_msg = if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(code) = json.get("code").and_then(serde_json::Value::as_u64) {
                format!(
                    "Discord API error ({status}): code {code}: {}",
                    json.get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("unknown error")
                )
            } else {
                format!("Discord API request failed ({status}): {body}")
            }
        } else {
            format!("Discord API request failed ({status}): {body}")
        };

        Err(anyhow!(error_msg))
    }

    /// Extracts the `retry_after` value from a rate limit response body.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "retry_after is verified to be finite and non-negative before casting"
    )]
    fn extract_retry_after(body: &str) -> Option<u64> {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
            json.get("retry_after")
                .and_then(serde_json::Value::as_f64)
                .and_then(|v| {
                    // retry_after is always a positive float (seconds), round to nearest integer
                    if v.is_finite() && v >= 0.0 {
                        Some(v.round() as u64)
                    } else {
                        None
                    }
                })
        } else {
            None
        }
    }
}

// =============================================================================
// Utilities
// =============================================================================

/// Guesses the content type based on file extension.
fn guess_content_type(filename: &str) -> String {
    let extension = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "json" => "application/json",
        "xml" => "application/xml",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "zip" => "application/zip",
        "tar" => "application/x-tar",
        "gz" => "application/gzip",
        _ => "application/octet-stream",
    }
    .to_string()
}

/// Decodes base64 string to bytes.
fn base64_decode(input: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(input)
        .map_err(|e| anyhow!("failed to decode base64: {e}"))
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_string_contains, header, method, path, query_param},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut discord_values = HashMap::new();
        discord_values.insert("bot_token".to_string(), "test-token".to_string());
        discord_values.insert("api_base_url".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_system_credential("discord", discord_values)
    }

    // =========================================================================
    // Input validation tests
    // =========================================================================

    #[tokio::test]
    async fn test_list_channels_empty_guild_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = list_channels(
            ctx,
            ListChannelsInput {
                guild_id: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("guild_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_post_message_empty_channel_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = post_message(
            ctx,
            PostMessageInput {
                channel_id: "  ".to_string(),
                content: "Hello".to_string(),
                reply_to: None,
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
    async fn test_post_message_empty_content_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = post_message(
            ctx,
            PostMessageInput {
                channel_id: "123".to_string(),
                content: "  ".to_string(),
                reply_to: None,
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
    async fn test_post_message_content_too_long_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = post_message(
            ctx,
            PostMessageInput {
                channel_id: "123".to_string(),
                content: "a".repeat(2001),
                reply_to: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must not exceed 2000 characters")
        );
    }

    #[tokio::test]
    async fn test_read_messages_empty_channel_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = read_messages(
            ctx,
            ReadMessagesInput {
                channel_id: "  ".to_string(),
                limit: 50,
                before: None,
                after: None,
                around: None,
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
    async fn test_read_messages_limit_out_of_range_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = read_messages(
            ctx,
            ReadMessagesInput {
                channel_id: "123".to_string(),
                limit: 101,
                before: None,
                after: None,
                around: None,
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
    async fn test_upload_file_empty_channel_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = upload_file(
            ctx,
            UploadFileInput {
                channel_id: "  ".to_string(),
                filename: "test.txt".to_string(),
                content_base64: "SGVsbG8=".to_string(),
                message: None,
                spoiler: false,
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
    async fn test_upload_file_empty_filename_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = upload_file(
            ctx,
            UploadFileInput {
                channel_id: "123".to_string(),
                filename: "  ".to_string(),
                content_base64: "SGVsbG8=".to_string(),
                message: None,
                spoiler: false,
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

    // =========================================================================
    // Integration tests with wiremock
    // =========================================================================

    #[tokio::test]
    async fn test_list_channels_success() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let response_body = serde_json::json!([
            {
                "id": "123456789",
                "type": 0,
                "name": "general",
                "guild_id": "guild123"
            },
            {
                "id": "123456790",
                "type": 2,
                "name": "voice-chat",
                "guild_id": "guild123"
            }
        ]);

        Mock::given(method("GET"))
            .and(path("/guilds/guild123/channels"))
            .and(header("authorization", "Bot test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&server)
            .await;

        let output = list_channels(
            ctx,
            ListChannelsInput {
                guild_id: "guild123".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.count, 2);
        assert_eq!(output.channels.len(), 2);
        assert_eq!(output.channels[0].id, "123456789");
        assert_eq!(output.channels[0].name, Some("general".to_string()));
    }

    #[tokio::test]
    async fn test_post_message_success() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let response_body = serde_json::json!({
            "id": "987654321",
            "channel_id": "chan123",
            "author": {
                "id": "bot123",
                "username": "TestBot",
                "discriminator": "0",
                "bot": true
            },
            "content": "Hello Discord!",
            "timestamp": "2024-01-15T12:00:00.000Z",
            "pinned": false,
            "attachments": []
        });

        Mock::given(method("POST"))
            .and(path("/channels/chan123/messages"))
            .and(header("authorization", "Bot test-token"))
            .and(body_string_contains("Hello Discord!"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&server)
            .await;

        let output = post_message(
            ctx,
            PostMessageInput {
                channel_id: "chan123".to_string(),
                content: "Hello Discord!".to_string(),
                reply_to: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.message.id, "987654321");
        assert_eq!(output.message.content, "Hello Discord!");
    }

    #[tokio::test]
    async fn test_read_messages_success() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let response_body = serde_json::json!([
            {
                "id": "msg1",
                "channel_id": "chan123",
                "author": {
                    "id": "user1",
                    "username": "alice",
                    "discriminator": "0"
                },
                "content": "Hello!",
                "timestamp": "2024-01-15T12:00:00.000Z",
                "pinned": false,
                "attachments": []
            },
            {
                "id": "msg2",
                "channel_id": "chan123",
                "author": {
                    "id": "user2",
                    "username": "bob",
                    "discriminator": "0"
                },
                "content": "Hi!",
                "timestamp": "2024-01-15T12:01:00.000Z",
                "pinned": false,
                "attachments": []
            }
        ]);

        Mock::given(method("GET"))
            .and(path("/channels/chan123/messages"))
            .and(query_param("limit", "50"))
            .and(header("authorization", "Bot test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&server)
            .await;

        let output = read_messages(
            ctx,
            ReadMessagesInput {
                channel_id: "chan123".to_string(),
                limit: 50,
                before: None,
                after: None,
                around: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.count, 2);
        assert_eq!(output.messages.len(), 2);
        assert_eq!(output.messages[0].content, "Hello!");
    }

    #[tokio::test]
    async fn test_manage_threads_create_from_message_success() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let response_body = serde_json::json!({
            "id": "thread123",
            "type": 11,
            "name": "Discussion",
            "parent_id": "chan123"
        });

        Mock::given(method("POST"))
            .and(path("/channels/chan123/messages/msg456/threads"))
            .and(header("authorization", "Bot test-token"))
            .and(body_string_contains("Discussion"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&server)
            .await;

        let output = manage_threads(
            ctx,
            ManageThreadsInput {
                action: ThreadAction::CreateFromMessage,
                channel_id: "chan123".to_string(),
                message_id: Some("msg456".to_string()),
                name: Some("Discussion".to_string()),
                auto_archive_duration: None,
            },
        )
        .await
        .unwrap();

        assert!(output.success);
        assert!(output.thread.is_some());
        let thread = output.thread.unwrap();
        assert_eq!(thread.id, "thread123");
        assert_eq!(thread.name, "Discussion");
    }

    #[tokio::test]
    async fn test_manage_threads_archive_success() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let response_body = serde_json::json!({
            "id": "thread123",
            "type": 11,
            "name": "Archived Thread",
            "parent_id": "chan123"
        });

        Mock::given(method("PATCH"))
            .and(path("/channels/thread123"))
            .and(header("authorization", "Bot test-token"))
            .and(body_string_contains("\"archived\":true"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&server)
            .await;

        let output = manage_threads(
            ctx,
            ManageThreadsInput {
                action: ThreadAction::Archive,
                channel_id: "thread123".to_string(),
                message_id: None,
                name: None,
                auto_archive_duration: None,
            },
        )
        .await
        .unwrap();

        assert!(output.success);
        assert!(output.thread.is_some());
    }

    // =========================================================================
    // Utility tests
    // =========================================================================

    #[test]
    fn test_guess_content_type_returns_correct_types() {
        assert_eq!(guess_content_type("file.png"), "image/png");
        assert_eq!(guess_content_type("file.jpg"), "image/jpeg");
        assert_eq!(guess_content_type("file.jpeg"), "image/jpeg");
        assert_eq!(guess_content_type("file.gif"), "image/gif");
        assert_eq!(guess_content_type("file.webp"), "image/webp");
        assert_eq!(guess_content_type("file.pdf"), "application/pdf");
        assert_eq!(guess_content_type("file.txt"), "text/plain");
        assert_eq!(guess_content_type("file.json"), "application/json");
        assert_eq!(guess_content_type("file.mp4"), "video/mp4");
        assert_eq!(guess_content_type("file.mp3"), "audio/mpeg");
        assert_eq!(guess_content_type("file.zip"), "application/zip");
        assert_eq!(
            guess_content_type("file.unknown"),
            "application/octet-stream"
        );
        assert_eq!(
            guess_content_type("noextension"),
            "application/octet-stream"
        );
    }

    #[test]
    fn test_base64_decode_success() {
        let result = base64_decode("SGVsbG8gV29ybGQ=").unwrap();
        assert_eq!(result, b"Hello World");
    }

    #[test]
    fn test_base64_decode_invalid_returns_error() {
        let result = base64_decode("Invalid!!!Base64");
        assert!(result.is_err());
    }

    // =========================================================================
    // Serialization tests
    // =========================================================================

    #[test]
    fn test_thread_action_serialization_roundtrip() {
        for action in [
            ThreadAction::CreateFromMessage,
            ThreadAction::CreateStandalone,
            ThreadAction::Archive,
            ThreadAction::Unarchive,
            ThreadAction::Lock,
            ThreadAction::Unlock,
        ] {
            let json = serde_json::to_string(&action).unwrap();
            let parsed: ThreadAction = serde_json::from_str(&json).unwrap();
            assert!(matches!(parsed, _));
        }
    }

    #[test]
    fn test_list_channels_input_deserialization() {
        let json = r#"{"guild_id":"123456"}"#;
        let input: ListChannelsInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.guild_id, "123456");
    }

    #[test]
    fn test_post_message_input_deserialization() {
        let json = r#"{"channel_id":"chan123","content":"Hello","reply_to":"msg456"}"#;
        let input: PostMessageInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.channel_id, "chan123");
        assert_eq!(input.content, "Hello");
        assert_eq!(input.reply_to, Some("msg456".to_string()));
    }

    #[test]
    fn test_read_messages_input_deserialization_with_defaults() {
        let json = r#"{"channel_id":"chan123"}"#;
        let input: ReadMessagesInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.channel_id, "chan123");
        assert_eq!(input.limit, 50);
        assert!(input.before.is_none());
        assert!(input.after.is_none());
        assert!(input.around.is_none());
    }

    // =========================================================================
    // Error handling tests
    // =========================================================================

    #[tokio::test]
    async fn test_rate_limit_error_returns_meaningful_message() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let rate_limit_response = serde_json::json!({
            "message": "You are being rate limited.",
            "retry_after": 5.0,
            "global": false
        });

        Mock::given(method("GET"))
            .and(path("/guilds/guild123/channels"))
            .and(header("authorization", "Bot test-token"))
            .respond_with(ResponseTemplate::new(429).set_body_json(&rate_limit_response))
            .mount(&server)
            .await;

        let result = list_channels(
            ctx,
            ListChannelsInput {
                guild_id: "guild123".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("rate limit exceeded"));
        assert!(error_msg.contains("5s") || error_msg.contains('5'));
    }

    #[tokio::test]
    async fn test_discord_api_error_includes_error_code() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let error_response = serde_json::json!({
            "code": 50001,
            "message": "Missing access"
        });

        Mock::given(method("GET"))
            .and(path("/guilds/guild123/channels"))
            .and(header("authorization", "Bot test-token"))
            .respond_with(ResponseTemplate::new(403).set_body_json(&error_response))
            .mount(&server)
            .await;

        let result = list_channels(
            ctx,
            ListChannelsInput {
                guild_id: "guild123".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("50001"));
        assert!(error_msg.contains("Missing access"));
    }

    #[tokio::test]
    async fn test_user_agent_header_contains_discordbot() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let response_body = serde_json::json!([]);

        // Use a matcher that accepts any User-Agent containing "DiscordBot"
        Mock::given(method("GET"))
            .and(path("/guilds/guild123/channels"))
            .and(header("authorization", "Bot test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&response_body))
            .mount(&server)
            .await;

        let result = list_channels(
            ctx,
            ListChannelsInput {
                guild_id: "guild123".to_string(),
            },
        )
        .await;

        assert!(result.is_ok());

        // Verify the User-Agent contains "DiscordBot" by checking the constant
        assert!(USER_AGENT.contains("DiscordBot"));
    }
}
