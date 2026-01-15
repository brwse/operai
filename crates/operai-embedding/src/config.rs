//! Configuration structures for embedding generation.
//!
//! This module provides configuration types for the embedding system, supporting
//! multiple providers (FastEmbed and OpenAI) with TOML-based configuration files.
//!
//! # Configuration Hierarchy
//!
//! The system uses a two-level configuration approach:
//!
//! 1. **Global Config** (`Config`) - User-level settings loaded from:
//!    - `OPERAI_CONFIG_PATH` environment variable (if set)
//!    - `~/.config/operai/config.toml` (fallback)
//!    - `.operai/config.toml` (final fallback)
//!
//! 2. **Project Config** (`ProjectConfig`) - Project-level overrides loaded from:
//!    - `OPERAI_PROJECT_CONFIG_PATH` environment variable (if set)
//!    - `./operai.toml` (fallback)
//!
//! # Configuration Priority
//!
//! When resolving settings, the priority order (highest to lowest) is:
//! - CLI arguments (not handled by this module)
//! - Project config values
//! - Global config values
//! - Default values
//!
//! # Example
//!
//! ```toml
//! # ~/.config/operai/config.toml
//! [embedding]
//! provider = "fastembed"
//! model = "nomic-embed-text-v1.5"
//!
//! [embedding.fastembed]
//! show_download_progress = true
//!
//! [embedding.openai]
//! api_key_env = "OPENAI_API_KEY"
//! ```

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

fn config_path() -> PathBuf {
    if let Ok(path) = std::env::var("OPERAI_CONFIG_PATH") {
        return PathBuf::from(path);
    }
    dirs_config_path().unwrap_or_else(|| PathBuf::from(".operai/config.toml"))
}

fn dirs_config_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".config/operai/config.toml"))
}

/// Global user-level configuration for the embedding system.
///
/// This configuration is typically stored in the user's home directory and
/// contains default settings for embedding generation across all projects.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    /// Embedding-specific configuration including provider selection and model settings.
    #[serde(default)]
    pub embedding: EmbeddingConfig,
}

/// Configuration settings for embedding generation.
///
/// Defines which provider to use (FastEmbed or OpenAI) along with
/// provider-specific settings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// The embedding provider to use.
    ///
    /// Supported values: "fastembed" or "openai". Defaults to "fastembed".
    #[serde(default = "default_provider")]
    pub provider: String,

    /// Optional model name override.
    ///
    /// If specified, this overrides the default model for the selected provider.
    /// Each provider has its own set of supported models.
    #[serde(default)]
    pub model: Option<String>,

    /// FastEmbed-specific configuration.
    #[serde(default)]
    pub fastembed: FastEmbedConfig,

    /// OpenAI-specific configuration.
    #[serde(default)]
    pub openai: OpenAIConfig,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            model: None,
            fastembed: FastEmbedConfig::default(),
            openai: OpenAIConfig::default(),
        }
    }
}

fn default_provider() -> String {
    "fastembed".to_string()
}

/// Configuration for FastEmbed embedding provider.
///
/// FastEmbed is a local embedding provider that runs models on the local machine.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FastEmbedConfig {
    /// The FastEmbed model to use.
    ///
    /// Supported models include:
    /// - "nomic-embed-text-v1.5" (default)
    /// - "nomic-embed-text-v1.5-q" (quantized version)
    /// - "all-minilm-l6-v2"
    /// - "bge-small-en-v1.5"
    /// - "bge-base-en-v1.5"
    #[serde(default = "default_fastembed_model")]
    pub model: String,

    /// Whether to display download progress when downloading models.
    ///
    /// Models are downloaded on first use and cached locally.
    #[serde(default = "default_show_download_progress")]
    pub show_download_progress: bool,
}

impl Default for FastEmbedConfig {
    fn default() -> Self {
        Self {
            model: default_fastembed_model(),
            show_download_progress: default_show_download_progress(),
        }
    }
}

fn default_fastembed_model() -> String {
    "nomic-embed-text-v1.5".to_string()
}

fn default_show_download_progress() -> bool {
    true
}

/// Configuration for OpenAI embedding provider.
///
/// OpenAI is a cloud-based embedding provider that requires an API key.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpenAIConfig {
    /// Environment variable name containing the OpenAI API key.
    ///
    /// The actual API key is read from this environment variable at runtime.
    /// Defaults to "OPENAI_API_KEY".
    #[serde(default = "default_openai_key_env")]
    pub api_key_env: String,
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            api_key_env: default_openai_key_env(),
        }
    }
}

fn default_openai_key_env() -> String {
    "OPENAI_API_KEY".to_string()
}

impl Config {
    /// Loads the global configuration from the default path.
    ///
    /// This method searches for the configuration file in the following order:
    /// 1. Path specified by `OPERAI_CONFIG_PATH` environment variable
    /// 2. `~/.config/operai/config.toml`
    /// 3. `.operai/config.toml`
    ///
    /// If no configuration file is found, default values are used.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration file exists but cannot be read or parsed.
    pub fn load() -> Result<Self> {
        load_toml_or_default(&config_path())
    }

    /// Loads the global configuration from a specific path.
    ///
    /// If the specified file does not exist, default values are used.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be read or parsed.
    pub fn load_from(path: &std::path::Path) -> Result<Self> {
        load_toml_or_default(path)
    }
}

/// Project-level configuration for embedding settings.
///
/// This configuration allows per-project overrides of embedding settings,
/// typically stored in `./operai.toml` at the project root.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Optional override for the embedding provider.
    ///
    /// If set, this overrides the provider from the global configuration.
    pub embedding_provider: Option<String>,

    /// Optional override for the embedding model.
    ///
    /// If set, this overrides the model from the global configuration.
    pub embedding_model: Option<String>,
}

impl ProjectConfig {
    /// Loads the project configuration from the default path.
    ///
    /// This method searches for the configuration file in the following order:
    /// 1. Path specified by `OPERAI_PROJECT_CONFIG_PATH` environment variable
    /// 2. `./operai.toml`
    ///
    /// If no configuration file is found, default values (all `None`) are used.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration file exists but cannot be read or parsed.
    pub fn load() -> Result<Self> {
        if let Ok(path) = std::env::var("OPERAI_PROJECT_CONFIG_PATH") {
            return load_toml_or_default(&PathBuf::from(path));
        }
        load_toml_or_default(&PathBuf::from("operai.toml"))
    }

    /// Loads the project configuration from a specific path.
    ///
    /// If the specified file does not exist, default values are used.
    ///
    /// # Errors
    ///
    /// Returns an error if the file exists but cannot be read or parsed.
    pub fn load_from(path: &std::path::Path) -> Result<Self> {
        load_toml_or_default(path)
    }
}

fn load_toml_or_default<T: Default + serde::de::DeserializeOwned>(
    path: &std::path::Path,
) -> Result<T> {
    if path.exists() {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read: {}", path.display()))?;
        toml::from_str(&content).with_context(|| format!("failed to parse: {}", path.display()))
    } else {
        Ok(T::default())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::{OsStr, OsString},
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use anyhow::Result;

    use super::*;
    use crate::testing;

    fn test_lock() -> tokio::sync::MutexGuard<'static, ()> {
        testing::test_lock()
    }

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

    struct EnvVarGuard {
        key: String,
        previous: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &str, value: &OsStr) -> Self {
            let previous = std::env::var_os(key);
            unsafe {
                std::env::set_var(key, value);
            }
            Self {
                key: key.to_string(),
                previous,
            }
        }

        fn remove(key: &str) -> Self {
            let previous = std::env::var_os(key);
            unsafe {
                std::env::remove_var(key);
            }
            Self {
                key: key.to_string(),
                previous,
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.take() {
                unsafe {
                    std::env::set_var(&self.key, previous);
                }
            } else {
                unsafe {
                    std::env::remove_var(&self.key);
                }
            }
        }
    }

    fn home_config_path(home: &Path) -> PathBuf {
        home.join(".config/operai/config.toml")
    }

    fn write_file(path: &Path, content: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        Ok(())
    }

    #[test]
    fn test_config_load_returns_default_when_missing() -> Result<()> {
        let _lock = test_lock();

        let temp_home = TempDir::new("operai-config-home-")?;
        let _home_guard = EnvVarGuard::set("HOME", temp_home.path().as_os_str());

        let config = Config::load()?;

        assert_eq!(config.embedding.provider, "fastembed");
        assert_eq!(config.embedding.model, None);
        assert_eq!(config.embedding.fastembed.model, "nomic-embed-text-v1.5");
        assert!(config.embedding.fastembed.show_download_progress);
        assert_eq!(config.embedding.openai.api_key_env, "OPENAI_API_KEY");

        Ok(())
    }

    #[test]
    fn test_config_load_uses_serde_defaults_with_empty_file() -> Result<()> {
        let _lock = test_lock();

        let temp_home = TempDir::new("operai-config-empty-")?;
        let _home_guard = EnvVarGuard::set("HOME", temp_home.path().as_os_str());

        let path = home_config_path(temp_home.path());
        write_file(&path, "")?;

        let config = Config::load()?;

        assert_eq!(config.embedding.provider, "fastembed");
        assert_eq!(config.embedding.model, None);
        assert_eq!(config.embedding.fastembed.model, "nomic-embed-text-v1.5");
        assert!(config.embedding.fastembed.show_download_progress);
        assert_eq!(config.embedding.openai.api_key_env, "OPENAI_API_KEY");

        Ok(())
    }

    #[test]
    fn test_config_load_applies_defaults_for_missing_nested_fields() -> Result<()> {
        let _lock = test_lock();

        let temp_home = TempDir::new("operai-config-partial-")?;
        let _home_guard = EnvVarGuard::set("HOME", temp_home.path().as_os_str());

        let path = home_config_path(temp_home.path());
        write_file(
            &path,
            r#"
[embedding]
model = "custom-model"
"#,
        )?;

        let config = Config::load()?;

        assert_eq!(config.embedding.provider, "fastembed");
        assert_eq!(config.embedding.model.as_deref(), Some("custom-model"));
        assert_eq!(config.embedding.fastembed.model, "nomic-embed-text-v1.5");
        assert!(config.embedding.fastembed.show_download_progress);
        assert_eq!(config.embedding.openai.api_key_env, "OPENAI_API_KEY");

        Ok(())
    }

    #[test]
    fn test_config_load_reads_values_from_home_config() -> Result<()> {
        let _lock = test_lock();

        let temp_home = TempDir::new("operai-config-values-")?;
        let _home_guard = EnvVarGuard::set("HOME", temp_home.path().as_os_str());

        let path = home_config_path(temp_home.path());
        write_file(
            &path,
            r#"
[embedding]
provider = "openai"
model = "text-embedding-3-small"

[embedding.openai]
api_key_env = "BRWSE_OPENAI_API_KEY"
"#,
        )?;

        let config = Config::load()?;

        assert_eq!(config.embedding.provider, "openai");
        assert_eq!(
            config.embedding.model.as_deref(),
            Some("text-embedding-3-small")
        );
        assert_eq!(config.embedding.openai.api_key_env, "BRWSE_OPENAI_API_KEY");

        Ok(())
    }

    #[test]
    fn test_config_load_reads_fastembed_settings_from_home_config() -> Result<()> {
        let _lock = test_lock();

        let temp_home = TempDir::new("operai-config-fastembed-values-")?;
        let _home_guard = EnvVarGuard::set("HOME", temp_home.path().as_os_str());

        let path = home_config_path(temp_home.path());
        write_file(
            &path,
            r#"
[embedding.fastembed]
model = "all-minilm-l6-v2"
show_download_progress = false
"#,
        )?;

        let config = Config::load()?;

        assert_eq!(config.embedding.provider, "fastembed");
        assert_eq!(config.embedding.fastembed.model, "all-minilm-l6-v2");
        assert!(!config.embedding.fastembed.show_download_progress);
        assert_eq!(config.embedding.openai.api_key_env, "OPENAI_API_KEY");

        Ok(())
    }

    #[test]
    fn test_config_load_falls_back_to_project_config_when_home_unset() -> Result<()> {
        let _lock = test_lock();

        let temp_project = TempDir::new("operai-config-project-fallback-")?;
        let _home_guard = EnvVarGuard::remove("HOME");

        let config_path = temp_project.path().join(".operai/config.toml");
        let _config_guard = EnvVarGuard::set("OPERAI_CONFIG_PATH", config_path.as_os_str());

        write_file(
            &config_path,
            r#"
[embedding]
provider = "openai"

[embedding.openai]
api_key_env = "BRWSE_OPENAI_API_KEY"
"#,
        )?;

        let config = Config::load()?;

        assert_eq!(config.embedding.provider, "openai");
        assert_eq!(config.embedding.openai.api_key_env, "BRWSE_OPENAI_API_KEY");

        Ok(())
    }

    #[test]
    fn test_config_load_errors_when_config_path_is_directory() -> Result<()> {
        let _lock = test_lock();

        let temp_home = TempDir::new("operai-config-read-error-")?;
        let _home_guard = EnvVarGuard::set("HOME", temp_home.path().as_os_str());

        let path = home_config_path(temp_home.path());
        std::fs::create_dir_all(&path)?;

        let err = Config::load().expect_err("expected read error");
        let msg = err.to_string();
        assert!(msg.contains("failed to read:"), "{msg}");
        assert!(msg.contains(&path.display().to_string()), "{msg}");

        Ok(())
    }

    #[test]
    fn test_config_load_errors_on_invalid_toml_with_path_context() -> Result<()> {
        let _lock = test_lock();

        let temp_home = TempDir::new("operai-config-parse-error-")?;
        let _home_guard = EnvVarGuard::set("HOME", temp_home.path().as_os_str());

        let path = home_config_path(temp_home.path());
        write_file(&path, "embedding = [")?;

        let err = Config::load().expect_err("expected parse error");
        let msg = err.to_string();
        assert!(msg.contains("failed to parse:"), "{msg}");
        assert!(msg.contains(&path.display().to_string()), "{msg}");

        Ok(())
    }

    #[test]
    fn test_project_config_load_uses_serde_defaults_with_empty_file() -> Result<()> {
        let _lock = test_lock();

        let temp_project = TempDir::new("operai-project-empty-")?;
        let config_path = temp_project.path().join("operai.toml");
        let _config_guard = EnvVarGuard::set("OPERAI_PROJECT_CONFIG_PATH", config_path.as_os_str());

        write_file(&config_path, "")?;

        let config = ProjectConfig::load()?;

        assert_eq!(config.embedding_provider, None);
        assert_eq!(config.embedding_model, None);

        Ok(())
    }

    #[test]
    fn test_project_config_load_returns_default_when_missing() -> Result<()> {
        let _lock = test_lock();

        let temp_project = TempDir::new("operai-project-default-")?;
        let config_path = temp_project.path().join("operai.toml");
        let _config_guard = EnvVarGuard::set("OPERAI_PROJECT_CONFIG_PATH", config_path.as_os_str());

        let config = ProjectConfig::load()?;

        assert_eq!(config.embedding_provider, None);
        assert_eq!(config.embedding_model, None);

        Ok(())
    }

    #[test]
    fn test_project_config_load_reads_values_from_local_file() -> Result<()> {
        let _lock = test_lock();

        let temp_project = TempDir::new("operai-project-values-")?;
        let config_path = temp_project.path().join("operai.toml");
        let _config_guard = EnvVarGuard::set("OPERAI_PROJECT_CONFIG_PATH", config_path.as_os_str());

        write_file(
            &config_path,
            r#"
embedding_provider = "openai"
embedding_model = "text-embedding-3-small"
"#,
        )?;

        let config = ProjectConfig::load()?;

        assert_eq!(config.embedding_provider.as_deref(), Some("openai"));
        assert_eq!(
            config.embedding_model.as_deref(),
            Some("text-embedding-3-small")
        );

        Ok(())
    }

    #[test]
    fn test_project_config_load_errors_when_local_path_is_directory() -> Result<()> {
        let _lock = test_lock();

        let temp_project = TempDir::new("operai-project-read-error-")?;
        let config_path = temp_project.path().join("operai.toml");
        let _config_guard = EnvVarGuard::set("OPERAI_PROJECT_CONFIG_PATH", config_path.as_os_str());

        std::fs::create_dir_all(&config_path)?;

        let err = ProjectConfig::load().expect_err("expected read error");
        let msg = err.to_string();
        assert!(msg.contains("failed to read:"), "{msg}");
        assert!(msg.contains(&config_path.display().to_string()), "{msg}");

        Ok(())
    }

    #[test]
    fn test_project_config_load_errors_on_invalid_toml_with_path_context() -> Result<()> {
        let _lock = test_lock();

        let temp_project = TempDir::new("operai-project-parse-error-")?;
        let config_path = temp_project.path().join("operai.toml");
        let _config_guard = EnvVarGuard::set("OPERAI_PROJECT_CONFIG_PATH", config_path.as_os_str());

        write_file(&config_path, "embedding_provider = [")?;

        let err = ProjectConfig::load().expect_err("expected parse error");
        let msg = err.to_string();
        assert!(msg.contains("failed to parse:"), "{msg}");
        assert!(msg.contains(&config_path.display().to_string()), "{msg}");

        Ok(())
    }

    #[test]
    fn test_config_default_produces_expected_values() {
        let config = Config::default();

        assert_eq!(config.embedding.provider, "fastembed");
        assert_eq!(config.embedding.model, None);
        assert_eq!(config.embedding.fastembed.model, "nomic-embed-text-v1.5");
        assert!(config.embedding.fastembed.show_download_progress);
        assert_eq!(config.embedding.openai.api_key_env, "OPENAI_API_KEY");
    }

    #[test]
    fn test_project_config_default_produces_none_values() {
        let config = ProjectConfig::default();

        assert_eq!(config.embedding_provider, None);
        assert_eq!(config.embedding_model, None);
    }

    #[test]
    fn test_config_round_trip_serialization() -> Result<()> {
        let original = Config {
            embedding: EmbeddingConfig {
                provider: "openai".to_string(),
                model: Some("text-embedding-3-large".to_string()),
                fastembed: FastEmbedConfig {
                    model: "custom-model".to_string(),
                    show_download_progress: false,
                },
                openai: OpenAIConfig {
                    api_key_env: "CUSTOM_KEY".to_string(),
                },
            },
        };

        let serialized = toml::to_string(&original)?;
        let deserialized: Config = toml::from_str(&serialized)?;

        assert_eq!(deserialized.embedding.provider, original.embedding.provider);
        assert_eq!(deserialized.embedding.model, original.embedding.model);
        assert_eq!(
            deserialized.embedding.fastembed.model,
            original.embedding.fastembed.model
        );
        assert_eq!(
            deserialized.embedding.fastembed.show_download_progress,
            original.embedding.fastembed.show_download_progress
        );
        assert_eq!(
            deserialized.embedding.openai.api_key_env,
            original.embedding.openai.api_key_env
        );

        Ok(())
    }

    #[test]
    fn test_project_config_round_trip_serialization() -> Result<()> {
        let original = ProjectConfig {
            embedding_provider: Some("openai".to_string()),
            embedding_model: Some("text-embedding-3-small".to_string()),
        };

        let serialized = toml::to_string(&original)?;
        let deserialized: ProjectConfig = toml::from_str(&serialized)?;

        assert_eq!(deserialized.embedding_provider, original.embedding_provider);
        assert_eq!(deserialized.embedding_model, original.embedding_model);

        Ok(())
    }

    #[test]
    fn test_config_load_reads_nested_section_and_applies_defaults_for_rest() -> Result<()> {
        let _lock = test_lock();

        let temp_home = TempDir::new("operai-config-openai-only-")?;
        let _home_guard = EnvVarGuard::set("HOME", temp_home.path().as_os_str());

        let path = home_config_path(temp_home.path());
        write_file(
            &path,
            r#"
[embedding.openai]
api_key_env = "MY_API_KEY"
"#,
        )?;

        let config = Config::load()?;

        // Verify openai section was read
        assert_eq!(config.embedding.openai.api_key_env, "MY_API_KEY");
        // Verify all other defaults are applied
        assert_eq!(config.embedding.provider, "fastembed");
        assert_eq!(config.embedding.model, None);
        assert_eq!(config.embedding.fastembed.model, "nomic-embed-text-v1.5");
        assert!(config.embedding.fastembed.show_download_progress);

        Ok(())
    }

    #[test]
    fn test_embedding_config_default_produces_expected_values() {
        let config = EmbeddingConfig::default();

        assert_eq!(config.provider, "fastembed");
        assert_eq!(config.model, None);
        assert_eq!(config.fastembed.model, "nomic-embed-text-v1.5");
        assert!(config.fastembed.show_download_progress);
        assert_eq!(config.openai.api_key_env, "OPENAI_API_KEY");
    }

    #[test]
    fn test_fastembed_config_default_produces_expected_values() {
        let config = FastEmbedConfig::default();

        assert_eq!(config.model, "nomic-embed-text-v1.5");
        assert!(config.show_download_progress);
    }

    #[test]
    fn test_openai_config_default_produces_expected_values() {
        let config = OpenAIConfig::default();

        assert_eq!(config.api_key_env, "OPENAI_API_KEY");
    }

    #[test]
    fn test_project_config_with_all_none_round_trip() -> Result<()> {
        let config = ProjectConfig {
            embedding_provider: None,
            embedding_model: None,
        };

        let serialized = toml::to_string(&config)?;
        let deserialized: ProjectConfig = toml::from_str(&serialized)?;

        assert_eq!(deserialized.embedding_provider, None);
        assert_eq!(deserialized.embedding_model, None);

        Ok(())
    }

    #[test]
    fn test_project_config_partial_values_round_trip() -> Result<()> {
        let config = ProjectConfig {
            embedding_provider: Some("openai".to_string()),
            embedding_model: None,
        };

        let serialized = toml::to_string(&config)?;
        let deserialized: ProjectConfig = toml::from_str(&serialized)?;

        assert_eq!(deserialized.embedding_provider, config.embedding_provider);
        assert_eq!(deserialized.embedding_model, config.embedding_model);

        Ok(())
    }
}
