//! Text embedding generation with support for multiple backends.
//!
//! This module provides a unified interface for generating text embeddings using
//! different providers. It supports both local (FastEmbed) and remote (OpenAI) embedding
//! generation, with automatic fallback and configuration management.
//!
//! # Key Components
//!
//! - [`Provider`]: Enum representing available embedding providers
//! - [`EmbeddingGenerator`]: Main API for generating embeddings
//! - [`write_embedding_file`]: Utility for persisting embeddings to disk
//!
//! # Configuration
//!
//! Embedding generation is configured via:
//! - Global config: `~/.config/operai/config.toml`
//! - Project config: `./operai.toml`
//! - Environment variables (for OpenAI API key)
//!
//! # Example
//!
//! ```no_run
//! use operai_embedding::EmbeddingGenerator;
//! use std::path::Path;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Create generator from config
//! let generator = EmbeddingGenerator::from_config(None, None)?;
//!
//! // Generate embedding for text
//! let embedding = generator.embed("Hello, world!").await?;
//!
//! // Generate embedding for entire crate
//! let crate_embedding = generator.embed_crate(Path::new("./my-crate")).await?;
//! # Ok(())
//! # }
//! ```

use std::{fmt::Write as _, path::Path, sync::Arc};

use anyhow::{Context, Result, bail};
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::embeddings::{CreateEmbeddingRequest, EmbeddingInput},
};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use tracing::info;
use walkdir::WalkDir;

use crate::config::{Config, ProjectConfig};

/// Embedding provider backend.
///
/// Represents the available embedding generation backends. Each provider
/// has different capabilities, performance characteristics, and default models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    /// Local embedding generation using FastEmbed.
    /// Runs inference locally on CPU with quantized models.
    FastEmbed,
    /// Remote embedding generation using OpenAI's API.
    /// Requires API key and network connection.
    OpenAI,
}

impl std::str::FromStr for Provider {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "fastembed" => Ok(Self::FastEmbed),
            "openai" => Ok(Self::OpenAI),
            _ => Err(()),
        }
    }
}

impl Provider {
    /// Returns the default model name for this provider.
    ///
    /// # Defaults
    ///
    /// - `FastEmbed`: `nomic-embed-text-v1.5`
    /// - `OpenAI`: `text-embedding-3-small`
    #[must_use]
    pub const fn default_model(self) -> &'static str {
        match self {
            Self::FastEmbed => "nomic-embed-text-v1.5",
            Self::OpenAI => "text-embedding-3-small",
        }
    }
}

fn resolve_fastembed_model(model_name: &str) -> Result<EmbeddingModel> {
    match model_name.to_lowercase().as_str() {
        "nomic-embed-text-v1" | "nomic-embed-text-v1.0" => Ok(EmbeddingModel::NomicEmbedTextV1),
        "nomic-embed-text-v1.5" | "nomic-embed-text-v15" => Ok(EmbeddingModel::NomicEmbedTextV15),
        "nomic-embed-text-v1.5-q" | "nomic-embed-text-v15-q" => {
            Ok(EmbeddingModel::NomicEmbedTextV15Q)
        }
        "all-minilm-l6-v2" => Ok(EmbeddingModel::AllMiniLML6V2),
        "bge-small-en-v1.5" => Ok(EmbeddingModel::BGESmallENV15),
        "bge-base-en-v1.5" => Ok(EmbeddingModel::BGEBaseENV15),
        _ => bail!(
            "unknown fastembed model: {model_name}. Supported: nomic-embed-text-v1, \
             nomic-embed-text-v1.5, nomic-embed-text-v1.5-q, all-minilm-l6-v2, bge-small-en-v1.5, \
             bge-base-en-v1.5"
        ),
    }
}

/// OpenAI backend for embedding generation.
///
/// Internal wrapper around the OpenAI client that stores the model name
/// and handles API communication.
struct OpenAIBackend {
    client: Client<OpenAIConfig>,
    model: String,
}

/// Internal enum representing the active embedding backend.
///
/// This allows [`EmbeddingGenerator`] to use different backends interchangeably
/// while maintaining type safety and proper resource management.
///
/// Each backend is wrapped in appropriate synchronization primitives:
/// - FastEmbed: `Mutex` since its `embed()` method requires `&mut self`
/// - OpenAI: No wrapper needed since the client is already thread-safe
enum EmbeddingBackend {
    FastEmbed(std::sync::Mutex<TextEmbedding>),
    OpenAI(OpenAIBackend),
}

/// Generator for text embeddings using configurable backends.
///
/// [`EmbeddingGenerator`] provides a unified interface for generating embeddings
/// from either local (FastEmbed) or remote (OpenAI) providers. It supports
/// embedding individual strings or entire Rust crates.
///
/// # Concurrency
///
/// The generator supports concurrent embedding generation. Multiple `embed()` calls
/// can run in parallel without blocking each other. This is safe because:
/// - FastEmbed's `TextEmbedding` uses immutable shared state
/// - OpenAI's client is designed for concurrent async requests
///
/// You can clone the generator cheaply via `Arc` or share references across tasks.
///
/// # Creation
///
/// Use [`EmbeddingGenerator::from_config`] to create a generator from configuration
/// files, or use the provider-specific constructors:
/// - [`EmbeddingGenerator::new_fastembed`]
/// - [`EmbeddingGenerator::new_openai`]
///
/// # Example
///
/// ```no_run
/// # use operai_embedding::EmbeddingGenerator;
/// # async fn example() -> anyhow::Result<()> {
/// let generator = EmbeddingGenerator::from_config(None, None)?;
///
/// // Concurrent embeddings
/// let (emb1, emb2) = tokio::try_join!(
///     generator.embed("hello"),
///     generator.embed("world")
/// )?;
/// # Ok(())
/// # }
/// ```
pub struct EmbeddingGenerator {
    provider: Provider,
    backend: Arc<EmbeddingBackend>,
}

impl EmbeddingGenerator {
    /// Creates a new FastEmbed-based embedding generator.
    ///
    /// # Arguments
    ///
    /// * `model` - Optional model name. Defaults to `nomic-embed-text-v1.5`
    /// * `show_download_progress` - Whether to display model download progress
    ///
    /// # Supported Models
    ///
    /// FastEmbed supports several local models:
    /// - `nomic-embed-text-v1` / `nomic-embed-text-v1.0`
    /// - `nomic-embed-text-v1.5` / `nomic-embed-text-v15`
    /// - `nomic-embed-text-v1.5-q` / `nomic-embed-text-v15-q` (quantized)
    /// - `all-minilm-l6-v2`
    /// - `bge-small-en-v1.5`
    /// - `bge-base-en-v1.5`
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The model name is not recognized
    /// - FastEmbed fails to initialize (e.g., download failure)
    pub fn new_fastembed(model: Option<String>, show_download_progress: bool) -> Result<Self> {
        let model_name = model.unwrap_or_else(|| Provider::FastEmbed.default_model().to_string());
        let embedding_model = resolve_fastembed_model(&model_name)?;

        let text_embedding = TextEmbedding::try_new(
            InitOptions::new(embedding_model).with_show_download_progress(show_download_progress),
        )
        .context("failed to initialize FastEmbed model")?;

        Ok(Self {
            provider: Provider::FastEmbed,
            backend: Arc::new(EmbeddingBackend::FastEmbed(std::sync::Mutex::new(text_embedding))),
        })
    }

    /// Creates a new OpenAI-based embedding generator.
    ///
    /// # Arguments
    ///
    /// * `model` - Optional model name. Defaults to `text-embedding-3-small`
    /// * `api_key` - Optional OpenAI API key. If `None`, reads from environment variable
    /// * `base_url` - Optional custom API base URL (for compatible endpoints)
    ///
    /// # Notes
    ///
    /// This function does not validate the API key or network connectivity.
    /// Errors occur during the first [`EmbeddingGenerator::embed`] call if
    /// credentials are invalid or the service is unreachable.
    #[must_use]
    pub fn new_openai(
        model: Option<String>,
        api_key: Option<String>,
        base_url: Option<String>,
    ) -> Self {
        let model = model.unwrap_or_else(|| Provider::OpenAI.default_model().to_string());

        let mut config = OpenAIConfig::new();

        if let Some(key) = api_key {
            config = config.with_api_key(key);
        }

        if let Some(url) = base_url {
            config = config.with_api_base(url);
        }

        let client = Client::with_config(config);

        Self {
            provider: Provider::OpenAI,
            backend: Arc::new(EmbeddingBackend::OpenAI(OpenAIBackend { client, model })),
        }
    }

    /// Creates an embedding generator from configuration files.
    ///
    /// This method loads configuration from:
    /// 1. Global config: `~/.config/operai/config.toml` (or `OPERAI_CONFIG_PATH`)
    /// 2. Project config: `./operai.toml` (or `OPERAI_PROJECT_CONFIG_PATH`)
    ///
    /// Configuration precedence (highest to lowest):
    /// - Function parameters (`override_provider`, `override_model`)
    /// - Project config (`operai.toml`)
    /// - Global config (`~/.config/operai/config.toml`)
    /// - Provider defaults
    ///
    /// # Arguments
    ///
    /// * `override_provider` - Override the configured provider
    /// * `override_model` - Override the configured model name
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The provider name is invalid
    /// - The provider-specific initialization fails
    pub fn from_config(
        override_provider: Option<&str>,
        override_model: Option<&str>,
    ) -> Result<Self> {
        let config = Config::load().unwrap_or_default();
        let project_config = ProjectConfig::load().unwrap_or_default();

        let provider_str = override_provider
            .map(ToString::to_string)
            .or(project_config.embedding_provider)
            .unwrap_or(config.embedding.provider);

        let provider: Provider = provider_str
            .parse()
            .map_err(|()| anyhow::anyhow!("unknown embedding provider: {provider_str}"))?;

        let model = override_model
            .map(ToString::to_string)
            .or(project_config.embedding_model)
            .or(config.embedding.model);

        match provider {
            Provider::FastEmbed => {
                Self::new_fastembed(model, config.embedding.fastembed.show_download_progress)
            }
            Provider::OpenAI => {
                let api_key_env = &config.embedding.openai.api_key_env;
                let api_key = std::env::var(api_key_env).ok();
                Ok(Self::new_openai(model, api_key, None))
            }
        }
    }

    /// Generates an embedding vector for the given text.
    ///
    /// # Arguments
    ///
    /// * `text` - The input text to embed
    ///
    /// # Returns
    ///
    /// A vector of `f32` values representing the text embedding.
    /// The vector length depends on the model:
    /// - OpenAI `text-embedding-3-small`: 1536 dimensions
    /// - Nomic models: 768 or 1024 dimensions
    ///
    /// # Concurrency
    ///
    /// This method can be called concurrently from multiple tasks. For example:
    ///
    /// ```ignore
    /// let (emb1, emb2) = tokio::try_join!(
    ///     generator.embed("hello"),
    ///     generator.embed("world")
    /// )?;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The backend fails to generate the embedding
    /// - The API request fails (OpenAI only)
    /// - No embedding is returned by the backend
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        info!(
            provider = ?self.provider,
            text_len = text.len(),
            "Generating embedding"
        );

        match self.backend.as_ref() {
            EmbeddingBackend::FastEmbed(model) => {
                let mut model = model.lock().map_err(|e| {
                    anyhow::anyhow!("failed to lock FastEmbed mutex: {}", e)
                })?;
                let embeddings = model
                    .embed(vec![text], None)
                    .context("failed to generate FastEmbed embedding")?;

                embeddings
                    .into_iter()
                    .next()
                    .context("no embedding returned from FastEmbed")
            }
            EmbeddingBackend::OpenAI(backend) => {
                let request = CreateEmbeddingRequest {
                    model: backend.model.clone(),
                    input: EmbeddingInput::String(text.to_string()),
                    encoding_format: None,
                    dimensions: None,
                    user: None,
                };

                let response: async_openai::types::embeddings::CreateEmbeddingResponse = backend
                    .client
                    .embeddings()
                    .create(request)
                    .await
                    .context("failed to create OpenAI embedding")?;

                let embedding = response
                    .data
                    .into_iter()
                    .next()
                    .context("no embedding returned from OpenAI")?;

                Ok(embedding.embedding)
            }
        }
    }

    /// Generates an embedding for an entire Rust crate.
    ///
    /// This method collects all `.rs` files from the crate's `src/` directory
    /// and optionally the `Cargo.toml` file, concatenates them with file headers,
    /// and generates a single embedding for the combined content.
    ///
    /// # Arguments
    ///
    /// * `crate_path` - Path to the crate root (containing `src/` and `Cargo.toml`)
    ///
    /// # Behavior
    ///
    /// - Collects all `.rs` files from `src/` recursively
    /// - Prepends `// File: <path>` comments before each file
    /// - Appends `Cargo.toml` contents if present
    /// - Truncates to 30,000 characters if content is too large
    /// - Uses the configured embedding backend to generate the vector
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The crate has no `src/` directory
    /// - No `.rs` files are found in `src/`
    /// - Any source file cannot be read
    /// - The embedding generation fails
    pub async fn embed_crate(&self, crate_path: &Path) -> Result<Vec<f32>> {
        // text-embedding-3-small supports ~8191 tokens (~32k chars)
        // nomic-embed-text-v1.5 supports 8192 tokens
        const MAX_CHARS: usize = 30_000;

        let src_path = crate_path.join("src");

        if !src_path.exists() {
            bail!("no src directory found in crate: {}", crate_path.display());
        }

        let mut source_content = String::new();

        for entry in WalkDir::new(&src_path)
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

        info!(
            total_chars = source_content.len(),
            "Collected source content for embedding"
        );

        let text = if source_content.len() > MAX_CHARS {
            info!(
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

/// Writes an embedding vector to a binary file.
///
/// The embedding is written as raw little-endian `f32` values, suitable
/// for later reading and processing.
///
/// # Arguments
///
/// * `path` - Destination file path
/// * `embedding` - Slice of `f32` values to write
///
/// # File Format
///
/// Binary file containing concatenated little-endian 32-bit floats:
/// ```text
/// [f32 bytes 0][f32 bytes 1]...[f32 bytes N]
/// ```
/// Each `f32` is 4 bytes.
///
/// # Errors
///
/// Returns an error if:
/// - The parent directory does not exist
/// - The file cannot be written (e.g., permission denied, path is a directory)
pub fn write_embedding_file(path: &Path, embedding: &[f32]) -> Result<()> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(embedding));
    for value in embedding {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    std::fs::write(path, bytes)
        .with_context(|| format!("failed to write embedding file: {}", path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use anyhow::Result;
    use serde_json::Value;
    use tokio::{
        io::{AsyncReadExt as _, AsyncWriteExt as _},
        net::TcpListener,
        sync::oneshot,
    };

    use super::*;

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Result<Self> {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let dir_name = format!("{prefix}{nanos}-{}", std::process::id());
            let dir_name = format!("{dir_name}-{unique}");

            let path = std::env::temp_dir().join(dir_name);
            std::fs::create_dir_all(&path)?;
            Ok(Self { path })
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn write_file(path: &Path, content: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        Ok(())
    }

    #[derive(Debug)]
    struct MockRequest {
        method: String,
        path: String,
        body: Value,
    }

    async fn spawn_openai_mock_server(
        response_body: Value,
    ) -> Result<(String, oneshot::Receiver<MockRequest>)> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let (tx, rx) = oneshot::channel();
        let response_body = response_body.to_string();

        tokio::spawn(async move {
            if let Err(err) = handle_one_http_request(listener, tx, &response_body).await {
                eprintln!("mock OpenAI server error: {err:#}");
            }
        });

        Ok((format!("http://{addr}/v1"), rx))
    }

    fn find_double_crlf(haystack: &[u8]) -> Option<usize> {
        haystack.windows(4).position(|window| window == b"\r\n\r\n")
    }

    async fn handle_one_http_request(
        listener: TcpListener,
        request_tx: oneshot::Sender<MockRequest>,
        response_body: &str,
    ) -> Result<()> {
        let (mut socket, _) = listener.accept().await?;

        let mut buf = Vec::new();
        let header_end = loop {
            let mut chunk = [0_u8; 4096];
            let n = socket.read(&mut chunk).await?;
            anyhow::ensure!(n != 0, "unexpected EOF while reading request headers");
            buf.extend_from_slice(&chunk[..n]);

            if let Some(pos) = find_double_crlf(&buf) {
                break pos + 4;
            }

            anyhow::ensure!(buf.len() <= 64 * 1024, "request headers too large");
        };

        let (header_bytes, rest) = buf.split_at(header_end);
        let header_str = std::str::from_utf8(header_bytes).context("request headers not utf-8")?;

        let mut lines = header_str.split("\r\n");
        let request_line = lines.next().unwrap_or_default();
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts.next().unwrap_or_default().to_string();
        let path = request_parts.next().unwrap_or_default().to_string();

        let mut content_length = None;
        let mut expect_continue = false;

        for line in lines {
            if line.is_empty() {
                break;
            }
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim();
            match name.as_str() {
                "content-length" => {
                    content_length = value.parse::<usize>().ok();
                }
                "expect" => {
                    expect_continue = value.to_ascii_lowercase().contains("100-continue");
                }
                _ => {}
            }
        }

        anyhow::ensure!(
            content_length.is_some(),
            "missing Content-Length header in request"
        );

        if expect_continue {
            socket.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").await?;
        }

        let content_length = content_length.unwrap_or_default();
        let mut body_bytes = Vec::with_capacity(content_length);
        body_bytes.extend_from_slice(rest);

        while body_bytes.len() < content_length {
            let mut chunk = [0_u8; 4096];
            let n = socket.read(&mut chunk).await?;
            anyhow::ensure!(n != 0, "unexpected EOF while reading request body");
            body_bytes.extend_from_slice(&chunk[..n]);
        }
        body_bytes.truncate(content_length);

        let body_str = std::str::from_utf8(&body_bytes).context("request body not utf-8")?;
        let body: Value = serde_json::from_str(body_str).context("request body not JSON")?;

        let _ = request_tx.send(MockRequest { method, path, body });

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        socket.write_all(response.as_bytes()).await?;
        Ok(())
    }

    fn extract_input_string(body: &Value) -> Option<&str> {
        match &body["input"] {
            Value::String(s) => Some(s),
            Value::Array(items) => items.first().and_then(Value::as_str),
            _ => None,
        }
    }

    #[test]
    fn test_provider_from_str_returns_correct_variant() {
        // Arrange / Act / Assert
        assert_eq!("fastembed".parse(), Ok(Provider::FastEmbed));
        assert_eq!("openai".parse(), Ok(Provider::OpenAI));
    }

    #[test]
    fn test_provider_from_str_is_case_insensitive() {
        // Arrange
        let fastembed_cases = ["fastembed", "FASTEMBED", "FastEmbed", "FASTEMBED"];
        let openai_cases = ["openai", "OPENAI", "OpenAI", "OpenAI"];

        // Act / Assert
        for case in fastembed_cases {
            assert_eq!(
                case.parse::<Provider>(),
                Ok(Provider::FastEmbed),
                "expected {case}.parse() to return FastEmbed"
            );
        }
        for case in openai_cases {
            assert_eq!(
                case.parse::<Provider>(),
                Ok(Provider::OpenAI),
                "expected {case}.parse() to return OpenAI"
            );
        }
    }

    #[test]
    fn test_provider_from_str_returns_err_for_unknown_provider() {
        // Arrange
        let unknown = "not-a-provider";

        // Act
        let provider = unknown.parse::<Provider>();

        // Assert
        assert!(provider.is_err());
    }

    #[test]
    fn test_provider_from_str_returns_err_for_empty_string() {
        // Arrange / Act / Assert
        assert!("".parse::<Provider>().is_err());
    }

    #[test]
    fn test_provider_default_model_returns_expected_values() {
        // Arrange / Act / Assert
        assert_eq!(Provider::FastEmbed.default_model(), "nomic-embed-text-v1.5");
        assert_eq!(Provider::OpenAI.default_model(), "text-embedding-3-small");
    }

    #[test]
    fn test_new_fastembed_with_unknown_model_returns_error() {
        // Arrange
        let unknown_model = Some("not-a-real-model".to_string());

        // Act
        let Err(err) = EmbeddingGenerator::new_fastembed(unknown_model, false) else {
            panic!("expected unknown model to error");
        };

        // Assert
        let msg = err.to_string();
        assert!(msg.contains("unknown fastembed model:"), "{msg}");
        assert!(msg.contains("not-a-real-model"), "{msg}");
        assert!(msg.contains("Supported:"), "{msg}");
    }

    #[test]
    fn test_from_config_with_unknown_provider_returns_error_with_context() {
        // Arrange
        let override_provider = Some("definitely-not-valid");

        // Act
        let Err(err) = EmbeddingGenerator::from_config(override_provider, None) else {
            panic!("expected unknown provider to error");
        };

        // Assert
        let msg = err.to_string();
        assert!(msg.contains("unknown embedding provider:"), "{msg}");
        assert!(msg.contains("definitely-not-valid"), "{msg}");
    }

    #[tokio::test]
    async fn test_embed_crate_returns_error_when_src_directory_missing() -> Result<()> {
        // Arrange
        let temp = TempDir::new("operai-embed-crate-missing-src-")?;
        let generator = EmbeddingGenerator::new_openai(
            Some("test-model".to_string()),
            Some("test-api-key".to_string()),
            None,
        );

        // Act
        let err = generator
            .embed_crate(temp.path())
            .await
            .expect_err("expected error when src directory missing");

        // Assert
        let msg = err.to_string();
        assert!(msg.contains("no src directory found in crate:"), "{msg}");
        assert!(msg.contains(&temp.path().display().to_string()), "{msg}");

        Ok(())
    }

    #[tokio::test]
    async fn test_embed_crate_returns_error_when_no_rust_files_found() -> Result<()> {
        // Arrange
        let temp = TempDir::new("operai-embed-crate-no-rs-")?;
        std::fs::create_dir_all(temp.path().join("src"))?;
        write_file(&temp.path().join("src/readme.txt"), "not rust")?;

        let generator = EmbeddingGenerator::new_openai(
            Some("test-model".to_string()),
            Some("test-api-key".to_string()),
            None,
        );

        // Act
        let err = generator
            .embed_crate(temp.path())
            .await
            .expect_err("expected error when no rust files found");

        // Assert
        let msg = err.to_string();
        assert!(msg.contains("no Rust source files found in:"), "{msg}");
        assert!(msg.contains("src"), "{msg}");

        Ok(())
    }

    #[tokio::test]
    async fn test_openai_embed_sends_expected_request_and_returns_embedding() -> Result<()> {
        // Arrange
        let response_body = serde_json::json!({
            "object": "list",
            "data": [
                { "object": "embedding", "index": 0, "embedding": [1.0, 2.0, 3.0] }
            ],
            "model": "test-model",
            "usage": { "prompt_tokens": 1, "total_tokens": 1 }
        });
        let (base_url, request_rx) = spawn_openai_mock_server(response_body).await?;
        let generator = EmbeddingGenerator::new_openai(
            Some("test-model".to_string()),
            Some("test-api-key".to_string()),
            Some(base_url),
        );

        // Act
        let embedding = generator.embed("hello world").await?;
        let request = tokio::time::timeout(std::time::Duration::from_secs(5), request_rx)
            .await
            .context("timed out waiting for mock server request")?
            .context("mock server dropped request channel")?;

        // Assert
        assert_eq!(embedding, vec![1.0, 2.0, 3.0]);
        assert_eq!(request.method, "POST");
        assert!(
            request.path.ends_with("/embeddings"),
            "unexpected request path: {}",
            request.path
        );
        assert_eq!(request.body["model"], "test-model");
        assert_eq!(
            extract_input_string(&request.body),
            Some("hello world"),
            "unexpected request input: {}",
            request.body
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_openai_embed_returns_error_when_response_has_no_embeddings() -> Result<()> {
        // Arrange
        let response_body = serde_json::json!({
            "object": "list",
            "data": [],
            "model": "test-model",
            "usage": { "prompt_tokens": 1, "total_tokens": 1 }
        });
        let (base_url, _request_rx) = spawn_openai_mock_server(response_body).await?;
        let generator = EmbeddingGenerator::new_openai(
            Some("test-model".to_string()),
            Some("test-api-key".to_string()),
            Some(base_url),
        );

        // Act
        let err = generator
            .embed("hello world")
            .await
            .expect_err("expected error when no embedding returned");

        // Assert
        let msg = err.to_string();
        assert!(msg.contains("no embedding returned from OpenAI"), "{msg}");

        Ok(())
    }

    #[tokio::test]
    async fn test_embed_crate_includes_rust_sources_and_cargo_toml() -> Result<()> {
        // Arrange
        let temp = TempDir::new("operai-embed-crate-content-")?;
        write_file(
            &temp.path().join("src/lib.rs"),
            "pub fn example() -> &'static str { \"ok\" }\n",
        )?;
        write_file(
            &temp.path().join("Cargo.toml"),
            r#"
[package]
name = "example-crate"
version = "0.1.0"
"#,
        )?;

        let response_body = serde_json::json!({
            "object": "list",
            "data": [
                { "object": "embedding", "index": 0, "embedding": [0.0] }
            ],
            "model": "test-model",
            "usage": { "prompt_tokens": 1, "total_tokens": 1 }
        });
        let (base_url, request_rx) = spawn_openai_mock_server(response_body).await?;
        let generator = EmbeddingGenerator::new_openai(
            Some("test-model".to_string()),
            Some("test-api-key".to_string()),
            Some(base_url),
        );

        // Act
        let _embedding = generator.embed_crate(temp.path()).await?;
        let request = tokio::time::timeout(std::time::Duration::from_secs(5), request_rx)
            .await
            .context("timed out waiting for mock server request")?
            .context("mock server dropped request channel")?;

        // Assert
        let input = extract_input_string(&request.body).context("request input missing")?;
        assert!(
            input.contains("pub fn example"),
            "missing Rust source content"
        );
        assert!(input.contains("Cargo.toml:"), "missing Cargo.toml header");
        assert!(
            input.contains("name = \"example-crate\""),
            "missing Cargo.toml content"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_embed_crate_truncates_source_content_to_max_chars() -> Result<()> {
        // Arrange
        let temp = TempDir::new("operai-embed-crate-truncate-")?;
        let src_path = temp.path().join("src/lib.rs");
        let big_content = "a".repeat(40_000);
        write_file(&src_path, &big_content)?;

        let response_body = serde_json::json!({
            "object": "list",
            "data": [
                { "object": "embedding", "index": 0, "embedding": [0.0] }
            ],
            "model": "test-model",
            "usage": { "prompt_tokens": 1, "total_tokens": 1 }
        });
        let (base_url, request_rx) = spawn_openai_mock_server(response_body).await?;
        let generator = EmbeddingGenerator::new_openai(
            Some("test-model".to_string()),
            Some("test-api-key".to_string()),
            Some(base_url),
        );

        // Act
        let _embedding = generator.embed_crate(temp.path()).await?;
        let request = tokio::time::timeout(std::time::Duration::from_secs(5), request_rx)
            .await
            .context("timed out waiting for mock server request")?
            .context("mock server dropped request channel")?;

        // Assert
        let input = extract_input_string(&request.body).context("request input missing")?;
        assert_eq!(input.len(), 30_000);

        Ok(())
    }

    #[test]
    fn test_write_embedding_file_writes_raw_little_endian_f32_bytes() -> Result<()> {
        // Arrange
        let temp = TempDir::new("operai-embedding-file-")?;
        let path = temp.path().join("embedding.bin");
        let embedding = [1.0_f32, -2.5_f32, 0.0_f32];
        let expected: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        // Act
        write_embedding_file(&path, &embedding)?;
        let actual = std::fs::read(&path)?;

        // Assert
        assert_eq!(actual, expected);

        Ok(())
    }

    #[test]
    fn test_write_embedding_file_returns_error_when_path_is_directory() -> Result<()> {
        // Arrange
        let temp = TempDir::new("operai-embedding-file-dir-")?;

        // Act
        let err = write_embedding_file(temp.path(), &[1.0_f32])
            .expect_err("expected error when writing to a directory path");

        // Assert
        let msg = err.to_string();
        assert!(msg.contains("failed to write embedding file:"), "{msg}");
        assert!(msg.contains(&temp.path().display().to_string()), "{msg}");

        Ok(())
    }

    #[test]
    fn test_resolve_fastembed_model_accepts_all_documented_aliases() {
        // Arrange - each row is (input, expected_variant_name) for documentation
        let valid_models = [
            "nomic-embed-text-v1",
            "nomic-embed-text-v1.0",
            "nomic-embed-text-v1.5",
            "nomic-embed-text-v15",
            "nomic-embed-text-v1.5-q",
            "nomic-embed-text-v15-q",
            "all-minilm-l6-v2",
            "bge-small-en-v1.5",
            "bge-base-en-v1.5",
        ];

        // Act / Assert
        for model in valid_models {
            assert!(
                resolve_fastembed_model(model).is_ok(),
                "expected resolve_fastembed_model to accept {model}"
            );
        }
    }

    #[test]
    fn test_resolve_fastembed_model_is_case_insensitive() {
        // Arrange / Act / Assert
        assert!(resolve_fastembed_model("NOMIC-EMBED-TEXT-V1.5").is_ok());
        assert!(resolve_fastembed_model("All-MiniLM-L6-V2").is_ok());
        assert!(resolve_fastembed_model("BGE-SMALL-EN-V1.5").is_ok());
    }

    #[test]
    fn test_resolve_fastembed_model_error_lists_supported_models() {
        // Arrange
        let unknown = "not-a-model";

        // Act
        let err = resolve_fastembed_model(unknown).expect_err("expected error for unknown model");

        // Assert - error message should help users know valid options
        let msg = err.to_string();
        assert!(
            msg.contains("unknown fastembed model: not-a-model"),
            "{msg}"
        );
        assert!(msg.contains("nomic-embed-text-v1"), "{msg}");
        assert!(msg.contains("all-minilm-l6-v2"), "{msg}");
        assert!(msg.contains("bge-small-en-v1.5"), "{msg}");
    }

    #[test]
    fn test_write_embedding_file_handles_empty_embedding() -> Result<()> {
        // Arrange
        let temp = TempDir::new("operai-embedding-empty-")?;
        let path = temp.path().join("empty.bin");
        let embedding: [f32; 0] = [];

        // Act
        write_embedding_file(&path, &embedding)?;
        let actual = std::fs::read(&path)?;

        // Assert
        assert!(actual.is_empty());

        Ok(())
    }

    #[test]
    fn test_provider_equality_and_clone() {
        // Arrange
        let fastembed = Provider::FastEmbed;
        let openai = Provider::OpenAI;

        // Act / Assert - PartialEq/Eq
        assert_eq!(fastembed, Provider::FastEmbed);
        assert_ne!(fastembed, openai);

        // Act / Assert - Clone/Copy
        let cloned = fastembed;
        assert_eq!(cloned, fastembed);
    }

    #[test]
    fn test_provider_debug_output() {
        // Arrange / Act
        let fastembed_debug = format!("{:?}", Provider::FastEmbed);
        let openai_debug = format!("{:?}", Provider::OpenAI);

        // Assert
        assert_eq!(fastembed_debug, "FastEmbed");
        assert_eq!(openai_debug, "OpenAI");
    }

    #[test]
    fn test_write_embedding_file_roundtrip_preserves_values() -> Result<()> {
        // Arrange
        let temp = TempDir::new("operai-embedding-roundtrip-")?;
        let path = temp.path().join("roundtrip.bin");
        let original = [1.0_f32, -2.5_f32, 0.0_f32, f32::MIN, f32::MAX];

        // Act
        write_embedding_file(&path, &original)?;
        let bytes = std::fs::read(&path)?;

        // Assert - reconstruct f32 values from bytes
        let reconstructed: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        assert_eq!(reconstructed, original);

        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_embeddings_with_openai() -> Result<()> {
        // Arrange
        let response_body = serde_json::json!({
            "object": "list",
            "data": [
                { "object": "embedding", "index": 0, "embedding": [1.0, 2.0, 3.0] }
            ],
            "model": "test-model",
            "usage": { "prompt_tokens": 1, "total_tokens": 1 }
        });
        let (base_url, _request_rx) = spawn_openai_mock_server(response_body).await?;
        let generator = std::sync::Arc::new(EmbeddingGenerator::new_openai(
            Some("test-model".to_string()),
            Some("test-api-key".to_string()),
            Some(base_url),
        ));

        // Act - verify we can call embed() from shared Arc<EmbeddingGenerator>
        // This tests that embed() takes &self, not &mut self, enabling concurrent use
        let embedding = generator.embed("test query").await?;

        // Assert
        assert_eq!(embedding, vec![1.0, 2.0, 3.0]);

        Ok(())
    }

    #[tokio::test]
    async fn test_embed_takes_shared_reference_not_mutable() -> Result<()> {
        // Arrange
        let response_body = serde_json::json!({
            "object": "list",
            "data": [
                { "object": "embedding", "index": 0, "embedding": [1.0] }
            ],
            "model": "test-model",
            "usage": { "prompt_tokens": 1, "total_tokens": 1 }
        });
        let (base_url, _request_rx) = spawn_openai_mock_server(response_body).await?;
        let generator = EmbeddingGenerator::new_openai(
            Some("test-model".to_string()),
            Some("test-api-key".to_string()),
            Some(base_url),
        );

        // Act - verify we can call embed with &self, not &mut self
        // This is important for concurrent access - we should be able to
        // call embed() without requiring &mut self
        let embedding = generator.embed("test").await?;

        // Assert
        assert_eq!(embedding, vec![1.0]);

        Ok(())
    }

    #[tokio::test]
    async fn test_generator_can_be_cloned_and_shared() -> Result<()> {
        // Arrange
        let response_body = serde_json::json!({
            "object": "list",
            "data": [
                { "object": "embedding", "index": 0, "embedding": [1.0, 2.0] }
            ],
            "model": "test-model",
            "usage": { "prompt_tokens": 1, "total_tokens": 1 }
        });
        let (base_url, _request_rx) = spawn_openai_mock_server(response_body).await?;
        let generator1 = EmbeddingGenerator::new_openai(
            Some("test-model".to_string()),
            Some("test-api-key".to_string()),
            Some(base_url),
        );

        // Act - Arc allows cheap cloning for sharing across threads
        let generator = std::sync::Arc::new(generator1);
        let gen_clone = generator.clone();

        // Verify that Arc cloning works and both can call embed
        // (Only making one request since mock server handles only one)
        let embedding = gen_clone.embed("query").await?;

        // Assert
        assert_eq!(embedding, vec![1.0, 2.0]);

        Ok(())
    }
}
