//! IMAP/SMTP email integration for Operai Toolbox.
//!
//! This integration provides tools for interacting with generic IMAP/SMTP email
//! servers:
//! - List folders (IMAP mailboxes)
//! - Fetch messages
//! - Send messages via SMTP
//! - Mark messages as read
//! - Delete messages
//!
//! Note: This is a reference implementation. For production use, you would need
//! to configure actual IMAP/SMTP server credentials.

use operai::{
    Context, JsonSchema, Result, define_system_credential, info, init, schemars, shutdown, tool,
};
use serde::{Deserialize, Serialize};

// =============================================================================
// Credentials
// =============================================================================

// IMAP credentials for reading email
define_system_credential! {
    ImapCredential("imap") {
        /// IMAP server hostname (e.g., "imap.gmail.com")
        host: String,
        /// IMAP server port (typically 993 for SSL)
        port: u16,
        /// Username for authentication
        username: String,
        /// Password or app-specific password
        password: String,
    }
}

// SMTP credentials for sending email
define_system_credential! {
    SmtpCredential("smtp") {
        /// SMTP server hostname (e.g., "smtp.gmail.com")
        host: String,
        /// SMTP server port (typically 587 for TLS or 465 for SSL)
        port: u16,
        /// Username for authentication
        username: String,
        /// Password or app-specific password
        password: String,
    }
}

/// Initialize the IMAP/SMTP tool library.
#[init]
async fn setup() -> Result<()> {
    info!("IMAP/SMTP integration initialized");
    Ok(())
}

/// Clean up resources when the library is unloaded.
#[shutdown]
fn cleanup() {
    info!("IMAP/SMTP integration shutting down");
}

// =============================================================================
// List Folders Tool
// =============================================================================

/// Input for the `list_folders` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListFoldersInput {
    /// Optional pattern to filter folders (e.g., "*" for all, "INBOX*" for
    /// inbox subtree).
    #[serde(default)]
    pub pattern: Option<String>,
}

/// A single email folder/mailbox.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Folder {
    /// The full path/name of the folder.
    pub name: String,
    /// Folder delimiter character (e.g., "/" or ".").
    pub delimiter: Option<String>,
    /// Folder attributes (e.g., `\\Noselect`, `\\HasChildren`).
    pub attributes: Vec<String>,
    /// Total number of messages in the folder (if available).
    pub total_messages: Option<u32>,
    /// Number of unread messages (if available).
    pub unread_count: Option<u32>,
}

/// Output from the `list_folders` tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ListFoldersOutput {
    /// List of folders/mailboxes.
    pub folders: Vec<Folder>,
    /// Total count of folders returned.
    pub count: usize,
}

/// # List IMAP Folders
///
/// Lists all IMAP folders/mailboxes available in the email account, providing
/// a hierarchical view of the mailbox structure. Use this tool when the user
/// wants to explore their email folder organization, discover available
/// folders, or check folder metadata (message counts, unread counts).
///
/// This tool supports optional pattern matching to filter folders (e.g.,
/// "INBOX*" to show only folders under the INBOX hierarchy). Returns detailed
/// information about each folder including name, delimiter, attributes, total
/// message count, and unread count.
///
/// ## When to use
/// - User asks to "show me my folders" or "list mailboxes"
/// - User wants to browse their email account structure
/// - User needs to know which folders are available before performing
///   operations
/// - User wants to check message counts or unread counts per folder
///
/// ## Key inputs
/// - `pattern` (optional): Wildcard pattern to filter folders (default: "*" for
///   all)
///
/// ## Key outputs
/// - List of folders with metadata (name, delimiter, attributes, counts)
/// - Total count of folders returned
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - email
/// - imap
///
/// # Errors
///
/// This function will not return an error in the current mock implementation.
/// In a real implementation, it could return an error if:
/// - IMAP credentials are missing or invalid
/// - The IMAP server connection fails
/// - The LIST command fails
#[tool]
pub async fn list_folders(_ctx: Context, input: ListFoldersInput) -> Result<ListFoldersOutput> {
    // Reference implementation - returns example data
    // In a real implementation, this would:
    // 1. Connect to the IMAP server using the ImapCredential
    // 2. Execute the LIST command with the provided pattern
    // 3. Parse and return the folder list

    let pattern = input.pattern.as_deref().unwrap_or("*");

    // Example data for demonstration
    let folders = vec![
        Folder {
            name: "INBOX".to_string(),
            delimiter: Some("/".to_string()),
            attributes: vec!["\\HasNoChildren".to_string()],
            total_messages: Some(10),
            unread_count: Some(2),
        },
        Folder {
            name: "Sent".to_string(),
            delimiter: Some("/".to_string()),
            attributes: vec!["\\HasNoChildren".to_string()],
            total_messages: Some(50),
            unread_count: Some(0),
        },
        Folder {
            name: "Drafts".to_string(),
            delimiter: Some("/".to_string()),
            attributes: vec!["\\HasNoChildren".to_string()],
            total_messages: Some(3),
            unread_count: Some(0),
        },
    ];

    let count = folders.len();
    info!("Listed {} folders matching pattern '{}'", count, pattern);

    Ok(ListFoldersOutput { folders, count })
}

// =============================================================================
// Fetch Message Tool
// =============================================================================

/// Input for the `fetch_message` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FetchMessageInput {
    /// The folder/mailbox to fetch from (e.g., "INBOX").
    pub folder: String,
    /// The message UID to fetch.
    pub uid: u32,
    /// Whether to include the full body (default: true).
    #[serde(default)]
    pub include_body: Option<bool>,
}

/// An email address with optional display name.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EmailAddress {
    /// Display name (e.g., "John Doe").
    pub name: Option<String>,
    /// Email address (e.g., "john@example.com").
    pub address: String,
}

/// An email attachment.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Attachment {
    /// Filename of the attachment.
    pub filename: String,
    /// MIME type (e.g., "application/pdf").
    pub content_type: String,
    /// Size in bytes.
    pub size: u64,
    /// Content ID for inline attachments.
    pub content_id: Option<String>,
}

/// Output from the `fetch_message` tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct FetchMessageOutput {
    /// Message UID.
    pub uid: u32,
    /// Message ID header.
    pub message_id: Option<String>,
    /// Subject of the message.
    pub subject: Option<String>,
    /// Sender of the message.
    pub from: Option<EmailAddress>,
    /// Recipients (To field).
    pub to: Vec<EmailAddress>,
    /// CC recipients.
    pub cc: Vec<EmailAddress>,
    /// Date the message was sent (RFC 2822 format).
    pub date: Option<String>,
    /// Plain text body content.
    pub body_text: Option<String>,
    /// HTML body content.
    pub body_html: Option<String>,
    /// List of attachments.
    pub attachments: Vec<Attachment>,
    /// Message flags (e.g., "\\Seen", "\\Flagged").
    pub flags: Vec<String>,
    /// Whether the message has been read.
    pub is_read: bool,
}

/// # Fetch IMAP Message
///
/// Fetches a single email message by UID from the specified IMAP folder,
/// returning complete message data including headers, body content, and
/// attachment metadata. Use this tool when the user wants to read the full
/// content of a specific email message.
///
/// This tool retrieves the complete message structure including sender,
/// recipients, subject, date, plain text and/or HTML body, attachment list,
/// and message flags (read, flagged, etc.). The UID is a unique identifier
/// for the message within the specified folder.
///
/// ## When to use
/// - User asks to "show me the email", "read message", or "fetch email"
/// - User wants to view the full content of a specific message
/// - User needs to see message headers, body, or attachments
/// - User provides a message UID or references a specific email
///
/// ## Key inputs
/// - `folder` (required): The IMAP folder name (e.g., "INBOX", "Sent")
/// - `uid` (required): Unique message identifier within the folder
/// - `include_body` (optional): Whether to fetch full body content (default:
///   true)
///
/// ## Key outputs
/// - Complete message headers (subject, from, to, cc, date, message-id)
/// - Plain text and/or HTML body content
/// - Attachment metadata (filename, content type, size)
/// - Message flags and read status
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - email
/// - imap
///
/// # Errors
///
/// This function will not return an error in the current mock implementation.
/// In a real implementation, it could return an error if:
/// - IMAP credentials are missing or invalid
/// - The IMAP server connection fails
/// - The folder cannot be selected
/// - The message UID does not exist
/// - The FETCH command fails
#[tool]
pub async fn fetch_message(_ctx: Context, input: FetchMessageInput) -> Result<FetchMessageOutput> {
    // Reference implementation - returns example data
    // In a real implementation, this would:
    // 1. Connect to the IMAP server
    // 2. Select the specified folder
    // 3. Execute UID FETCH with BODY[] to get message content
    // 4. Parse headers, body, and attachments using mailparse

    info!(
        "Fetching message UID {} from folder '{}'",
        input.uid, input.folder
    );

    // Example data for demonstration
    Ok(FetchMessageOutput {
        uid: input.uid,
        message_id: Some("<example@example.com>".to_string()),
        subject: Some("Example Email Subject".to_string()),
        from: Some(EmailAddress {
            name: Some("John Doe".to_string()),
            address: "john@example.com".to_string(),
        }),
        to: vec![EmailAddress {
            name: Some("Jane Smith".to_string()),
            address: "jane@example.com".to_string(),
        }],
        cc: vec![],
        date: Some("Mon, 01 Jan 2025 12:00:00 +0000".to_string()),
        body_text: Some("This is the plain text body of the email.".to_string()),
        body_html: Some("<p>This is the <strong>HTML</strong> body of the email.</p>".to_string()),
        attachments: vec![],
        flags: vec!["\\Seen".to_string()],
        is_read: true,
    })
}

// =============================================================================
// Send Message Tool
// =============================================================================

/// Input for the `send_message` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SendMessageInput {
    /// Recipients (To field).
    pub to: Vec<EmailAddress>,
    /// CC recipients.
    #[serde(default)]
    pub cc: Option<Vec<EmailAddress>>,
    /// BCC recipients.
    #[serde(default)]
    pub bcc: Option<Vec<EmailAddress>>,
    /// Email subject.
    pub subject: String,
    /// Plain text body.
    #[serde(default)]
    pub body_text: Option<String>,
    /// HTML body.
    #[serde(default)]
    pub body_html: Option<String>,
    /// Message ID to reply to (for threading).
    #[serde(default)]
    pub in_reply_to: Option<String>,
    /// References header (for threading).
    #[serde(default)]
    pub references: Option<Vec<String>>,
}

/// Output from the `send_message` tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SendMessageOutput {
    /// Whether the message was sent successfully.
    pub success: bool,
    /// Generated Message-ID for the sent message.
    pub message_id: String,
    /// Number of recipients the message was sent to.
    pub recipients_count: usize,
}

/// # Send SMTP Message
///
/// Sends an email message via SMTP, supporting plain text and/or HTML body
/// content, multiple recipients (to, cc, bcc), and message threading. Use this
/// tool when the user wants to compose and send a new email message or reply to
/// an existing message.
///
/// This tool creates and sends a complete email message with proper MIME
/// formatting, handling both plain text and HTML content. It supports message
/// threading through in-reply-to and references headers for email
/// conversations. At least one recipient and one body type (text or HTML) is
/// required.
///
/// ## When to use
/// - User asks to "send an email", "compose message", or "email someone"
/// - User wants to send a new message or reply to an existing email
/// - User provides recipients, subject, and message content
/// - User needs to send to multiple recipients (to, cc, bcc)
///
/// ## Key inputs
/// - `to` (required): List of primary recipients (email addresses)
/// - `subject` (required): Email subject line
/// - `body_text` or `body_html` (at least one required): Message content
/// - `cc` (optional): Carbon copy recipients
/// - `bcc` (optional): Blind carbon copy recipients
/// - `in_reply_to` (optional): Message-ID being replied to (for threading)
/// - `references` (optional): Thread reference headers
///
/// ## Key outputs
/// - Success status indicating if the message was sent
/// - Generated Message-ID for the sent message
/// - Total recipient count (to + cc + bcc)
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - email
/// - smtp
///
/// # Errors
///
/// This function will not return an error in the current mock implementation.
/// In a real implementation, it could return an error if:
/// - SMTP credentials are missing or invalid
/// - The SMTP server connection fails
/// - The message cannot be built (invalid headers, etc.)
/// - The SMTP server rejects the message
#[tool]
pub async fn send_message(_ctx: Context, input: SendMessageInput) -> Result<SendMessageOutput> {
    // Reference implementation - validates input and returns example response
    // In a real implementation, this would:
    // 1. Build the email message using lettre
    // 2. Connect to the SMTP server
    // 3. Authenticate and send the message
    // 4. Return the generated Message-ID

    if input.to.is_empty() {
        return Err(operai::anyhow::anyhow!(
            "At least one recipient is required"
        ));
    }

    if input.body_text.is_none() && input.body_html.is_none() {
        return Err(operai::anyhow::anyhow!("Email body is required"));
    }

    let recipients_count = input.to.len()
        + input.cc.as_ref().map_or(0, Vec::len)
        + input.bcc.as_ref().map_or(0, Vec::len);

    info!(
        "Sending email to {} recipients: {}",
        recipients_count, input.subject
    );

    // Generate a Message-ID for the sent message
    let message_id = format!(
        "<{}@example.com>",
        std::time::SystemTime::now()
            .elapsed()
            .unwrap_or_default()
            .as_secs()
    );

    Ok(SendMessageOutput {
        success: true,
        message_id,
        recipients_count,
    })
}

// =============================================================================
// Mark Read Tool
// =============================================================================

/// Input for the `mark_read` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct MarkReadInput {
    /// The folder containing the message.
    pub folder: String,
    /// The message UID(s) to mark.
    pub uids: Vec<u32>,
    /// Whether to mark as read (true) or unread (false). Defaults to true.
    #[serde(default)]
    pub read: Option<bool>,
}

/// Output from the `mark_read` tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct MarkReadOutput {
    /// Whether the operation was successful.
    pub success: bool,
    /// Number of messages updated.
    pub updated_count: usize,
    /// The new read state applied.
    pub marked_as_read: bool,
}

/// # Mark IMAP Message Read
///
/// Marks one or more email messages as read or unread by updating the IMAP
/// \\Seen flag. Use this tool when the user wants to mark messages as read to
/// clear unread indicators, or mark them as unread to keep them highlighted for
/// later attention.
///
/// This tool can process multiple messages in a single operation by accepting
/// a list of UIDs. It can both add the \\Seen flag (mark as read) or remove it
/// (mark as unread). This is useful for managing email workflow and organizing
/// which messages require attention.
///
/// ## When to use
/// - User asks to "mark as read", "mark as unread", or "mark messages"
/// - User wants to clear unread indicators for processed messages
/// - User wants to flag messages for later review by marking as unread
/// - User needs to bulk update read status for multiple messages
///
/// ## Key inputs
/// - `folder` (required): The IMAP folder containing the messages
/// - `uids` (required): List of message UIDs to update
/// - `read` (optional): true to mark as read, false to mark as unread (default:
///   true)
///
/// ## Key outputs
/// - Success status indicating if the update was applied
/// - Count of messages that were updated
/// - The read state that was applied (true = read, false = unread)
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - email
/// - imap
///
/// # Errors
///
/// This function will not return an error in the current mock implementation.
/// In a real implementation, it could return an error if:
/// - IMAP credentials are missing or invalid
/// - The IMAP server connection fails
/// - The folder cannot be selected
/// - The message UIDs do not exist
/// - The STORE command fails
#[tool]
pub async fn mark_read(_ctx: Context, input: MarkReadInput) -> Result<MarkReadOutput> {
    // Reference implementation
    // In a real implementation, this would:
    // 1. Connect to the IMAP server
    // 2. Select the folder
    // 3. Execute UID STORE with +FLAGS (\\Seen) or -FLAGS (\\Seen)

    let marked_as_read = input.read.unwrap_or(true);
    let updated_count = input.uids.len();

    info!(
        "Marking {} messages in '{}' as {}",
        updated_count,
        input.folder,
        if marked_as_read { "read" } else { "unread" }
    );

    Ok(MarkReadOutput {
        success: true,
        updated_count,
        marked_as_read,
    })
}

// =============================================================================
// Delete Message Tool
// =============================================================================

/// Input for the `delete_message` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteMessageInput {
    /// The folder containing the message(s).
    pub folder: String,
    /// The message UID(s) to delete.
    pub uids: Vec<u32>,
    /// If true, permanently delete (expunge). If false, move to Trash. Defaults
    /// to false.
    #[serde(default)]
    pub permanent: Option<bool>,
}

/// Output from the `delete_message` tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct DeleteMessageOutput {
    /// Whether the operation was successful.
    pub success: bool,
    /// Number of messages deleted.
    pub deleted_count: usize,
    /// Whether messages were permanently deleted or moved to trash.
    pub permanently_deleted: bool,
}

/// # Delete IMAP Message
///
/// Deletes one or more email messages from a specified IMAP folder, with the
/// option to move them to Trash or permanently remove them. Use this tool when
/// the user wants to delete unwanted messages, clean up spam, or permanently
/// remove sensitive emails.
///
/// This tool supports both soft delete (move to Trash folder for recovery)
/// and hard delete (permanent removal with no recovery). By default, messages
/// are moved to Trash unless permanent deletion is explicitly requested.
/// Multiple messages can be deleted in a single operation.
///
/// ## When to use
/// - User asks to "delete email", "remove message", or "trash email"
/// - User wants to permanently delete sensitive information
/// - User needs to clean up spam or unwanted messages
/// - User requests to bulk delete multiple messages
///
/// ## Key inputs
/// - `folder` (required): The IMAP folder containing the messages
/// - `uids` (required): List of message UIDs to delete
/// - `permanent` (optional): true to permanently delete, false to move to Trash
///   (default: false)
///
/// ## Key outputs
/// - Success status indicating if the deletion was completed
/// - Count of messages that were deleted
/// - Whether messages were permanently deleted or moved to trash
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - email
/// - imap
///
/// # Errors
///
/// This function will not return an error in the current mock implementation.
/// In a real implementation, it could return an error if:
/// - IMAP credentials are missing or invalid
/// - The IMAP server connection fails
/// - The folder cannot be selected
/// - The message UIDs do not exist
/// - The STORE or EXPUNGE commands fail
#[tool]
pub async fn delete_message(
    _ctx: Context,
    input: DeleteMessageInput,
) -> Result<DeleteMessageOutput> {
    // Reference implementation
    // In a real implementation, this would:
    // 1. Connect to the IMAP server
    // 2. Select the folder
    // 3. If permanent: UID STORE +FLAGS (\\Deleted) and EXPUNGE
    // 4. If not permanent: Try to COPY to Trash, then mark as deleted

    let permanently_deleted = input.permanent.unwrap_or(false);
    let deleted_count = input.uids.len();

    info!(
        "Deleting {} messages from '{}' (permanent: {})",
        deleted_count, input.folder, permanently_deleted
    );

    Ok(DeleteMessageOutput {
        success: true,
        deleted_count,
        permanently_deleted,
    })
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // =========================================================================
    // List Folders Tests
    // =========================================================================

    #[test]
    fn test_list_folders_input_deserializes_without_pattern() {
        let input_json = "{}";
        let input: ListFoldersInput = serde_json::from_str(input_json).unwrap();
        assert!(input.pattern.is_none());
    }

    #[test]
    fn test_list_folders_output_serializes_correctly() {
        let output = ListFoldersOutput {
            folders: vec![Folder {
                name: "INBOX".to_string(),
                delimiter: Some("/".to_string()),
                attributes: vec!["\\HasNoChildren".to_string()],
                total_messages: Some(10),
                unread_count: Some(2),
            }],
            count: 1,
        };

        let json = serde_json::to_value(&output).unwrap();
        assert_eq!(json["count"], 1);
        assert_eq!(json["folders"][0]["name"], "INBOX");
    }

    // =========================================================================
    // Fetch Message Tests
    // =========================================================================

    #[test]
    fn test_fetch_message_input_deserializes_with_required_fields() {
        let input_json = r#"{"folder": "INBOX", "uid": 100}"#;
        let input: FetchMessageInput = serde_json::from_str(input_json).unwrap();
        assert_eq!(input.folder, "INBOX");
        assert_eq!(input.uid, 100);
        assert!(input.include_body.is_none());
    }

    #[test]
    fn test_fetch_message_input_missing_folder_returns_error() {
        let input_json = r#"{"uid": 100}"#;
        let err = serde_json::from_str::<FetchMessageInput>(input_json).unwrap_err();
        assert!(err.to_string().contains("missing field `folder`"));
    }

    #[test]
    fn test_fetch_message_input_missing_uid_returns_error() {
        let input_json = r#"{"folder": "INBOX"}"#;
        let err = serde_json::from_str::<FetchMessageInput>(input_json).unwrap_err();
        assert!(err.to_string().contains("missing field `uid`"));
    }

    #[test]
    fn test_email_address_serialization_roundtrip() {
        let addr = EmailAddress {
            name: Some("John Doe".to_string()),
            address: "john@example.com".to_string(),
        };

        let json = serde_json::to_string(&addr).unwrap();
        let parsed: EmailAddress = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name, Some("John Doe".to_string()));
        assert_eq!(parsed.address, "john@example.com");
    }

    #[test]
    fn test_email_address_without_name_serialization_roundtrip() {
        let addr = EmailAddress {
            name: None,
            address: "jane@example.com".to_string(),
        };

        let json = serde_json::to_string(&addr).unwrap();
        let parsed: EmailAddress = serde_json::from_str(&json).unwrap();

        assert!(parsed.name.is_none());
        assert_eq!(parsed.address, "jane@example.com");
    }

    #[test]
    fn test_fetch_message_output_serialization() {
        let output = FetchMessageOutput {
            uid: 123,
            message_id: Some("<test@example.com>".to_string()),
            subject: Some("Test Subject".to_string()),
            from: Some(EmailAddress {
                name: Some("Sender".to_string()),
                address: "sender@example.com".to_string(),
            }),
            to: vec![EmailAddress {
                name: None,
                address: "recipient@example.com".to_string(),
            }],
            cc: vec![],
            date: Some("Mon, 01 Jan 2025 12:00:00 +0000".to_string()),
            body_text: Some("Plain text body".to_string()),
            body_html: None,
            attachments: vec![],
            flags: vec!["\\Seen".to_string()],
            is_read: true,
        };

        let json = serde_json::to_value(&output).unwrap();
        assert_eq!(json["uid"], 123);
        assert_eq!(json["subject"], "Test Subject");
        assert_eq!(json["from"]["name"], "Sender");
        assert_eq!(json["from"]["address"], "sender@example.com");
    }

    // =========================================================================
    // Send Message Tests
    // =========================================================================

    #[test]
    fn test_send_message_input_deserializes_minimal() {
        let input_json = r#"{
            "to": [{"address": "alice@example.com"}],
            "subject": "Hello"
        }"#;

        let input: SendMessageInput = serde_json::from_str(input_json).unwrap();

        assert_eq!(input.to.len(), 1);
        assert_eq!(input.subject, "Hello");
        assert!(input.body_text.is_none());
    }

    #[test]
    fn test_send_message_input_with_all_fields() {
        let input_json = r#"{
            "to": [{"address": "alice@example.com"}],
            "cc": [{"address": "bob@example.com"}],
            "bcc": [{"address": "carol@example.com"}],
            "subject": "Hello",
            "body_text": "Plain text body",
            "body_html": "<p>HTML body</p>"
        }"#;

        let input: SendMessageInput = serde_json::from_str(input_json).unwrap();

        assert_eq!(input.to.len(), 1);
        assert_eq!(input.cc.as_ref().map_or(0, Vec::len), 1);
        assert_eq!(input.bcc.as_ref().map_or(0, Vec::len), 1);
        assert_eq!(input.subject, "Hello");
        assert_eq!(input.body_text, Some("Plain text body".to_string()));
        assert_eq!(input.body_html, Some("<p>HTML body</p>".to_string()));
    }

    #[test]
    fn test_send_message_input_missing_to_returns_error() {
        let input_json = r#"{"subject": "Hello"}"#;
        let err = serde_json::from_str::<SendMessageInput>(input_json).unwrap_err();
        assert!(err.to_string().contains("missing field `to`"));
    }

    #[test]
    fn test_send_message_input_missing_subject_returns_error() {
        let input_json = r#"{"to": [{"address": "alice@example.com"}]}"#;
        let err = serde_json::from_str::<SendMessageInput>(input_json).unwrap_err();
        assert!(err.to_string().contains("missing field `subject`"));
    }

    #[test]
    fn test_send_message_output_serializes() {
        let output = SendMessageOutput {
            success: true,
            message_id: "<test@example.com>".to_string(),
            recipients_count: 2,
        };

        let json = serde_json::to_value(&output).unwrap();
        assert_eq!(
            json,
            json!({
                "success": true,
                "message_id": "<test@example.com>",
                "recipients_count": 2
            })
        );
    }

    // =========================================================================
    // Mark Read Tests
    // =========================================================================

    #[test]
    fn test_mark_read_input_deserializes() {
        let input_json = r#"{"folder": "INBOX", "uids": [1, 2, 3]}"#;
        let input: MarkReadInput = serde_json::from_str(input_json).unwrap();
        assert_eq!(input.folder, "INBOX");
        assert_eq!(input.uids, vec![1, 2, 3]);
        assert!(input.read.is_none());
    }

    #[test]
    fn test_mark_read_input_with_read_flag() {
        let input_json = r#"{"folder": "INBOX", "uids": [1], "read": false}"#;
        let input: MarkReadInput = serde_json::from_str(input_json).unwrap();
        assert_eq!(input.read, Some(false));
    }

    #[test]
    fn test_mark_read_output_serializes() {
        let output = MarkReadOutput {
            success: true,
            updated_count: 3,
            marked_as_read: true,
        };

        let json = serde_json::to_value(&output).unwrap();
        assert_eq!(
            json,
            json!({
                "success": true,
                "updated_count": 3,
                "marked_as_read": true
            })
        );
    }

    #[test]
    fn test_mark_read_input_missing_folder_returns_error() {
        let input_json = r#"{"uids": [1, 2, 3]}"#;
        let err = serde_json::from_str::<MarkReadInput>(input_json).unwrap_err();
        assert!(err.to_string().contains("missing field `folder`"));
    }

    #[test]
    fn test_mark_read_input_missing_uids_returns_error() {
        let input_json = r#"{"folder": "INBOX"}"#;
        let err = serde_json::from_str::<MarkReadInput>(input_json).unwrap_err();
        assert!(err.to_string().contains("missing field `uids`"));
    }

    // =========================================================================
    // Delete Message Tests
    // =========================================================================

    #[test]
    fn test_delete_message_input_deserializes() {
        let input_json = r#"{"folder": "INBOX", "uids": [100, 200]}"#;
        let input: DeleteMessageInput = serde_json::from_str(input_json).unwrap();
        assert_eq!(input.folder, "INBOX");
        assert_eq!(input.uids, vec![100, 200]);
        assert!(input.permanent.is_none());
    }

    #[test]
    fn test_delete_message_input_with_permanent_flag() {
        let input_json = r#"{"folder": "INBOX", "uids": [100], "permanent": true}"#;
        let input: DeleteMessageInput = serde_json::from_str(input_json).unwrap();
        assert_eq!(input.permanent, Some(true));
    }

    #[test]
    fn test_delete_message_output_serializes() {
        let output = DeleteMessageOutput {
            success: true,
            deleted_count: 2,
            permanently_deleted: false,
        };

        let json = serde_json::to_value(&output).unwrap();
        assert_eq!(
            json,
            json!({
                "success": true,
                "deleted_count": 2,
                "permanently_deleted": false
            })
        );
    }

    #[test]
    fn test_delete_message_input_missing_folder_returns_error() {
        let input_json = r#"{"uids": [100, 200]}"#;
        let err = serde_json::from_str::<DeleteMessageInput>(input_json).unwrap_err();
        assert!(err.to_string().contains("missing field `folder`"));
    }

    #[test]
    fn test_delete_message_input_missing_uids_returns_error() {
        let input_json = r#"{"folder": "INBOX"}"#;
        let err = serde_json::from_str::<DeleteMessageInput>(input_json).unwrap_err();
        assert!(err.to_string().contains("missing field `uids`"));
    }

    // =========================================================================
    // Credential Tests
    // =========================================================================

    #[test]
    fn test_imap_credential_deserializes_with_all_fields() {
        let json = r#"{
            "host": "imap.example.com",
            "port": 993,
            "username": "user@example.com",
            "password": "secret123"
        }"#;

        let cred: ImapCredential = serde_json::from_str(json).unwrap();

        assert_eq!(cred.host, "imap.example.com");
        assert_eq!(cred.port, 993);
        assert_eq!(cred.username, "user@example.com");
        assert_eq!(cred.password, "secret123");
    }

    #[test]
    fn test_imap_credential_missing_host_returns_error() {
        let json = r#"{
            "port": 993,
            "username": "user@example.com",
            "password": "secret123"
        }"#;

        let err = serde_json::from_str::<ImapCredential>(json).unwrap_err();
        assert!(err.to_string().contains("missing field `host`"));
    }

    #[test]
    fn test_smtp_credential_deserializes_with_all_fields() {
        let json = r#"{
            "host": "smtp.example.com",
            "port": 587,
            "username": "user@example.com",
            "password": "secret123"
        }"#;

        let cred: SmtpCredential = serde_json::from_str(json).unwrap();

        assert_eq!(cred.host, "smtp.example.com");
        assert_eq!(cred.port, 587);
        assert_eq!(cred.username, "user@example.com");
        assert_eq!(cred.password, "secret123");
    }

    #[test]
    fn test_smtp_credential_missing_password_returns_error() {
        let json = r#"{
            "host": "smtp.example.com",
            "port": 587,
            "username": "user@example.com"
        }"#;

        let err = serde_json::from_str::<SmtpCredential>(json).unwrap_err();
        assert!(err.to_string().contains("missing field `password`"));
    }
}
