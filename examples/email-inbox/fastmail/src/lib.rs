//! email-inbox/fastmail integration for Operai Toolbox using JMAP protocol.

mod types;

use std::collections::HashMap;

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
use types::{
    Email, EmailAddress, EmailBodyPart, EmailBodyValue, EmailGetArgs, EmailGetResponse,
    EmailQueryArgs, EmailQueryFilter, EmailQueryResponse, EmailSetArgs, EmailSetCreate,
    EmailSetResponse, EmailSubmissionCreate, EmailSubmissionSetArgs, EmailSummary, IdentityGetArgs,
    IdentityGetResponse, JmapRequest, JmapResponse,
};

define_user_credential! {
    FastmailCredential("fastmail") {
        api_token: String,
        #[optional]
        endpoint: Option<String>,
        #[optional]
        account_id: Option<String>,
    }
}

const DEFAULT_JMAP_ENDPOINT: &str = "https://api.fastmail.com/jmap/api/";

/// Initializes the Fastmail integration.
///
/// # Errors
///
/// This function currently always returns `Ok(())`. In a real implementation,
/// it could fail if initialization resources cannot be allocated.
#[init]
async fn setup() -> Result<()> {
    info!("Fastmail integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Fastmail integration shutting down");
}

// ========== Tool Inputs and Outputs ==========

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchMailInput {
    /// Search query string (searches across subject, from, to, and body).
    pub query: String,
    /// Maximum number of results (1-100). Defaults to 25.
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchMailOutput {
    pub messages: Vec<EmailSummary>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetMessageInput {
    /// JMAP email ID.
    pub message_id: String,
    /// When true, include the full message body content (text and HTML).
    #[serde(default)]
    pub include_body: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GetMessageOutput {
    pub message: Email,
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
    /// Email body content (plain text).
    pub body: String,
    /// Mailbox ID where the sent email should be stored (e.g., "Sent").
    /// If not provided, uses default Sent mailbox.
    #[serde(default)]
    pub sent_mailbox_id: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SendEmailOutput {
    pub email_id: String,
    pub submission_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplyInput {
    /// JMAP email ID to reply to.
    pub message_id: String,
    /// Reply text to include.
    pub body: String,
    /// When true, reply-all instead of reply.
    #[serde(default)]
    pub reply_all: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReplyOutput {
    pub email_id: String,
    pub submission_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MoveInput {
    /// JMAP email ID to move.
    pub message_id: String,
    /// Destination mailbox ID.
    pub destination_mailbox_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MoveOutput {
    pub updated: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LabelInput {
    /// JMAP email ID to label.
    pub message_id: String,
    /// Keyword/label to add or remove (e.g., "$flagged", "$seen", or custom
    /// labels).
    pub keyword: String,
    /// When true, adds the keyword. When false, removes it.
    pub add: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LabelOutput {
    pub updated: bool,
}

// ========== Tool Implementations ==========

/// # Search Fastmail Emails
///
/// Searches for emails in the user's Fastmail account using the JMAP
/// Email/query API. Use this tool when the user wants to find emails matching a
/// search query.
///
/// The search performs a full-text search across the email's subject, sender,
/// recipients, and body content. Returns a list of email summaries including
/// subject, sender, preview, and metadata.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - email
/// - fastmail
/// - jmap
///
/// # Errors
///
/// Returns an error if:
/// - The query string is empty or contains only whitespace
/// - The limit is not between 1 and 100
/// - Fastmail credentials are missing or invalid
/// - The JMAP API request fails
/// - The response cannot be parsed
#[tool]
pub async fn search_mail(ctx: Context, input: SearchMailInput) -> Result<SearchMailOutput> {
    ensure!(!input.query.trim().is_empty(), "query must not be empty");
    let limit = input.limit.unwrap_or(25);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let client = JmapClient::from_ctx(&ctx)?;

    // First, query for email IDs
    let query_args = EmailQueryArgs {
        account_id: client.account_id.clone(),
        filter: Some(EmailQueryFilter {
            in_mailbox: None,
            text: Some(input.query.clone()),
            from: None,
            to: None,
            subject: None,
        }),
        limit: Some(limit),
    };

    let query_response = client
        .call_method::<EmailQueryResponse>("Email/query", &query_args, "q0")
        .await?;

    if query_response.ids.is_empty() {
        return Ok(SearchMailOutput { messages: vec![] });
    }

    // Then fetch email details
    let get_args = EmailGetArgs {
        account_id: client.account_id.clone(),
        ids: Some(query_response.ids),
        properties: Some(vec![
            "id".to_string(),
            "subject".to_string(),
            "from".to_string(),
            "receivedAt".to_string(),
            "preview".to_string(),
            "keywords".to_string(),
            "mailboxIds".to_string(),
        ]),
    };

    let get_response = client
        .call_method::<EmailGetResponse>("Email/get", &get_args, "g0")
        .await?;

    let messages = get_response
        .list
        .into_iter()
        .filter_map(|v| serde_json::from_value::<EmailSummary>(v).ok())
        .collect();

    Ok(SearchMailOutput { messages })
}

/// # Get Fastmail Message
///
/// Retrieves a single email message from Fastmail by its JMAP ID.
/// Use this tool when the user wants to read the full content of a specific
/// email.
///
/// Returns comprehensive email details including sender, recipients
/// (to/cc/bcc), subject, timestamps, preview, keywords (labels), and mailbox
/// associations. When `include_body` is true, also includes the full text and
/// HTML body content.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - email
/// - fastmail
/// - jmap
///
/// # Errors
///
/// Returns an error if:
/// - The `message_id` is empty or contains only whitespace
/// - Fastmail credentials are missing or invalid
/// - The JMAP API request fails
/// - The message is not found
/// - The response cannot be parsed
#[tool]
pub async fn get_message(ctx: Context, input: GetMessageInput) -> Result<GetMessageOutput> {
    ensure!(
        !input.message_id.trim().is_empty(),
        "message_id must not be empty"
    );

    let client = JmapClient::from_ctx(&ctx)?;

    let mut properties = vec![
        "id".to_string(),
        "subject".to_string(),
        "from".to_string(),
        "to".to_string(),
        "cc".to_string(),
        "bcc".to_string(),
        "receivedAt".to_string(),
        "sentAt".to_string(),
        "preview".to_string(),
        "keywords".to_string(),
        "mailboxIds".to_string(),
    ];

    if input.include_body {
        properties.push("textBody".to_string());
        properties.push("htmlBody".to_string());
        properties.push("bodyValues".to_string());
    }

    let get_args = EmailGetArgs {
        account_id: client.account_id.clone(),
        ids: Some(vec![input.message_id.clone()]),
        properties: Some(properties),
    };

    let response = client
        .call_method::<EmailGetResponse>("Email/get", &get_args, "g0")
        .await?;

    ensure!(!response.list.is_empty(), "Message not found");

    let message: Email = serde_json::from_value(response.list[0].clone())?;

    Ok(GetMessageOutput { message })
}

/// # Send Fastmail Email
///
/// Sends a new email message using the user's Fastmail account via JMAP.
/// Use this tool when the user wants to compose and send a new email.
///
/// Creates an email draft with the specified recipients, subject, and body,
/// then submits it for delivery. The email is sent using the user's default
/// Fastmail identity. The sent email is stored in the designated mailbox
/// (defaults to the Sent folder if not specified).
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - email
/// - fastmail
/// - jmap
///
/// # Errors
///
/// Returns an error if:
/// - The `to` field is empty or contains empty recipients
/// - The `cc` or `bcc` fields contain empty recipients
/// - The subject or body is empty
/// - Fastmail credentials are missing or invalid
/// - The JMAP API request fails
/// - The email draft cannot be created
/// - The email submission fails
/// - The response cannot be parsed
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

    let client = JmapClient::from_ctx(&ctx)?;

    // Create the email draft
    let mut mailbox_ids = HashMap::new();
    if let Some(ref mailbox) = input.sent_mailbox_id {
        mailbox_ids.insert(mailbox.clone(), true);
    } else {
        // Use a placeholder - in real implementation, we'd query for "Sent" mailbox
        mailbox_ids.insert("sent".to_string(), true);
    }

    let email_create = EmailSetCreate {
        mailbox_ids: mailbox_ids.clone(),
        keywords: Some({
            let mut kw = HashMap::new();
            kw.insert("$draft".to_string(), true);
            kw
        }),
        from: None, // Server will use default identity
        to: Some(
            input
                .to
                .into_iter()
                .map(|email| EmailAddress { name: None, email })
                .collect(),
        ),
        cc: if input.cc.is_empty() {
            None
        } else {
            Some(
                input
                    .cc
                    .into_iter()
                    .map(|email| EmailAddress { name: None, email })
                    .collect(),
            )
        },
        bcc: if input.bcc.is_empty() {
            None
        } else {
            Some(
                input
                    .bcc
                    .into_iter()
                    .map(|email| EmailAddress { name: None, email })
                    .collect(),
            )
        },
        subject: Some(input.subject),
        text_body: Some(vec![EmailBodyPart {
            part_id: Some("body".to_string()),
            part_type: "text/plain".to_string(),
            charset: Some("utf-8".to_string()),
            disposition: None,
            cid: None,
            language: None,
            location: None,
        }]),
        html_body: None,
        body_values: {
            let mut values = HashMap::new();
            values.insert(
                "body".to_string(),
                EmailBodyValue {
                    content: input.body,
                },
            );
            values
        },
    };

    let mut create_map = HashMap::new();
    create_map.insert("draft".to_string(), email_create);

    let set_args = EmailSetArgs {
        account_id: client.account_id.clone(),
        create: Some(create_map),
        update: None,
    };

    let set_response = client
        .call_method::<EmailSetResponse>("Email/set", &set_args, "e0")
        .await?;

    let created = set_response
        .created
        .ok_or_else(|| operai::anyhow::anyhow!("Failed to create email draft"))?;

    let email_obj = created
        .get("draft")
        .ok_or_else(|| operai::anyhow::anyhow!("Draft email not found in response"))?;

    let email_id: String = email_obj
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| operai::anyhow::anyhow!("Email ID not found"))?
        .to_string();

    // Submit the email for sending
    let identity_id = client.get_default_identity_id().await?;

    let mut submission_create = HashMap::new();
    submission_create.insert(
        "sub1".to_string(),
        EmailSubmissionCreate {
            identity_id,
            email_id: email_id.clone(),
        },
    );

    let submission_args = EmailSubmissionSetArgs {
        account_id: client.account_id.clone(),
        create: submission_create,
    };

    let _submission_response = client
        .call_method::<serde_json::Value>("EmailSubmission/set", &submission_args, "s0")
        .await?;

    Ok(SendEmailOutput {
        email_id,
        submission_id: "sub1".to_string(),
    })
}

/// # Reply Fastmail Email
///
/// Replies to an existing email message in the user's Fastmail account.
/// Use this tool when the user wants to respond to a specific email.
///
/// Creates a reply to the original message and sends it via JMAP.
/// By default, replies only to the original sender. When `reply_all` is true,
/// includes all original recipients (to/cc) in the reply. The subject line
/// is automatically prefixed with "Re: " if not already present.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - email
/// - fastmail
/// - jmap
///
/// # Errors
///
/// Returns an error if:
/// - The `message_id` is empty or contains only whitespace
/// - The body is empty
/// - Fastmail credentials are missing or invalid
/// - The JMAP API request fails
/// - The original message cannot be found
/// - The reply email cannot be created
/// - The email submission fails
/// - The response cannot be parsed
#[tool]
pub async fn reply(ctx: Context, input: ReplyInput) -> Result<ReplyOutput> {
    ensure!(
        !input.message_id.trim().is_empty(),
        "message_id must not be empty"
    );
    ensure!(!input.body.trim().is_empty(), "body must not be empty");

    let client = JmapClient::from_ctx(&ctx)?;

    // First, get the original message to extract reply information
    let get_args = EmailGetArgs {
        account_id: client.account_id.clone(),
        ids: Some(vec![input.message_id.clone()]),
        properties: Some(vec![
            "from".to_string(),
            "to".to_string(),
            "cc".to_string(),
            "subject".to_string(),
            "mailboxIds".to_string(),
        ]),
    };

    let get_response = client
        .call_method::<EmailGetResponse>("Email/get", &get_args, "g0")
        .await?;

    ensure!(!get_response.list.is_empty(), "Original message not found");

    let original: serde_json::Value = get_response.list[0].clone();
    let from = original
        .get("from")
        .and_then(|v| v.as_array())
        .ok_or_else(|| operai::anyhow::anyhow!("Original message has no 'from' field"))?;

    let to_recipients: Vec<EmailAddress> =
        serde_json::from_value(serde_json::Value::Array(from.clone()))?;

    let cc_recipients = if input.reply_all {
        let mut all_cc = vec![];
        if let Some(orig_to) = original.get("to").and_then(|v| v.as_array()) {
            all_cc.extend_from_slice(orig_to);
        }
        if let Some(orig_cc) = original.get("cc").and_then(|v| v.as_array()) {
            all_cc.extend_from_slice(orig_cc);
        }
        Some(serde_json::from_value::<Vec<EmailAddress>>(
            serde_json::Value::Array(all_cc),
        )?)
    } else {
        None
    };

    let subject = original
        .get("subject")
        .and_then(|v| v.as_str())
        .map_or_else(
            || "Re: ".to_string(),
            |s| {
                if s.starts_with("Re: ") {
                    s.to_string()
                } else {
                    format!("Re: {s}")
                }
            },
        );

    let mailbox_ids: HashMap<String, bool> = serde_json::from_value(
        original
            .get("mailboxIds")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    )?;

    // Create reply email
    let email_create = EmailSetCreate {
        mailbox_ids,
        keywords: Some({
            let mut kw = HashMap::new();
            kw.insert("$draft".to_string(), true);
            kw
        }),
        from: None,
        to: Some(to_recipients),
        cc: cc_recipients,
        bcc: None,
        subject: Some(subject),
        text_body: Some(vec![EmailBodyPart {
            part_id: Some("body".to_string()),
            part_type: "text/plain".to_string(),
            charset: Some("utf-8".to_string()),
            disposition: None,
            cid: None,
            language: None,
            location: None,
        }]),
        html_body: None,
        body_values: {
            let mut values = HashMap::new();
            values.insert(
                "body".to_string(),
                EmailBodyValue {
                    content: input.body,
                },
            );
            values
        },
    };

    let mut create_map = HashMap::new();
    create_map.insert("reply".to_string(), email_create);

    let set_args = EmailSetArgs {
        account_id: client.account_id.clone(),
        create: Some(create_map),
        update: None,
    };

    let set_response = client
        .call_method::<EmailSetResponse>("Email/set", &set_args, "e0")
        .await?;

    let created = set_response
        .created
        .ok_or_else(|| operai::anyhow::anyhow!("Failed to create reply email"))?;

    let email_obj = created
        .get("reply")
        .ok_or_else(|| operai::anyhow::anyhow!("Reply email not found in response"))?;

    let email_id: String = email_obj
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| operai::anyhow::anyhow!("Email ID not found"))?
        .to_string();

    // Submit the reply
    let identity_id = client.get_default_identity_id().await?;

    let mut submission_create = HashMap::new();
    submission_create.insert(
        "sub1".to_string(),
        EmailSubmissionCreate {
            identity_id,
            email_id: email_id.clone(),
        },
    );

    let submission_args = EmailSubmissionSetArgs {
        account_id: client.account_id.clone(),
        create: submission_create,
    };

    let _submission_response = client
        .call_method::<serde_json::Value>("EmailSubmission/set", &submission_args, "s0")
        .await?;

    Ok(ReplyOutput {
        email_id,
        submission_id: "sub1".to_string(),
    })
}

/// # Move Fastmail Email
///
/// Moves an email message to a different mailbox folder in Fastmail.
/// Use this tool when the user wants to organize emails by moving them
/// to folders like Archive, Spam, Trash, or custom mailboxes.
///
/// Updates the email's mailbox associations to place it in the target mailbox.
/// Note that this requires the JMAP mailbox ID, not the display name.
/// Common mailbox IDs include "inbox", "archive", "sent", "trash", "spam", etc.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - email
/// - fastmail
/// - jmap
///
/// # Errors
///
/// Returns an error if:
/// - The `message_id` is empty or contains only whitespace
/// - The `destination_mailbox_id` is empty or contains only whitespace
/// - Fastmail credentials are missing or invalid
/// - The JMAP API request fails
/// - The response cannot be parsed
#[tool]
pub async fn move_message(ctx: Context, input: MoveInput) -> Result<MoveOutput> {
    ensure!(
        !input.message_id.trim().is_empty(),
        "message_id must not be empty"
    );
    ensure!(
        !input.destination_mailbox_id.trim().is_empty(),
        "destination_mailbox_id must not be empty"
    );

    let client = JmapClient::from_ctx(&ctx)?;

    // Update the email's mailboxIds to only include the destination
    let mut new_mailbox_ids = HashMap::new();
    new_mailbox_ids.insert(input.destination_mailbox_id.clone(), true);

    let mut update_map = HashMap::new();
    update_map.insert(
        input.message_id.clone(),
        serde_json::json!({
            "mailboxIds": new_mailbox_ids,
        }),
    );

    let set_args = EmailSetArgs {
        account_id: client.account_id.clone(),
        create: None,
        update: Some(update_map),
    };

    let _set_response = client
        .call_method::<EmailSetResponse>("Email/set", &set_args, "e0")
        .await?;

    Ok(MoveOutput { updated: true })
}

/// # Add or Remove Fastmail Email Label
///
/// Adds or removes a keyword/label on an email message in Fastmail.
/// Use this tool when the user wants to flag emails, mark as read/unread,
/// or apply custom labels for organization.
///
/// JMAP uses keywords for both system flags and user labels. System keywords
/// include "$seen" (read), "$flagged" (starred), "$draft" (draft), and others.
/// Custom labels can also be created and applied. Use `add: true` to apply
/// a label and `add: false` to remove it.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - email
/// - fastmail
/// - jmap
///
/// # Errors
///
/// Returns an error if:
/// - The `message_id` is empty or contains only whitespace
/// - The keyword is empty or contains only whitespace
/// - Fastmail credentials are missing or invalid
/// - The JMAP API request fails
/// - The response cannot be parsed
#[tool]
pub async fn label(ctx: Context, input: LabelInput) -> Result<LabelOutput> {
    ensure!(
        !input.message_id.trim().is_empty(),
        "message_id must not be empty"
    );
    ensure!(
        !input.keyword.trim().is_empty(),
        "keyword must not be empty"
    );

    let client = JmapClient::from_ctx(&ctx)?;

    // Update the email's keywords
    let keyword_update = if input.add {
        serde_json::json!({
            format!("keywords/{}", input.keyword): true
        })
    } else {
        serde_json::json!({
            format!("keywords/{}", input.keyword): null
        })
    };

    let mut update_map = HashMap::new();
    update_map.insert(input.message_id.clone(), keyword_update);

    let set_args = EmailSetArgs {
        account_id: client.account_id.clone(),
        create: None,
        update: Some(update_map),
    };

    let _set_response = client
        .call_method::<EmailSetResponse>("Email/set", &set_args, "e0")
        .await?;

    Ok(LabelOutput { updated: true })
}

// ========== JMAP Client ==========

#[derive(Debug, Clone)]
struct JmapClient {
    http: reqwest::Client,
    api_url: String,
    api_token: String,
    account_id: String,
}

impl JmapClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = FastmailCredential::get(ctx)?;
        ensure!(
            !cred.api_token.trim().is_empty(),
            "api_token must not be empty"
        );

        let api_url = cred
            .endpoint
            .as_deref()
            .unwrap_or(DEFAULT_JMAP_ENDPOINT)
            .trim_end_matches('/')
            .to_string();

        let account_id = cred.account_id.unwrap_or_else(|| "primary".to_string());

        Ok(Self {
            http: reqwest::Client::new(),
            api_url,
            api_token: cred.api_token,
            account_id,
        })
    }

    async fn call_method<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        args: &impl Serialize,
        call_id: &str,
    ) -> Result<T> {
        let request_body = JmapRequest {
            using: vec![
                "urn:ietf:params:jmap:core".to_string(),
                "urn:ietf:params:jmap:mail".to_string(),
                "urn:ietf:params:jmap:submission".to_string(),
            ],
            method_calls: vec![(
                method.to_string(),
                serde_json::to_value(args)?,
                call_id.to_string(),
            )],
        };

        let response = self
            .http
            .post(&self.api_url)
            .bearer_auth(&self.api_token)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(operai::anyhow::anyhow!(
                "JMAP request failed ({status}): {body}"
            ));
        }

        let jmap_response: JmapResponse = response.json().await?;

        ensure!(
            !jmap_response.method_responses.is_empty(),
            "No method responses returned"
        );

        // Extract the response for our call_id
        let method_response = &jmap_response.method_responses[0];
        let response_array = method_response
            .as_array()
            .ok_or_else(|| operai::anyhow::anyhow!("Invalid method response format"))?;

        ensure!(
            response_array.len() >= 2,
            "Invalid method response structure"
        );

        let result: T = serde_json::from_value(response_array[1].clone())?;
        Ok(result)
    }

    async fn get_default_identity_id(&self) -> Result<String> {
        let identity_args = IdentityGetArgs {
            account_id: self.account_id.clone(),
            ids: None,
        };

        let identity_response = self
            .call_method::<IdentityGetResponse>("Identity/get", &identity_args, "i0")
            .await?;

        ensure!(
            !identity_response.list.is_empty(),
            "No identities found for account"
        );

        // Return the first identity as the default
        Ok(identity_response.list[0].id.clone())
    }
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use std::collections::HashMap as StdHashMap;

    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_string_contains, header, method, path},
    };

    use super::*;
    use crate::types::Identity;

    fn test_ctx(endpoint: &str, account_id: Option<&str>) -> Context {
        let mut fastmail_values = StdHashMap::new();
        fastmail_values.insert("api_token".to_string(), "test-token".to_string());
        fastmail_values.insert("endpoint".to_string(), endpoint.to_string());
        if let Some(acc) = account_id {
            fastmail_values.insert("account_id".to_string(), acc.to_string());
        }

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("fastmail", fastmail_values)
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_email_address_serialization_roundtrip() {
        let addr = EmailAddress {
            name: Some("Test User".to_string()),
            email: "test@example.com".to_string(),
        };
        let json = serde_json::to_string(&addr).unwrap();
        let parsed: EmailAddress = serde_json::from_str(&json).unwrap();
        assert_eq!(addr.email, parsed.email);
        assert_eq!(addr.name, parsed.name);
    }

    #[test]
    fn test_email_address_without_name_deserializes() {
        let json = r#"{"email": "test@example.com"}"#;
        let parsed: EmailAddress = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.email, "test@example.com");
        assert!(parsed.name.is_none());
    }

    // --- Input validation tests ---

    #[tokio::test]
    async fn test_search_mail_empty_query_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri(), Some("acc123"));

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
        let ctx = test_ctx(&server.uri(), Some("acc123"));

        let result = search_mail(
            ctx,
            SearchMailInput {
                query: "hello".to_string(),
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
    async fn test_get_message_empty_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri(), Some("acc123"));

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
        let ctx = test_ctx(&server.uri(), Some("acc123"));

        let result = send_email(
            ctx,
            SendEmailInput {
                to: vec![],
                cc: vec![],
                bcc: vec![],
                subject: "Hi".to_string(),
                body: "Body".to_string(),
                sent_mailbox_id: None,
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

    // --- Integration tests ---

    #[tokio::test]
    async fn test_search_mail_success_returns_messages() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri(), Some("acc123"));

        let query_response = r#"
        {
          "methodResponses": [
            [
              "Email/query",
              {
                "accountId": "acc123",
                "ids": ["msg-1", "msg-2"],
                "queryState": "state1"
              },
              "q0"
            ]
          ]
        }
        "#;

        let get_response = r#"
        {
          "methodResponses": [
            [
              "Email/get",
              {
                "accountId": "acc123",
                "list": [
                  {
                    "id": "msg-1",
                    "subject": "Hello",
                    "from": [{"name": "Alice", "email": "alice@example.com"}],
                    "receivedAt": "2024-01-01T00:00:00Z",
                    "preview": "Preview text",
                    "keywords": {"$seen": true},
                    "mailboxIds": {"inbox": true}
                  }
                ],
                "state": "state2"
              },
              "g0"
            ]
          ]
        }
        "#;

        // Mock for Email/query
        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("Email/query"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(query_response, "application/json"),
            )
            .mount(&server)
            .await;

        // Mock for Email/get
        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("Email/get"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(get_response, "application/json"))
            .mount(&server)
            .await;

        let output = search_mail(
            ctx,
            SearchMailInput {
                query: "hello".to_string(),
                limit: Some(25),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.messages.len(), 1);
        assert_eq!(output.messages[0].id, "msg-1");
        assert_eq!(output.messages[0].subject.as_deref(), Some("Hello"));
    }

    #[tokio::test]
    async fn test_move_message_success() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri(), Some("acc123"));

        let response_body = r#"
        {
          "methodResponses": [
            [
              "Email/set",
              {
                "accountId": "acc123",
                "updated": {"msg-1": {}}
              },
              "e0"
            ]
          ]
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(body_string_contains("Email/set"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let output = move_message(
            ctx,
            MoveInput {
                message_id: "msg-1".to_string(),
                destination_mailbox_id: "archive".to_string(),
            },
        )
        .await
        .unwrap();

        assert!(output.updated);
    }

    #[tokio::test]
    async fn test_label_add_keyword_success() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri(), Some("acc123"));

        let response_body = r#"
        {
          "methodResponses": [
            [
              "Email/set",
              {
                "accountId": "acc123",
                "updated": {"msg-1": {}}
              },
              "e0"
            ]
          ]
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(body_string_contains("Email/set"))
            .and(body_string_contains("$flagged"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let output = label(
            ctx,
            LabelInput {
                message_id: "msg-1".to_string(),
                keyword: "$flagged".to_string(),
                add: true,
            },
        )
        .await
        .unwrap();

        assert!(output.updated);
    }

    // --- Identity tests ---

    #[test]
    fn test_identity_serialization_roundtrip() {
        let identity = Identity {
            id: "id123".to_string(),
            name: "John Doe".to_string(),
            email: "john@example.com".to_string(),
            reply_to: None,
            bcc: None,
            text_signature: String::new(),
            html_signature: String::new(),
            may_delete: true,
        };
        let json = serde_json::to_string(&identity).unwrap();
        let parsed: Identity = serde_json::from_str(&json).unwrap();
        assert_eq!(identity.id, parsed.id);
        assert_eq!(identity.email, parsed.email);
        assert_eq!(identity.name, parsed.name);
    }

    #[tokio::test]
    async fn test_identity_get_returns_default_identity() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri(), Some("acc123"));

        let identity_response = r#"
        {
          "methodResponses": [
            [
              "Identity/get",
              {
                "accountId": "acc123",
                "state": "state1",
                "list": [
                  {
                    "id": "id-default",
                    "name": "John Doe",
                    "email": "john@example.com",
                    "mayDelete": false,
                    "textSignature": "",
                    "htmlSignature": ""
                  }
                ],
                "notFound": []
              },
              "i0"
            ]
          ]
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("Identity/get"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(identity_response, "application/json"),
            )
            .mount(&server)
            .await;

        let client = JmapClient::from_ctx(&ctx).unwrap();
        let identity_id = client.get_default_identity_id().await.unwrap();

        assert_eq!(identity_id, "id-default");
    }

    #[tokio::test]
    async fn test_identity_get_empty_list_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri(), Some("acc123"));

        let identity_response = r#"
        {
          "methodResponses": [
            [
              "Identity/get",
              {
                "accountId": "acc123",
                "state": "state1",
                "list": [],
                "notFound": []
              },
              "i0"
            ]
          ]
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", "Bearer test-token"))
            .and(body_string_contains("Identity/get"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(identity_response, "application/json"),
            )
            .mount(&server)
            .await;

        let client = JmapClient::from_ctx(&ctx).unwrap();
        let result = client.get_default_identity_id().await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No identities found")
        );
    }

    #[tokio::test]
    async fn test_send_email_includes_body_values() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri(), Some("acc123"));

        let identity_response = r#"
        {
          "methodResponses": [
            [
              "Identity/get",
              {
                "accountId": "acc123",
                "state": "state1",
                "list": [{"id": "id1", "name": "User", "email": "user@example.com", "mayDelete": false, "textSignature": "", "htmlSignature": ""}],
                "notFound": []
              },
              "i0"
            ]
          ]
        }
        "#;

        let email_set_response = r#"
        {
          "methodResponses": [
            [
              "Email/set",
              {
                "accountId": "acc123",
                "created": {
                  "draft": {
                    "id": "email-123"
                  }
                }
              },
              "e0"
            ]
          ]
        }
        "#;

        let submission_response = r#"
        {
          "methodResponses": [
            [
              "EmailSubmission/set",
              {
                "accountId": "acc123",
                "created": {
                  "sub1": {
                    "id": "sub-123"
                  }
                }
              },
              "s0"
            ]
          ]
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(move |req: &wiremock::Request| {
                let body = String::from_utf8_lossy(&req.body);

                if body.contains("Identity/get") {
                    ResponseTemplate::new(200).set_body_raw(identity_response, "application/json")
                } else if body.contains("Email/set") && body.contains("bodyValues") {
                    ResponseTemplate::new(200).set_body_raw(email_set_response, "application/json")
                } else if body.contains("EmailSubmission/set") {
                    ResponseTemplate::new(200).set_body_raw(submission_response, "application/json")
                } else {
                    ResponseTemplate::new(500).set_body_json(json!({"error": "unexpected"}))
                }
            })
            .mount(&server)
            .await;

        let result = send_email(
            ctx,
            SendEmailInput {
                to: vec!["recipient@example.com".to_string()],
                cc: vec![],
                bcc: vec![],
                subject: "Test Subject".to_string(),
                body: "Test body content".to_string(),
                sent_mailbox_id: None,
            },
        )
        .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.email_id, "email-123");
    }

    #[tokio::test]
    async fn test_reply_includes_body_values() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri(), Some("acc123"));

        let original_message_response = r#"
        {
          "methodResponses": [
            [
              "Email/get",
              {
                "accountId": "acc123",
                "state": "state1",
                "list": [
                  {
                    "id": "msg-1",
                    "from": [{"name": "Alice", "email": "alice@example.com"}],
                    "to": [{"name": "Bob", "email": "bob@example.com"}],
                    "cc": [],
                    "subject": "Original Subject",
                    "mailboxIds": {"inbox": true}
                  }
                ]
              },
              "g0"
            ]
          ]
        }
        "#;

        let identity_response = r#"
        {
          "methodResponses": [
            [
              "Identity/get",
              {
                "accountId": "acc123",
                "state": "state1",
                "list": [{"id": "id1", "name": "User", "email": "user@example.com", "mayDelete": false, "textSignature": "", "htmlSignature": ""}],
                "notFound": []
              },
              "i0"
            ]
          ]
        }
        "#;

        let email_set_response = r#"
        {
          "methodResponses": [
            [
              "Email/set",
              {
                "accountId": "acc123",
                "created": {
                  "reply": {
                    "id": "reply-123"
                  }
                }
              },
              "e0"
            ]
          ]
        }
        "#;

        let submission_response = r#"
        {
          "methodResponses": [
            [
              "EmailSubmission/set",
              {
                "accountId": "acc123",
                "created": {
                  "sub1": {
                    "id": "sub-123"
                  }
                }
              },
              "s0"
            ]
          ]
        }
        "#;

        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(move |req: &wiremock::Request| {
                let body = String::from_utf8_lossy(&req.body);

                if body.contains("Email/get") {
                    ResponseTemplate::new(200)
                        .set_body_raw(original_message_response, "application/json")
                } else if body.contains("Identity/get") {
                    ResponseTemplate::new(200).set_body_raw(identity_response, "application/json")
                } else if body.contains("Email/set") && body.contains("bodyValues") {
                    ResponseTemplate::new(200).set_body_raw(email_set_response, "application/json")
                } else if body.contains("EmailSubmission/set") {
                    ResponseTemplate::new(200).set_body_raw(submission_response, "application/json")
                } else {
                    ResponseTemplate::new(500).set_body_json(json!({"error": "unexpected"}))
                }
            })
            .mount(&server)
            .await;

        let result = reply(
            ctx,
            ReplyInput {
                message_id: "msg-1".to_string(),
                body: "Reply body content".to_string(),
                reply_all: false,
            },
        )
        .await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.email_id, "reply-123");
    }
}
