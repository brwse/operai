//! Build command for Operai tools.
//!
//! This module implements the `cargo operai build` command, which:
//! - Optionally generates an embedding for the tool's codebase
//! - Builds the tool in release mode using `cargo build --release`
//!
//! The build process generates an embedding file (`.brwse-embedding`) by default,
//! which can be used for semantic search and code understanding. This step can be
//! skipped with the `--skip-embed` flag.
//!
//! # Error Handling
//!
//! Embedding generation failures are non-fatal - the command will print a warning
//! and continue with the cargo build. However, cargo build failures will terminate
//! the command with an error.

use std::{ffi::OsStr, path::PathBuf, process::Command};

use anyhow::{Context, Result};
use clap::Args;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use crate::embedding::{EmbeddingGenerator, write_embedding_file};
use tracing::info;

/// Command-line arguments for the build command.
#[derive(Args)]
pub struct BuildArgs {
    /// Path to the crate directory to build.
    ///
    /// Defaults to the current directory if not specified.
    #[arg(short, long)]
    pub path: Option<PathBuf>,

    /// Skip embedding generation.
    ///
    /// When set to `true`, the build process will not generate an embedding file
    /// and will proceed directly to the cargo build step.
    #[arg(long)]
    pub skip_embed: bool,

    /// Additional arguments to pass to `cargo build`.
    ///
    /// These arguments are passed through directly to cargo and can be used to
    /// specify features, target, etc. They must appear after `--` on the command line.
    #[arg(last = true)]
    pub cargo_args: Vec<String>,
}

/// Runs the build command with the given arguments.
///
/// This is the main entry point for the `cargo operai build` command.
/// It delegates to `run_with` with "cargo" as the program.
///
/// # Arguments
///
/// * `args` - Command-line arguments for the build command
/// * `config` - Operai project config
///
/// # Errors
///
/// Returns an error if:
/// - The cargo build process fails to execute
/// - The cargo build command returns a non-zero exit code
pub async fn run(args: &BuildArgs, config: &operai_core::Config) -> Result<()> {
    run_with(args, "cargo", config).await
}

/// Runs the build command with a custom cargo program.
///
/// This function is primarily used for testing to inject a fake cargo binary.
/// It performs the following steps:
///
/// 1. Determines the crate path (defaults to current directory if not specified)
/// 2. If `skip_embed` is false, attempts to generate an embedding:
///    - Uses the provided config to get embedding settings
///    - Creates an `EmbeddingGenerator` from the config
///    - Generates an embedding for the crate
///    - Writes the embedding to `.brwse-embedding` in the crate directory
///    - Embedding failures are logged but do not stop the build
/// 3. Runs `cargo build --release` with any additional cargo arguments
/// 4. Returns an error if cargo build fails
///
/// # Type Parameters
///
/// * `P` - A type that can be converted to an OS string (e.g., `&str`, `PathBuf`)
///
/// # Arguments
///
/// * `args` - Command-line arguments for the build command
/// * `cargo_program` - Path to the cargo executable (for testing)
/// * `config` - Operai project config
///
/// # Errors
///
/// Returns an error if:
/// - The cargo program cannot be executed
/// - The cargo build command returns a non-zero exit code
async fn run_with<P>(args: &BuildArgs, cargo_program: P, config: &operai_core::Config) -> Result<()>
where
    P: AsRef<OsStr>,
{
    let crate_path = args.path.clone().unwrap_or_else(|| PathBuf::from("."));

    if !args.skip_embed {
        println!("{} Generating embedding...", style("→").cyan());

        let output_path = crate_path.join(".brwse-embedding");
        let cargo_toml = crate_path.join("Cargo.toml");

        if let Err(e) = async {
            if !cargo_toml.exists() {
                anyhow::bail!("no Cargo.toml found in: {}", crate_path.display());
            }

            let generator = EmbeddingGenerator::from_config(config)?;

            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} {msg}")
                    .expect("invalid template"),
            );
            pb.set_message("Generating embedding...");
            pb.enable_steady_tick(std::time::Duration::from_millis(100));

            let embedding = generator.embed_crate(&crate_path).await?;

            pb.finish_and_clear();

            write_embedding_file(&output_path, &embedding)
                .context("failed to write embedding file")?;

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

            Ok::<(), anyhow::Error>(())
        }
        .await
        {
            println!(
                "{} Embedding generation failed: {} (continuing without embedding)",
                style("⚠").yellow(),
                e
            );
        }
    }

    println!("{} Building tool...", style("→").cyan());

    let mut cmd = Command::new(cargo_program);
    cmd.arg("build").arg("--release").current_dir(&crate_path);

    for arg in &args.cargo_args {
        cmd.arg(arg);
    }

    let status = cmd.status().context("failed to run cargo build")?;

    if !status.success() {
        anyhow::bail!("cargo build failed with exit code: {:?}", status.code());
    }

    println!("{} Build complete!", style("✓").green().bold());

    let target_dir = crate_path.join("target/release");
    println!("\nBuilt artifacts in: {}", target_dir.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;
    use crate::testing;

    /// Acquires the async test lock to prevent concurrent test execution.
    async fn test_lock_async() -> tokio::sync::MutexGuard<'static, ()> {
        testing::test_lock_async().await
    }

    /// RAII guard that restores the previous working directory when dropped.
    ///
    /// This is used in tests to temporarily change the current directory and
    /// automatically restore it when the test completes.
    struct CurrentDirGuard {
        previous: PathBuf,
    }

    impl CurrentDirGuard {
        /// Sets the current directory to `path` and returns a guard that will
        /// restore the previous directory when dropped.
        fn set(path: &Path) -> anyhow::Result<Self> {
            let previous = std::env::current_dir()?;
            std::env::set_current_dir(path)?;
            Ok(Self { previous })
        }
    }

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.previous);
        }
    }

    /// RAII guard that creates a temporary directory and deletes it when dropped.
    ///
    /// The directory name includes:
    /// - The provided prefix
    /// - A nanosecond timestamp
    /// - The process ID
    /// - A counter to ensure uniqueness
    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        /// Creates a new temporary directory with a unique name.
        fn new(prefix: &str) -> anyhow::Result<Self> {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);

            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let mut path = std::env::temp_dir();
            path.push(format!("{prefix}-{nanos}-{}-{unique}", std::process::id()));
            fs::create_dir_all(&path)?;
            Ok(Self { path })
        }

        /// Returns a reference to the temporary directory path.
        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    /// Installs a fake cargo binary in the specified directory.
    ///
    /// The fake cargo records:
    /// - Its current working directory to `cargo_cwd.txt`
    /// - Its arguments to `cargo_args.txt`
    /// - Exits with the specified `exit_code`
    ///
    /// This is used to test that the build command correctly invokes cargo
    /// with the right arguments and in the right directory.
    fn install_fake_cargo(bin_dir: &Path, exit_code: i32) -> anyhow::Result<PathBuf> {
        #[cfg(windows)]
        {
            let script_path = bin_dir.join("cargo.bat");
            let script = format!(
                "@echo off\r\ncd > cargo_cwd.txt\r\ntype nul > cargo_args.txt\r\nfor %%a in (%*) \
                 do echo %%a>>cargo_args.txt\r\nexit /b {exit_code}\r\n"
            );
            fs::write(script_path, script)?;
            Ok(bin_dir.join("cargo.bat"))
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let script_path = bin_dir.join("cargo");
            let script = format!(
                "#!/bin/sh\nset -eu\npwd > cargo_cwd.txt\nprintf '%s\\n' \"$@\" > \
                 cargo_args.txt\nexit {exit_code}\n"
            );
            fs::write(&script_path, script)?;
            let mut permissions = fs::metadata(&script_path)?.permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&script_path, permissions)?;
            Ok(script_path)
        }
    }

    /// Reads a file and returns its lines as a vector of strings.
    fn read_lines(path: &Path) -> anyhow::Result<Vec<String>> {
        Ok(fs::read_to_string(path)?
            .lines()
            .map(str::to_owned)
            .collect())
    }

    /// Creates an empty config for testing.
    fn test_config() -> operai_core::Config {
        operai_core::Config::empty()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_run_skips_embedding_when_skip_embed_true() -> anyhow::Result<()> {
        // Arrange
        let temp = TestTempDir::new("operai-build")?;
        let crate_dir = temp.path().join("crate");
        let bin_dir = temp.path().join("bin");
        fs::create_dir_all(&crate_dir)?;
        fs::create_dir_all(&bin_dir)?;

        let cargo_path = install_fake_cargo(&bin_dir, 0)?;

        let args = BuildArgs {
            path: Some(crate_dir.clone()),
            skip_embed: true,
            cargo_args: vec!["--features".to_owned(), "foo".to_owned()],
        };

        // Act
        run_with(&args, cargo_path, &test_config()).await?;

        // Assert
        let cargo_args = read_lines(&crate_dir.join("cargo_args.txt"))?;
        assert_eq!(
            cargo_args,
            vec![
                "build".to_owned(),
                "--release".to_owned(),
                "--features".to_owned(),
                "foo".to_owned(),
            ]
        );

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_run_continues_on_embed_error() -> anyhow::Result<()> {
        // Arrange
        let temp = TestTempDir::new("operai-build")?;
        let crate_dir = temp.path().join("crate");
        let bin_dir = temp.path().join("bin");
        fs::create_dir_all(&crate_dir)?;
        fs::create_dir_all(&bin_dir)?;
        // Create a minimal Cargo.toml to avoid the "no Cargo.toml" error
        fs::write(
            crate_dir.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )?;
        // Create an operai.toml with invalid embedding config to trigger embedding failure
        fs::write(
            crate_dir.join("operai.toml"),
            "[embeddings]\ntype = \"invalid-type-that-does-not-exist\"\nmodel = \"some-model\"\n",
        )?;

        let cargo_path = install_fake_cargo(&bin_dir, 0)?;

        let args = BuildArgs {
            path: Some(crate_dir.clone()),
            skip_embed: false,
            cargo_args: Vec::new(),
        };

        // Act - embedding will fail but cargo build should succeed
        run_with(&args, cargo_path, &test_config()).await?;

        // Assert
        let cargo_args = read_lines(&crate_dir.join("cargo_args.txt"))?;
        assert_eq!(cargo_args, vec!["build".to_owned(), "--release".to_owned()]);

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_run_returns_error_when_cargo_build_fails() -> anyhow::Result<()> {
        // Arrange
        let temp = TestTempDir::new("operai-build")?;
        let crate_dir = temp.path().join("crate");
        let bin_dir = temp.path().join("bin");
        fs::create_dir_all(&crate_dir)?;
        fs::create_dir_all(&bin_dir)?;

        let cargo_path = install_fake_cargo(&bin_dir, 42)?;

        let args = BuildArgs {
            path: Some(crate_dir),
            skip_embed: true,
            cargo_args: Vec::new(),
        };

        // Act
        let error = run_with(&args, cargo_path, &test_config())
            .await
            .expect_err("expected cargo build failure");

        // Assert
        let message = error.to_string();
        assert!(message.contains("cargo build failed with exit code"));
        assert!(message.contains("Some(42)"));

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_run_adds_context_when_cargo_cannot_be_executed() -> anyhow::Result<()> {
        // Arrange
        let temp = TestTempDir::new("operai-build")?;
        let crate_dir = temp.path().join("crate");
        fs::create_dir_all(&crate_dir)?;

        let args = BuildArgs {
            path: Some(crate_dir),
            skip_embed: true,
            cargo_args: Vec::new(),
        };

        // Act
        let missing_cargo_path = temp.path().join("missing-cargo");
        let error = run_with(&args, missing_cargo_path, &test_config())
            .await
            .expect_err("expected cargo to be missing");

        // Assert
        assert!(error.to_string().contains("failed to run cargo build"));

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_run_executes_cargo_in_crate_directory() -> anyhow::Result<()> {
        // Arrange
        let temp = TestTempDir::new("operai-build")?;
        let crate_dir = temp.path().join("my-crate");
        let bin_dir = temp.path().join("bin");
        fs::create_dir_all(&crate_dir)?;
        fs::create_dir_all(&bin_dir)?;

        let cargo_path = install_fake_cargo(&bin_dir, 0)?;

        let args = BuildArgs {
            path: Some(crate_dir.clone()),
            skip_embed: true,
            cargo_args: Vec::new(),
        };

        // Act
        run_with(&args, cargo_path, &test_config()).await?;

        // Assert - verify cargo was run in the crate directory
        let cargo_cwd = fs::read_to_string(crate_dir.join("cargo_cwd.txt"))?;
        let cargo_cwd = cargo_cwd.trim();

        // Canonicalize both paths for comparison (handles symlinks like /var ->
        // /private/var on macOS)
        let expected_cwd = fs::canonicalize(&crate_dir)?;
        let actual_cwd = fs::canonicalize(cargo_cwd)?;
        assert_eq!(actual_cwd, expected_cwd);

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_run_defaults_to_current_directory_when_path_is_none() -> anyhow::Result<()> {
        // Acquire test lock for current directory manipulation
        let _lock = test_lock_async().await;

        // Arrange
        let temp = TestTempDir::new("operai-build")?;
        let bin_dir = temp.path().join("bin");
        fs::create_dir_all(&bin_dir)?;

        let cargo_path = install_fake_cargo(&bin_dir, 0)?;

        // Use CurrentDirGuard to ensure directory is restored BEFORE temp is dropped.
        // Drop order is reverse of declaration: _cwd_guard drops first, then temp.
        let _cwd_guard = CurrentDirGuard::set(temp.path())?;

        let args = BuildArgs {
            path: None, // Should default to current directory
            skip_embed: true,
            cargo_args: Vec::new(),
        };

        // Act
        run_with(&args, cargo_path, &test_config()).await?;

        // Assert - verify cargo was run in temp.path() (the "current directory")
        let cargo_cwd = fs::read_to_string(temp.path().join("cargo_cwd.txt"))?;
        let cargo_cwd = cargo_cwd.trim();

        let expected_cwd = fs::canonicalize(temp.path())?;
        let actual_cwd = fs::canonicalize(cargo_cwd)?;
        assert_eq!(actual_cwd, expected_cwd);

        Ok(())
    }
}
