//! Embedding utilities backed by embed_anything.
//!
//! Supports both local and remote embedding providers:
//! - **Local**: Hugging Face models via embed_anything (default)
//! - **Remote**: OpenAI, Gemini, Cohere cloud APIs

use std::{fmt::Write as _, path::Path, sync::Arc};

use anyhow::{Context, Result, bail};
use embed_anything::embeddings::embed::Embedder;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Provider {
    Local,
    OpenAI,
    Gemini,
    Cohere,
}

impl Provider {
    fn from_str(value: &str) -> Result<Self> {
        match value.to_lowercase().as_str() {
            "openai" => Ok(Self::OpenAI),
            "gemini" => Ok(Self::Gemini),
            "cohere" => Ok(Self::Cohere),
            "local" | "fastembed" | "hf" | "huggingface" => Ok(Self::Local),
            _ => bail!(
                "unknown embedding provider: {value}. Supported remote: openai, gemini, cohere; local: fastembed"
            ),
        }
    }

    const fn default_model(self) -> &'static str {
        match self {
            Self::Local => "nomic-embed-text-v1.5",
            Self::OpenAI => "text-embedding-3-small",
            Self::Gemini => "text-embedding-004",
            Self::Cohere => "embed-english-v3.0",
        }
    }

    const fn api_key_env(self) -> &'static str {
        match self {
            Self::OpenAI => "OPENAI_API_KEY",
            Self::Gemini => "GEMINI_API_KEY",
            Self::Cohere => "COHERE_API_KEY",
            Self::Local => "",
        }
    }

    const fn cloud_name(self) -> &'static str {
        match self {
            Self::OpenAI => "OpenAI",
            Self::Gemini => "Gemini",
            Self::Cohere => "Cohere",
            Self::Local => panic!("local provider does not have cloud name"),
        }
    }
}

#[derive(Clone)]
pub struct EmbeddingGenerator {
    embedder: Arc<Embedder>,
}

impl EmbeddingGenerator {
    /// Creates an embedding generator from project config.
    ///
    /// # Arguments
    ///
    /// * `config` - Operai project config containing embedding settings
    pub fn from_config(config: &operai_core::Config) -> Result<Self> {
        let embedding_type = config
            .embedding
            .as_ref()
            .map(|emb| emb.r#type.as_str())
            .unwrap_or("local");

        let provider = match embedding_type {
            "remote" => {
                let kind_str = config
                    .embedding
                    .as_ref()
                    .and_then(|emb| emb.kind.as_deref())
                    .ok_or_else(|| anyhow::anyhow!("kind is required when type is 'remote'"))?;
                Provider::from_str(kind_str)?
            }
            "local" => Provider::Local,
            _ => bail!(
                "unknown embedding type: {embedding_type}. Supported: 'local', 'remote'"
            ),
        };

        match provider {
            Provider::Local => {
                let model_id = config
                    .embedding
                    .as_ref()
                    .and_then(|emb| emb.model.as_deref())
                    .unwrap_or_else(|| Provider::Local.default_model())
                    .to_string();
                let resolved_model = resolve_local_model_id(&model_id)?;
                let token = std::env::var("HF_TOKEN").ok();
                let embedder = Embedder::from_pretrained_hf(&resolved_model, None, token.as_deref(), None)
                    .context("failed to initialize local embedder")?;
                Ok(Self {
                    embedder: Arc::new(embedder),
                })
            }
            Provider::OpenAI | Provider::Gemini | Provider::Cohere => {
                let model_id = config
                    .embedding
                    .as_ref()
                    .and_then(|emb| emb.model.as_deref())
                    .unwrap_or_else(|| provider.default_model())
                    .to_string();
                let api_key_env = provider.api_key_env();
                let api_key = std::env::var(api_key_env)
                    .with_context(|| format!("API key not found: {api_key_env} not set"))?;
                let cloud_name = provider.cloud_name();
                let embedder = Embedder::from_pretrained_cloud(cloud_name, &model_id, Some(api_key))
                    .context(format!("failed to initialize {cloud_name} embedder"))?;
                Ok(Self {
                    embedder: Arc::new(embedder),
                })
            }
        }
    }

    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self
            .embedder
            .embed(&[text], None, None)
            .await
            .context("failed to generate embedding")?;
        let embedding = embeddings
            .into_iter()
            .next()
            .context("no embedding returned")?;
        let dense = embedding
            .to_dense()
            .context("failed to convert embedding to dense vector")?;
        Ok(dense)
    }

    pub async fn embed_crate(&self, crate_path: &Path) -> Result<Vec<f32>> {
        // text-embedding-3-small supports ~8191 tokens (~32k chars)
        // nomic-embed-text-v1.5 supports 8192 tokens
        const MAX_CHARS: usize = 30_000;

        let src_path = crate_path.join("src");

        if !src_path.exists() {
            bail!("no src directory found in crate: {}", crate_path.display());
        }

        let mut source_content = String::new();

        for entry in walkdir::WalkDir::new(&src_path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
        {
            let path = entry.path();
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read source file: {}", path.display()))?;

            let _ = writeln!(source_content, "// File: {}", path.display());
            source_content.push_str(&content);
            source_content.push_str("\n\n");
        }

        if source_content.is_empty() {
            bail!("no Rust source files found in: {}", src_path.display());
        }

        let cargo_toml_path = crate_path.join("Cargo.toml");
        if cargo_toml_path.exists() {
            let cargo_toml =
                std::fs::read_to_string(&cargo_toml_path).context("failed to read Cargo.toml")?;
            source_content.push_str("// Cargo.toml:\n");
            source_content.push_str(&cargo_toml);
        }

        tracing::info!(
            total_chars = source_content.len(),
            "Collected source content for embedding"
        );

        let text = if source_content.len() > MAX_CHARS {
            tracing::info!(
                truncated_from = source_content.len(),
                truncated_to = MAX_CHARS,
                "Truncating source content for embedding"
            );
            &source_content[..MAX_CHARS]
        } else {
            &source_content
        };

        self.embed(text).await
    }
}

/// Resolves local model alias to Hugging Face model ID.
fn resolve_local_model_id(model: &str) -> Result<String> {
    let resolved = match model.to_lowercase().as_str() {
        "nomic-embed-text-v1" | "nomic-embed-text-v1.0" => "nomic-ai/nomic-embed-text-v1",
        "nomic-embed-text-v1.5"
        | "nomic-embed-text-v15"
        | "nomic-embed-text-v1.5-q"
        | "nomic-embed-text-v15-q" => "nomic-ai/nomic-embed-text-v1.5",
        "all-minilm-l6-v2" => "sentence-transformers/all-MiniLM-L6-v2",
        "bge-small-en-v1.5" => "BAAI/bge-small-en-v1.5",
        "bge-base-en-v1.5" => "BAAI/bge-base-en-v1.5",
        _ => {
            if model.contains('/') {
                return Ok(model.to_string());
            }
            bail!(
                "unknown local model: {model}. Provide a supported alias or a full Hugging Face model id"
            );
        }
    };

    Ok(resolved.to_string())
}

pub fn write_embedding_file(path: &Path, embedding: &[f32]) -> Result<()> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(embedding));
    for value in embedding {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    std::fs::write(path, bytes)
        .with_context(|| format!("failed to write embedding file: {}", path.display()))?;

    Ok(())
}
