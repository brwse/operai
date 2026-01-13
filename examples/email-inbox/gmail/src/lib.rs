//! email-inbox/gmail integration for Operai Toolbox.

use std::fmt::Write;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};

mod types;
use types::{
    GmailMessage, ListMessagesResponse, MessageHeader, MessagePart, ModifyMessageRequest,
    SendMessageRequest,
};

define_user_credential! {
    GmailCredential("gmail") {
        access_token: String,
        #[optional]
        endpoint: Option<String>,
    }
}

const DEFAULT_GMAIL_ENDPOINT: &str = "https://gmail.googleapis.com/gmail/v1";

#[init]
async fn setup() -> Result<()> {
    info!("Gmail integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Gmail integration shutting down");
}

// ============================================================================
// Public Output Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EmailAddress {
    pub address: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MessageSummary {
    pub id: String,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub from: Option<EmailAddress>,
    #[serde(default)]
    pub to: Vec<EmailAddress>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub snippet: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MessageDetail {
    pub id: String,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub from: Option<EmailAddress>,
    #[serde(default)]
    pub to: Vec<EmailAddress>,
    #[serde(default)]
    pub cc: Vec<EmailAddress>,
    #[serde(default)]
    pub bcc: Vec<EmailAddress>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub snippet: Option<String>,
    #[serde(default)]
    pub body_text: Option<String>,
    #[serde(default)]
    pub body_html: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
}

// ============================================================================
// Tool: search_messages
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchMessagesInput {
    /// Search query string (Gmail search syntax).
    pub query: String,
    /// Maximum number of results (1-100). Defaults to 10.
    #[serde(default)]
    pub max_results: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchMessagesOutput {
    pub messages: Vec<MessageSummary>,
}

/// # Search Gmail Messages
///
/// Searches for messages in the user's Gmail mailbox using Gmail's powerful
/// search syntax. Use this tool when a user wants to find emails matching
/// specific criteria such as sender, recipient, subject, date range, labels, or
/// any other Gmail search operators.
///
/// The query parameter supports the same search syntax as the Gmail web
/// interface, including:
/// - `from:sender@example.com` - messages from a specific sender
/// - `to:recipient@example.com` - messages to a specific recipient
/// - `subject:keywords` - messages with keywords in the subject
/// - `has:attachment` - messages with attachments
/// - `label:LABEL_NAME` - messages with a specific label
/// - `is:unread`, `is:starred`, `is:important` - messages with specific status
/// - `before:YYYY/MM/DD` or `after:YYYY/MM/DD` - date range queries
/// - Boolean operators like OR, AND, NOT (e.g., `from:alice OR from:bob`)
///
/// Returns a list of message summaries including ID, subject, sender,
/// recipients, date, snippet preview, and labels. Use `get_message` to retrieve
/// full message content including body.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - email
/// - gmail
/// - google
///
/// # Errors
///
/// Returns an error if:
/// - The query string is empty or contains only whitespace
/// - `max_results` is not between 1 and 100
/// - No Gmail credentials are configured in the context
/// - The access token in credentials is empty
/// - The Gmail API endpoint URL is invalid
/// - The HTTP request to the Gmail API fails
/// - The API response cannot be parsed as valid JSON
#[tool]
pub async fn search_messages(
    ctx: Context,
    input: SearchMessagesInput,
) -> Result<SearchMessagesOutput> {
    ensure!(!input.query.trim().is_empty(), "query must not be empty");
    let max_results = input.max_results.unwrap_or(10);
    ensure!(
        (1..=100).contains(&max_results),
        "max_results must be between 1 and 100"
    );

    let client = GmailClient::from_ctx(&ctx)?;

    let query_params = [
        ("q", input.query.as_str()),
        ("maxResults", &max_results.to_string()),
    ];

    let list_response: ListMessagesResponse = client
        .get_json(&["users", "me", "messages"], &query_params)
        .await?;

    let mut summaries = Vec::new();
    for msg_ref in list_response.messages {
        let message: GmailMessage = client
            .get_json(
                &["users", "me", "messages", &msg_ref.id],
                &[("format", "metadata")],
            )
            .await?;

        // Prefer threadId from list response (more efficient), fallback to message
        // threadId
        let thread_id = msg_ref.thread_id.clone().or(message.thread_id.clone());
        let mut summary = map_summary(message);
        summary.thread_id = thread_id;

        summaries.push(summary);
    }

    Ok(SearchMessagesOutput {
        messages: summaries,
    })
}

// ============================================================================
// Tool: get_message
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetMessageInput {
    /// Gmail message ID.
    pub message_id: String,
    /// Include full message body content.
    #[serde(default)]
    pub include_body: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetMessageOutput {
    pub message: MessageDetail,
}

/// # Get Gmail Message
///
/// Retrieves a single Gmail message by its unique message ID.
/// Use this tool when a user wants to read the full content of a specific email
/// message.
///
/// This tool fetches complete message metadata including headers (From, To, Cc,
/// Bcc, Subject, Date), labels, thread ID, and snippet preview. When
/// `include_body` is set to true, it also extracts the message body content in
/// both plain text and HTML formats (if available).
///
/// Use `search_messages` first to find message IDs, then use this tool to
/// retrieve the full content. The message ID can be obtained from a previous
/// search or from the message's URL in Gmail.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - email
/// - gmail
/// - google
///
/// # Errors
///
/// Returns an error if:
/// - The `message_id` is empty or contains only whitespace
/// - No Gmail credentials are configured in the context
/// - The access token in credentials is empty
/// - The Gmail API endpoint URL is invalid
/// - The HTTP request to the Gmail API fails
/// - The API response cannot be parsed as valid JSON
#[tool]
pub async fn get_message(ctx: Context, input: GetMessageInput) -> Result<GetMessageOutput> {
    ensure!(
        !input.message_id.trim().is_empty(),
        "message_id must not be empty"
    );

    let client = GmailClient::from_ctx(&ctx)?;

    let format = if input.include_body {
        "full"
    } else {
        "metadata"
    };
    let message: GmailMessage = client
        .get_json(
            &["users", "me", "messages", &input.message_id],
            &[("format", format)],
        )
        .await?;

    Ok(GetMessageOutput {
        message: map_detail(message, input.include_body),
    })
}

// ============================================================================
// Tool: send_email
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SendEmailInput {
    /// Recipient email addresses (To).
    pub to: Vec<String>,
    /// CC recipients (optional).
    #[serde(default)]
    pub cc: Vec<String>,
    /// BCC recipients (optional).
    #[serde(default)]
    pub bcc: Vec<String>,
    /// Email subject.
    pub subject: String,
    /// Email body text.
    pub body: String,
    /// Reply-To email address (optional).
    #[serde(default)]
    pub reply_to: Option<String>,
    /// In-Reply-To header for threading replies (optional).
    #[serde(default)]
    pub in_reply_to: Option<String>,
    /// References header for threading replies (optional).
    #[serde(default)]
    pub references: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SendEmailOutput {
    pub message_id: String,
    pub thread_id: String,
}

/// # Send Gmail Email
///
/// Sends a new email message from the authenticated user's Gmail account.
/// Use this tool when a user wants to compose and send a new email to one or
/// more recipients.
///
/// This tool creates and sends a new email with the specified recipients (To,
/// Cc, Bcc), subject line, and plain text body content. The email will appear
/// in the user's Sent folder in Gmail with a "Sent" label applied
/// automatically.
///
/// For reply scenarios where you want to maintain email thread context, use the
/// `reply` tool instead which automatically handles threading headers
/// (In-Reply-To, References). Alternatively, you can manually provide those
/// headers when using this tool for advanced threading control.
///
/// Returns the assigned message ID and thread ID of the sent email, which can
/// be used for tracking or further operations on the message.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - email
/// - gmail
/// - google
///
/// # Errors
///
/// Returns an error if:
/// - The to field is empty (no recipients specified)
/// - Any email address in to, cc, or bcc fields is empty
/// - The subject is empty or contains only whitespace
/// - The body is empty or contains only whitespace
/// - No Gmail credentials are configured in the context
/// - The access token in credentials is empty
/// - The Gmail API endpoint URL is invalid
/// - The HTTP request to the Gmail API fails
/// - The API response cannot be parsed as valid JSON
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

    let client = GmailClient::from_ctx(&ctx)?;

    let raw_message = build_raw_message(&input);
    let encoded = URL_SAFE_NO_PAD.encode(raw_message.as_bytes());

    let request = SendMessageRequest { raw: encoded };

    let sent: GmailMessage = client
        .post_json(&["users", "me", "messages", "send"], &request)
        .await?;

    Ok(SendEmailOutput {
        message_id: sent.id,
        thread_id: sent.thread_id.unwrap_or_default(),
    })
}

// ============================================================================
// Tool: reply
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplyInput {
    /// Gmail message ID to reply to.
    pub message_id: String,
    /// Reply text.
    pub body: String,
    /// When true, reply to all recipients.
    #[serde(default)]
    pub reply_all: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReplyOutput {
    pub message_id: String,
    pub thread_id: String,
}

/// # Reply to Gmail Message
///
/// Sends a reply to an existing Gmail message while preserving thread context.
/// Use this tool when a user wants to respond to an email they received.
///
/// This tool automatically handles proper email threading by:
/// - Extracting the original message's subject and prepending "Re:" if needed
/// - Setting the In-Reply-To header to reference the original message
/// - Building the References header for full thread history
/// - Preserving the thread ID so the reply appears in the same conversation
///
/// When `reply_all` is true, the reply is sent to all original recipients (From
/// + To + Cc). When false, the reply is sent only to the original sender.
///
/// The tool fetches the original message metadata first to extract threading
/// information and recipient details, then sends the reply as a new message in
/// the same thread.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - email
/// - gmail
/// - google
///
/// # Errors
///
/// Returns an error if:
/// - The `message_id` is empty or contains only whitespace
/// - The body is empty or contains only whitespace
/// - No Gmail credentials are configured in the context
/// - The access token in credentials is empty
/// - The Gmail API endpoint URL is invalid
/// - The HTTP request to fetch the original message fails
/// - The HTTP request to send the reply fails
/// - The API response cannot be parsed as valid JSON
#[tool]
pub async fn reply(ctx: Context, input: ReplyInput) -> Result<ReplyOutput> {
    ensure!(
        !input.message_id.trim().is_empty(),
        "message_id must not be empty"
    );
    ensure!(!input.body.trim().is_empty(), "body must not be empty");

    let client = GmailClient::from_ctx(&ctx)?;

    let original: GmailMessage = client
        .get_json(
            &["users", "me", "messages", &input.message_id],
            &[("format", "metadata")],
        )
        .await?;

    let thread_id = original.thread_id.unwrap_or_else(|| original.id.clone());

    let headers = original.payload.as_ref().map(|p| &p.headers);
    let subject = extract_header(headers, "Subject").unwrap_or("(no subject)");
    let from = extract_header(headers, "From").unwrap_or("");
    let to = extract_header(headers, "To").unwrap_or("");
    let cc = extract_header(headers, "Cc");
    let message_id = extract_header(headers, "Message-ID");
    let references = extract_header(headers, "References");

    let reply_to = if input.reply_all {
        let mut recipients = Vec::new();
        recipients.push(from);
        if !to.is_empty() {
            recipients.push(to);
        }
        if let Some(cc_val) = cc
            && !cc_val.is_empty()
        {
            recipients.push(cc_val);
        }
        recipients.join(", ")
    } else {
        from.to_string()
    };

    let reply_subject = if subject.to_lowercase().starts_with("re:") {
        subject.to_string()
    } else {
        format!("Re: {subject}")
    };

    // Build References header for threading
    let new_references = if let Some(refs) = references {
        if refs.contains(&original.id) {
            refs.to_string()
        } else {
            format!("{refs} {}", original.id)
        }
    } else {
        original.id.clone()
    };

    let reply_message = SendEmailInput {
        to: vec![reply_to],
        cc: vec![],
        bcc: vec![],
        subject: reply_subject,
        body: input.body,
        reply_to: None,
        in_reply_to: message_id.map(std::string::ToString::to_string),
        references: Some(new_references),
    };

    let raw_message = build_raw_message(&reply_message);
    let encoded = URL_SAFE_NO_PAD.encode(raw_message.as_bytes());

    let request = SendMessageRequest { raw: encoded };

    let sent: GmailMessage = client
        .post_json(&["users", "me", "messages", "send"], &request)
        .await?;

    Ok(ReplyOutput {
        message_id: sent.id,
        thread_id: sent.thread_id.unwrap_or(thread_id),
    })
}

// ============================================================================
// Tool: label_message
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LabelMessageInput {
    /// Gmail message ID.
    pub message_id: String,
    /// Label IDs to add.
    #[serde(default)]
    pub add_labels: Vec<String>,
    /// Label IDs to remove.
    #[serde(default)]
    pub remove_labels: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LabelMessageOutput {
    pub message_id: String,
    pub labels: Vec<String>,
}

/// # Label Gmail Message
///
/// Adds or removes Gmail labels on a message to organize and categorize emails.
/// Use this tool when a user wants to apply or remove labels like STARRED,
/// IMPORTANT, SPAM, TRASH, or custom labels to manage their inbox organization.
///
/// This tool can add labels, remove labels, or do both in a single operation.
/// Common system labels include: INBOX, STARRED, IMPORTANT, SPAM, TRASH, SENT,
/// DRAFT, and personal categories like TRAVEL, FINANCE, SOCIAL, PROMOTIONS,
/// etc.
///
/// Label operations are useful for:
/// - Marking messages as important or starring them for quick access
/// - Moving messages out of the inbox (e.g., remove INBOX label to archive)
/// - Categorizing messages with custom labels for filtering
/// - Marking messages as spam or moving to trash
///
/// Returns the updated list of labels applied to the message after the
/// operation.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - email
/// - gmail
/// - google
///
/// # Errors
///
/// Returns an error if:
/// - The `message_id` is empty or contains only whitespace
/// - Both `add_labels` and `remove_labels` are empty (no labels specified)
/// - No Gmail credentials are configured in the context
/// - The access token in credentials is empty
/// - The Gmail API endpoint URL is invalid
/// - The HTTP request to the Gmail API fails
/// - The API response cannot be parsed as valid JSON
#[tool]
pub async fn label_message(ctx: Context, input: LabelMessageInput) -> Result<LabelMessageOutput> {
    ensure!(
        !input.message_id.trim().is_empty(),
        "message_id must not be empty"
    );
    ensure!(
        !input.add_labels.is_empty() || !input.remove_labels.is_empty(),
        "must specify at least one label to add or remove"
    );

    let client = GmailClient::from_ctx(&ctx)?;

    let request = ModifyMessageRequest {
        add_label_ids: input.add_labels,
        remove_label_ids: input.remove_labels,
    };

    let modified: GmailMessage = client
        .post_json(
            &["users", "me", "messages", &input.message_id, "modify"],
            &request,
        )
        .await?;

    Ok(LabelMessageOutput {
        message_id: modified.id,
        labels: modified.label_ids,
    })
}

// ============================================================================
// Tool: archive_message
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ArchiveMessageInput {
    /// Gmail message ID to archive.
    pub message_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ArchiveMessageOutput {
    pub message_id: String,
    pub archived: bool,
}

/// # Archive Gmail Message
///
/// Archives a Gmail message by removing it from the inbox while keeping it in
/// All Mail. Use this tool when a user wants to clean up their inbox without
/// deleting the message.
///
/// Archiving removes the INBOX label from the message, causing it to no longer
/// appear in the inbox view. The message remains accessible in All Mail and
/// retains any other labels it had (e.g., STARRED, IMPORTANT, custom labels).
///
/// This is Gmail's recommended workflow for managing processed emails—archive
/// instead of delete to keep a complete record of all communications. The
/// message can still be found via search and will appear under its other
/// labels.
///
/// Use this tool when a user has read or processed an email and wants to remove
/// it from the inbox without permanently deleting it.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - email
/// - gmail
/// - google
///
/// # Errors
///
/// Returns an error if:
/// - The `message_id` is empty or contains only whitespace
/// - No Gmail credentials are configured in the context
/// - The access token in credentials is empty
/// - The Gmail API endpoint URL is invalid
/// - The HTTP request to the Gmail API fails
/// - The API response cannot be parsed as valid JSON
#[tool]
pub async fn archive_message(
    ctx: Context,
    input: ArchiveMessageInput,
) -> Result<ArchiveMessageOutput> {
    ensure!(
        !input.message_id.trim().is_empty(),
        "message_id must not be empty"
    );

    let client = GmailClient::from_ctx(&ctx)?;

    let request = ModifyMessageRequest {
        add_label_ids: vec![],
        remove_label_ids: vec!["INBOX".to_string()],
    };

    let _: GmailMessage = client
        .post_json(
            &["users", "me", "messages", &input.message_id, "modify"],
            &request,
        )
        .await?;

    Ok(ArchiveMessageOutput {
        message_id: input.message_id,
        archived: true,
    })
}

// ============================================================================
// Helper Functions
// ============================================================================

fn build_raw_message(input: &SendEmailInput) -> String {
    let mut message = String::new();

    let _ = write!(message, "To: {}\r\n", input.to.join(", "));

    if !input.cc.is_empty() {
        let _ = write!(message, "Cc: {}\r\n", input.cc.join(", "));
    }

    if !input.bcc.is_empty() {
        let _ = write!(message, "Bcc: {}\r\n", input.bcc.join(", "));
    }

    if let Some(reply_to) = &input.reply_to {
        let _ = write!(message, "Reply-To: {reply_to}\r\n");
    }

    // Add threading headers for replies
    if let Some(in_reply_to) = &input.in_reply_to {
        let _ = write!(message, "In-Reply-To: {in_reply_to}\r\n");
    }

    if let Some(references) = &input.references {
        let _ = write!(message, "References: {references}\r\n");
    }

    let _ = write!(message, "Subject: {}\r\n", input.subject);
    message.push_str("Content-Type: text/plain; charset=UTF-8\r\n");
    message.push_str("\r\n");
    message.push_str(&input.body);

    message
}

fn extract_header<'a>(headers: Option<&'a Vec<MessageHeader>>, name: &str) -> Option<&'a str> {
    headers?
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str())
}

fn parse_email_address(raw: &str) -> EmailAddress {
    if let (Some(start), Some(end)) = (raw.find('<'), raw.find('>')) {
        let name = raw[..start].trim().trim_matches('"');
        let address = raw[start + 1..end].trim();
        return EmailAddress {
            address: address.to_string(),
            name: if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            },
        };
    }

    EmailAddress {
        address: raw.trim().to_string(),
        name: None,
    }
}

fn parse_email_addresses(raw: &str) -> Vec<EmailAddress> {
    raw.split(',')
        .map(|s| parse_email_address(s.trim()))
        .collect()
}

fn map_summary(message: GmailMessage) -> MessageSummary {
    let headers = message.payload.as_ref().map(|p| &p.headers);

    MessageSummary {
        id: message.id,
        thread_id: message.thread_id,
        subject: extract_header(headers, "Subject").map(String::from),
        from: extract_header(headers, "From").map(parse_email_address),
        to: extract_header(headers, "To")
            .map(parse_email_addresses)
            .unwrap_or_default(),
        date: extract_header(headers, "Date").map(String::from),
        snippet: message.snippet,
        labels: message.label_ids,
    }
}

fn map_detail(message: GmailMessage, extract_body: bool) -> MessageDetail {
    let headers = message.payload.as_ref().map(|p| &p.headers);

    let (body_text, body_html) = if extract_body {
        extract_body_content(message.payload.as_ref())
    } else {
        (None, None)
    };

    MessageDetail {
        id: message.id,
        thread_id: message.thread_id,
        subject: extract_header(headers, "Subject").map(String::from),
        from: extract_header(headers, "From").map(parse_email_address),
        to: extract_header(headers, "To")
            .map(parse_email_addresses)
            .unwrap_or_default(),
        cc: extract_header(headers, "Cc")
            .map(parse_email_addresses)
            .unwrap_or_default(),
        bcc: extract_header(headers, "Bcc")
            .map(parse_email_addresses)
            .unwrap_or_default(),
        date: extract_header(headers, "Date").map(String::from),
        snippet: message.snippet,
        body_text,
        body_html,
        labels: message.label_ids,
    }
}

fn extract_body_content(payload: Option<&MessagePart>) -> (Option<String>, Option<String>) {
    let Some(payload) = payload else {
        return (None, None);
    };

    let mut text_body = None;
    let mut html_body = None;

    find_body_recursive(payload, &mut text_body, &mut html_body);

    (text_body, html_body)
}

fn decode_body_data(part: &MessagePart) -> Option<String> {
    let data = part.body.as_ref()?.data.as_ref()?;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(data)
        .ok()?;
    String::from_utf8(decoded).ok()
}

fn find_body_recursive(part: &MessagePart, text: &mut Option<String>, html: &mut Option<String>) {
    if let Some(mime_type) = &part.mime_type {
        if mime_type == "text/plain" {
            if let Some(decoded) = decode_body_data(part) {
                *text = Some(decoded);
            }
        } else if mime_type == "text/html"
            && let Some(decoded) = decode_body_data(part)
        {
            *html = Some(decoded);
        }
    }

    for subpart in &part.parts {
        find_body_recursive(subpart, text, html);
    }
}

// ============================================================================
// Gmail Client
// ============================================================================

#[derive(Debug, Clone)]
struct GmailClient {
    http: reqwest::Client,
    base_url: String,
    access_token: String,
}

impl GmailClient {
    /// Creates a new `GmailClient` from the provided context.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No Gmail credentials are configured in the context
    /// - The access token in credentials is empty
    /// - The endpoint URL (if provided) or default URL is invalid
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = GmailCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        let base_url =
            normalize_base_url(cred.endpoint.as_deref().unwrap_or(DEFAULT_GMAIL_ENDPOINT))?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            access_token: cred.access_token,
        })
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        segments: &[&str],
        query: &[(&str, &str)],
    ) -> Result<T> {
        let url = self.build_url(segments)?;
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

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        segments: &[&str],
        body: &TReq,
    ) -> Result<TRes> {
        let url = self.build_url(segments)?;
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

    /// Builds a complete URL by appending path segments to the base URL.
    ///
    /// # Panics
    ///
    /// Panics if the base URL cannot be turned into a cannot-be-a-base URL
    /// (should not happen with properly configured base URLs).
    ///
    /// # Errors
    ///
    /// Returns an error if the `base_url` is not an absolute URL.
    fn build_url(&self, segments: &[&str]) -> Result<reqwest::Url> {
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
                "Gmail API request failed ({status}): {body}"
            ))
        }
    }
}

/// Normalizes a base URL by trimming whitespace and removing trailing slashes.
///
/// # Errors
///
/// Returns an error if the endpoint string is empty or contains only
/// whitespace.
fn normalize_base_url(endpoint: &str) -> Result<String> {
    let trimmed = endpoint.trim();
    ensure!(!trimmed.is_empty(), "endpoint must not be empty");
    Ok(trimmed.trim_end_matches('/').to_string())
}

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
        let mut gmail_values = HashMap::new();
        gmail_values.insert("access_token".to_string(), "test-token".to_string());
        gmail_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("gmail", gmail_values)
    }

    fn endpoint_for(server: &MockServer) -> String {
        format!("{}/gmail/v1", server.uri())
    }

    // --- Serialization tests ---

    #[test]
    fn test_email_address_serialization_roundtrip() {
        let email = EmailAddress {
            address: "test@example.com".to_string(),
            name: Some("Test User".to_string()),
        };
        let json = serde_json::to_string(&email).unwrap();
        let parsed: EmailAddress = serde_json::from_str(&json).unwrap();
        assert_eq!(email.address, parsed.address);
        assert_eq!(email.name, parsed.name);
    }

    #[test]
    fn test_parse_email_address_with_name() {
        let result = parse_email_address("\"John Doe\" <john@example.com>");
        assert_eq!(result.address, "john@example.com");
        assert_eq!(result.name.as_deref(), Some("John Doe"));
    }

    #[test]
    fn test_parse_email_address_without_name() {
        let result = parse_email_address("john@example.com");
        assert_eq!(result.address, "john@example.com");
        assert!(result.name.is_none());
    }

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://gmail.googleapis.com/").unwrap();
        assert_eq!(result, "https://gmail.googleapis.com");
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
    async fn test_search_messages_empty_query_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = search_messages(
            ctx,
            SearchMessagesInput {
                query: "   ".to_string(),
                max_results: None,
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
    async fn test_search_messages_max_results_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = search_messages(
            ctx,
            SearchMessagesInput {
                query: "test".to_string(),
                max_results: Some(0),
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("max_results must be between 1 and 100")
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
                include_body: false,
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
                subject: "Hi".to_string(),
                body: "Body".to_string(),
                reply_to: None,
                in_reply_to: None,
                references: None,
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
                to: vec!["test@example.com".to_string()],
                cc: vec![],
                bcc: vec![],
                subject: "  ".to_string(),
                body: "Body".to_string(),
                reply_to: None,
                in_reply_to: None,
                references: None,
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
    async fn test_label_message_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = label_message(
            ctx,
            LabelMessageInput {
                message_id: "  ".to_string(),
                add_labels: vec!["STARRED".to_string()],
                remove_labels: vec![],
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
    async fn test_label_message_no_labels_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = label_message(
            ctx,
            LabelMessageInput {
                message_id: "msg-1".to_string(),
                add_labels: vec![],
                remove_labels: vec![],
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must specify at least one label")
        );
    }

    #[tokio::test]
    async fn test_archive_message_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&endpoint_for(&server));

        let result = archive_message(
            ctx,
            ArchiveMessageInput {
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

    // --- Integration tests ---

    #[tokio::test]
    async fn test_search_messages_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/gmail/v1/users/me/messages"))
            .and(query_param("q", "test"))
            .and(query_param("maxResults", "5"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"messages": [{"id": "msg-1", "threadId": "thread-1"}]}"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/gmail/v1/users/me/messages/msg-1"))
            .and(query_param("format", "metadata"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{
                        "id": "msg-1",
                        "threadId": "thread-1",
                        "labelIds": ["INBOX"],
                        "snippet": "Test message",
                        "payload": {
                            "headers": [
                                {"name": "From", "value": "alice@example.com"},
                                {"name": "To", "value": "bob@example.com"},
                                {"name": "Subject", "value": "Test"},
                                {"name": "Date", "value": "Wed, 01 Jan 2025 12:00:00 +0000"}
                            ]
                        }
                    }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = search_messages(
            ctx,
            SearchMessagesInput {
                query: "test".to_string(),
                max_results: Some(5),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.messages.len(), 1);
        assert_eq!(output.messages[0].id, "msg-1");
        assert_eq!(output.messages[0].subject.as_deref(), Some("Test"));
    }

    #[tokio::test]
    async fn test_send_email_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("POST"))
            .and(path("/gmail/v1/users/me/messages/send"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("\"raw\":"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"id": "msg-sent", "threadId": "thread-1"}"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = send_email(
            ctx,
            SendEmailInput {
                to: vec!["alice@example.com".to_string()],
                cc: vec![],
                bcc: vec![],
                subject: "Hello".to_string(),
                body: "Test body".to_string(),
                reply_to: None,
                in_reply_to: None,
                references: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.message_id, "msg-sent");
        assert_eq!(output.thread_id, "thread-1");
    }

    #[tokio::test]
    async fn test_label_message_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("POST"))
            .and(path("/gmail/v1/users/me/messages/msg-1/modify"))
            .and(body_string_contains("\"addLabelIds\":[\"STARRED\"]"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"id": "msg-1", "labelIds": ["INBOX", "STARRED"]}"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = label_message(
            ctx,
            LabelMessageInput {
                message_id: "msg-1".to_string(),
                add_labels: vec!["STARRED".to_string()],
                remove_labels: vec![],
            },
        )
        .await
        .unwrap();

        assert_eq!(output.message_id, "msg-1");
        assert!(output.labels.contains(&"STARRED".to_string()));
    }

    #[tokio::test]
    async fn test_archive_message_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("POST"))
            .and(path("/gmail/v1/users/me/messages/msg-1/modify"))
            .and(body_string_contains("\"removeLabelIds\":[\"INBOX\"]"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw(r#"{"id": "msg-1", "labelIds": []}"#, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = archive_message(
            ctx,
            ArchiveMessageInput {
                message_id: "msg-1".to_string(),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.message_id, "msg-1");
        assert!(output.archived);
    }

    // --- Additional integration tests ---

    #[tokio::test]
    async fn test_get_message_with_body_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/gmail/v1/users/me/messages/msg-1"))
            .and(query_param("format", "full"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{
                    "id": "msg-1",
                    "threadId": "thread-1",
                    "labelIds": ["INBOX"],
                    "payload": {
                        "mimeType": "text/plain",
                        "headers": [
                            {"name": "From", "value": "alice@example.com"},
                            {"name": "To", "value": "bob@example.com"},
                            {"name": "Subject", "value": "Test"}
                        ],
                        "body": {
                            "data": "VGVzdCBib2R5"
                        }
                    }
                }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = get_message(
            ctx,
            GetMessageInput {
                message_id: "msg-1".to_string(),
                include_body: true,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.message.id, "msg-1");
        assert_eq!(output.message.body_text.as_deref(), Some("Test body"));
    }

    #[tokio::test]
    async fn test_reply_to_message_success() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/gmail/v1/users/me/messages/msg-1"))
            .and(query_param("format", "metadata"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{
                    "id": "msg-1",
                    "threadId": "thread-1",
                    "payload": {
                        "headers": [
                            {"name": "From", "value": "alice@example.com"},
                            {"name": "To", "value": "bob@example.com"},
                            {"name": "Subject", "value": "Hello"},
                            {"name": "Message-ID", "value": "<original@example.com>"}
                        ]
                    }
                }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/gmail/v1/users/me/messages/send"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"id": "msg-reply", "threadId": "thread-1"}"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = reply(
            ctx,
            ReplyInput {
                message_id: "msg-1".to_string(),
                body: "Thanks!".to_string(),
                reply_all: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.message_id, "msg-reply");
        assert_eq!(output.thread_id, "thread-1");
    }

    #[tokio::test]
    async fn test_search_messages_with_thread_id_from_list() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/gmail/v1/users/me/messages"))
            .and(query_param("q", "test"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{
                    "messages": [
                        {"id": "msg-1", "threadId": "thread-from-list"}
                    ],
                    "nextPageToken": "token-123",
                    "resultSizeEstimate": 100
                }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/gmail/v1/users/me/messages/msg-1"))
            .and(query_param("format", "metadata"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{
                    "id": "msg-1",
                    "threadId": "thread-from-message",
                    "labelIds": ["INBOX"],
                    "payload": {
                        "headers": [
                            {"name": "From", "value": "alice@example.com"},
                            {"name": "Subject", "value": "Test"}
                        ]
                    }
                }"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let output = search_messages(
            ctx,
            SearchMessagesInput {
                query: "test".to_string(),
                max_results: Some(10),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.messages.len(), 1);
        // Code prefers threadId from list response (msg_ref.thread_id)
        // over the threadId from the full message
        assert_eq!(
            output.messages[0].thread_id.as_deref(),
            Some("thread-from-list")
        );
    }

    #[tokio::test]
    async fn test_parse_email_address_with_angle_brackets() {
        let result = parse_email_address("bob@example.com");
        assert_eq!(result.address, "bob@example.com");
        assert!(result.name.is_none());
    }

    #[tokio::test]
    async fn test_parse_multiple_email_addresses() {
        let results = parse_email_addresses("alice@example.com, \"Bob Smith\" <bob@example.com>");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].address, "alice@example.com");
        assert_eq!(results[1].address, "bob@example.com");
        assert_eq!(results[1].name.as_deref(), Some("Bob Smith"));
    }

    #[tokio::test]
    async fn test_search_messages_api_error_handling() {
        let server = MockServer::start().await;
        let endpoint = endpoint_for(&server);

        Mock::given(method("GET"))
            .and(path("/gmail/v1/users/me/messages"))
            .respond_with(ResponseTemplate::new(401).set_body_raw(
                r#"{"error": {"code": 401, "message": "Invalid credentials"}}"#,
                "application/json",
            ))
            .mount(&server)
            .await;

        let ctx = test_ctx(&endpoint);
        let result = search_messages(
            ctx,
            SearchMessagesInput {
                query: "test".to_string(),
                max_results: None,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("401"));
    }

    #[tokio::test]
    async fn test_message_summary_serialization() {
        let summary = MessageSummary {
            id: "msg-1".to_string(),
            thread_id: Some("thread-1".to_string()),
            subject: Some("Test".to_string()),
            from: Some(EmailAddress {
                address: "alice@example.com".to_string(),
                name: Some("Alice".to_string()),
            }),
            to: vec![EmailAddress {
                address: "bob@example.com".to_string(),
                name: None,
            }],
            date: Some("Mon, 01 Jan 2025 12:00:00 +0000".to_string()),
            snippet: Some("Test message".to_string()),
            labels: vec!["INBOX".to_string()],
        };

        let json = serde_json::to_string(&summary).unwrap();
        let parsed: MessageSummary = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, "msg-1");
        assert_eq!(parsed.subject, Some("Test".to_string()));
        assert_eq!(
            parsed.from.as_ref().unwrap().name,
            Some("Alice".to_string())
        );
    }

    #[tokio::test]
    async fn test_message_detail_serialization() {
        let detail = MessageDetail {
            id: "msg-1".to_string(),
            thread_id: Some("thread-1".to_string()),
            subject: Some("Test".to_string()),
            from: Some(EmailAddress {
                address: "alice@example.com".to_string(),
                name: None,
            }),
            to: vec![],
            cc: vec![],
            bcc: vec![],
            date: None,
            snippet: None,
            body_text: Some("Body".to_string()),
            body_html: None,
            labels: vec![],
        };

        let json = serde_json::to_string(&detail).unwrap();
        let parsed: MessageDetail = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, "msg-1");
        assert_eq!(parsed.body_text, Some("Body".to_string()));
    }
}
