//! Embedding generation support for AI-powered code operations.
//!
//! This crate provides functionality for generating vector embeddings from text
//! and Rust crate source code, using configurable backend providers (FastEmbed
//! or OpenAI). Embeddings are used for semantic search, similarity matching, and
//! other AI-powered operations.
//!
//! # Configuration
//!
//! The crate uses a two-tier configuration system:
//! - **Global config** (`Config`): User-level settings in `~/.config/operai/config.toml`
//! - **Project config** (`ProjectConfig`): Project-level overrides in `./operai.toml`
//!
//! # Providers
//!
//! Two embedding providers are supported:
//! - **FastEmbed**: Local embedding generation using quantized models
//! - **OpenAI**: Cloud-based embedding generation via OpenAI API
//!
//! # Example
//!
//! ```no_run
//! use operai_embedding::EmbeddingGenerator;
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! // Generate embeddings using default configuration
//! let mut generator = EmbeddingGenerator::from_config(None, None)?;
//! let embedding = generator.embed("Hello, world!").await?;
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod embedding;

pub use config::{Config, EmbeddingConfig, FastEmbedConfig, OpenAIConfig, ProjectConfig};
pub use embedding::{EmbeddingGenerator, Provider, write_embedding_file};

#[cfg(test)]
mod testing {
    use std::sync::OnceLock;

    use tokio::sync::Mutex;

    /// Test synchronization lock.
    ///
    /// Returns a static mutex guard used to serialize tests that modify
    /// global state (environment variables, file system, etc.). This prevents
    /// test interference when running in parallel.
    pub(crate) fn test_lock() -> tokio::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).blocking_lock()
    }
}
