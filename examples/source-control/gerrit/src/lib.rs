//! source-control/gerrit integration for Operai Toolbox.

mod types;

use std::collections::HashMap;

use operai::{
    Context, JsonSchema, Result, define_user_credential, ensure, info, init, schemars, shutdown,
    tool,
};
use serde::{Deserialize, Serialize};
use types::{
    AbandonInput, AccountInfo, ChangeSummary, CommentInput, CommentRange, ReviewInput,
    ReviewResult, SubmitInput,
};

define_user_credential! {
    GerritCredential("gerrit") {
        username: String,
        password: String,
        #[optional]
        endpoint: Option<String>,
    }
}

#[init]
async fn setup() -> Result<()> {
    info!("Gerrit integration initialized");
    Ok(())
}

#[shutdown]
fn cleanup() {
    info!("Gerrit integration shutting down");
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchChangesInput {
    /// Search query string using Gerrit search operators.
    /// Examples: "status:open", "project:myproject", "owner:self"
    pub query: String,
    /// Maximum number of results to return (1-100). Defaults to 25.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Number of changes to skip. Useful for pagination.
    #[serde(default)]
    pub skip: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchChangesOutput {
    pub changes: Vec<ChangeSummary>,
}

/// # Search Gerrit Changes
///
/// Searches for Gerrit code review changes using flexible query operators.
///
/// Use this tool when the user wants to find or list code changes in Gerrit.
/// Supports powerful search operators to filter changes by status, project,
/// owner, reviewer, branch, and more.
///
/// Common query examples:
/// - "status:open" - Find all open changes
/// - "owner:self" - Find changes owned by the current user
/// - "project:myproject" - Find changes in a specific project
/// - "reviewer:self" - Find changes where you are a reviewer
/// - "is:submittable" - Find changes that are ready to submit
/// - "status:reviewed" - Find changes that have been reviewed
///
/// Query operators can be combined for precise filtering:
/// - "status:open owner:self project:myproject"
/// - "is:submittable reviewer:self"
///
/// This tool supports pagination through the `skip` parameter and allows
/// controlling result count with the `limit` parameter (1-100, default 25).
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - gerrit
/// - code-review
/// - source-control
///
/// # Errors
///
/// Returns an error if:
/// - The query string is empty or contains only whitespace
/// - The limit is not between 1 and 100 (inclusive)
/// - Gerrit credentials are not configured or are invalid
/// - The Gerrit API request fails due to network or authentication issues
/// - The response from Gerrit cannot be parsed as valid JSON
#[tool]
pub async fn search_changes(
    ctx: Context,
    input: SearchChangesInput,
) -> Result<SearchChangesOutput> {
    ensure!(!input.query.trim().is_empty(), "query must not be empty");
    let limit = input.limit.unwrap_or(25);
    ensure!(
        (1..=100).contains(&limit),
        "limit must be between 1 and 100"
    );

    let client = GerritClient::from_ctx(&ctx)?;

    let mut query_params = vec![("q", input.query.clone()), ("n", limit.to_string())];

    if let Some(skip) = input.skip {
        query_params.push(("S", skip.to_string()));
    }

    let changes: Vec<GerritChange> = client.get_json("/changes/", &query_params).await?;

    Ok(SearchChangesOutput {
        changes: changes.into_iter().map(map_change_summary).collect(),
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReviewChangeInput {
    /// Change ID (numeric ID or "project~branch~change-id" format).
    pub change_id: String,
    /// Revision ID (commit SHA or "current"). Defaults to "current".
    #[serde(default)]
    pub revision_id: Option<String>,
    /// Review message/comment.
    #[serde(default)]
    pub message: Option<String>,
    /// Map of label names to vote values (e.g., {"Code-Review": 1, "Verified":
    /// 1}).
    #[serde(default)]
    pub labels: HashMap<String, i32>,
    /// Mark the change as ready for review.
    #[serde(default)]
    pub ready: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReviewChangeOutput {
    pub labels: HashMap<String, i32>,
    pub ready: bool,
}

/// # Review Gerrit Change
///
/// Posts a review with optional votes/scores on a Gerrit change.
///
/// Use this tool when the user wants to:
/// - Approve or reject a code change with label votes (e.g., Code-Review:
///   +2/-2)
/// - Leave a general review message or feedback
/// - Mark a work-in-progress change as ready for review
///
/// Label voting is commonly used for code review workflow:
/// - "Code-Review": 2 (Approved), +1 (Looks good), 0 (No score), -1 (Needs
///   work), -2 (Rejected)
/// - "Verified": 1 (Verified), 0 (No score), -1 (Failed verification)
/// - Custom project-specific labels may also be available
///
/// The `change_id` can be provided as:
/// - Numeric ID: "12345"
/// - Full triplet: "project~branch~Change-Id"
///
/// The `revision_id` defaults to "current" but can be a specific commit SHA.
/// Set `ready` to true to mark a draft change as ready for reviewer attention.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - gerrit
/// - code-review
/// - source-control
///
/// # Errors
///
/// Returns an error if:
/// - The `change_id` is empty or contains only whitespace
/// - Gerrit credentials are not configured or are invalid
/// - The change or revision does not exist
/// - The Gerrit API request fails due to network or authentication issues
/// - The response from Gerrit cannot be parsed as valid JSON
#[tool]
pub async fn review_change(ctx: Context, input: ReviewChangeInput) -> Result<ReviewChangeOutput> {
    ensure!(
        !input.change_id.trim().is_empty(),
        "change_id must not be empty"
    );

    let revision_id = input.revision_id.as_deref().unwrap_or("current");
    let client = GerritClient::from_ctx(&ctx)?;

    let review_input = ReviewInput {
        message: input.message,
        tag: None,
        labels: if input.labels.is_empty() {
            None
        } else {
            Some(input.labels)
        },
        comments: None,
        ready: if input.ready { Some(true) } else { None },
        notify: None,
        on_behalf_of: None,
        reviewers: None,
    };

    let path = format!(
        "/changes/{}/revisions/{}/review",
        encode_change_id(&input.change_id),
        revision_id
    );

    let result: ReviewResult = client.post_json(&path, &review_input).await?;

    Ok(ReviewChangeOutput {
        labels: result.labels,
        ready: result.ready.unwrap_or(false),
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CommentChangeInput {
    /// Change ID (numeric ID or "project~branch~change-id" format).
    pub change_id: String,
    /// Revision ID (commit SHA or "current"). Defaults to "current".
    #[serde(default)]
    pub revision_id: Option<String>,
    /// Comment message text.
    pub message: String,
    /// Optional file path for inline comment.
    #[serde(default)]
    pub file_path: Option<String>,
    /// Optional line number for inline comment.
    #[serde(default)]
    pub line: Option<i32>,
    /// Optional range for inline comment (more precise than line).
    #[serde(default)]
    pub range: Option<CommentRange>,
    /// Whether this comment should be marked as unresolved.
    #[serde(default)]
    pub unresolved: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CommentChangeOutput {
    pub accepted: bool,
}

/// # Comment on Gerrit Change
///
/// Posts a comment on a Gerrit change, either as a general message or inline on
/// specific code.
///
/// Use this tool when the user wants to:
/// - Leave general feedback or questions on a change (general comment)
/// - Comment on specific lines of code (inline comment)
/// - Provide code review feedback at the file or line level
///
/// For inline code comments:
/// - Provide `file_path` to specify which file to comment on
/// - Use `line` for a single-line comment, or `range` for multi-line precision
/// - The `range` parameter (`start_line`, `start_character`, `end_line`,
///   `end_character`) is more precise than `line` and should be preferred when
///   available
///
/// For general comments:
/// - Omit `file_path`, `line`, and `range` to post a general change-level
///   message
///
/// The `change_id` can be provided as:
/// - Numeric ID: "12345"
/// - Full triplet: "project~branch~Change-Id"
///
/// The `revision_id` defaults to "current" but can be a specific commit SHA.
/// Mark comments as `unresolved: true` to track follow-up items.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - gerrit
/// - code-review
/// - source-control
///
/// # Errors
///
/// Returns an error if:
/// - The `change_id` is empty or contains only whitespace
/// - The `message` is empty or contains only whitespace
/// - Gerrit credentials are not configured or are invalid
/// - The change or revision does not exist
/// - The Gerrit API request fails due to network or authentication issues
/// - The response from Gerrit cannot be parsed as valid JSON
#[tool]
pub async fn comment_change(
    ctx: Context,
    input: CommentChangeInput,
) -> Result<CommentChangeOutput> {
    ensure!(
        !input.change_id.trim().is_empty(),
        "change_id must not be empty"
    );
    ensure!(
        !input.message.trim().is_empty(),
        "message must not be empty"
    );

    let revision_id = input.revision_id.as_deref().unwrap_or("current");
    let client = GerritClient::from_ctx(&ctx)?;

    let review_input = if let Some(file_path) = input.file_path {
        // Inline comment
        let mut comments = HashMap::new();
        comments.insert(
            file_path.clone(),
            vec![CommentInput {
                path: Some(file_path),
                line: input.line,
                range: input.range,
                message: input.message,
                in_reply_to: None,
                unresolved: input.unresolved,
            }],
        );

        ReviewInput {
            message: None,
            tag: None,
            labels: None,
            comments: Some(comments),
            ready: None,
            notify: None,
            on_behalf_of: None,
            reviewers: None,
        }
    } else {
        // General comment
        ReviewInput {
            message: Some(input.message),
            tag: None,
            labels: None,
            comments: None,
            ready: None,
            notify: None,
            on_behalf_of: None,
            reviewers: None,
        }
    };

    let path = format!(
        "/changes/{}/revisions/{}/review",
        encode_change_id(&input.change_id),
        revision_id
    );

    client
        .post_json::<_, ReviewResult>(&path, &review_input)
        .await?;

    Ok(CommentChangeOutput { accepted: true })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SubmitChangeInput {
    /// Change ID (numeric ID or "project~branch~change-id" format).
    pub change_id: String,
    /// Optional account ID to submit on behalf of.
    #[serde(default)]
    pub on_behalf_of: Option<i64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SubmitChangeOutput {
    pub change_id: String,
    pub status: String,
}

/// # Submit Gerrit Change
///
/// Submits (merges) a Gerrit change that has been approved and is ready to be
/// integrated.
///
/// Use this tool when the user wants to merge a change into the target branch.
/// The change must be in a submittable state, which typically requires:
/// - All required labels have been approved (e.g., Code-Review: +2, Verified:
///   +1)
/// - No blocking negative votes
/// - All CI checks have passed
/// - The change is up-to-date with the target branch (or merge conflicts
///   resolved)
///
/// The `change_id` can be provided as:
/// - Numeric ID: "12345"
/// - Full triplet: "project~branch~Change-Id"
///
/// This action will integrate the change into the target branch and update the
/// change status to "MERGED". This operation is irreversible.
///
/// The optional `on_behalf_of` parameter allows submitting on behalf of another
/// account (requires appropriate permissions).
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - gerrit
/// - code-review
/// - source-control
///
/// # Errors
///
/// Returns an error if:
/// - The `change_id` is empty or contains only whitespace
/// - Gerrit credentials are not configured or are invalid
/// - The change does not exist or is not in a submitable state
/// - The change has not been approved
/// - The change has merge conflicts
/// - The Gerrit API request fails due to network or authentication issues
/// - The response from Gerrit cannot be parsed as valid JSON
#[tool]
pub async fn submit_change(ctx: Context, input: SubmitChangeInput) -> Result<SubmitChangeOutput> {
    ensure!(
        !input.change_id.trim().is_empty(),
        "change_id must not be empty"
    );

    let client = GerritClient::from_ctx(&ctx)?;

    let submit_input = SubmitInput {
        on_behalf_of: input.on_behalf_of,
    };

    let path = format!("/changes/{}/submit", encode_change_id(&input.change_id));

    let result: GerritChange = client.post_json(&path, &submit_input).await?;

    Ok(SubmitChangeOutput {
        change_id: result.id,
        status: result.status.unwrap_or_else(|| "MERGED".to_string()),
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AbandonChangeInput {
    /// Change ID (numeric ID or "project~branch~change-id" format).
    pub change_id: String,
    /// Optional message explaining why the change is being abandoned.
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AbandonChangeOutput {
    pub change_id: String,
    pub status: String,
}

/// # Abandon Gerrit Change
///
/// Abandons a Gerrit change that is no longer needed or should be discarded.
///
/// Use this tool when the user wants to:
/// - Cancel a change that is out of date or superseded by another approach
/// - Stop work on a draft change that won't be completed
/// - Remove a change that is no longer relevant
///
/// This action sets the change status to "ABANDONED". The change remains
/// visible in Gerrit history but will not be merged. Abandoned changes can be
/// restored if needed (through the Gerrit web UI or REST API).
///
/// The `change_id` can be provided as:
/// - Numeric ID: "12345"
/// - Full triplet: "project~branch~Change-Id"
///
/// An optional `message` can be provided to explain why the change is being
/// abandoned. This message will be visible to other reviewers and maintainers.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - gerrit
/// - code-review
/// - source-control
///
/// # Errors
///
/// Returns an error if:
/// - The `change_id` is empty or contains only whitespace
/// - Gerrit credentials are not configured or are invalid
/// - The change does not exist or is already abandoned
/// - The change has already been merged (cannot abandon merged changes)
/// - The user lacks permission to abandon the change
/// - The Gerrit API request fails due to network or authentication issues
/// - The response from Gerrit cannot be parsed as valid JSON
#[tool]
pub async fn abandon_change(
    ctx: Context,
    input: AbandonChangeInput,
) -> Result<AbandonChangeOutput> {
    ensure!(
        !input.change_id.trim().is_empty(),
        "change_id must not be empty"
    );

    let client = GerritClient::from_ctx(&ctx)?;

    let abandon_input = AbandonInput {
        message: input.message,
    };

    let path = format!("/changes/{}/abandon", encode_change_id(&input.change_id));

    let result: GerritChange = client.post_json(&path, &abandon_input).await?;

    Ok(AbandonChangeOutput {
        change_id: result.id,
        status: result.status.unwrap_or_else(|| "ABANDONED".to_string()),
    })
}

// Internal API types that match Gerrit's wire format
#[derive(Debug, Deserialize)]
struct GerritChange {
    id: String,
    project: String,
    branch: String,
    #[serde(rename = "_number")]
    number: i64,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    owner: Option<AccountInfo>,
    #[serde(default)]
    updated: Option<String>,
    #[serde(default)]
    mergeable: Option<bool>,
}

fn map_change_summary(change: GerritChange) -> ChangeSummary {
    ChangeSummary {
        id: change.id,
        project: change.project,
        branch: change.branch,
        number: change.number,
        subject: change.subject,
        status: change.status,
        owner: change.owner,
        updated: change.updated,
        mergeable: change.mergeable,
    }
}

fn encode_change_id(change_id: &str) -> String {
    // URL encode the change ID, replacing special characters
    change_id.replace('~', "%7E").replace('/', "%2F")
}

#[derive(Debug, Clone)]
struct GerritClient {
    http: reqwest::Client,
    base_url: String,
    username: String,
    password: String,
}

impl GerritClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = GerritCredential::get(ctx)?;
        ensure!(
            !cred.username.trim().is_empty(),
            "username must not be empty"
        );
        ensure!(
            !cred.password.trim().is_empty(),
            "password must not be empty"
        );

        let base_url = normalize_base_url(
            cred.endpoint
                .as_deref()
                .unwrap_or("https://gerrit.example.com"),
        )?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            username: cred.username,
            password: cred.password,
        })
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T> {
        let url = format!("{}/a{}", self.base_url, path);

        let response = self
            .http
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .query(query)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(operai::anyhow::anyhow!(
                "Gerrit request failed ({status}): {body}"
            ));
        }

        let text = response.text().await?;
        // Gerrit prefixes JSON responses with `)]}'\n` to prevent XSSI attacks
        let json_text = text.strip_prefix(")]}'\n").unwrap_or(&text);
        Ok(operai::__private::serde_json::from_str(json_text)?)
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &TReq,
    ) -> Result<TRes> {
        let url = format!("{}/a{}", self.base_url, path);

        let response = self
            .http
            .post(&url)
            .basic_auth(&self.username, Some(&self.password))
            .json(body)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(operai::anyhow::anyhow!(
                "Gerrit request failed ({status}): {body}"
            ));
        }

        let text = response.text().await?;
        // Strip XSSI prefix
        let json_text = text.strip_prefix(")]}'\n").unwrap_or(&text);
        Ok(operai::__private::serde_json::from_str(json_text)?)
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
        matchers::{basic_auth, body_string_contains, method, path, query_param},
    };

    use super::*;

    fn test_ctx(endpoint: &str) -> Context {
        let mut gerrit_values = HashMap::new();
        gerrit_values.insert("username".to_string(), "test-user".to_string());
        gerrit_values.insert("password".to_string(), "test-password".to_string());
        gerrit_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("gerrit", gerrit_values)
    }

    // --- Serialization roundtrip tests ---

    #[test]
    fn test_change_summary_serialization_roundtrip() {
        let summary = ChangeSummary {
            id: "project~branch~I123".to_string(),
            project: "myproject".to_string(),
            branch: "main".to_string(),
            number: 12345,
            subject: Some("Test change".to_string()),
            status: Some("NEW".to_string()),
            owner: None,
            updated: Some("2024-01-01 12:00:00".to_string()),
            mergeable: Some(true),
        };

        let json = serde_json::to_string(&summary).unwrap();
        let parsed: ChangeSummary = serde_json::from_str(&json).unwrap();

        assert_eq!(summary.id, parsed.id);
        assert_eq!(summary.project, parsed.project);
        assert_eq!(summary.number, parsed.number);
    }

    #[test]
    fn test_review_input_serialization() {
        let mut labels = HashMap::new();
        labels.insert("Code-Review".to_string(), 1);

        let input = ReviewInput {
            message: Some("LGTM".to_string()),
            tag: Some("tag1".to_string()),
            labels: Some(labels),
            comments: None,
            ready: Some(true),
            notify: Some("NONE".to_string()),
            on_behalf_of: Some(12345),
            reviewers: None,
        };

        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("Code-Review"));
        assert!(json.contains("LGTM"));
        assert!(json.contains("tag1"));
        assert!(json.contains("notify"));
        assert!(json.contains("on_behalf_of"));
    }

    #[test]
    fn test_comment_range_serialization() {
        let range = CommentRange {
            start_line: 10,
            start_character: 5,
            end_line: 12,
            end_character: 20,
        };

        let json = serde_json::to_string(&range).unwrap();
        let parsed: CommentRange = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.start_line, 10);
        assert_eq!(parsed.start_character, 5);
        assert_eq!(parsed.end_line, 12);
        assert_eq!(parsed.end_character, 20);
    }

    #[test]
    fn test_comment_input_with_range_serialization() {
        let range = CommentRange {
            start_line: 1,
            start_character: 0,
            end_line: 2,
            end_character: 10,
        };

        let input = CommentInput {
            path: Some("src/main.rs".to_string()),
            line: None,
            range: Some(range),
            message: "Fix this bug".to_string(),
            in_reply_to: None,
            unresolved: Some(true),
        };

        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("range"));
        assert!(json.contains("unresolved"));
        assert!(json.contains("Fix this bug"));
    }

    // --- normalize_base_url tests ---

    #[test]
    fn test_normalize_base_url_trims_trailing_slash() {
        let result = normalize_base_url("https://gerrit.example.com/").unwrap();
        assert_eq!(result, "https://gerrit.example.com");
    }

    #[test]
    fn test_normalize_base_url_trims_whitespace() {
        let result = normalize_base_url("  https://gerrit.example.com  ").unwrap();
        assert_eq!(result, "https://gerrit.example.com");
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

    // --- encode_change_id tests ---

    #[test]
    fn test_encode_change_id_with_tildes() {
        let result = encode_change_id("project~branch~I123");
        assert_eq!(result, "project%7Ebranch%7EI123");
    }

    #[test]
    fn test_encode_change_id_numeric() {
        let result = encode_change_id("12345");
        assert_eq!(result, "12345");
    }

    // --- Input validation tests ---

    #[tokio::test]
    async fn test_search_changes_empty_query_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = search_changes(
            ctx,
            SearchChangesInput {
                query: "   ".to_string(),
                limit: None,
                skip: None,
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
    async fn test_search_changes_limit_zero_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = search_changes(
            ctx,
            SearchChangesInput {
                query: "status:open".to_string(),
                limit: Some(0),
                skip: None,
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
    async fn test_search_changes_limit_exceeds_max_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = search_changes(
            ctx,
            SearchChangesInput {
                query: "status:open".to_string(),
                limit: Some(101),
                skip: None,
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
    async fn test_review_change_empty_change_id_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = review_change(
            ctx,
            ReviewChangeInput {
                change_id: "  ".to_string(),
                revision_id: None,
                message: Some("LGTM".to_string()),
                labels: HashMap::new(),
                ready: false,
            },
        )
        .await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("change_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_comment_change_empty_message_returns_error() {
        let server = MockServer::start().await;
        let ctx = test_ctx(&server.uri());

        let result = comment_change(
            ctx,
            CommentChangeInput {
                change_id: "12345".to_string(),
                revision_id: None,
                message: "  ".to_string(),
                file_path: None,
                line: None,
                range: None,
                unresolved: None,
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

    // --- Integration tests ---

    #[tokio::test]
    async fn test_search_changes_success() {
        let server = MockServer::start().await;

        let response_body = r#")]}'
[
  {
    "id": "myproject~master~I123",
    "project": "myproject",
    "branch": "master",
    "_number": 12345,
    "subject": "Fix bug",
    "status": "NEW",
    "owner": {
      "_account_id": 1000,
      "name": "John Doe",
      "email": "john@example.com"
    },
    "updated": "2024-01-01 12:00:00",
    "mergeable": true
  }
]"#;

        Mock::given(method("GET"))
            .and(path("/a/changes/"))
            .and(basic_auth("test-user", "test-password"))
            .and(query_param("q", "status:open"))
            .and(query_param("n", "10"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = search_changes(
            ctx,
            SearchChangesInput {
                query: "status:open".to_string(),
                limit: Some(10),
                skip: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.changes.len(), 1);
        assert_eq!(output.changes[0].number, 12345);
        assert_eq!(output.changes[0].project, "myproject");
    }

    #[tokio::test]
    async fn test_review_change_success() {
        let server = MockServer::start().await;

        let response_body = r#")]}'
{
  "labels": {
    "Code-Review": 1
  },
  "ready": true
}"#;

        Mock::given(method("POST"))
            .and(path("/a/changes/12345/revisions/current/review"))
            .and(basic_auth("test-user", "test-password"))
            .and(body_string_contains("Code-Review"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let mut labels = HashMap::new();
        labels.insert("Code-Review".to_string(), 1);

        let output = review_change(
            ctx,
            ReviewChangeInput {
                change_id: "12345".to_string(),
                revision_id: None,
                message: Some("LGTM".to_string()),
                labels,
                ready: true,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.labels.get("Code-Review"), Some(&1));
        assert!(output.ready);
    }

    #[tokio::test]
    async fn test_comment_change_general_comment_success() {
        let server = MockServer::start().await;

        let response_body = r#")]}'
{
  "labels": {}
}"#;

        Mock::given(method("POST"))
            .and(path("/a/changes/12345/revisions/current/review"))
            .and(basic_auth("test-user", "test-password"))
            .and(body_string_contains("Nice work"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = comment_change(
            ctx,
            CommentChangeInput {
                change_id: "12345".to_string(),
                revision_id: None,
                message: "Nice work".to_string(),
                file_path: None,
                line: None,
                range: None,
                unresolved: None,
            },
        )
        .await
        .unwrap();

        assert!(output.accepted);
    }

    #[tokio::test]
    async fn test_submit_change_success() {
        let server = MockServer::start().await;

        let response_body = r#")]}'
{
  "id": "myproject~master~I123",
  "project": "myproject",
  "branch": "master",
  "_number": 12345,
  "status": "MERGED"
}"#;

        Mock::given(method("POST"))
            .and(path("/a/changes/12345/submit"))
            .and(basic_auth("test-user", "test-password"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = submit_change(
            ctx,
            SubmitChangeInput {
                change_id: "12345".to_string(),
                on_behalf_of: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(output.status, "MERGED");
    }

    #[tokio::test]
    async fn test_abandon_change_success() {
        let server = MockServer::start().await;

        let response_body = r#")]}'
{
  "id": "myproject~master~I123",
  "project": "myproject",
  "branch": "master",
  "_number": 12345,
  "status": "ABANDONED"
}"#;

        Mock::given(method("POST"))
            .and(path("/a/changes/12345/abandon"))
            .and(basic_auth("test-user", "test-password"))
            .and(body_string_contains("No longer needed"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let output = abandon_change(
            ctx,
            AbandonChangeInput {
                change_id: "12345".to_string(),
                message: Some("No longer needed".to_string()),
            },
        )
        .await
        .unwrap();

        assert_eq!(output.status, "ABANDONED");
    }

    #[tokio::test]
    async fn test_search_changes_error_response_returns_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/a/changes/"))
            .respond_with(
                ResponseTemplate::new(401).set_body_raw("Authentication required", "text/plain"),
            )
            .mount(&server)
            .await;

        let ctx = test_ctx(&server.uri());
        let result = search_changes(
            ctx,
            SearchChangesInput {
                query: "status:open".to_string(),
                limit: Some(10),
                skip: None,
            },
        )
        .await;

        assert!(result.is_err());
        let message = result.unwrap_err().to_string();
        assert!(message.contains("401"));
    }
}
