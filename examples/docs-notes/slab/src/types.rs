//! Internal types for Slab GraphQL API responses.
//!
//! These types are used to deserialize responses from the Slab GraphQL API.
//! They are kept separate from the public tool input/output types to allow
//! for easy evolution of the API mapping layer.

use serde::{Deserialize, Serialize};

/// GraphQL response wrapper.
#[derive(Debug, Deserialize)]
pub struct GraphQLResponse<T> {
    pub data: Option<T>,
    #[serde(default)]
    pub errors: Vec<GraphQLError>,
}

/// GraphQL error.
#[derive(Debug, Deserialize)]
pub struct GraphQLError {
    pub message: String,
}

// =============================================================================
// Search Posts Query Response
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct SearchPostsData {
    pub search: SearchConnection,
}

/// Slab uses GraphQL Relay cursor-based pagination for search results.
#[derive(Debug, Deserialize)]
pub struct SearchConnection {
    pub edges: Vec<SearchEdge>,
}

#[derive(Debug, Deserialize)]
pub struct SearchEdge {
    pub node: SearchNode,
}

/// Search node can be different types (`PostSearchResult`, `TopicSearchResult`,
/// etc.)
#[derive(Debug, Deserialize)]
#[expect(
    clippy::large_enum_variant,
    reason = "PostSearchResult contains nested data structures"
)]
#[serde(tag = "__typename")]
pub enum SearchNode {
    #[serde(rename = "PostSearchResult")]
    PostSearchResult(PostSearchResult),
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct PostSearchResult {
    pub title: String,
    pub highlight: Option<String>,
    pub post: Post,
}

// =============================================================================
// Post Types
// =============================================================================

/// Slab Post type - matches actual GraphQL schema
/// Field names verified against: <https://studio.apollographql.com/public/Slab/variant/current/explorer>
#[derive(Debug, Deserialize)]
pub struct Post {
    pub id: String,
    pub title: String,
    /// Content in Quill Delta format (JSON array of operations)
    #[serde(default)]
    pub content: Option<serde_json::Value>,
    /// Slab uses insertedAt, not createdAt
    #[serde(rename = "insertedAt")]
    pub inserted_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(default)]
    pub version: Option<i32>,
    /// Slab uses owner, not author
    pub owner: User,
    #[serde(default)]
    pub topics: Vec<Topic>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Topic {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
}

// =============================================================================
// Get Post Query Response
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct GetPostData {
    pub post: Post,
}

// =============================================================================
// GraphQL Request Variables
// =============================================================================

#[derive(Debug, Serialize)]
pub struct SearchPostsVariables {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GetPostVariables {
    pub id: String,
}
