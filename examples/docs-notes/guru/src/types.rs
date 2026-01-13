//! API types for Guru integration.

use serde::{Deserialize, Serialize};

/// Guru API search response structure.
#[derive(Debug, Deserialize)]
pub struct GuruSearchResponse {
    #[serde(default)]
    pub results: Vec<GuruCard>,
    #[serde(default)]
    pub count: u32,
}

/// Guru card structure from API responses.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuruCard {
    pub id: String,
    #[serde(rename = "preferredPhrase")]
    pub title: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub collection: Option<GuruCollection>,
    #[serde(rename = "verificationState")]
    #[serde(default)]
    pub verification_status: Option<String>,
    #[serde(default)]
    pub owner: Option<GuruUser>,
    #[serde(default)]
    pub verifier: Option<GuruUser>,
    #[serde(default)]
    pub last_modified_date: Option<String>,
    #[serde(default)]
    pub date_created: Option<String>,
    #[serde(default)]
    pub tags: Vec<GuruTag>,
    #[serde(rename = "verificationInterval")]
    #[serde(default)]
    pub verification_interval: Option<u32>,
    #[serde(default)]
    pub next_verification_date: Option<String>,
}

/// Guru collection information.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuruCollection {
    pub id: String,
    pub name: String,
}

/// Guru user information.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuruUser {
    pub email: String,
    #[serde(default)]
    pub first_name: Option<String>,
    #[serde(default)]
    pub last_name: Option<String>,
}

/// Guru tag structure.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuruTag {
    #[serde(default)]
    pub id: Option<String>,
    pub value: String,
}

/// Request body for creating a card.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCardRequest {
    pub preferred_phrase: String,
    pub content: String,
    pub collection: CollectionRef,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<GuruTag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_interval: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verifiers: Option<Vec<VerifierRef>>,
    pub share_status: String,
}

/// Collection reference for card creation/update.
#[derive(Debug, Serialize)]
pub struct CollectionRef {
    pub id: String,
}

/// Verifier reference for card creation/update.
#[derive(Debug, Serialize)]
pub struct VerifierRef {
    #[serde(rename = "type")]
    pub verifier_type: String,
    pub user: UserRef,
}

/// User reference structure.
#[derive(Debug, Serialize)]
pub struct UserRef {
    pub email: String,
}

/// Request body for updating a card.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCardRequest {
    pub preferred_phrase: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<GuruTag>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub share_status: Option<String>,
}
