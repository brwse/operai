//! `cargo operai embed` command implementation.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use operai_embedding::{EmbeddingGenerator, write_embedding_file};
use tracing::info;

/// Arguments for the `embed` command.
#[derive(Args)]
pub struct EmbedArgs {
    /// Path to the crate to embed (defaults to current directory).
    #[arg(short, long)]
    pub path: Option<PathBuf>,

    /// Embedding provider (fastembed or openai).
    #[arg(short = 'P', long)]
    pub provider: Option<String>,

    /// Embedding model to use.
    #[arg(short, long)]
    pub model: Option<String>,

    /// Output file for the embedding (defaults to .brwse-embedding).
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

async fn run_with_generator<G, CreateGenerator, EmbedCrate, EmbedFuture>(
    args: &EmbedArgs,
    create_generator: CreateGenerator,
    embed_crate: EmbedCrate,
) -> Result<()>
where
    CreateGenerator: FnOnce(Option<&str>, Option<&str>) -> Result<G>,
    EmbedCrate: FnOnce(G, PathBuf) -> EmbedFuture,
    EmbedFuture: std::future::Future<Output = Result<Vec<f32>>>,
{
    let crate_path = args.path.clone().unwrap_or_else(|| PathBuf::from("."));
    let output_path = args
        .output
        .clone()
        .unwrap_or_else(|| crate_path.join(".brwse-embedding"));

    let cargo_toml = crate_path.join("Cargo.toml");
    if !cargo_toml.exists() {
        anyhow::bail!("no Cargo.toml found in: {}", crate_path.display());
    }

    let generator = create_generator(args.provider.as_deref(), args.model.as_deref())?;

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .expect("invalid template"),
    );
    pb.set_message("Generating embedding...");
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    let embedding = embed_crate(generator, crate_path).await?;

    pb.finish_and_clear();

    write_embedding_file(&output_path, &embedding).context("failed to write embedding file")?;

    info!(
        dimension = embedding.len(),
        output = %output_path.display(),
        "Embedding generated"
    );

    println!(
        "{} Generated embedding ({} dimensions) -> {}",
        style("✓").green().bold(),
        embedding.len(),
        output_path.display()
    );

    Ok(())
}

/// Generates a vector embedding for a crate and writes it to a file.
pub async fn run(args: &EmbedArgs) -> Result<()> {
    run_with_generator(
        args,
        EmbeddingGenerator::from_config,
        |mut generator, crate_path| async move { generator.embed_crate(&crate_path).await },
    )
    .await
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::{OsStr, OsString},
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    use anyhow::{Context, Result};

    use super::*;
    use crate::testing::test_lock_async;

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
            let dir_name = format!("{prefix}{nanos}-{}-{unique}", std::process::id());
            let path = std::env::temp_dir().join(dir_name);
            std::fs::create_dir_all(&path)
                .with_context(|| format!("create temp dir: {}", path.display()))?;
            Ok(Self { path })
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write_file(&self, relative: impl AsRef<Path>, contents: &str) -> Result<PathBuf> {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("create dir: {}", parent.display()))?;
            }
            std::fs::write(&path, contents)
                .with_context(|| format!("write file: {}", path.display()))?;
            Ok(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &OsStr) -> Self {
            let previous = std::env::var_os(key);
            // SAFETY: Tests are run with `flavor = "current_thread"` ensuring
            // single-threaded execution. The guard restores the previous value
            // on drop.
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            // SAFETY: Tests are run with `flavor = "current_thread"` ensuring
            // single-threaded execution. This restores the environment to its
            // previous state.
            match self.previous.take() {
                Some(value) => unsafe { std::env::set_var(self.key, value) },
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }

    struct CurrentDirGuard {
        previous: PathBuf,
    }

    impl CurrentDirGuard {
        fn set(path: &Path) -> Result<Self> {
            let previous = std::env::current_dir().context("get current dir")?;
            std::env::set_current_dir(path)
                .with_context(|| format!("set current dir: {}", path.display()))?;
            Ok(Self { previous })
        }
    }

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.previous);
        }
    }

    fn embedding_bytes(embedding: &[f32]) -> Vec<u8> {
        embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_run_returns_error_when_cargo_toml_missing() -> Result<()> {
        let _lock = test_lock_async().await;

        let temp_crate = TempDir::new("cargo-operai-embed-missing-cargo-toml-")?;

        let args = EmbedArgs {
            path: Some(temp_crate.path().to_path_buf()),
            provider: None,
            model: None,
            output: None,
        };

        let err = run(&args)
            .await
            .expect_err("expected missing Cargo.toml to error");
        assert_eq!(
            err.to_string(),
            format!("no Cargo.toml found in: {}", temp_crate.path().display())
        );

        assert!(
            !temp_crate.path().join(".brwse-embedding").exists(),
            "embedding file should not be written when Cargo.toml is missing"
        );

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_run_returns_error_when_provider_unknown() -> Result<()> {
        let _lock = test_lock_async().await;

        let temp_home = TempDir::new("cargo-operai-embed-home-")?;
        let _home_guard = EnvVarGuard::set("HOME", temp_home.path().as_os_str());

        let temp_crate = TempDir::new("cargo-operai-embed-unknown-provider-")?;
        temp_crate.write_file(
            "Cargo.toml",
            "[package]\nname = \"t\"\nversion = \"0.1.0\"\n",
        )?;
        let _cwd_guard = CurrentDirGuard::set(temp_crate.path())?;

        let args = EmbedArgs {
            path: Some(temp_crate.path().to_path_buf()),
            provider: Some("not-a-provider".to_owned()),
            model: None,
            output: None,
        };

        let err = run(&args)
            .await
            .expect_err("expected unknown provider override to error");
        let message = err.to_string();
        assert!(
            message.contains("unknown embedding provider: not-a-provider"),
            "unexpected error message: {message}"
        );

        assert!(
            !temp_crate.path().join(".brwse-embedding").exists(),
            "embedding file should not be written when provider config fails"
        );

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_run_defaults_to_current_directory_and_writes_default_output_file() -> Result<()> {
        let _lock = test_lock_async().await;

        let temp_crate = TempDir::new("cargo-operai-embed-default-path-")?;
        temp_crate.write_file(
            "Cargo.toml",
            "[package]\nname = \"t\"\nversion = \"0.1.0\"\n",
        )?;
        let _cwd_guard = CurrentDirGuard::set(temp_crate.path())?;

        let args = EmbedArgs {
            path: None,
            provider: None,
            model: None,
            output: None,
        };

        let embedding = vec![1.0_f32, 2.5_f32];
        run_with_generator(
            &args,
            |_provider, _model| Ok(()),
            |(), _crate_path| async { Ok(embedding.clone()) },
        )
        .await?;

        let output_path = temp_crate.path().join(".brwse-embedding");
        let bytes = std::fs::read(&output_path)
            .with_context(|| format!("read {}", output_path.display()))?;

        assert_eq!(bytes, embedding_bytes(&embedding));
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_run_uses_explicit_output_path() -> Result<()> {
        let _lock = test_lock_async().await;

        let temp_crate = TempDir::new("cargo-operai-embed-explicit-output-")?;
        temp_crate.write_file(
            "Cargo.toml",
            "[package]\nname = \"t\"\nversion = \"0.1.0\"\n",
        )?;

        let output_path = temp_crate.path().join("custom-output.bin");
        let args = EmbedArgs {
            path: Some(temp_crate.path().to_path_buf()),
            provider: Some("ignored".to_owned()),
            model: Some("also-ignored".to_owned()),
            output: Some(output_path.clone()),
        };

        let embedding = vec![0.0_f32, -1.25_f32, 42.0_f32];
        run_with_generator(
            &args,
            |provider, model| {
                assert_eq!(provider, Some("ignored"));
                assert_eq!(model, Some("also-ignored"));
                Ok(())
            },
            |(), _crate_path| async { Ok(embedding.clone()) },
        )
        .await?;

        let bytes = std::fs::read(&output_path)
            .with_context(|| format!("read {}", output_path.display()))?;
        assert_eq!(bytes, embedding_bytes(&embedding));

        assert!(
            !temp_crate.path().join(".brwse-embedding").exists(),
            "default output path should not be written when output is specified"
        );

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_run_adds_context_when_write_embedding_file_fails() -> Result<()> {
        let _lock = test_lock_async().await;

        let temp_crate = TempDir::new("cargo-operai-embed-write-failure-")?;
        temp_crate.write_file(
            "Cargo.toml",
            "[package]\nname = \"t\"\nversion = \"0.1.0\"\n",
        )?;

        let output_path = temp_crate.path().join("output-dir");
        std::fs::create_dir_all(&output_path)
            .with_context(|| format!("create dir {}", output_path.display()))?;

        let args = EmbedArgs {
            path: Some(temp_crate.path().to_path_buf()),
            provider: None,
            model: None,
            output: Some(output_path.clone()),
        };

        let err = run_with_generator(
            &args,
            |_provider, _model| Ok(()),
            |(), _crate_path| async { Ok(vec![1.0_f32]) },
        )
        .await
        .expect_err("expected writing embedding file to a directory to error");

        let message = err.to_string();
        assert!(
            message.contains("failed to write embedding file"),
            "unexpected error message: {message}"
        );
        let output_path_str = output_path.display().to_string();
        assert!(
            err.chain()
                .any(|cause| cause.to_string().contains(&output_path_str)),
            "expected output path to appear somewhere in error chain: {message}"
        );

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_run_propagates_error_from_embed_operation() -> Result<()> {
        let _lock = test_lock_async().await;

        let temp_crate = TempDir::new("cargo-operai-embed-embed-error-")?;
        temp_crate.write_file(
            "Cargo.toml",
            "[package]\nname = \"t\"\nversion = \"0.1.0\"\n",
        )?;

        let args = EmbedArgs {
            path: Some(temp_crate.path().to_path_buf()),
            provider: None,
            model: None,
            output: None,
        };

        let err = run_with_generator(
            &args,
            |_provider, _model| Ok(()),
            |(), _crate_path| async { Err(anyhow::anyhow!("embedding model failed to process")) },
        )
        .await
        .expect_err("expected embed operation error to propagate");

        assert_eq!(err.to_string(), "embedding model failed to process");

        assert!(
            !temp_crate.path().join(".brwse-embedding").exists(),
            "embedding file should not be written when embed operation fails"
        );

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_run_propagates_error_from_generator_creation() -> Result<()> {
        let _lock = test_lock_async().await;

        let temp_crate = TempDir::new("cargo-operai-embed-generator-error-")?;
        temp_crate.write_file(
            "Cargo.toml",
            "[package]\nname = \"t\"\nversion = \"0.1.0\"\n",
        )?;

        let args = EmbedArgs {
            path: Some(temp_crate.path().to_path_buf()),
            provider: None,
            model: None,
            output: None,
        };

        let err = run_with_generator(
            &args,
            |_provider, _model| Err(anyhow::anyhow!("failed to initialize generator")),
            |(): (), _crate_path| async { Ok(vec![1.0_f32]) },
        )
        .await
        .expect_err("expected generator creation error to propagate");

        assert_eq!(err.to_string(), "failed to initialize generator");

        assert!(
            !temp_crate.path().join(".brwse-embedding").exists(),
            "embedding file should not be written when generator creation fails"
        );

        Ok(())
    }
}
