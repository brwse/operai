//! Semantic search functionality for tool discovery.
//!
//! This module provides the [`SearchEmbedder`] trait for generating vector
//! embeddings from text queries. This enables semantic similarity search across
//! the tool registry, allowing clients to discover relevant tools using natural
//! language queries.

use futures::future::BoxFuture;

/// Future type for embedding generation.
///
/// Used by [`SearchEmbedder`] to asynchronously generate query embeddings
/// for semantic tool search.
pub type SearchEmbedFuture<'a> = BoxFuture<'a, Result<Vec<f32>, String>>;

/// Embedding generator for semantic tool search.
///
/// Implementors generate vector embeddings from text queries, enabling
/// semantic similarity search across the tool registry. The embedding
/// is compared against pre-computed tool embeddings to find relevant tools.
///
/// # Example
///
/// ```ignore
/// struct MyEmbedder;
///
/// impl SearchEmbedder for MyEmbedder {
///     fn embed_query(&self, query: &str) -> SearchEmbedFuture<'_> {
///         Box::pin(async {
///             // Generate embedding vector (e.g., using an ML model)
///             Ok(vec![0.1, 0.2, 0.3, /* ... */])
///         })
///     }
/// }
/// ```
pub trait SearchEmbedder: Send + Sync {
    /// Generate an embedding vector for the given query text.
    ///
    /// The returned vector should have the same dimensionality as the
    /// tool embeddings used when indexing the tool registry.
    fn embed_query(&self, query: &str) -> SearchEmbedFuture<'_>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestEmbedder;

    impl SearchEmbedder for TestEmbedder {
        fn embed_query(&self, _query: &str) -> SearchEmbedFuture<'_> {
            Box::pin(async { Ok(vec![1.0, 0.0]) })
        }
    }

    #[tokio::test]
    async fn test_embedder_returns_vector() {
        let embedder = TestEmbedder;
        let result = embedder.embed_query("test query").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![1.0, 0.0]);
    }
}
