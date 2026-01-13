//! email-inbox/proton-mail integration for Operai Toolbox.
//!
//! This is a reference implementation for Proton Mail integration.
//! Note: Proton Mail does not provide an official public REST API.
//! This implementation follows patterns from the unofficial community APIs
//! and would need to be adapted based on actual Proton Mail API access.

mod types;

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
use types::{
    Message, MessageSummary, ProtonListResponse, ProtonResponse, Recipient, SendMessage,
    SendMessageRequest,
};

define_user_credential! {
    ProtonCredential("proton") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_PROTON_ENDPOINT: &str = "https://mail.proton.me/api";

#[init]
async fn setup() -> Result<()> {
    info!("Proton Mail integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Proton Mail integration shutting down");
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchMailInput {
    /// Search query string (searches in subject, from, to).
    pub query: String,
    /// Maximum number of results (1-100). Defaults to 50.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchMailOutput {
    pub messages: Vec<MessageSummary>,
    pub total: i32,
}

/// # Search Proton Mail Messages
///
/// Searches for emails in the user's Proton Mail account using a query string.
///
/// Use this tool when a user wants to find emails matching specific criteria.
/// The search query searches across email subject lines, sender addresses, and
/// recipient addresses. This is the primary tool for discovering and locating
/// emails within the Proton Mail inbox.
///
/// ## Input Behavior
/// - The `query` parameter accepts a search string that matches against
///   subject, from, and to fields
/// - The `limit` parameter controls the maximum number of results returned
///   (1-100, defaults to 50)
/// - Returns both the message list and total count of matching messages
///
/// ## Output
/// Returns a list of message summaries including: message ID, subject, sender
/// information, timestamp, size, unread status, starred status, and associated
/// label IDs. Use the `get_message` tool to retrieve the full message body and
/// details.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - email
/// - proton
/// - proton-mail
///
/// # Errors
///
/// Returns an error if:
/// - The query is empty or contains only whitespace
/// - The limit is not between 1 and 100
/// - The Proton credential is missing or the access token is empty
/// - The endpoint URL is invalid
/// - The HTTP request to the Proton API fails
/// - The API response cannot be parsed
#[tool]
pub async fn search_mail(ctx: Context, input: SearchMailInput) -> Result<SearchMailOutput> {
    ensure!(!input.query.trim().is_empty(), "query must not be empty");
    let limit = input.limit.unwrap_or(50);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let client = ProtonClient::from_ctx(&ctx)?;

    // Proton API uses query parameters for search
    let query_params = [
        ("Keyword", input.query.as_str()),
        ("Limit", &limit.to_string()),
    ];

    let response: ProtonListResponse<MessageSummary> = client
        .get_json(client.url_with_path("/messages")?, &query_params)
        .await?;

    Ok(SearchMailOutput {
        messages: response.messages,
        total: response.total.unwrap_or(0),
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetMessageInput {
    /// Proton Mail message ID.
    pub message_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetMessageOutput {
    pub message: Message,
}

/// # Get Proton Mail Message
///
/// Retrieves the full details of a specific Proton Mail email message by its
/// ID.
///
/// Use this tool after searching for messages with `search_mail` when the user
/// wants to read the complete email content including the body. This tool
/// returns the full message with all recipient lists (To, CC, BCC), complete
/// sender information, subject, body content, timestamp, and MIME type.
///
/// ## Input Behavior
/// - Requires a valid `message_id` which is typically obtained from
///   `search_mail` results
/// - The message ID must exist in the user's Proton Mail account
///
/// ## Output
/// Returns the complete Message object including:
/// - Message ID and subject
/// - Full sender information (address and name)
/// - Complete recipient lists: `ToList`, `CCList`, B`CCList`
/// - Message body content
/// - MIME type (text/plain or text/html)
/// - Timestamp and metadata
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - email
/// - proton
/// - proton-mail
///
/// # Errors
///
/// Returns an error if:
/// - The `message_id` is empty or contains only whitespace
/// - The Proton credential is missing or the access token is empty
/// - The endpoint URL is invalid
/// - The HTTP request to the Proton API fails
/// - The API response cannot be parsed
#[tool]
pub async fn get_message(ctx: Context, input: GetMessageInput) -> Result<GetMessageOutput> {
    ensure!(
        !input.message_id.trim().is_empty(),
        "message_id must not be empty"
    );

    let client = ProtonClient::from_ctx(&ctx)?;

    let response: ProtonResponse<Message> = client
        .get_json(
            client.url_with_path(&format!("/messages/{}", input.message_id))?,
            &[],
        )
        .await?;

    Ok(GetMessageOutput {
        message: response.message,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SendEmailInput {
    /// One or more "To" recipients (email addresses).
    pub to: Vec<String>,
    /// Optional CC recipients (email addresses).
    #[serde(default)]
    pub cc: Vec<String>,
    /// Optional BCC recipients (email addresses).
    #[serde(default)]
    pub bcc: Vec<String>,
    /// Email subject.
    pub subject: String,
    /// Email body content.
    pub body: String,
    /// MIME type ("text/plain" or "text/html"). Defaults to "text/plain".
    #[serde(default)]
    pub mime_type: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SendEmailOutput {
    pub sent: bool,
    pub message_id: String,
}

/// # Send Proton Mail Email
///
/// Sends a new email message from the authenticated user's Proton Mail account.
///
/// Use this tool when the user wants to compose and send a new email. This is
/// the primary tool for outbound email communication. The tool supports
/// multiple recipients in the To, CC, and BCC fields, and allows sending both
/// plain text and HTML emails.
///
/// ## Input Behavior
/// - The `to` field must contain at least one recipient email address
/// - The `cc` and `bcc` fields are optional for additional recipients
/// - All recipient email addresses must be non-empty and valid
/// - The `subject` and `body` fields are required and must not be empty
/// - The `mime_type` parameter is optional and defaults to "text/plain" if not
///   specified
/// - Supported MIME types: "text/plain" or "text/html"
///
/// ## Output
/// Returns a confirmation that the email was sent along with the new message ID
/// assigned by Proton Mail. The message ID can be used to reference this email
/// in future operations.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - email
/// - proton
/// - proton-mail
///
/// # Errors
///
/// Returns an error if:
/// - The `to` list is empty
/// - Any recipient address (to/cc/bcc) is empty or contains only whitespace
/// - The subject is empty or contains only whitespace
/// - The body is empty or contains only whitespace
/// - The Proton credential is missing or the access token is empty
/// - The endpoint URL is invalid
/// - The HTTP request to the Proton API fails
/// - The API response cannot be parsed
#[tool]
pub async fn send_email(ctx: Context, input: SendEmailInput) -> Result<SendEmailOutput> {
    ensure!(
        !input.to.is_empty(),
        "to must contain at least one recipient"
    );
    ensure!(
        input.to.iter().all(|v| !v.trim().is_empty()),
        "to recipients must not be empty"
    );
    ensure!(
        input.cc.iter().all(|v| !v.trim().is_empty()),
        "cc recipients must not be empty"
    );
    ensure!(
        input.bcc.iter().all(|v| !v.trim().is_empty()),
        "bcc recipients must not be empty"
    );
    ensure!(
        !input.subject.trim().is_empty(),
        "subject must not be empty"
    );
    ensure!(!input.body.trim().is_empty(), "body must not be empty");

    let mime_type = input.mime_type.unwrap_or_else(|| "text/plain".to_string());

    let client = ProtonClient::from_ctx(&ctx)?;

    let request = SendMessageRequest {
        message: SendMessage {
            subject: input.subject,
            body: input.body,
            to_list: input.to.into_iter().map(proton_recipient).collect(),
            cc_list: input.cc.into_iter().map(proton_recipient).collect(),
            bcc_list: input.bcc.into_iter().map(proton_recipient).collect(),
            mime_type,
        },
    };

    let response: ProtonResponse<Message> = client
        .post_json(client.url_with_path("/messages")?, &request)
        .await?;

    Ok(SendEmailOutput {
        sent: true,
        message_id: response.message.id,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplyInput {
    /// Proton Mail message ID to reply to.
    pub message_id: String,
    /// Reply body content.
    pub body: String,
    /// When true, reply-all instead of reply.
    #[serde(default)]
    pub reply_all: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReplyOutput {
    pub sent: bool,
    pub message_id: String,
}

/// # Reply to Proton Mail Message
///
/// Sends a reply to an existing Proton Mail message, with support for both
/// reply and reply-all functionality.
///
/// Use this tool when the user wants to respond to an existing email thread.
/// The tool automatically fetches the original message to determine the
/// appropriate recipients and subject. For a standard reply, it responds only
/// to the original sender. For a reply-all, it includes all original recipients
/// (To and CC lists).
///
/// ## Input Behavior
/// - Requires the `message_id` of the email being replied to
/// - The `body` parameter contains the reply text content
/// - The `reply_all` flag controls recipient behavior:
///   - When false (default): replies only to the original sender
///   - When true: replies to sender + all original To/CC recipients
/// - The original message is automatically fetched to extract recipient
///   information
/// - The subject is preserved from the original message (typically with "Re:"
///   prefix added by the mail client)
///
/// ## Output
/// Returns a confirmation that the reply was sent along with the new message ID
/// assigned by Proton Mail for the reply message.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - email
/// - proton
/// - proton-mail
///
/// # Errors
///
/// Returns an error if:
/// - The `message_id` is empty or contains only whitespace
/// - The body is empty or contains only whitespace
/// - The Proton credential is missing or the access token is empty
/// - The endpoint URL is invalid
/// - The HTTP request to fetch the original message fails
/// - The HTTP request to send the reply fails
/// - The API response cannot be parsed
#[tool]
pub async fn reply(ctx: Context, input: ReplyInput) -> Result<ReplyOutput> {
    ensure!(
        !input.message_id.trim().is_empty(),
        "message_id must not be empty"
    );
    ensure!(!input.body.trim().is_empty(), "body must not be empty");

    let client = ProtonClient::from_ctx(&ctx)?;

    // First, get the original message to extract recipients
    let original: ProtonResponse<Message> = client
        .get_json(
            client.url_with_path(&format!("/messages/{}", input.message_id))?,
            &[],
        )
        .await?;

    let mut to_list = vec![];
    let mut cc_list = vec![];

    if input.reply_all {
        // Reply-all: include sender + all original To/CC recipients
        if let Some(sender) = original.message.sender {
            to_list.push(Recipient {
                address: sender.address,
                name: sender.name,
            });
        }
        to_list.extend(original.message.to_list);
        cc_list.extend(original.message.cc_list);
    } else {
        // Reply: only to sender
        if let Some(sender) = original.message.sender {
            to_list.push(Recipient {
                address: sender.address,
                name: sender.name,
            });
        }
    }

    let request = SendMessageRequest {
        message: SendMessage {
            subject: original.message.subject.clone().unwrap_or_default(),
            body: input.body,
            to_list,
            cc_list,
            bcc_list: vec![],
            mime_type: "text/plain".to_string(),
        },
    };

    let response: ProtonResponse<Message> = client
        .post_json(client.url_with_path("/messages")?, &request)
        .await?;

    Ok(ReplyOutput {
        sent: true,
        message_id: response.message.id,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MoveInput {
    /// Proton Mail message ID to move.
    pub message_id: String,
    /// Destination folder/label ID (e.g., "0" for inbox, "6" for trash, "1" for
    /// drafts).
    pub folder_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MoveOutput {
    pub moved: bool,
    pub message_id: String,
    pub folder_id: String,
}

/// # Move Proton Mail Message to Folder
///
/// Moves a Proton Mail message to a different folder or label in the user's
/// mailbox.
///
/// Use this tool when the user wants to organize their emails by moving
/// messages between folders. This is commonly used for archiving emails to
/// specific folders, moving messages to trash, or organizing emails into custom
/// folders. The tool applies a label/folder to the message, effectively
/// changing its location.
///
/// ## Input Behavior
/// - Requires the `message_id` of the email to be moved
/// - Requires the ``folder_id`` representing the destination folder or label
/// - Common folder IDs in Proton Mail (may vary):
///   - "0" - Inbox
///   - "1" - Drafts
///   - "3" - Sent
///   - "4" - Starred
///   - "6" - Trash
///   - "7" - Spam
///   - Custom folder/label IDs are user-defined
///
/// ## Output
/// Returns a confirmation that the message was moved successfully, including
/// the message ID and the `folder_id` it was moved to.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - email
/// - proton
/// - proton-mail
///
/// # Errors
///
/// Returns an error if:
/// - The `message_id` is empty or contains only whitespace
/// - The ``folder_id`` is empty or contains only whitespace
/// - The Proton credential is missing or the access token is empty
/// - The endpoint URL is invalid
/// - The HTTP request to the Proton API fails
#[tool]
pub async fn move_message(ctx: Context, input: MoveInput) -> Result<MoveOutput> {
    ensure!(
        !input.message_id.trim().is_empty(),
        "message_id must not be empty"
    );
    ensure!(
        !input.folder_id.trim().is_empty(),
        "`folder_id` must not be empty"
    );

    let client = ProtonClient::from_ctx(&ctx)?;

    let request = types::MoveRequest {
        label_id: input.folder_id.clone(),
    };

    client
        .put_empty(
            client.url_with_path(&format!("/messages/{}", input.message_id))?,
            &request,
        )
        .await?;

    Ok(MoveOutput {
        moved: true,
        message_id: input.message_id,
        folder_id: input.folder_id,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LabelInput {
    /// Proton Mail message ID to label.
    pub message_id: String,
    /// Label ID to apply.
    pub label_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LabelOutput {
    pub labeled: bool,
    pub message_id: String,
    pub label_id: String,
}

/// # Apply Proton Mail Label to Message
///
/// Applies a label to a Proton Mail message for categorization and
/// organization.
///
/// Use this tool when the user wants to tag or categorize an email with a
/// specific label. Labels in Proton Mail are a flexible way to organize emails
/// without moving them to different folders. A single message can have multiple
/// labels applied to it, allowing for cross-cutting organization (e.g., "Work",
/// "Important", "Project X"). This tool adds the specified label to the
/// message.
///
/// ## Input Behavior
/// - Requires the `message_id` of the email to be labeled
/// - Requires the ``label_id`` of the label to apply
/// - Labels can be user-defined or system labels
/// - The label must exist in the user's Proton Mail account
/// - Multiple labels can be applied to the same message by calling this tool
///   multiple times
///
/// ## Output
/// Returns a confirmation that the label was successfully applied, including
/// the message ID and the `label_id` that was added.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - email
/// - proton
/// - proton-mail
///
/// # Errors
///
/// Returns an error if:
/// - The `message_id` is empty or contains only whitespace
/// - The ``label_id`` is empty or contains only whitespace
/// - The Proton credential is missing or the access token is empty
/// - The endpoint URL is invalid
/// - The HTTP request to the Proton API fails
#[tool]
pub async fn label_message(ctx: Context, input: LabelInput) -> Result<LabelOutput> {
    ensure!(
        !input.message_id.trim().is_empty(),
        "message_id must not be empty"
    );
    ensure!(
        !input.label_id.trim().is_empty(),
        "`label_id` must not be empty"
    );

    let client = ProtonClient::from_ctx(&ctx)?;

    let request = types::LabelRequest {
        label_id: input.label_id.clone(),
    };

    client
        .put_empty(
            client.url_with_path(&format!("/messages/{}/label", input.message_id))?,
            &request,
        )
        .await?;

    Ok(LabelOutput {
        labeled: true,
        message_id: input.message_id,
        label_id: input.label_id,
    })
}

fn proton_recipient(address: String) -> Recipient {
    Recipient {
        address,
        name: None,
    }
}

#[derive(Debug, Clone)]
struct ProtonClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl ProtonClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = ProtonCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_PROTON_ENDPOINT))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
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
        query: &[(&str, &str)],
    ) -> Result<T> {
        let request = self.http.get(url).query(query);
        let response = self.send_request(request).await?;
        Ok(response.json::<T>().await?)
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
    ) -> Result<TRes> {
        let request = self.http.post(url).json(body);
        let response = self.send_request(request).await?;
        Ok(response.json::<TRes>().await?)
    }

    async fn put_empty<TReq: Serialize>(&self, url: reqwest::Url, body: &TReq) -> Result<()> {
        let request = self.http.put(url).json(body);
        self.send_request(request).await?;
        Ok(())
    }

    async fn send_request(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let response = request
            .header("x-pm-apiversion", "3")
            .header("x-pm-appversion", "web-mail@5.0")
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
                "Proton Mail API request failed ({status}): {body}"
            ))
        }
    }
}

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
        matchers::{header, method, path, query_param},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut proton_values = HashMap::new();
        proton_values.insert("access_token".to_string(), "test-token".to_string());
        proton_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("proton", proton_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        server.uri()
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_message_summary_serialization_roundtrip() {
        let summary = MessageSummary {
            id: "msg-1".to_string(),
            subject: Some("Test".to_string()),
            sender: Some(types::EmailAddress {
                address: "test@proton.me".to_string(),
                name: Some("Test User".to_string()),
            }),
            time: Some(1_234_567_890),
            size: Some(1024),
            unread: Some(1),
            starred: Some(0),
            label_ids: vec!["0".to_string()],
        };
        let json = serde_json::to_string(&summary).unwrap();
        let parsed: MessageSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(summary.id, parsed.id);
        assert_eq!(summary.subject, parsed.subject);
    }

    #[test]
    fn test_recipient_serialization_roundtrip() {
        let recipient = Recipient {
            address: "test@proton.me".to_string(),
            name: Some("Test User".to_string()),
        };
        let json = serde_json::to_string(&recipient).unwrap();
        let parsed: Recipient = serde_json::from_str(&json).unwrap();
        assert_eq!(recipient.address, parsed.address);
        assert_eq!(recipient.name, parsed.name);
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://mail.proton.me/api/").unwrap();
        assert_eq!(result, "https://mail.proton.me/api");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://mail.proton.me/api  ").unwrap();
        assert_eq!(result, "https://mail.proton.me/api");
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
    async fn test_search_mail_empty_query_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = search_mail(
            ctx,
            SearchMailInput {
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
    async fn test_search_mail_limit_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = search_mail(
            ctx,
            SearchMailInput {
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
                .contains("limit must be between 1 and 100")
        );
    }

    #[tokio::test]
    async fn test_search_mail_limit_exceeds_max_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = search_mail(
            ctx,
            SearchMailInput {
                query: "test".to_string(),
                limit: Some(101),
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
    async fn test_get_message_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = get_message(
            ctx,
            GetMessageInput {
                message_id: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("message_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_send_email_empty_to_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = send_email(
            ctx,
            SendEmailInput {
                to: vec![],
                cc: vec![],
                bcc: vec![],
                subject: "Test".to_string(),
                body: "Body".to_string(),
                mime_type: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("to must contain at least one recipient")
        );
    }

    #[tokio::test]
    async fn test_send_email_empty_subject_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = send_email(
            ctx,
            SendEmailInput {
                to: vec!["test@proton.me".to_string()],
                cc: vec![],
                bcc: vec![],
                subject: "  ".to_string(),
                body: "Body".to_string(),
                mime_type: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("subject must not be empty")
        );
    }

    #[tokio::test]
    async fn test_reply_empty_message_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = reply(
            ctx,
            ReplyInput {
                message_id: "  ".to_string(),
                body: "Thanks".to_string(),
                reply_all: false,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("message_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_move_message_empty_message_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = move_message(
            ctx,
            MoveInput {
                message_id: "  ".to_string(),
                folder_id: "6".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("message_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_label_message_empty_label_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = label_message(
            ctx,
            LabelInput {
                message_id: "msg-1".to_string(),
                label_id: "  ".to_string(),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("`label_id` must not be empty")
        );
    }

    // --- Integration tests ---

    #[tokio::test]
    async fn test_search_mail_success_returns_messages() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "Code": 1000,
          "Total": 1,
          "Messages": [
            {
              "ID": "msg-1",
              "Subject": "Test Email",
              "Sender": { "Address": "alice@proton.me", "Name": "Alice" },
              "Time": 1234567890,
              "Size": 1024,
              "Unread": 1,
              "Starred": 0,
              "LabelIDs": ["0"]
            }
          ]
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/messages"))
            .and(header("authorization", "Bearer test-token"))
            .and(query_param("Keyword", "test"))
            .and(query_param("Limit", "50"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = search_mail(
            ctx,
            SearchMailInput {
                query: "test".to_string(),
                limit: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.messages.len(), 1);
        assert_eq!(output.messages[0].id, "msg-1");
        assert_eq!(output.messages[0].subject.as_deref(), Some("Test Email"));
        assert_eq!(output.total, 1);
    }

    #[tokio::test]
    async fn test_get_message_success_returns_full_message() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "Code": 1000,
          "Message": {
            "ID": "msg-1",
            "Subject": "Test Email",
            "Sender": { "Address": "alice@proton.me", "Name": "Alice" },
            "ToList": [{ "Address": "bob@proton.me", "Name": "Bob" }],
            "CCList": [],
            "BCCList": [],
            "Time": 1234567890,
            "Body": "Hello world",
            "MIMEType": "text/plain"
          }
        }
        "#;

        Mock::given(method("GET"))
            .and(path("/messages/msg-1"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = get_message(
            ctx,
            GetMessageInput {
                message_id: "msg-1".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.message.id, "msg-1");
        assert_eq!(output.message.subject.as_deref(), Some("Test Email"));
        assert_eq!(output.message.body.as_deref(), Some("Hello world"));
    }

    #[tokio::test]
    async fn test_send_email_success_returns_message_id() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        let response_body = r#"
        {
          "Code": 1000,
          "Message": {
            "ID": "msg-new",
            "Subject": "Test Subject"
          }
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/messages"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = send_email(
            ctx,
            SendEmailInput {
                to: vec!["bob@proton.me".to_string()],
                cc: vec![],
                bcc: vec![],
                subject: "Test Subject".to_string(),
                body: "Test body".to_string(),
                mime_type: None,
            },
        )
        .await
        .unwrap();

        assert!(output.sent);
        assert_eq!(output.message_id, "msg-new");
    }

    #[tokio::test]
    async fn test_move_message_success_returns_moved() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("PUT"))
            .and(path("/messages/msg-1"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = move_message(
            ctx,
            MoveInput {
                message_id: "msg-1".to_string(),
                folder_id: "6".to_string(),
            },
        )
        .await
        .unwrap();

        assert!(output.moved);
        assert_eq!(output.message_id, "msg-1");
        assert_eq!(output.folder_id, "6");
    }

    #[tokio::test]
    async fn test_label_message_success_returns_labeled() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("PUT"))
            .and(path("/messages/msg-1/label"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = label_message(
            ctx,
            LabelInput {
                message_id: "msg-1".to_string(),
                label_id: "custom-label".to_string(),
            },
        )
        .await
        .unwrap();

        assert!(output.labeled);
        assert_eq!(output.message_id, "msg-1");
        assert_eq!(output.label_id, "custom-label");
    }
}
