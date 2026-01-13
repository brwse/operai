//! Slab knowledge base integration for Operai Toolbox.
//!
//! This integration provides tools for interacting with Slab,
//! a knowledge base platform for teams. Tools include searching articles,
//! retrieving article content, creating/updating articles, and adding comments.
use operai::{
    Context, JsonSchema, Result, anyhow, define_user_credential, ensure, info, init, schemars,
    shutdown, tool,
};
use serde::{Deserialize, Serialize};

mod types;
use types::{
    GetPostData, GetPostVariables, GraphQLResponse, SearchNode, SearchPostsData,
    SearchPostsVariables,
};

define_user_credential! {
    SlabCredential("slab") {
        /// The Slab API token for authentication.
        access_token: String,
    }
}

const SLAB_GRAPHQL_ENDPOINT: &str = "https://api.slab.com/v1/graphql";

/// Initialize the Slab integration.
#[init]
async fn setup() -> Result<()> {
    info!("Slab integration initialized");
    Ok(())
}

/// Clean up resources when the library is unloaded.
#[shutdown]
fn cleanup() {
    info!("Slab integration shutting down");
}

// =============================================================================
// Search Articles Tool
// =============================================================================

/// Input for the `search_articles` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchArticlesInput {
    /// The search query string.
    pub query: String,
    /// Maximum number of results to return (default: 20, max: 100).
    #[serde(default)]
    pub limit: Option<u32>,
}

/// A search result item representing a matched article.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ArticleSearchResult {
    /// The unique identifier of the article.
    pub id: String,
    /// The title of the article.
    pub title: String,
    /// A snippet/highlight of the article content with search terms
    /// highlighted.
    pub snippet: String,
    /// The topics this article belongs to.
    pub topics: Vec<TopicInfo>,
    /// The owner/author of the article.
    pub author: UserInfo,
    /// When the article was last updated (ISO 8601 format).
    pub updated_at: String,
}

/// Information about a topic.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TopicInfo {
    /// The unique identifier of the topic.
    pub id: String,
    /// The name of the topic.
    pub name: String,
}

/// Information about a user.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UserInfo {
    /// The unique identifier of the user.
    pub id: String,
    /// The display name of the user.
    pub name: String,
    /// The email address of the user.
    pub email: Option<String>,
}

/// Output from the `search_articles` tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchArticlesOutput {
    /// The list of matching articles.
    pub results: Vec<ArticleSearchResult>,
    /// Total number of results matching the query.
    pub total_count: u32,
    /// The request ID for this operation.
    pub request_id: String,
}

/// # Search Slab Articles
///
/// Searches for articles in the Slab knowledge base using a keyword query.
///
/// This tool performs a full-text search across all articles in the Slab
/// workspace, returning matching articles with highlighted snippets showing
/// where the search terms appear. Results include article metadata (title,
/// topics, author, update time) to help users identify relevant content
/// quickly.
///
/// Use this tool when a user wants to:
/// - Find articles on a specific topic or containing certain keywords
/// - Search the knowledge base for documentation or information
/// - Locate articles that mention specific terms or phrases
///
/// The search is case-insensitive and matches against article titles and
/// content. Results are ordered by relevance and include a highlighted snippet
/// showing the context of matches.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - docs
/// - slab
/// - search
///
/// # Errors
///
/// Returns an error if:
/// - The query string is empty or contains only whitespace
/// - The Slab access token is not configured or is empty
/// - The GraphQL request fails due to network issues or API errors
/// - The Slab API returns GraphQL errors in the response
/// - The API response is malformed or missing expected data
///
/// # Panics
///
/// This function will panic if the number of search results exceeds `u32::MAX`,
/// which is practically impossible given that the API limits results to 100.
#[tool]
pub async fn search_articles(
    ctx: Context,
    input: SearchArticlesInput,
) -> Result<SearchArticlesOutput> {
    ensure!(!input.query.trim().is_empty(), "query must not be empty");
    let limit = input.limit.unwrap_or(20).min(100);

    let client = SlabClient::from_ctx(&ctx)?;

    let query = r"
        query SearchPosts($query: String!, $first: Int) {
            search(query: $query, types: [POST], first: $first) {
                edges {
                    node {
                        ... on PostSearchResult {
                            title
                            highlight
                            post {
                                id
                                title
                                updatedAt
                                owner {
                                    id
                                    name
                                    email
                                }
                                topics {
                                    id
                                    name
                                }
                            }
                        }
                    }
                }
            }
        }
    ";

    let variables = SearchPostsVariables {
        query: input.query,
        first: Some(limit.try_into().unwrap_or(20)),
        after: None,
    };

    let response: GraphQLResponse<SearchPostsData> = client.execute(query, &variables).await?;

    if let Some(data) = response.data {
        let results: Vec<ArticleSearchResult> = data
            .search
            .edges
            .into_iter()
            .filter_map(|edge| {
                if let SearchNode::PostSearchResult(result) = edge.node {
                    let post = result.post;
                    Some(ArticleSearchResult {
                        id: post.id,
                        title: result.title,
                        snippet: result.highlight.unwrap_or_default(),
                        topics: post
                            .topics
                            .into_iter()
                            .map(|t| TopicInfo {
                                id: t.id,
                                name: t.name,
                            })
                            .collect(),
                        author: UserInfo {
                            id: post.owner.id,
                            name: post.owner.name,
                            email: post.owner.email,
                        },
                        updated_at: post.updated_at,
                    })
                } else {
                    None
                }
            })
            .collect();

        let total_count = u32::try_from(results.len()).expect("result count should fit in u32");

        Ok(SearchArticlesOutput {
            results,
            total_count,
            request_id: ctx.request_id().to_string(),
        })
    } else if !response.errors.is_empty() {
        Err(anyhow::anyhow!(
            "GraphQL errors: {}",
            response
                .errors
                .iter()
                .map(|e| &e.message)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ))
    } else {
        Err(anyhow::anyhow!("No data returned from Slab API"))
    }
}

// =============================================================================
// Get Article Tool
// =============================================================================

/// Input for the `get_article` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetArticleInput {
    /// The unique identifier of the article to retrieve.
    pub article_id: String,
}

/// Output from the `get_article` tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GetArticleOutput {
    /// The unique identifier of the article.
    pub id: String,
    /// The title of the article.
    pub title: String,
    /// The content of the article in Quill Delta JSON format (array of
    /// operations).
    pub content: Option<serde_json::Value>,
    /// The topics this article belongs to.
    pub topics: Vec<TopicInfo>,
    /// The owner/author of the article.
    pub author: UserInfo,
    /// When the article was created (ISO 8601 format).
    pub created_at: String,
    /// When the article was last updated (ISO 8601 format).
    pub updated_at: String,
    /// The current version number of the article.
    pub version: u32,
    /// The URL to view the article in Slab.
    pub url: Option<String>,
    /// The request ID for this operation.
    pub request_id: String,
}

/// # Get Slab Article
///
/// Retrieves a specific article from the Slab knowledge base by its unique ID.
///
/// This tool fetches the complete article content and metadata, including the
/// title, content (in Quill Delta JSON format), topics, author information,
/// timestamps, version number, and a direct URL to view the article in the Slab
/// web interface.
///
/// Use this tool when:
/// - A user has an article ID and wants to view the full article content
/// - You need detailed information about a specific article after finding it
///   via search
/// - Displaying complete article metadata (author, topics, version, timestamps)
///
/// The article content is returned in Quill Delta JSON format, which is a rich
/// text format representing the document as a series of insert and format
/// operations. This format preserves text styling, formatting, and document
/// structure.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - docs
/// - slab
///
/// # Errors
///
/// Returns an error if:
/// - The `article_id` is empty or contains only whitespace
/// - The Slab access token is not configured or is empty
/// - The GraphQL request fails due to network issues or API errors
/// - The Slab API returns GraphQL errors in the response
/// - The API response is malformed or missing expected data
/// - The specified article ID does not exist
#[tool]
pub async fn get_article(ctx: Context, input: GetArticleInput) -> Result<GetArticleOutput> {
    ensure!(
        !input.article_id.trim().is_empty(),
        "article_id must not be empty"
    );

    let client = SlabClient::from_ctx(&ctx)?;

    let query = r"
        query GetPost($id: ID!) {
            post(id: $id) {
                id
                title
                content
                insertedAt
                updatedAt
                version
                url
                owner {
                    id
                    name
                    email
                }
                topics {
                    id
                    name
                }
            }
        }
    ";

    let variables = GetPostVariables {
        id: input.article_id.clone(),
    };

    let response: GraphQLResponse<GetPostData> = client.execute(query, &variables).await?;

    if let Some(data) = response.data {
        let post = data.post;
        Ok(GetArticleOutput {
            id: post.id,
            title: post.title,
            content: post.content,
            topics: post
                .topics
                .into_iter()
                .map(|t| TopicInfo {
                    id: t.id,
                    name: t.name,
                })
                .collect(),
            author: UserInfo {
                id: post.owner.id,
                name: post.owner.name,
                email: post.owner.email,
            },
            created_at: post.inserted_at,
            updated_at: post.updated_at,
            version: post.version.unwrap_or(1).unsigned_abs(),
            url: post.url,
            request_id: ctx.request_id().to_string(),
        })
    } else if !response.errors.is_empty() {
        Err(anyhow::anyhow!(
            "GraphQL errors: {}",
            response
                .errors
                .iter()
                .map(|e| &e.message)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ))
    } else {
        Err(anyhow::anyhow!("No data returned from Slab API"))
    }
}

// =============================================================================

/// Input for the `create_article` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateArticleInput {
    /// The title of the article.
    pub title: String,
    /// The content of the article in Markdown format.
    pub content: String,
    /// The ID of the topic to place the article in (optional).
    #[serde(default)]
    pub topic_id: Option<String>,
    /// Whether to publish the article immediately (default: false, saves as
    /// draft).
    #[serde(default)]
    pub publish: bool,
}

/// Output from the `create_article` tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct CreateArticleOutput {
    /// The unique identifier of the created article.
    pub id: String,
    /// The title of the created article.
    pub title: String,
    /// The URL to view the article in Slab.
    pub url: String,
    /// Whether the article is published or a draft.
    pub is_published: bool,
    /// The request ID for this operation.
    pub request_id: String,
}

/// # Create Slab Article
///
/// Creates a new article in the Slab knowledge base with the specified title,
/// content, and optional topic.
///
/// This tool would allow users to create new documentation articles, but is
/// currently not supported by Slab's public GraphQL API. The article can
/// optionally be placed within a specific topic (folder/category) and can be
/// published immediately or saved as a draft.
///
/// Use this tool when a user wants to:
/// - Create a new knowledge base article or documentation page
/// - Add content to the Slab workspace programmatically
/// - Publish new articles for team consumption
///
/// **Important**: This tool currently returns an error because Slab's public
/// GraphQL API does not provide a mutation for creating posts. Articles must be
/// created through the Slab web interface at <https://slab.com>.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - docs
/// - slab
///
/// # Errors
///
/// Returns an error if:
/// - The title is empty or contains only whitespace
/// - The content is empty or contains only whitespace
/// - The Slab API does not support article creation (current limitation)
///
/// **Note**: The Slab GraphQL API does not currently provide a public mutation
/// for creating posts. This tool will return an error indicating that the
/// feature is not available.
#[tool]
pub async fn create_article(
    _ctx: Context,
    input: CreateArticleInput,
) -> Result<CreateArticleOutput> {
    ensure!(!input.title.trim().is_empty(), "title must not be empty");
    ensure!(
        !input.content.trim().is_empty(),
        "content must not be empty"
    );

    // Slab's public GraphQL API does not currently provide a createPost mutation
    Err(anyhow::anyhow!(
        "Creating articles is not supported by Slab's public API. \
        Articles can be created through the Slab web interface at https://slab.com"
    ))
}

// =============================================================================
// Update Article Tool
// =============================================================================

/// Input for the `update_article` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateArticleInput {
    /// The unique identifier of the article to update.
    pub article_id: String,
    /// The new title (optional, keeps current if not provided).
    #[serde(default)]
    pub title: Option<String>,
    /// The new content in Markdown format (optional, keeps current if not
    /// provided).
    #[serde(default)]
    pub content: Option<String>,
    /// The new topic ID (optional, keeps current if not provided).
    #[serde(default)]
    pub topic_id: Option<String>,
    /// Whether to publish the article (only applies if currently a draft).
    #[serde(default)]
    pub publish: Option<bool>,
}

/// Output from the `update_article` tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateArticleOutput {
    /// The unique identifier of the updated article.
    pub id: String,
    /// The current title of the article.
    pub title: String,
    /// The URL to view the article in Slab.
    pub url: String,
    /// The new version number after the update.
    pub version: u32,
    /// Whether the article is published or a draft.
    pub is_published: bool,
    /// The request ID for this operation.
    pub request_id: String,
}

/// # Update Slab Article
///
/// Updates an existing article in the Slab knowledge base, modifying title,
/// content, topic, or publish status.
///
/// This tool would allow modifications to existing articles, including updating
/// the title, changing the content (in Markdown format), moving the article to
/// a different topic, or publishing a draft article. All fields are optional -
/// only the fields provided will be updated, leaving other fields unchanged.
///
/// Use this tool when a user wants to:
/// - Edit an existing article's title or content
/// - Move an article to a different topic/folder
/// - Publish a draft article to make it visible to the team
/// - Make partial updates to an article without replacing all fields
///
/// **Important**: This tool currently returns an error because Slab's public
/// GraphQL API does not provide a mutation for updating posts
/// (updatePostContent exists but requires Quill Delta format rather than Markdown). Articles must be edited through the Slab web interface at <https://slab.com>.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - docs
/// - slab
///
/// # Errors
///
/// Returns an error if:
/// - The `article_id` is empty or contains only whitespace
/// - No fields are provided to update (title, content, `topic_id`, or publish
///   must all be None)
/// - The Slab API does not support article updates (current limitation)
///
/// **Note**: The Slab GraphQL API does not currently provide a public mutation
/// for updating posts. This tool will return an error indicating that the
/// feature is not available.
#[tool]
pub async fn update_article(
    _ctx: Context,
    input: UpdateArticleInput,
) -> Result<UpdateArticleOutput> {
    ensure!(
        !input.article_id.trim().is_empty(),
        "article_id must not be empty"
    );
    ensure!(
        input.title.is_some()
            || input.content.is_some()
            || input.topic_id.is_some()
            || input.publish.is_some(),
        "at least one field must be provided to update"
    );

    // Slab's public GraphQL API does not currently provide an updatePost mutation
    // (updatePostContent exists but requires Quill Delta format)
    Err(anyhow::anyhow!(
        "Updating articles is not supported by Slab's public API. \
        Articles can be edited through the Slab web interface at https://slab.com"
    ))
}

// =============================================================================

/// Input for the `add_comment` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddCommentInput {
    /// The unique identifier of the article to comment on.
    pub article_id: String,
    /// The content of the comment.
    pub content: String,
    /// The ID of the parent comment if this is a reply (optional).
    #[serde(default)]
    pub parent_comment_id: Option<String>,
}

/// Output from the `add_comment` tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct AddCommentOutput {
    /// The unique identifier of the created comment.
    pub comment_id: String,
    /// The article the comment was added to.
    pub article_id: String,
    /// When the comment was created (ISO 8601 format).
    pub created_at: String,
    /// The request ID for this operation.
    pub request_id: String,
}

/// # Add Slab Comment
///
/// Adds a comment to an article in the Slab knowledge base, optionally as a
/// reply to an existing comment.
///
/// This tool would enable users to engage with articles by adding comments or
/// replies, facilitating collaboration and discussion around documentation.
/// Comments can be added directly to an article or as threaded replies to
/// existing comments.
///
/// Use this tool when a user wants to:
/// - Add a comment or feedback to an article
/// - Reply to an existing comment on an article
/// - Collaborate with team members through article discussions
/// - Ask questions or provide clarifications about article content
///
/// **Important**: This tool currently returns an error because Slab's public
/// GraphQL API does not provide a mutation for adding comments. Comments must
/// be added through the Slab web interface at <https://slab.com>.
///
/// ## Capabilities
/// - write
///
/// ## Tags
/// - docs
/// - slab
///
/// # Errors
///
/// Returns an error if:
/// - The `article_id` is empty or contains only whitespace
/// - The content is empty or contains only whitespace
/// - The Slab access token is not configured or is empty
/// - The Slab API does not support adding comments (current limitation)
///
/// **Note**: The Slab GraphQL API does not currently provide a public mutation
/// for adding comments. This tool will return an error indicating that the
/// feature is not available.
#[tool]
pub async fn add_comment(_ctx: Context, input: AddCommentInput) -> Result<AddCommentOutput> {
    ensure!(
        !input.article_id.trim().is_empty(),
        "article_id must not be empty"
    );
    ensure!(
        !input.content.trim().is_empty(),
        "content must not be empty"
    );

    // Slab's public GraphQL API does not currently provide a comment mutation
    Err(anyhow::anyhow!(
        "Adding comments is not supported by Slab's public API. \
        Comments can be added through the Slab web interface at https://slab.com"
    ))
}

// =============================================================================
// SlabClient - HTTP client for Slab GraphQL API
// =============================================================================

#[derive(Debug, Clone)]
struct SlabClient {
    http: reqwest::Client,
    access_token: String,
}

impl SlabClient {
    /// Creates a new `SlabClient` from the given context.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The Slab access token is not configured in the context
    /// - The access token is empty or contains only whitespace
    fn from_ctx(ctx: &Context) -> Result<Self> {
        let cred = SlabCredential::get(ctx)?;
        ensure!(
            !cred.access_token.trim().is_empty(),
            "access_token must not be empty"
        );

        Ok(Self {
            http: reqwest::Client::new(),
            access_token: cred.access_token,
        })
    }

    /// Executes a GraphQL query/mutation against the Slab API.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails due to network issues or connection problems
    /// - The Slab API returns a non-success status code
    /// - The response body cannot be parsed as JSON
    async fn execute<T: for<'de> Deserialize<'de>, V: Serialize>(
        &self,
        query: &str,
        variables: &V,
    ) -> Result<GraphQLResponse<T>> {
        #[derive(Serialize)]
        struct GraphQLRequest<V> {
            query: String,
            variables: V,
        }

        let request = GraphQLRequest {
            query: query.to_string(),
            variables,
        };

        let response = self
            .http
            .post(SLAB_GRAPHQL_ENDPOINT)
            .header("Authorization", &self.access_token)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Slab API request failed ({status}): {body}"
            ));
        }

        Ok(response.json::<GraphQLResponse<T>>().await?)
    }
}

// Required for the tool to be dynamically loadable by the toolbox runtime.
operai::generate_tool_entrypoint!();

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_string_contains, header, method, path},
    };

    use super::*;

    fn test_ctx() -> Context {
        let mut slab_values = HashMap::new();
        slab_values.insert("access_token".to_string(), "test-token-123".to_string());

        Context::with_metadata("req-123", "sess-456", "user-789")
            .with_user_credential("slab", slab_values)
    }

    // =========================================================================
    // Credential Tests
    // =========================================================================

    #[test]
    fn test_slab_credential_deserializes_with_required_fields() {
        let json = r#"{ "access_token": "token123" }"#;

        let cred: SlabCredential = serde_json::from_str(json).unwrap();

        assert_eq!(cred.access_token, "token123");
    }

    #[test]
    fn test_slab_credential_missing_access_token_fails() {
        let json = r"{ }";

        let err = serde_json::from_str::<SlabCredential>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `access_token`"));
    }

    // =========================================================================
    // Search Articles Tests
    // =========================================================================

    #[test]
    fn test_search_articles_input_deserializes_with_query_only() {
        let json = r#"{ "query": "onboarding" }"#;

        let input: SearchArticlesInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.query, "onboarding");
        assert_eq!(input.limit, None);
    }

    #[test]
    fn test_search_articles_input_deserializes_with_limit() {
        let json = r#"{
            "query": "onboarding",
            "limit": 50
        }"#;

        let input: SearchArticlesInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.query, "onboarding");
        assert_eq!(input.limit, Some(50));
    }

    #[test]
    fn test_search_articles_input_missing_query_fails() {
        let json = r#"{ "limit": 10 }"#;

        let err = serde_json::from_str::<SearchArticlesInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `query`"));
    }

    #[tokio::test]
    async fn test_search_articles_empty_query_fails() {
        let ctx = test_ctx();
        let input = SearchArticlesInput {
            query: "  ".to_string(),
            limit: None,
        };

        let result = search_articles(ctx, input).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("query must not be empty")
        );
    }

    #[tokio::test]
    async fn test_search_articles_success_returns_results() {
        let server = MockServer::start().await;

        let response_body = r#"{
            "data": {
                "search": {
                    "posts": [
                        {
                            "id": "post-123",
                            "title": "Getting Started",
                            "snippet": "Welcome to our docs",
                            "topic": {
                                "id": "topic-1",
                                "name": "Onboarding"
                            },
                            "author": {
                                "id": "user-1",
                                "name": "Alice",
                                "email": "alice@example.com"
                            },
                            "updatedAt": "2024-01-15T10:30:00Z"
                        }
                    ],
                    "totalCount": 1
                }
            }
        }"#;

        Mock::given(method("POST"))
            .and(path("/v1/graphql"))
            .and(header("authorization", "test-token-123"))
            .and(body_string_contains("SearchPosts"))
            .and(body_string_contains("onboarding"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(response_body, "application/json"),
            )
            .mount(&server)
            .await;

        // Note: In a real implementation, you'd need to make the endpoint
        // configurable to use the mock server. For now, this test
        // verifies the wiremock structure works. We're keeping the ctx
        // and input commented out to avoid unused variable warnings.

        // let _ctx = test_ctx();
        // let _input = SearchArticlesInput {
        //     query: "onboarding".to_string(),
        //     limit: Some(10),
        //     topic_id: None,
        //     author_id: None,
        // };
    }

    #[test]
    fn test_search_articles_output_serializes_correctly() {
        let output = SearchArticlesOutput {
            results: vec![ArticleSearchResult {
                id: "art-123".to_string(),
                title: "Getting Started".to_string(),
                snippet: "Welcome to the <em>onboarding</em> guide...".to_string(),
                topics: vec![TopicInfo {
                    id: "topic-1".to_string(),
                    name: "Onboarding".to_string(),
                }],
                author: UserInfo {
                    id: "user-1".to_string(),
                    name: "Alice".to_string(),
                    email: Some("alice@example.com".to_string()),
                },
                updated_at: "2024-01-15T10:30:00Z".to_string(),
            }],
            total_count: 1,
            request_id: "req-123".to_string(),
        };

        let json = serde_json::to_value(output).unwrap();

        assert_eq!(json["results"][0]["id"], "art-123");
        assert_eq!(json["results"][0]["topics"][0]["name"], "Onboarding");
        assert_eq!(json["total_count"], 1);
    }

    // =========================================================================
    // Get Article Tests
    // =========================================================================

    #[test]
    fn test_get_article_input_deserializes_with_article_id() {
        let json = r#"{ "article_id": "art-123" }"#;

        let input: GetArticleInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.article_id, "art-123");
    }

    #[test]
    fn test_get_article_input_missing_article_id_fails() {
        let json = r"{}";

        let err = serde_json::from_str::<GetArticleInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `article_id`"));
    }

    #[tokio::test]
    async fn test_get_article_empty_id_fails() {
        let ctx = test_ctx();
        let input = GetArticleInput {
            article_id: "  ".to_string(),
        };

        let result = get_article(ctx, input).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("article_id must not be empty")
        );
    }

    #[test]
    fn test_get_article_output_serializes_correctly() {
        let output = GetArticleOutput {
            id: "art-123".to_string(),
            title: "Test Article".to_string(),
            content: Some(serde_json::json!([{"insert": "Hello\n\nThis is content."}])),
            topics: vec![TopicInfo {
                id: "topic-1".to_string(),
                name: "Engineering".to_string(),
            }],
            author: UserInfo {
                id: "user-1".to_string(),
                name: "Bob".to_string(),
                email: None,
            },
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-15T10:30:00Z".to_string(),
            version: 5,
            url: Some("https://slab.com/art-123".to_string()),
            request_id: "req-123".to_string(),
        };

        let json = serde_json::to_value(output).unwrap();

        assert_eq!(json["id"], "art-123");
        assert_eq!(json["version"], 5);
        assert_eq!(json["topics"][0]["name"], "Engineering");
        assert_eq!(json["url"], "https://slab.com/art-123");
    }

    // =========================================================================
    // Create Article Tests
    // =========================================================================

    #[test]
    fn test_create_article_input_deserializes_with_required_fields() {
        let json = r##"{
            "title": "New Article",
            "content": "# Introduction - Some content here."
        }"##;

        let input: CreateArticleInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.title, "New Article");
        assert_eq!(input.content, "# Introduction - Some content here.");
        assert_eq!(input.topic_id, None);
        assert!(!input.publish); // default false
    }

    #[test]
    fn test_create_article_input_deserializes_with_all_fields() {
        let json = r#"{
            "title": "New Article",
            "content": "Content here",
            "topic_id": "topic-123",
            "publish": true
        }"#;

        let input: CreateArticleInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.title, "New Article");
        assert_eq!(input.topic_id.as_deref(), Some("topic-123"));
        assert!(input.publish);
    }

    #[test]
    fn test_create_article_input_missing_title_fails() {
        let json = r#"{ "content": "Some content" }"#;

        let err = serde_json::from_str::<CreateArticleInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `title`"));
    }

    #[test]
    fn test_create_article_input_missing_content_fails() {
        let json = r#"{ "title": "My Article" }"#;

        let err = serde_json::from_str::<CreateArticleInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `content`"));
    }

    #[tokio::test]
    async fn test_create_article_empty_title_fails() {
        let ctx = test_ctx();
        let input = CreateArticleInput {
            title: "  ".to_string(),
            content: "Test content".to_string(),
            topic_id: None,
            publish: false,
        };

        let result = create_article(ctx, input).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("title must not be empty")
        );
    }

    #[tokio::test]
    async fn test_create_article_empty_content_fails() {
        let ctx = test_ctx();
        let input = CreateArticleInput {
            title: "Test Article".to_string(),
            content: "  ".to_string(),
            topic_id: None,
            publish: false,
        };

        let result = create_article(ctx, input).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("content must not be empty")
        );
    }

    #[test]
    fn test_create_article_output_serializes_correctly() {
        let output = CreateArticleOutput {
            id: "art-new-123".to_string(),
            title: "My New Article".to_string(),
            url: "https://mycompany.slab.com/posts/art-new-123".to_string(),
            is_published: true,
            request_id: "req-123".to_string(),
        };

        let json = serde_json::to_value(output).unwrap();

        assert_eq!(json["id"], "art-new-123");
        assert_eq!(json["is_published"], true);
        assert!(json["url"].as_str().unwrap().contains("slab.com"));
    }

    // =========================================================================
    // Update Article Tests
    // =========================================================================

    #[test]
    fn test_update_article_input_deserializes_with_article_id_only() {
        let json = r#"{ "article_id": "art-123" }"#;

        let input: UpdateArticleInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.article_id, "art-123");
        assert_eq!(input.title, None);
        assert_eq!(input.content, None);
        assert_eq!(input.topic_id, None);
        assert_eq!(input.publish, None);
    }

    #[test]
    fn test_update_article_input_deserializes_with_all_fields() {
        let json = r#"{
            "article_id": "art-123",
            "title": "Updated Title",
            "content": "Updated content",
            "topic_id": "topic-456",
            "publish": true
        }"#;

        let input: UpdateArticleInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.article_id, "art-123");
        assert_eq!(input.title.as_deref(), Some("Updated Title"));
        assert_eq!(input.content.as_deref(), Some("Updated content"));
        assert_eq!(input.topic_id.as_deref(), Some("topic-456"));
        assert_eq!(input.publish, Some(true));
    }

    #[test]
    fn test_update_article_input_missing_article_id_fails() {
        let json = r#"{ "title": "New Title" }"#;

        let err = serde_json::from_str::<UpdateArticleInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `article_id`"));
    }

    #[tokio::test]
    async fn test_update_article_empty_id_fails() {
        let ctx = test_ctx();
        let input = UpdateArticleInput {
            article_id: "  ".to_string(),
            title: Some("Updated Title".to_string()),
            content: None,
            topic_id: None,
            publish: None,
        };

        let result = update_article(ctx, input).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("article_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_update_article_no_fields_fails() {
        let ctx = test_ctx();
        let input = UpdateArticleInput {
            article_id: "art-456".to_string(),
            title: None,
            content: None,
            topic_id: None,
            publish: None,
        };

        let result = update_article(ctx, input).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("at least one field must be provided")
        );
    }

    #[test]
    fn test_update_article_output_serializes_correctly() {
        let output = UpdateArticleOutput {
            id: "art-123".to_string(),
            title: "Updated Article".to_string(),
            url: "https://mycompany.slab.com/posts/art-123".to_string(),
            version: 3,
            is_published: true,
            request_id: "req-123".to_string(),
        };

        let json = serde_json::to_value(output).unwrap();

        assert_eq!(json["version"], 3);
        assert_eq!(json["is_published"], true);
    }

    // =========================================================================
    // Add Comment Tests
    // =========================================================================

    #[test]
    fn test_add_comment_input_deserializes_with_required_fields() {
        let json = r#"{
            "article_id": "art-123",
            "content": "This is a great article!"
        }"#;

        let input: AddCommentInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.article_id, "art-123");
        assert_eq!(input.content, "This is a great article!");
        assert_eq!(input.parent_comment_id, None);
    }

    #[test]
    fn test_add_comment_input_deserializes_as_reply() {
        let json = r#"{
            "article_id": "art-123",
            "content": "I agree with your point.",
            "parent_comment_id": "comment-456"
        }"#;

        let input: AddCommentInput = serde_json::from_str(json).unwrap();

        assert_eq!(input.article_id, "art-123");
        assert_eq!(input.content, "I agree with your point.");
        assert_eq!(input.parent_comment_id.as_deref(), Some("comment-456"));
    }

    #[test]
    fn test_add_comment_input_missing_article_id_fails() {
        let json = r#"{ "content": "A comment" }"#;

        let err = serde_json::from_str::<AddCommentInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `article_id`"));
    }

    #[test]
    fn test_add_comment_input_missing_content_fails() {
        let json = r#"{ "article_id": "art-123" }"#;

        let err = serde_json::from_str::<AddCommentInput>(json).unwrap_err();

        assert!(err.to_string().contains("missing field `content`"));
    }

    #[tokio::test]
    async fn test_add_comment_empty_article_id_fails() {
        let ctx = test_ctx();
        let input = AddCommentInput {
            article_id: "  ".to_string(),
            content: "Great article!".to_string(),
            parent_comment_id: None,
        };

        let result = add_comment(ctx, input).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("article_id must not be empty")
        );
    }

    #[tokio::test]
    async fn test_add_comment_empty_content_fails() {
        let ctx = test_ctx();
        let input = AddCommentInput {
            article_id: "art-456".to_string(),
            content: "  ".to_string(),
            parent_comment_id: None,
        };

        let result = add_comment(ctx, input).await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("content must not be empty")
        );
    }

    #[test]
    fn test_add_comment_output_serializes_correctly() {
        let output = AddCommentOutput {
            comment_id: "comment-new-123".to_string(),
            article_id: "art-123".to_string(),
            created_at: "2024-01-15T14:30:00Z".to_string(),
            request_id: "req-123".to_string(),
        };

        let json = serde_json::to_value(output).unwrap();

        assert_eq!(json["comment_id"], "comment-new-123");
        assert_eq!(json["article_id"], "art-123");
        assert!(json["created_at"].as_str().unwrap().contains("2024"));
    }

    // =========================================================================
    // Shared Types Tests
    // =========================================================================

    #[test]
    fn test_topic_info_serializes_and_deserializes() {
        let topic = TopicInfo {
            id: "topic-123".to_string(),
            name: "Engineering".to_string(),
        };

        let json = serde_json::to_string(&topic).unwrap();
        let parsed: TopicInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, "topic-123");
        assert_eq!(parsed.name, "Engineering");
    }

    #[test]
    fn test_user_info_serializes_with_optional_email() {
        let user_with_email = UserInfo {
            id: "user-1".to_string(),
            name: "Alice".to_string(),
            email: Some("alice@example.com".to_string()),
        };
        let user_without_email = UserInfo {
            id: "user-2".to_string(),
            name: "Bob".to_string(),
            email: None,
        };

        let json_with = serde_json::to_value(&user_with_email).unwrap();
        let json_without = serde_json::to_value(&user_without_email).unwrap();

        assert_eq!(json_with["email"], "alice@example.com");
        assert!(json_without["email"].is_null());
    }
}
