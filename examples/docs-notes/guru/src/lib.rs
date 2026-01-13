//! Guru knowledge management integration for Operai Toolbox.
//!
//! This integration provides tools for searching, reading, creating, updating,
//! and verifying knowledge cards in Guru.

use operai::{
    Context, JsonSchema, Result, anyhow, define_system_credential, ensure, info, init, schemars,
    shutdown, tool,
};
use serde::{Deserialize, Serialize};

mod types;
use types::{
    CollectionRef, CreateCardRequest, GuruCard, GuruSearchResponse, GuruTag, GuruUser,
    UpdateCardRequest, UserRef, VerifierRef,
};

define_system_credential! {
    GuruCredential("guru") {
        /// Guru API token for authentication.
        api_token: String,
        /// Guru user email associated with the API token.
        user_email: String,
        /// Optional custom API endpoint (defaults to https://api.getguru.com/api/v1).
        #[optional]
        endpoint: Option<String>,
    }
}

/// Initialize the Guru tool library.
#[init]
async fn setup() -> Result<()> {
    info!("Guru integration initialized");
    Ok(())
}

/// Clean up resources when the library is unloaded.
#[shutdown]
fn cleanup() {
    info!("Guru integration shutting down");
}

const DEFAULT_GURU_ENDPOINT: &str = "https://api.getguru.com/api/v1";

/// HTTP client for making Guru API requests.
#[derive(Debug, Clone)]
struct GuruClient {
    http: reqwest::Client,
    base_url: String,
    user_email: String,
    api_token: String,
}

impl GuruClient {
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = GuruCredential::get(ctx)?;
        ensure!(
            !cred.api_token.trim().is_empty(),
            "api_token must not be empty"
        );
        ensure!(
            !cred.user_email.trim().is_empty(),
            "user_email must not be empty"
        );

        let base_url = cred
            .endpoint
            .as_deref()
            .unwrap_or(DEFAULT_GURU_ENDPOINT)
            .trim_end_matches('/')
            .to_string();

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            user_email: cred.user_email,
            api_token: cred.api_token,
        })
    }

    fn url_with_segments(&self, segments: &[&str]) -> Result<reqwest::Url> {
        let mut url = reqwest::Url::parse(&self.base_url)?;
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|()| anyhow::anyhow!("base_url must be an absolute URL"))?;
            for segment in segments {
                path.push(segment);
            }
        }
        Ok(url)
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        query: &[(&str, String)],
    ) -> Result<T> {
        let response = self
            .http
            .get(url)
            .basic_auth(&self.user_email, Some(&self.api_token))
            .query(query)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.json::<T>().await?)
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Guru API request failed ({status}): {body}"
            ))
        }
    }

    async fn post_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
    ) -> Result<TRes> {
        let response = self
            .http
            .post(url)
            .basic_auth(&self.user_email, Some(&self.api_token))
            .header(reqwest::header::ACCEPT, "application/json")
            .json(body)
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.json::<TRes>().await?)
        } else {
            let body_text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Guru API request failed ({status}): {body_text}"
            ))
        }
    }

    async fn put_json<TReq: Serialize, TRes: for<'de> Deserialize<'de>>(
        &self,
        url: reqwest::Url,
        body: &TReq,
    ) -> Result<TRes> {
        let response = self
            .http
            .put(url)
            .basic_auth(&self.user_email, Some(&self.api_token))
            .header(reqwest::header::ACCEPT, "application/json")
            .json(body)
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.json::<TRes>().await?)
        } else {
            let body_text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Guru API request failed ({status}): {body_text}"
            ))
        }
    }

    async fn put_empty(&self, url: reqwest::Url) -> Result<()> {
        let response = self
            .http
            .put(url)
            .basic_auth(&self.user_email, Some(&self.api_token))
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Guru API request failed ({status}): {body}"
            ))
        }
    }
}

// =============================================================================
// Search Cards Tool
// =============================================================================

/// Input for searching Guru cards.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchCardsInput {
    /// The search query string.
    pub query: String,
    /// Optional collection ID to filter results.
    #[serde(default)]
    pub collection_id: Option<String>,
    /// Maximum number of results to return (1-50, defaults to 20).
    #[serde(default)]
    pub max_results: Option<u32>,
    /// Filter by card verification status.
    #[serde(default)]
    pub verification_status: Option<VerificationStatus>,
}

/// Verification status filter for cards.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VerificationStatus {
    /// Card is verified and trusted.
    Trusted,
    /// Card needs verification.
    NeedsVerification,
    /// Card is unverified.
    Unverified,
}

/// A card summary returned from search results.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct CardSummary {
    /// Unique identifier for the card.
    pub id: String,
    /// Title of the card.
    pub title: String,
    /// Slug for the card URL.
    pub slug: String,
    /// Collection the card belongs to.
    pub collection: Option<CollectionInfo>,
    /// Current verification status.
    pub verification_status: VerificationStatus,
    /// Relevance score from search (0.0-1.0).
    pub relevance_score: f64,
    /// Last modified timestamp (ISO 8601).
    pub last_modified: String,
}

/// Basic collection information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CollectionInfo {
    /// Unique identifier for the collection.
    pub id: String,
    /// Name of the collection.
    pub name: String,
}

/// Output from searching Guru cards.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchCardsOutput {
    /// List of matching cards.
    pub cards: Vec<CardSummary>,
    /// Total number of results available.
    pub total_count: u32,
    /// The query that was executed.
    pub query: String,
}

/// # Search Guru Cards
///
/// Searches for knowledge cards in the Guru knowledge base using keyword
/// matching. Returns a list of card summaries with metadata including title,
/// collection, verification status, and last modified date.
///
/// Use this tool when you need to:
/// - Find cards on a specific topic or keyword
/// - Discover documentation in the Guru knowledge base
/// - Locate cards that may need verification (filter by status)
/// - Browse cards within a specific collection
///
/// The search performs keyword matching against card titles and content.
/// Results can be filtered by collection ID and verification status (Trusted,
/// Needs Verification, or Unverified). Specify `max_results` (1-50) to limit
/// the number of results returned.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - docs
/// - guru
/// - search
///
/// # Errors
///
/// Returns an error if:
/// - The provided query string is empty
/// - Guru credentials are not configured or are invalid
/// - The Guru API request fails due to network or authentication issues
/// - The API response cannot be parsed
#[tool]
pub async fn search_cards(ctx: Context, input: SearchCardsInput) -> Result<SearchCardsOutput> {
    ensure!(!input.query.trim().is_empty(), "query must not be empty");

    let max_results = input.max_results.unwrap_or(20).clamp(1, 50);

    let client = GuruClient::from_ctx(&ctx)?;

    // Build query parameters
    let mut query = vec![
        ("searchTerms", input.query.clone()),
        ("maxResults", max_results.to_string()),
    ];

    // Add verification status filter if provided
    if let Some(status) = &input.verification_status {
        let status_str = match status {
            VerificationStatus::Trusted => "TRUSTED",
            VerificationStatus::NeedsVerification => "NEEDS_VERIFICATION",
            VerificationStatus::Unverified => "UNVERIFIED",
        };
        query.push(("verificationState", status_str.to_string()));
    }

    // Add collection filter if provided
    if let Some(collection_id) = &input.collection_id {
        query.push(("collection", collection_id.clone()));
    }

    let url = client.url_with_segments(&["search", "cardmgr"])?;
    let response: GuruSearchResponse = client.get_json(url, &query).await?;

    let cards = response.results.into_iter().map(map_card_summary).collect();

    Ok(SearchCardsOutput {
        cards,
        total_count: response.count,
        query: input.query,
    })
}

fn map_card_summary(card: GuruCard) -> CardSummary {
    let verification_status = parse_verification_status(card.verification_status.as_deref());

    CardSummary {
        id: card.id,
        title: card.title,
        slug: card.slug,
        collection: card.collection.map(|c| CollectionInfo {
            id: c.id,
            name: c.name,
        }),
        verification_status,
        relevance_score: 0.0, // Guru API doesn't provide relevance score
        last_modified: card.last_modified_date.unwrap_or_default(),
    }
}

// =============================================================================
// Get Card Tool
// =============================================================================

/// Input for getting a specific Guru card.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCardInput {
    /// The unique ID of the card to retrieve.
    pub card_id: String,
    /// Whether to include the full HTML content (defaults to true).
    #[serde(default = "default_true")]
    pub include_content: bool,
}

fn default_true() -> bool {
    true
}

/// A full Guru card with all details.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Card {
    /// Unique identifier for the card.
    pub id: String,
    /// Title of the card.
    pub title: String,
    /// Slug for the card URL.
    pub slug: String,
    /// Full HTML content of the card (if requested).
    pub content: Option<String>,
    /// Collection the card belongs to.
    pub collection: Option<CollectionInfo>,
    /// Current verification status.
    pub verification_status: VerificationStatus,
    /// User who owns this card.
    pub owner: Option<UserInfo>,
    /// User who last verified this card.
    pub verifier: Option<UserInfo>,
    /// Date when verification is next due (ISO 8601).
    pub verification_due_date: Option<String>,
    /// Last modified timestamp (ISO 8601).
    pub last_modified: String,
    /// Created timestamp (ISO 8601).
    pub created_at: String,
    /// Tags associated with this card.
    pub tags: Vec<String>,
}

/// Basic user information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserInfo {
    /// User's email address.
    pub email: String,
    /// User's display name.
    pub name: Option<String>,
}

/// Output from getting a Guru card.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GetCardOutput {
    /// The retrieved card.
    pub card: Card,
}

/// # Get Guru Card
///
/// Retrieves a specific Guru knowledge card by its unique ID, returning the
/// complete card details including title, HTML content, collection,
/// verification status, owner, verifier, tags, and timestamps.
///
/// Use this tool when you need to:
/// - Read the full content of a specific card
/// - View all metadata about a card (who owns/verified it, when it was
///   modified)
/// - Get detailed information about a card found via search
/// - Access the complete HTML content for display or editing
///
/// The `include_content` parameter controls whether the full HTML content is
/// returned (defaults to true). Set to false if you only need metadata.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - docs
/// - guru
///
/// # Errors
///
/// Returns an error if:
/// - The provided `card_id` is empty
/// - Guru credentials are not configured or are invalid
/// - The Guru API request fails due to network or authentication issues
/// - The requested card does not exist
/// - The API response cannot be parsed
#[tool]
pub async fn get_card(ctx: Context, input: GetCardInput) -> Result<GetCardOutput> {
    ensure!(
        !input.card_id.trim().is_empty(),
        "card_id must not be empty"
    );

    let client = GuruClient::from_ctx(&ctx)?;

    let url = client.url_with_segments(&["cards", &input.card_id, "extended"])?;
    let api_card: GuruCard = client.get_json(url, &[]).await?;

    let card = map_full_card(api_card, input.include_content);

    Ok(GetCardOutput { card })
}

fn map_full_card(card: GuruCard, include_content: bool) -> Card {
    let verification_status = parse_verification_status(card.verification_status.as_deref());

    Card {
        id: card.id,
        title: card.title,
        slug: card.slug,
        content: if include_content { card.content } else { None },
        collection: card.collection.map(|c| CollectionInfo {
            id: c.id,
            name: c.name,
        }),
        verification_status,
        owner: card.owner.map(map_user_info),
        verifier: card.verifier.map(map_user_info),
        verification_due_date: card.next_verification_date,
        last_modified: card.last_modified_date.unwrap_or_default(),
        created_at: card.date_created.unwrap_or_default(),
        tags: card.tags.into_iter().map(|t| t.value).collect(),
    }
}

fn map_user_info(user: GuruUser) -> UserInfo {
    let name = match (user.first_name, user.last_name) {
        (Some(first), Some(last)) => Some(format!("{first} {last}")),
        (Some(first), None) => Some(first),
        (None, Some(last)) => Some(last),
        (None, None) => None,
    };

    UserInfo {
        email: user.email,
        name,
    }
}

fn parse_verification_status(status: Option<&str>) -> VerificationStatus {
    match status {
        Some("TRUSTED") => VerificationStatus::Trusted,
        Some("NEEDS_VERIFICATION") => VerificationStatus::NeedsVerification,
        _ => VerificationStatus::Unverified,
    }
}

// =============================================================================
// Create Card Tool
// =============================================================================

/// Input for creating a new Guru card.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateCardInput {
    /// Title of the new card.
    pub title: String,
    /// HTML content of the card.
    pub content: String,
    /// ID of the collection to create the card in.
    pub collection_id: String,
    /// Optional tags to apply to the card.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Optional verification interval in days.
    #[serde(default)]
    pub verification_interval_days: Option<u32>,
    /// Email of the user to assign as verifier.
    #[serde(default)]
    pub verifier_email: Option<String>,
}

/// Output from creating a Guru card.
#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateCardOutput {
    /// The newly created card.
    pub card: Card,
    /// URL to view the card in Guru.
    pub web_url: String,
}

/// # Create Guru Card
///
/// Creates a new knowledge card in the Guru knowledge base with the provided
/// title, HTML content, and metadata. Returns the created card with its
/// generated ID and a URL to view it in the Guru web interface.
///
/// Use this tool when you need to:
/// - Add a new knowledge document to Guru
/// - Create documentation, guides, or reference materials
/// - Store information that should be shared with the team
/// - Build a knowledge base entry that can be verified and tracked
///
/// Cards must be created within a specific collection (specified by
/// `collection_id`). You can optionally add tags for categorization, assign a
/// verifier, and set a verification interval (in days) for how often the card
/// should be reviewed. Content should be formatted as HTML.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - docs
/// - guru
///
/// # Errors
///
/// Returns an error if:
/// - The provided title, content, or `collection_id` are empty
/// - Guru credentials are not configured or are invalid
/// - The specified collection does not exist
/// - The Guru API request fails due to network or authentication issues
/// - The API response cannot be parsed
#[tool]
pub async fn create_card(ctx: Context, input: CreateCardInput) -> Result<CreateCardOutput> {
    ensure!(!input.title.trim().is_empty(), "title must not be empty");
    ensure!(
        !input.content.trim().is_empty(),
        "content must not be empty"
    );
    ensure!(
        !input.collection_id.trim().is_empty(),
        "collection_id must not be empty"
    );

    let client = GuruClient::from_ctx(&ctx)?;

    let verifiers = input.verifier_email.map(|email| {
        vec![VerifierRef {
            verifier_type: "user".to_string(),
            user: UserRef { email },
        }]
    });

    let request = CreateCardRequest {
        preferred_phrase: input.title.clone(),
        content: input.content.clone(),
        collection: CollectionRef {
            id: input.collection_id,
        },
        tags: input
            .tags
            .into_iter()
            .map(|value| GuruTag { id: None, value })
            .collect(),
        verification_interval: input.verification_interval_days,
        verifiers,
        share_status: "TEAM".to_string(),
    };

    let url = client.url_with_segments(&["cards", "extended"])?;
    let api_card: GuruCard = client.post_json(url, &request).await?;

    let card = map_full_card(api_card, true);
    let web_url = format!("https://app.getguru.com/card/{}", card.id);

    Ok(CreateCardOutput { card, web_url })
}

// =============================================================================
// Update Card Tool
// =============================================================================

/// Input for updating an existing Guru card.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateCardInput {
    /// The unique ID of the card to update.
    pub card_id: String,
    /// New title for the card (optional).
    #[serde(default)]
    pub title: Option<String>,
    /// New HTML content for the card (optional).
    #[serde(default)]
    pub content: Option<String>,
    /// New tags for the card (replaces existing tags if provided).
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// New collection ID to move the card to (optional).
    #[serde(default)]
    pub collection_id: Option<String>,
    /// New verification interval in days (optional).
    #[serde(default)]
    pub verification_interval_days: Option<u32>,
}

/// Output from updating a Guru card.
#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateCardOutput {
    /// The updated card.
    pub card: Card,
    /// Fields that were modified.
    pub modified_fields: Vec<String>,
}

/// # Update Guru Card
///
/// Updates an existing Guru knowledge card by modifying one or more of its
/// properties: title, content, tags, collection, or verification interval.
/// All unspecified fields remain unchanged. Returns the updated card and a
/// list of which fields were modified.
///
/// Use this tool when you need to:
/// - Edit the content or title of an existing card
/// - Add, remove, or replace tags on a card
/// - Move a card to a different collection
/// - Change the verification schedule for a card
/// - Make corrections or additions to documentation
///
/// At least one field must be provided for update. This tool performs a
/// partial update - only the fields you specify will be changed. When updating
/// tags, the new tag list completely replaces existing tags (not additive).
/// Content should be formatted as HTML.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - docs
/// - guru
///
/// # Errors
///
/// Returns an error if:
/// - The provided `card_id` is empty
/// - No fields are provided for update (title, content, tags, etc.)
/// - Guru credentials are not configured or are invalid
/// - The specified card does not exist
/// - The Guru API request fails due to network or authentication issues
/// - The API response cannot be parsed
#[tool]
pub async fn update_card(ctx: Context, input: UpdateCardInput) -> Result<UpdateCardOutput> {
    ensure!(
        !input.card_id.trim().is_empty(),
        "card_id must not be empty"
    );
    ensure!(
        input.title.is_some()
            || input.content.is_some()
            || input.tags.is_some()
            || input.collection_id.is_some()
            || input.verification_interval_days.is_some(),
        "at least one field must be provided for update"
    );

    let client = GuruClient::from_ctx(&ctx)?;

    // First, get the existing card to preserve unchanged fields
    let get_url = client.url_with_segments(&["cards", &input.card_id, "extended"])?;
    let existing_card: GuruCard = client.get_json(get_url, &[]).await?;

    let mut modified_fields = Vec::new();

    let title = if let Some(new_title) = input.title {
        ensure!(
            !new_title.trim().is_empty(),
            "title must not be empty when provided"
        );
        modified_fields.push("title".to_string());
        new_title
    } else {
        existing_card.title.clone()
    };

    let content = if let Some(new_content) = input.content {
        ensure!(
            !new_content.trim().is_empty(),
            "content must not be empty when provided"
        );
        modified_fields.push("content".to_string());
        new_content
    } else {
        existing_card.content.clone().unwrap_or_default()
    };

    let tags = if let Some(new_tags) = input.tags {
        modified_fields.push("tags".to_string());
        Some(
            new_tags
                .into_iter()
                .map(|value| GuruTag { id: None, value })
                .collect(),
        )
    } else {
        Some(existing_card.tags.clone())
    };

    if input.collection_id.is_some() {
        modified_fields.push("collection_id".to_string());
    }
    if input.verification_interval_days.is_some() {
        modified_fields.push("verification_interval_days".to_string());
    }

    let request = UpdateCardRequest {
        preferred_phrase: title,
        content,
        tags,
        share_status: Some("TEAM".to_string()),
    };

    let url = client.url_with_segments(&["cards", &input.card_id, "extended"])?;
    let updated_card: GuruCard = client.put_json(url, &request).await?;

    let card = map_full_card(updated_card, true);

    Ok(UpdateCardOutput {
        card,
        modified_fields,
    })
}

// =============================================================================
// Verify Card Tool
// =============================================================================

/// Input for verifying a Guru card.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct VerifyCardInput {
    /// The unique ID of the card to verify.
    pub card_id: String,
    /// Optional comment explaining the verification.
    #[serde(default)]
    pub comment: Option<String>,
}

/// Output from verifying a Guru card.
#[derive(Debug, Serialize, JsonSchema)]
pub struct VerifyCardOutput {
    /// The card ID that was verified.
    pub card_id: String,
    /// New verification status (should be Trusted after verification).
    pub verification_status: VerificationStatus,
    /// User who performed the verification.
    pub verified_by: UserInfo,
    /// Timestamp when the verification was performed (ISO 8601).
    pub verified_at: String,
    /// Next verification due date (ISO 8601).
    pub next_verification_due: String,
}

/// # Verify Guru Card
///
/// Marks a Guru knowledge card as verified, confirming that its content is
/// accurate and up-to-date. Verification changes the card's status to
/// "Trusted" and resets the verification due date based on the card's
/// verification interval.
///
/// Use this tool when you need to:
/// - Confirm that a card's content is current and accurate
/// - Complete a required review of knowledge documentation
/// - Maintain compliance with knowledge verification workflows
/// - Signal to team members that the card content is trustworthy
///
/// This action associates the verification with the authenticated user and
/// records the verification timestamp. The next verification due date is
/// calculated based on the card's configured verification interval.
/// Optionally provide a comment explaining the verification context.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - docs
/// - guru
/// - verification
///
/// # Errors
///
/// Returns an error if:
/// - The provided `card_id` is empty
/// - Guru credentials are not configured or are invalid
/// - The specified card does not exist
/// - The Guru API request fails due to network or authentication issues
/// - The API response cannot be parsed
#[tool]
pub async fn verify_card(ctx: Context, input: VerifyCardInput) -> Result<VerifyCardOutput> {
    ensure!(
        !input.card_id.trim().is_empty(),
        "card_id must not be empty"
    );

    let client = GuruClient::from_ctx(&ctx)?;

    let url = client.url_with_segments(&["cards", &input.card_id, "verify"])?;
    client.put_empty(url).await?;

    // Fetch the updated card to get verification details
    let get_url = client.url_with_segments(&["cards", &input.card_id, "extended"])?;
    let updated_card: GuruCard = client.get_json(get_url, &[]).await?;

    let verification_status =
        parse_verification_status(updated_card.verification_status.as_deref());

    let verified_by = updated_card.verifier.map_or_else(
        || {
            let user_id = ctx.user_id();
            let email = if user_id.is_empty() {
                "unknown".to_string()
            } else {
                user_id.to_string()
            };
            UserInfo { email, name: None }
        },
        map_user_info,
    );

    Ok(VerifyCardOutput {
        card_id: input.card_id,
        verification_status,
        verified_by,
        verified_at: updated_card.last_modified_date.unwrap_or_default(),
        next_verification_due: updated_card.next_verification_date.unwrap_or_default(),
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
    fn test_guru_credential_deserializes_with_required_fields() {
        let json = r#"{
            "api_token": "token123",
            "user_email": "user@example.com"
        }"#;

        let cred: GuruCredential = serde_json::from_str(json).unwrap();

        assert_eq!(cred.api_token, "token123");
        assert_eq!(cred.user_email, "user@example.com");
        assert!(cred.endpoint.is_none());
    }

    #[test]
    fn test_guru_credential_deserializes_with_optional_endpoint() {
        let json = r#"{
            "api_token": "token123",
            "user_email": "user@example.com",
            "endpoint": "https://custom.guru.api/v1"
        }"#;

        let cred: GuruCredential = serde_json::from_str(json).unwrap();

        assert_eq!(cred.api_token, "token123");
        assert_eq!(cred.user_email, "user@example.com");
        assert_eq!(cred.endpoint.as_deref(), Some("https://custom.guru.api/v1"));
    }

    #[test]
    fn test_guru_credential_missing_api_token_returns_error() {
        let json = r#"{ "user_email": "user@example.com" }"#;

        let err = serde_json::from_str::<GuruCredential>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `api_token`"));
    }

    #[test]
    fn test_guru_credential_missing_user_email_returns_error() {
        let json = r#"{ "api_token": "token123" }"#;

        let err = serde_json::from_str::<GuruCredential>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `user_email`"));
    }

    // =========================================================================
    // Search Cards Tests
    // =========================================================================

    #[test]
    fn test_search_cards_input_deserializes_with_query_only() {
        let json = r#"{ "query": "onboarding process" }"#;

        let input: SearchCardsInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.query, "onboarding process");
        assert!(input.collection_id.is_none());
        assert!(input.max_results.is_none());
        assert!(input.verification_status.is_none());
    }

    #[test]
    fn test_search_cards_input_deserializes_with_all_fields() {
        let json = r#"{
            "query": "API documentation",
            "collection_id": "col-123",
            "max_results": 10,
            "verification_status": "TRUSTED"
        }"#;

        let input: SearchCardsInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.query, "API documentation");
        assert_eq!(input.collection_id.as_deref(), Some("col-123"));
        assert_eq!(input.max_results, Some(10));
        assert!(matches!(
            input.verification_status,
            Some(VerificationStatus::Trusted)
        ));
    }

    #[test]
    fn test_search_cards_input_missing_query_returns_error() {
        let json = r#"{ "collection_id": "col-123" }"#;

        let err = serde_json::from_str::<SearchCardsInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `query`"));
    }

    // =========================================================================
    // Integration Tests with wiremock
    // =========================================================================

    fn test_ctx(endpoint: &str) -> Context {
        use std::collections::HashMap;

        let mut guru_values = HashMap::new();
        guru_values.insert("api_token".to_string(), "test-token".to_string());
        guru_values.insert("user_email".to_string(), "user@example.com".to_string());
        guru_values.insert("endpoint".to_string(), endpoint.to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_system_credential("guru", guru_values)
    }

    #[tokio::test]
    async fn test_search_cards_with_mock_server() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{header, method, path, query_param},
        };

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/search/cardmgr"))
            .and(query_param("searchTerms", "onboarding"))
            .and(header("accept", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [
                    {
                        "id": "card-123",
                        "preferredPhrase": "Onboarding Guide",
                        "slug": "onboarding-guide",
                        "collection": {
                            "id": "col-abc",
                            "name": "Engineering"
                        },
                        "verificationState": "TRUSTED",
                        "lastModifiedDate": "2024-01-15T10:30:00Z"
                    }
                ],
                "count": 1
            })))
            .mount(&mock_server)
            .await;

        let ctx = test_ctx(&mock_server.uri());

        let input = SearchCardsInput {
            query: "onboarding".to_string(),
            collection_id: None,
            max_results: None,
            verification_status: None,
        };

        let result = search_cards(ctx, input).await.unwrap();

        assert_eq!(result.cards.len(), 1);
        assert_eq!(result.cards[0].id, "card-123");
        assert_eq!(result.cards[0].title, "Onboarding Guide");
        assert_eq!(result.total_count, 1);
    }

    #[tokio::test]
    async fn test_get_card_with_mock_server() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{header, method, path},
        };

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/cards/card-123/extended"))
            .and(header("accept", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "card-123",
                "preferredPhrase": "API Documentation",
                "slug": "api-docs",
                "content": "<h1>API Docs</h1><p>Content here</p>",
                "collection": {
                    "id": "col-456",
                    "name": "Documentation"
                },
                "verificationState": "TRUSTED",
                "owner": {
                    "email": "owner@example.com",
                    "firstName": "John",
                    "lastName": "Doe"
                },
                "lastModifiedDate": "2024-01-20T14:00:00Z",
                "dateCreated": "2024-01-01T09:00:00Z",
                "tags": [{"value": "api"}]
            })))
            .mount(&mock_server)
            .await;

        let ctx = test_ctx(&mock_server.uri());

        let input = GetCardInput {
            card_id: "card-123".to_string(),
            include_content: true,
        };

        let result = get_card(ctx, input).await.unwrap();

        assert_eq!(result.card.id, "card-123");
        assert_eq!(result.card.title, "API Documentation");
        assert_eq!(
            result.card.content.as_deref(),
            Some("<h1>API Docs</h1><p>Content here</p>")
        );
        assert!(result.card.collection.is_some());
        assert_eq!(
            result.card.collection.as_ref().unwrap().name,
            "Documentation"
        );
        assert!(matches!(
            result.card.verification_status,
            VerificationStatus::Trusted
        ));
    }

    #[tokio::test]
    async fn test_create_card_with_mock_server() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{body_partial_json, header, method, path},
        };

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/cards/extended"))
            .and(header("accept", "application/json"))
            .and(body_partial_json(serde_json::json!({
                "preferredPhrase": "New Card Title"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "new-card-789",
                "preferredPhrase": "New Card Title",
                "slug": "new-card-title",
                "content": "<p>New content</p>",
                "collection": {
                    "id": "col-123",
                    "name": "Engineering"
                },
                "verificationState": "UNVERIFIED",
                "lastModifiedDate": "2024-01-25T10:00:00Z",
                "dateCreated": "2024-01-25T10:00:00Z",
                "tags": []
            })))
            .mount(&mock_server)
            .await;

        let ctx = test_ctx(&mock_server.uri());

        let input = CreateCardInput {
            title: "New Card Title".to_string(),
            content: "<p>New content</p>".to_string(),
            collection_id: "col-123".to_string(),
            tags: vec![],
            verification_interval_days: None,
            verifier_email: None,
        };

        let result = create_card(ctx, input).await.unwrap();

        assert_eq!(result.card.id, "new-card-789");
        assert_eq!(result.card.title, "New Card Title");
        assert!(result.web_url.contains("new-card-789"));
    }

    #[tokio::test]
    async fn test_verify_card_with_mock_server() {
        use wiremock::{
            Mock, MockServer, ResponseTemplate,
            matchers::{header, method, path},
        };

        let mock_server = MockServer::start().await;

        Mock::given(method("PUT"))
            .and(path("/cards/card-to-verify/verify"))
            .and(header("accept", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true
            })))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/cards/card-to-verify/extended"))
            .and(header("accept", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "card-to-verify",
                "preferredPhrase": "Verified Card",
                "slug": "verified-card",
                "verificationState": "TRUSTED",
                "verifier": {
                    "email": "verifier@example.com",
                    "firstName": "Verifier",
                    "lastName": "User"
                },
                "lastModifiedDate": "2024-01-26T11:00:00Z",
                "nextVerificationDate": "2024-04-26T11:00:00Z"
            })))
            .mount(&mock_server)
            .await;

        let ctx = test_ctx(&mock_server.uri());

        let input = VerifyCardInput {
            card_id: "card-to-verify".to_string(),
            comment: None,
        };

        let result = verify_card(ctx, input).await.unwrap();

        assert_eq!(result.card_id, "card-to-verify");
        assert!(matches!(
            result.verification_status,
            VerificationStatus::Trusted
        ));
        assert_eq!(result.verified_by.email, "verifier@example.com");
    }

    // =========================================================================
    // Get Card Tests
    // =========================================================================

    #[test]
    fn test_get_card_input_deserializes_with_card_id_only() {
        let json = r#"{ "card_id": "card-123" }"#;

        let input: GetCardInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.card_id, "card-123");
        assert!(input.include_content); // defaults to true
    }

    #[test]
    fn test_get_card_input_deserializes_with_include_content_false() {
        let json = r#"{ "card_id": "card-456", "include_content": false }"#;

        let input: GetCardInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.card_id, "card-456");
        assert!(!input.include_content);
    }

    #[test]
    fn test_get_card_input_missing_card_id_returns_error() {
        let json = r#"{ "include_content": true }"#;

        let err = serde_json::from_str::<GetCardInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `card_id`"));
    }

    // =========================================================================
    // Create Card Tests
    // =========================================================================

    #[test]
    fn test_create_card_input_deserializes_with_required_fields() {
        let json = r#"{
            "title": "New Card",
            "content": "<p>Card content</p>",
            "collection_id": "col-123"
        }"#;

        let input: CreateCardInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.title, "New Card");
        assert_eq!(input.content, "<p>Card content</p>");
        assert_eq!(input.collection_id, "col-123");
        assert!(input.tags.is_empty());
        assert!(input.verification_interval_days.is_none());
        assert!(input.verifier_email.is_none());
    }

    #[test]
    fn test_create_card_input_deserializes_with_all_fields() {
        let json = r#"{
            "title": "Full Card",
            "content": "<h1>Content</h1>",
            "collection_id": "col-456",
            "tags": ["api", "documentation"],
            "verification_interval_days": 90,
            "verifier_email": "verifier@example.com"
        }"#;

        let input: CreateCardInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.title, "Full Card");
        assert_eq!(input.tags, vec!["api", "documentation"]);
        assert_eq!(input.verification_interval_days, Some(90));
        assert_eq!(
            input.verifier_email.as_deref(),
            Some("verifier@example.com")
        );
    }

    #[test]
    fn test_create_card_input_missing_title_returns_error() {
        let json = r#"{
            "content": "<p>Content</p>",
            "collection_id": "col-123"
        }"#;

        let err = serde_json::from_str::<CreateCardInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `title`"));
    }

    // =========================================================================
    // Update Card Tests
    // =========================================================================

    #[test]
    fn test_update_card_input_deserializes_with_card_id_only() {
        let json = r#"{ "card_id": "card-123" }"#;

        let input: UpdateCardInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.card_id, "card-123");
        assert!(input.title.is_none());
        assert!(input.content.is_none());
        assert!(input.tags.is_none());
        assert!(input.collection_id.is_none());
        assert!(input.verification_interval_days.is_none());
    }

    #[test]
    fn test_update_card_input_deserializes_with_partial_fields() {
        let json = r#"{
            "card_id": "card-456",
            "title": "Updated Title",
            "tags": ["new-tag"]
        }"#;

        let input: UpdateCardInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.card_id, "card-456");
        assert_eq!(input.title.as_deref(), Some("Updated Title"));
        assert!(input.content.is_none());
        assert_eq!(input.tags, Some(vec!["new-tag".to_string()]));
    }

    #[test]
    fn test_update_card_input_missing_card_id_returns_error() {
        let json = r#"{ "title": "New Title" }"#;

        let err = serde_json::from_str::<UpdateCardInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `card_id`"));
    }

    // =========================================================================
    // Verify Card Tests
    // =========================================================================

    #[test]
    fn test_verify_card_input_deserializes_with_card_id_only() {
        let json = r#"{ "card_id": "card-123" }"#;

        let input: VerifyCardInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.card_id, "card-123");
        assert!(input.comment.is_none());
    }

    #[test]
    fn test_verify_card_input_deserializes_with_comment() {
        let json = r#"{
            "card_id": "card-456",
            "comment": "Verified after quarterly review"
        }"#;

        let input: VerifyCardInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.card_id, "card-456");
        assert_eq!(
            input.comment.as_deref(),
            Some("Verified after quarterly review")
        );
    }

    #[test]
    fn test_verify_card_input_missing_card_id_returns_error() {
        let json = r#"{ "comment": "Looks good" }"#;

        let err = serde_json::from_str::<VerifyCardInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `card_id`"));
    }

    // =========================================================================
    // Serialization/Schema Tests
    // =========================================================================

    #[test]
    fn test_verification_status_serializes_to_screaming_snake_case() {
        let trusted = serde_json::to_value(VerificationStatus::Trusted).unwrap();
        let needs_verification =
            serde_json::to_value(VerificationStatus::NeedsVerification).unwrap();
        let unverified = serde_json::to_value(VerificationStatus::Unverified).unwrap();

        assert_eq!(trusted, "TRUSTED");
        assert_eq!(needs_verification, "NEEDS_VERIFICATION");
        assert_eq!(unverified, "UNVERIFIED");
    }

    #[test]
    fn test_card_summary_serializes_with_all_fields() {
        let summary = CardSummary {
            id: "card-123".to_string(),
            title: "Test Card".to_string(),
            slug: "test-card".to_string(),
            collection: Some(CollectionInfo {
                id: "col-456".to_string(),
                name: "Engineering".to_string(),
            }),
            verification_status: VerificationStatus::Trusted,
            relevance_score: 0.95,
            last_modified: "2024-01-15T10:30:00Z".to_string(),
        };

        let json = serde_json::to_value(&summary).unwrap();

        assert_eq!(json["id"], "card-123");
        assert_eq!(json["title"], "Test Card");
        assert_eq!(json["slug"], "test-card");
        assert_eq!(json["collection"]["id"], "col-456");
        assert_eq!(json["collection"]["name"], "Engineering");
        assert_eq!(json["verification_status"], "TRUSTED");
        assert_eq!(json["relevance_score"], 0.95);
    }

    #[test]
    fn test_card_serializes_with_optional_fields_as_null() {
        let card = Card {
            id: "card-min".to_string(),
            title: "Minimal Card".to_string(),
            slug: "minimal-card".to_string(),
            content: None,
            collection: None,
            verification_status: VerificationStatus::Unverified,
            owner: None,
            verifier: None,
            verification_due_date: None,
            last_modified: "2024-01-01T00:00:00Z".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            tags: vec![],
        };

        let json = serde_json::to_value(&card).unwrap();

        assert_eq!(json["id"], "card-min");
        assert!(json["content"].is_null());
        assert!(json["collection"].is_null());
        assert!(json["owner"].is_null());
        assert!(json["verifier"].is_null());
        assert!(json["verification_due_date"].is_null());
        assert!(json["tags"].as_array().unwrap().is_empty());
    }
}
