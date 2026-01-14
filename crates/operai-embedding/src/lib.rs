//! Shared embedding + config utilities for Operai.

pub mod config;
pub mod embedding;

pub use config::{Config, EmbeddingConfig, FastEmbedConfig, OpenAIConfig, ProjectConfig};
pub use embedding::{EmbeddingGenerator, Provider, write_embedding_file};

#[cfg(test)]
mod testing {
    use std::sync::OnceLock;

    use tokio::sync::Mutex;

    pub(crate) fn test_lock() -> tokio::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).blocking_lock()
    }
}
