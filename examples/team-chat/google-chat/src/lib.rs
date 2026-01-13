//! team-chat/google-chat integration for Operai Toolbox.

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

mod types;
use types::{Attachment, AttachmentDataRef, Message, Space, Thread};

define_user_credential! {
    GoogleChatCredential("google_chat") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_CHAT_ENDPOINT: &str = "https://chat.googleapis.com/v1";

#[init]
async fn setup() -> Result<()> {
    info!("Google Chat integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Google Chat integration shutting down");
}

// =============================================================================
// Tool: list_spaces
// =============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListSpacesInput {
    /// Maximum number of spaces to return (1-1000). Defaults to 100.
    #[serde(default)]
    pub page_size: Option<u32>,
    /// Token for the next page of results.
    #[serde(default)]
    pub page_token: Option<String>,
    /// Optional filter (e.g., "spaceType = SPACE" or "spaceType =
    /// `DIRECT_MESSAGE`").
    #[serde(default)]
    pub filter: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ListSpacesOutput {
    pub spaces: Vec<Space>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// # List Google Chat Spaces
///
/// Lists all Google Chat spaces (rooms and direct messages) accessible to the
/// authenticated user.
///
/// Use this tool when the user wants to:
/// - Browse available chat rooms and spaces
/// - Find a space ID to post messages to
/// - See all spaces the user has access to
///
/// The results can be filtered by space type (e.g., "SPACE" for rooms or
/// "`DIRECT_MESSAGE`" for 1:1 conversations) and paginated using `page_size`
/// and `page_token` parameters.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - chat
/// - google-chat
/// - collaboration
///
/// # Errors
///
/// Returns an error if:
/// - The user credentials are missing or invalid
/// - The `page_size` value is not between 1 and 1000
/// - The Google Chat API request fails due to network or authentication issues
/// - The API response cannot be parsed
#[tool]
pub async fn list_spaces(ctx: Context, input: ListSpacesInput) -> Result<ListSpacesOutput> {
    let page_size = input.page_size.unwrap_or(100);
    ensure!(
        (1..=1000).contains(&page_size),
        "page_size must be between 1 and 1000"
    );

    let client = ChatClient::from_ctx(&ctx)?;

    let mut query = vec![("pageSize", page_size.to_string())];
    if let Some(token) = input.page_token {
        query.push(("pageToken", token));
    }
    if let Some(filter) = input.filter {
        query.push(("filter", filter));
    }

    let response: ChatListResponse<Space> = client
        .get_json(client.url_with_path("/spaces")?, &query, &[])
        .await?;

    Ok(ListSpacesOutput {
        spaces: response.spaces.unwrap_or_default(),
        next_page_token: response.next_page_token,
    })
}

// =============================================================================
// Tool: post_message
// =============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PostMessageInput {
    /// The space to post the message to (e.g., "spaces/AAAA1234").
    pub space_name: String,
    /// The text content of the message.
    pub text: String,
    /// Optional thread to reply to. If not specified, creates a new thread.
    #[serde(default)]
    pub thread_name: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PostMessageOutput {
    pub message: Message,
}

/// # Send Google Chat Message
///
/// Sends a text message to a Google Chat space (room) or as a reply to an
/// existing thread.
///
/// Use this tool when the user wants to:
/// - Send a message to a Google Chat room
/// - Post a reply to an existing conversation thread
/// - Communicate with a team in a Google Chat space
///
/// If `thread_name` is provided, the message will be posted as a reply in that
/// thread. Otherwise, a new thread is created in the space.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - chat
/// - google-chat
/// - collaboration
///
/// # Errors
///
/// Returns an error if:
/// - The user credentials are missing or invalid
/// - `space_name` or `text` are empty or contain only whitespace
/// - The Google Chat API request fails due to network or authentication issues
/// - The API response cannot be parsed
#[tool]
pub async fn post_message(ctx: Context, input: PostMessageInput) -> Result<PostMessageOutput> {
    ensure!(
        !input.space_name.trim().is_empty(),
        "space_name must not be empty"
    );
    ensure!(!input.text.trim().is_empty(), "text must not be empty");

    let client = ChatClient::from_ctx(&ctx)?;

    let request = ChatMessageRequest {
        text: input.text,
        thread: input.thread_name.map(|name| Thread { name }),
    };

    let message: Message = client
        .post_json(
            client.url_with_path(&format!("/{}/messages", input.space_name))?,
            &request,
            &[],
        )
        .await?;

    Ok(PostMessageOutput { message })
}

// =============================================================================
// Tool: read_thread
// =============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadThreadInput {
    /// The space containing the messages (e.g., "spaces/AAAA1234").
    pub space_name: String,
    /// Optional filter to list messages in a specific thread.
    #[serde(default)]
    pub thread_name: Option<String>,
    /// Maximum number of messages to return (1-1000). Defaults to 50.
    #[serde(default)]
    pub page_size: Option<u32>,
    /// Token for the next page of results.
    #[serde(default)]
    pub page_token: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReadThreadOutput {
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// # Google Chat Read Thread
///
/// Retrieves messages from a Google Chat space or from a specific thread within
/// a space.
///
/// Use this tool when the user wants to:
/// - Read recent messages in a Google Chat room
/// - View a specific conversation thread
/// - Get message history from a space
///
/// If `thread_name` is provided, only messages from that specific thread are
/// returned. Otherwise, all messages from the space are returned. Messages are
/// paginated and can be retrieved in batches using `page_size` and `page_token`
/// parameters.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - chat
/// - google-chat
/// - collaboration
///
/// # Errors
///
/// Returns an error if:
/// - The user credentials are missing or invalid
/// - `space_name` is empty or contains only whitespace
/// - The `page_size` value is not between 1 and 1000
/// - The Google Chat API request fails due to network or authentication issues
/// - The API response cannot be parsed
#[tool]
pub async fn read_thread(ctx: Context, input: ReadThreadInput) -> Result<ReadThreadOutput> {
    ensure!(
        !input.space_name.trim().is_empty(),
        "space_name must not be empty"
    );

    let page_size = input.page_size.unwrap_or(50);
    ensure!(
        (1..=1000).contains(&page_size),
        "page_size must be between 1 and 1000"
    );

    let client = ChatClient::from_ctx(&ctx)?;

    let mut query = vec![("pageSize", page_size.to_string())];
    if let Some(token) = input.page_token {
        query.push(("pageToken", token));
    }
    if let Some(thread_name) = input.thread_name {
        query.push(("filter", format!("thread.name=\"{thread_name}\"")));
    }

    let response: ChatListResponse<Message> = client
        .get_json(
            client.url_with_path(&format!("/{}/messages", input.space_name))?,
            &query,
            &[],
        )
        .await?;

    Ok(ReadThreadOutput {
        messages: response.messages.unwrap_or_default(),
        next_page_token: response.next_page_token,
    })
}

// =============================================================================
// Tool: mention_user
// =============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MentionUserInput {
    /// The space to post the message to (e.g., "spaces/AAAA1234").
    pub space_name: String,
    /// The user to mention (user resource name, e.g., "users/123456").
    pub user_name: String,
    /// The message text (mention will be added automatically).
    pub text: String,
    /// Optional thread to reply to.
    #[serde(default)]
    pub thread_name: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MentionUserOutput {
    pub message: Message,
}

/// # Mention Google Chat User
///
/// Sends a message to a Google Chat space that explicitly mentions (notifies) a
/// specific user.
///
/// Use this tool when the user wants to:
/// - Send a message that notifies a specific person in a Google Chat room
/// - Draw someone's attention to a message in a space
/// - Directly address a team member in a group conversation
///
/// The `user_name` parameter must be a valid user resource name (e.g.,
/// "users/123456"). The mentioned user will receive a notification and the
/// message will include a proper @-mention annotation in the Chat interface.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - chat
/// - google-chat
/// - collaboration
///
/// # Errors
///
/// Returns an error if:
/// - The user credentials are missing or invalid
/// - `space_name`, `user_name`, or `text` are empty or contain only whitespace
/// - The Google Chat API request fails due to network or authentication issues
/// - The API response cannot be parsed
#[tool]
pub async fn mention_user(ctx: Context, input: MentionUserInput) -> Result<MentionUserOutput> {
    ensure!(
        !input.space_name.trim().is_empty(),
        "space_name must not be empty"
    );
    ensure!(
        !input.user_name.trim().is_empty(),
        "user_name must not be empty"
    );
    ensure!(!input.text.trim().is_empty(), "text must not be empty");

    let client = ChatClient::from_ctx(&ctx)?;

    // Format message with mention annotation
    // The mention text format is: <users/123456>
    let mention_text = format!("<{}>", input.user_name);
    let formatted_text = format!("{} {}", mention_text, input.text);

    let mention_length = i32::try_from(mention_text.len()).unwrap_or(i32::MAX);

    let request = ChatMessageWithAnnotationsRequest {
        text: formatted_text.clone(),
        thread: input.thread_name.map(|name| Thread { name }),
        annotations: vec![ChatAnnotation {
            annotation_type: "USER_MENTION".to_string(),
            start_index: Some(0),
            length: Some(mention_length),
            user_mention: Some(ChatUserMention {
                user: ChatUserReference {
                    name: input.user_name,
                },
                mention_type: Some("ADD".to_string()),
            }),
        }],
    };

    let message: Message = client
        .post_json(
            client.url_with_path(&format!("/{}/messages", input.space_name))?,
            &request,
            &[],
        )
        .await?;

    Ok(MentionUserOutput { message })
}

// =============================================================================
// Tool: upload_file
// =============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadFileInput {
    /// The space to upload the file to (e.g., "spaces/AAAA1234").
    pub space_name: String,
    /// The filename for the uploaded file.
    pub filename: String,
    /// Base64-encoded file content.
    pub content_base64: String,
    /// MIME type of the file (e.g., "image/png", "application/pdf").
    pub content_type: String,
    /// Optional message text to accompany the file.
    #[serde(default)]
    pub message_text: Option<String>,
    /// Optional thread to upload to.
    #[serde(default)]
    pub thread_name: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UploadFileOutput {
    pub message: Message,
}

/// # Upload Google Chat File
///
/// Uploads a file as an attachment to a Google Chat space and posts a message
/// with the file.
///
/// Use this tool when the user wants to:
/// - Share an image, document, or other file in a Google Chat room
/// - Upload a file that team members can access
/// - Send a file with an accompanying message in a space
///
/// The file content must be provided as a base64-encoded string. The tool
/// handles the two-step upload process: first uploading the file data, then
/// creating a message with the attachment reference. A default message text is
/// generated if `message_text` is not provided.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - chat
/// - google-chat
/// - collaboration
/// - upload
///
/// # Errors
///
/// Returns an error if:
/// - The user credentials are missing or invalid
/// - `space_name`, `filename`, or `content_base64` are empty or contain only
///   whitespace
/// - The `content_base64` string is not valid base64 encoding
/// - The Google Chat API request fails due to network or authentication issues
/// - The API response cannot be parsed
#[tool]
pub async fn upload_file(ctx: Context, input: UploadFileInput) -> Result<UploadFileOutput> {
    ensure!(
        !input.space_name.trim().is_empty(),
        "space_name must not be empty"
    );
    ensure!(
        !input.filename.trim().is_empty(),
        "filename must not be empty"
    );
    ensure!(
        !input.content_base64.trim().is_empty(),
        "content_base64 must not be empty"
    );

    let client = ChatClient::from_ctx(&ctx)?;

    // Step 1: Upload the file to get an attachment data reference
    // Decode base64 content
    let file_bytes = decode_base64(&input.content_base64)?;

    let upload_response: ChatUploadResponse = client
        .upload_attachment(&input.space_name, &input.filename, file_bytes)
        .await?;

    // Step 2: Create message with attachment
    let text = input
        .message_text
        .unwrap_or_else(|| format!("Uploaded file: {}", input.filename));

    let resource_name = upload_response.attachment_data_ref.resource_name.clone();

    let request = ChatMessageWithAttachmentRequest {
        text,
        thread: input.thread_name.map(|name| Thread { name }),
        attachment: vec![Attachment {
            name: resource_name
                .clone()
                .unwrap_or_else(|| input.filename.clone()),
            content_type: Some(input.content_type),
            data_ref: Some(AttachmentDataRef {
                resource_name: resource_name.clone(),
                attachment_upload_token: upload_response
                    .attachment_data_ref
                    .attachment_upload_token
                    .clone(),
            }),
        }],
    };

    let message: Message = client
        .post_json(
            client.url_with_path(&format!("/{}/messages", input.space_name))?,
            &request,
            &[],
        )
        .await?;

    Ok(UploadFileOutput { message })
}

// =============================================================================
// Internal API types and client
// =============================================================================

#[derive(Debug, Deserialize)]
struct ChatListResponse<T> {
    #[serde(default)]
    spaces: Option<Vec<T>>,
    #[serde(default)]
    messages: Option<Vec<T>>,
    #[serde(default, rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatMessageRequest {
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread: Option<Thread>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatMessageWithAnnotationsRequest {
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread: Option<Thread>,
    annotations: Vec<ChatAnnotation>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatAnnotation {
    #[serde(rename = "type")]
    annotation_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    length: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_mention: Option<ChatUserMention>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatUserMention {
    user: ChatUserReference,
    #[serde(skip_serializing_if = "Option::is_none")]
    mention_type: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChatUserReference {
    name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatMessageWithAttachmentRequest {
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread: Option<Thread>,
    attachment: Vec<Attachment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatUploadResponse {
    attachment_data_ref: AttachmentDataRef,
}

#[derive(Debug, Clone)]
struct ChatClient {
    http: reqwest::Client,
    base_url: String,
    upload_base_url: String,
    access_token: String,
}

impl ChatClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = GoogleChatCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_CHAT_ENDPOINT))?;

        // For uploads, we need to use the /upload/v1 endpoint
        let upload_base_url = base_url.replace("/v1", "/upload/v1");

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            upload_base_url,
            access_token: cred.access_token,
        })
    }

    fn url_with_path(&self, path: &str) -> Result<reqwest::Url> {
        let url_str = format!("{}{}", self.base_url, path);
        Ok(reqwest::Url::parse(&url_str)?)
    }

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

    async fn upload_attachment(
        &self,
        parent: &str,
        filename: &str,
        data: Vec<u8>,
    ) -> Result<ChatUploadResponse> {
        // Use the upload endpoint: https://chat.googleapis.com/upload/v1/{parent}/attachments:upload
        // The Google Chat API uses a special media upload endpoint where:
        // - The "filename" is sent as a query parameter
        // - The file data is sent as the request body with Content-Type:
        //   application/octet-stream
        let url_str = format!(
            "{}/{}/attachments:upload?filename={}",
            self.upload_base_url,
            parent,
            urlencoding::encode(filename)
        );
        let url = reqwest::Url::parse(&url_str)?;

        let response = self
            .http
            .post(url)
            .bearer_auth(&self.access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(data)
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.json::<ChatUploadResponse>().await?)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(operai::anyhow::anyhow!(
                "Google Chat API upload request failed ({status}): {body}"
            ))
        }
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
                "Google Chat API request failed ({status}): {body}"
            ))
        }
    }
}

fn normalize_base_url(endpoint: &str) -> Result<String> {
    let trimmed = endpoint.trim();
    ensure!(!trimmed.is_empty(), "endpoint must not be empty");
    Ok(trimmed.trim_end_matches('/').to_string())
}

fn decode_base64(input: &str) -> Result<Vec<u8>> {
    use operai::anyhow::Context as _;

    let trimmed = input.trim();
    let decoded = base64_simd::STANDARD
        .decode_to_vec(trimmed)
        .context("failed to decode base64 content")?;
    Ok(decoded)
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
        let mut google_chat_values = HashMap::new();
        google_chat_values.insert("access_token".to_string(), "test-token".to_string());
        google_chat_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("google_chat", google_chat_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        format!("{}/v1", server.uri())
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://chat.googleapis.com/v1/").unwrap();
        assert_eq!(result, "https://chat.googleapis.com/v1");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://chat.googleapis.com/v1  ").unwrap();
        assert_eq!(result, "https://chat.googleapis.com/v1");
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
    async fn test_list_spaces_page_size_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_spaces(
            ctx,
            ListSpacesInput {
                page_size: Some(0),
                page_token: None,
                filter: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("page_size must be between 1 and 1000")
        );
    }

    #[tokio::test]
    async fn test_list_spaces_page_size_exceeds_max_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = list_spaces(
            ctx,
            ListSpacesInput {
                page_size: Some(1001),
                page_token: None,
                filter: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("page_size must be between 1 and 1000")
        );
    }

    #[tokio::test]
    async fn test_post_message_empty_space_name_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = post_message(
            ctx,
            PostMessageInput {
                space_name: "  ".to_string(),
                text: "Hello".to_string(),
                thread_name: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("space_name must not be empty")
        );
    }

    #[tokio::test]
    async fn test_post_message_empty_text_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = post_message(
            ctx,
            PostMessageInput {
                space_name: "spaces/AAAA1234".to_string(),
                text: "  ".to_string(),
                thread_name: None,
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

    // --- Integration tests ---

    #[tokio::test]
    async fn test_list_spaces_success_returns_spaces() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"{
          "spaces": [
            {
              "name": "spaces/AAAA1234",
              "displayName": "Engineering Team",
              "spaceType": "SPACE"
            },
            {
              "name": "spaces/BBBB5678",
              "displayName": null,
              "spaceType": "DIRECT_MESSAGE"
            }
          ]
        }"#;

        Mock::given(method("GET"))
            .and(path("/v1/spaces"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("pageSize", "100"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = list_spaces(
            ctx,
            ListSpacesInput {
                page_size: None,
                page_token: None,
                filter: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.spaces.len(), 2);
        assert_eq!(output.spaces[0].name, "spaces/AAAA1234");
        assert_eq!(
            output.spaces[0].display_name.as_deref(),
            Some("Engineering Team")
        );
    }

    #[tokio::test]
    async fn test_post_message_success_creates_message() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"{
          "name": "spaces/AAAA1234/messages/MSG001",
          "text": "Hello world",
          "sender": {
            "name": "users/123456",
            "displayName": "Test User"
          },
          "createTime": "2024-01-15T10:30:00Z"
        }"#;

        Mock::given(method("POST"))
            .and(path("/v1/spaces/AAAA1234/messages"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("\"text\":\"Hello world\""))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = post_message(
            ctx,
            PostMessageInput {
                space_name: "spaces/AAAA1234".to_string(),
                text: "Hello world".to_string(),
                thread_name: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.message.name, "spaces/AAAA1234/messages/MSG001");
        assert_eq!(output.message.text.as_deref(), Some("Hello world"));
    }

    #[tokio::test]
    async fn test_read_thread_success_returns_messages() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"{
          "messages": [
            {
              "name": "spaces/AAAA1234/messages/MSG001",
              "text": "First message",
              "sender": {
                "name": "users/123456",
                "displayName": "Alice"
              },
              "createTime": "2024-01-15T09:00:00Z",
              "thread": {
                "name": "spaces/AAAA1234/threads/THREAD1"
              }
            },
            {
              "name": "spaces/AAAA1234/messages/MSG002",
              "text": "Second message",
              "sender": {
                "name": "users/789012",
                "displayName": "Bob"
              },
              "createTime": "2024-01-15T09:15:00Z",
              "thread": {
                "name": "spaces/AAAA1234/threads/THREAD1"
              }
            }
          ]
        }"#;

        Mock::given(method("GET"))
            .and(path("/v1/spaces/AAAA1234/messages"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("pageSize", "50"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = read_thread(
            ctx,
            ReadThreadInput {
                space_name: "spaces/AAAA1234".to_string(),
                thread_name: None,
                page_size: None,
                page_token: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.messages.len(), 2);
        assert_eq!(output.messages[0].text.as_deref(), Some("First message"));
        assert_eq!(output.messages[1].text.as_deref(), Some("Second message"));
    }

    #[tokio::test]
    async fn test_mention_user_success_creates_message_with_mention() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"{
          "name": "spaces/AAAA1234/messages/MSG003",
          "text": "<users/123456> Please review",
          "sender": {
            "name": "users/789012",
            "displayName": "Bot"
          },
          "createTime": "2024-01-15T10:45:00Z"
        }"#;

        Mock::given(method("POST"))
            .and(path("/v1/spaces/AAAA1234/messages"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("<users/123456> Please review"))
            .and(body_string_contains("annotations"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = mention_user(
            ctx,
            MentionUserInput {
                space_name: "spaces/AAAA1234".to_string(),
                user_name: "users/123456".to_string(),
                text: "Please review".to_string(),
                thread_name: None,
            },
        )
        .await
        .unwrap();

        assert!(
            output
                .message
                .text
                .as_ref()
                .is_some_and(|t| t.contains("users/123456"))
        );
    }

    #[tokio::test]
    async fn test_api_error_returns_error() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/v1/spaces"))
            .respond_with(ResponseTemplate::new(401).set_body_raw(
                r#"{ "error": { "code": 401, "message": "Unauthorized" } }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = list_spaces(
            ctx,
            ListSpacesInput {
                page_size: None,
                page_token: None,
                filter: None,
            },
        )
        .await;

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("401"));
    }

    // --- upload_file tests ---

    #[tokio::test]
    async fn test_upload_file_empty_space_name_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = upload_file(
            ctx,
            UploadFileInput {
                space_name: "  ".to_string(),
                filename: "test.txt".to_string(),
                content_base64: base64_simd::STANDARD.encode_to_string(b"hello"),
                content_type: "text/plain".to_string(),
                message_text: None,
                thread_name: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("space_name must not be empty")
        );
    }

    #[tokio::test]
    async fn test_upload_file_empty_filename_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = upload_file(
            ctx,
            UploadFileInput {
                space_name: "spaces/AAAA1234".to_string(),
                filename: "  ".to_string(),
                content_base64: base64_simd::STANDARD.encode_to_string(b"hello"),
                content_type: "text/plain".to_string(),
                message_text: None,
                thread_name: None,
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
    async fn test_upload_file_invalid_base64_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = upload_file(
            ctx,
            UploadFileInput {
                space_name: "spaces/AAAA1234".to_string(),
                filename: "test.txt".to_string(),
                content_base64: "not-valid-base64!!!".to_string(),
                content_type: "text/plain".to_string(),
                message_text: None,
                thread_name: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("base64"));
    }

    #[tokio::test]
    async fn test_upload_file_success_uploads_and_creates_message() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        // Mock the upload endpoint response
        let upload_response_body = r#"{
          "attachmentDataRef": {
            "resourceName": "spaces/AAAA1234/attachments/ATT123",
            "attachmentUploadToken": "upload_token_abc123"
          }
        }"#;

        Mock::given(method("POST"))
            .and(path("/upload/v1/spaces/AAAA1234/attachments:upload"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(upload_response_body, "application/json"),
            )
            .mount(&server)
            .await;

        // Mock the message creation endpoint response
        let message_response_body = r#"{
          "name": "spaces/AAAA1234/messages/MSG001",
          "text": "Uploaded file: test.txt",
          "attachment": [
            {
              "name": "spaces/AAAA1234/attachments/ATT123"
            }
          ]
        }"#;

        Mock::given(method("POST"))
            .and(path("/v1/spaces/AAAA1234/messages"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(message_response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = upload_file(
            ctx,
            UploadFileInput {
                space_name: "spaces/AAAA1234".to_string(),
                filename: "test.txt".to_string(),
                content_base64: base64_simd::STANDARD.encode_to_string(b"hello world"),
                content_type: "text/plain".to_string(),
                message_text: None,
                thread_name: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.message.name, "spaces/AAAA1234/messages/MSG001");
    }

    // --- mention_user annotation length tests ---

    #[tokio::test]
    async fn test_mention_user_annotation_length_is_correct() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"{
          "name": "spaces/AAAA1234/messages/MSG003",
          "text": "<users/123456> Please review",
          "sender": {
            "name": "users/789012",
            "displayName": "Bot"
          },
          "createTime": "2024-01-15T10:45:00Z"
        }"#;

        Mock::given(method("POST"))
            .and(path("/v1/spaces/AAAA1234/messages"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = mention_user(
            ctx,
            MentionUserInput {
                space_name: "spaces/AAAA1234".to_string(),
                user_name: "users/123456".to_string(),
                text: "Please review".to_string(),
                thread_name: None,
            },
        )
        .await
        .unwrap();

        // Verify the message contains the formatted mention
        assert_eq!(
            output.message.text.as_deref(),
            Some("<users/123456> Please review")
        );
    }
}
