//! Local gRPC server for serving Operai toolbox tools.
//!
//! This module provides functionality to run a local gRPC server that exposes
//! tools defined in an Operai config. The server supports gRPC reflection,
//! health checks, and graceful shutdown.

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use clap::Args;
use console::style;
use operai_runtime::{RuntimeBuilder, SearchEmbedder, proto, transports};
use tokio::signal;
use tonic::transport::Server;
use tonic_health::ServingStatus;
use tracing::info;

use crate::embedding::EmbeddingGenerator;

/// Command-line arguments for the serve command.
#[derive(Args)]
pub struct ServeArgs {
    /// Path to the Operai project config file (defaults to `operai.toml`).
    #[arg(short, long)]
    pub config: Option<PathBuf>,
    /// Port to listen on (defaults to 50051).
    #[arg(short, long, default_value = "50051")]
    pub port: u16,
}

/// Runs the gRPC server, listening for Ctrl+C to trigger graceful shutdown.
///
/// This is the main entry point for the serve command. It initializes the
/// runtime from the config and starts a gRPC server with health checks and
/// reflection.
pub async fn run(args: &ServeArgs, config: &operai_core::Config) -> Result<()> {
    let shutdown = async {
        let _ = signal::ctrl_c().await;
        info!("Received shutdown signal");
    };
    run_with_shutdown(args, shutdown, config).await
}

/// Runs the gRPC server with a custom shutdown trigger.
///
/// # Arguments
///
/// * `args` - Configuration for the server (config path and port)
/// * `shutdown` - A future that completes when shutdown should occur
/// * `config` - Operai project config
///
/// # Behavior
///
/// The server will:
/// 1. Load tools from the config (or `operai.toml` if not specified)
/// 2. Initialize search embedder from config if embedding is configured
/// 3. Start a gRPC server on the specified port (default 50051)
/// 4. Expose the toolbox service, health checks, and gRPC reflection
/// 5. Wait for the shutdown future to complete
/// 6. Drain in-flight requests before exiting
async fn run_with_shutdown<F>(
    args: &ServeArgs,
    shutdown: F,
    config: &operai_core::Config,
) -> Result<()>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    println!("{} Starting local toolbox server...", style("→").cyan());

    let config_path = args
        .config
        .clone()
        .unwrap_or_else(|| PathBuf::from("operai.toml"));

    let local_runtime = RuntimeBuilder::new()
        .with_config_path(config_path)
        .build_local()
        .await
        .context("failed to initialize runtime")?;

    println!(
        "{} Loaded {} tool(s)",
        style("✓").green().bold(),
        local_runtime.registry().len()
    );

    // Create search embedder from config if embedding is configured
    let search_embedder = config.embedding.as_ref().and_then(|_| {
        let generator =
            EmbeddingGenerator::from_config(config).context("failed to initialize search embedder");
        match generator {
            Ok(embedder) => {
                println!(
                    "{} {} Initialized search embedder",
                    style("→").cyan(),
                    style("✓").green().bold()
                );
                Some(Arc::new(CliSearchEmbedder::new(embedder)) as Arc<dyn SearchEmbedder>)
            }
            Err(e) => {
                println!(
                    "{} Failed to initialize search embedder: {} (search disabled)",
                    style("⚠").yellow(),
                    e
                );
                None
            }
        }
    });

    let local_runtime = if let Some(embedder) = search_embedder {
        local_runtime.with_search_embedder(Some(embedder))
    } else {
        local_runtime
    };

    let toolbox_service = transports::grpc::ToolboxService::from_runtime(local_runtime.clone());

    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_service_status("", ServingStatus::Serving)
        .await;
    health_reporter
        .set_service_status("brwse.toolbox.v1alpha1.Toolbox", ServingStatus::Serving)
        .await;

    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));

    info!(address = %addr, "Starting gRPC server");

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
        .build_v1()?;

    println!(
        "{} Server running on http://{}",
        style("✓").green().bold(),
        addr
    );
    println!("Press Ctrl+C to stop\n");

    Server::builder()
        .add_service(health_service)
        .add_service(reflection_service)
        .add_service(proto::toolbox_server::ToolboxServer::new(toolbox_service))
        .serve_with_shutdown(addr, shutdown)
        .await
        .context("server error")?;

    info!("Draining inflight requests");
    local_runtime.drain().await;

    info!("Operai Toolbox stopped");
    Ok(())
}

/// Wrapper that adapts [`EmbeddingGenerator`] to the [`SearchEmbedder`] trait.
///
/// This struct provides thread-safe access to an embedding generator,
/// allowing it to be used concurrently by multiple search requests.
struct CliSearchEmbedder {
    /// The underlying embedding generator.
    generator: EmbeddingGenerator,
}

impl CliSearchEmbedder {
    /// Creates a new CLI search embedder from an embedding generator.
    fn new(generator: EmbeddingGenerator) -> Self {
        Self { generator }
    }
}

impl SearchEmbedder for CliSearchEmbedder {
    /// Generates an embedding vector for the given query text.
    fn embed_query(&self, query: &str) -> operai_runtime::SearchEmbedFuture<'_> {
        let query = query.to_string();
        let generator = &self.generator;
        Box::pin(async move { generator.embed(&query).await.map_err(|err| err.to_string()) })
    }
}

/// Integration tests for the serve command.
///
/// These tests verify that:
/// - Command-line arguments parse correctly
/// - The server starts and accepts connections
/// - Tools are loaded and accessible via gRPC
///
/// Tests use the `hello-world` example crate as a test fixture.
#[cfg(test)]
mod tests {
    use std::{
        net::TcpListener,
        path::{Path, PathBuf},
        process::Command,
        sync::{
            OnceLock,
            atomic::{AtomicU64, Ordering},
        },
    };

    use clap::Parser;
    use tokio::sync::oneshot;

    use super::*;

    static HELLO_WORLD_CDYLIB_PATH: OnceLock<PathBuf> = OnceLock::new();
    static TEMP_CONFIG_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[derive(Parser)]
    struct ServeArgsCli {
        #[command(flatten)]
        serve: ServeArgs,
    }

    #[test]
    fn test_serve_args_defaults_port_to_50051() {
        let cli = ServeArgsCli::try_parse_from(["test"]).expect("args should parse");
        assert_eq!(cli.serve.port, 50051);
        assert_eq!(cli.serve.config, None);
    }

    #[test]
    fn test_serve_args_parses_config_and_port_flags() {
        let cli =
            ServeArgsCli::try_parse_from(["test", "--config", "custom.toml", "--port", "123"])
                .expect("args should parse");
        assert_eq!(cli.serve.port, 123);
        assert_eq!(cli.serve.config, Some(PathBuf::from("custom.toml")));
    }

    #[test]
    fn test_serve_args_parses_short_flags() {
        let cli = ServeArgsCli::try_parse_from(["test", "-c", "short.toml", "-p", "8080"])
            .expect("short flags should parse");
        assert_eq!(cli.serve.port, 8080);
        assert_eq!(cli.serve.config, Some(PathBuf::from("short.toml")));
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    fn cargo_target_dir_and_profile() -> (PathBuf, String) {
        let exe_path = std::env::current_exe().expect("test executable path should be available");
        let deps_dir = exe_path
            .parent()
            .expect("test executable should live in a deps directory");
        let profile_dir = deps_dir
            .parent()
            .expect("deps directory should have a profile directory parent");
        let profile = profile_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("debug")
            .to_string();
        let target_dir = profile_dir
            .parent()
            .expect("profile directory should have a target directory parent");
        (target_dir.to_path_buf(), profile)
    }

    fn expected_hello_world_cdylib_file_name() -> String {
        format!(
            "{}hello_world{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        )
    }

    /// Searches for the hello-world cdylib in the target directory.
    ///
    /// Looks in both `{profile}/` and `{profile}/deps/` for the library.
    fn find_hello_world_cdylib(target_dir: &Path, profile: &str) -> Option<PathBuf> {
        let file_name = expected_hello_world_cdylib_file_name();
        let profile_dir = target_dir.join(profile);

        let direct_path = profile_dir.join(&file_name);
        if direct_path.is_file() {
            return Some(direct_path);
        }

        let deps_dir = profile_dir.join("deps");
        let entries = std::fs::read_dir(deps_dir).ok()?;
        for entry in entries {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.file_name().and_then(|s| s.to_str()) == Some(file_name.as_str()) {
                return Some(path);
            }
        }

        None
    }

    /// Builds the hello-world crate as a cdylib.
    ///
    /// Builds in the specified profile and target directory. Panics if the
    /// build fails.
    fn build_hello_world_cdylib(target_dir: &Path, profile: &str) {
        let mut cmd = Command::new("cargo");
        cmd.current_dir(workspace_root());
        cmd.args(["build", "-p", "hello-world"]);
        if profile == "release" {
            cmd.arg("--release");
        }
        cmd.env("CARGO_TARGET_DIR", target_dir);

        let status = cmd.status().expect("cargo build should start");
        assert!(status.success(), "cargo build -p hello-world failed");
    }

    /// Gets or builds the hello-world cdylib path.
    ///
    /// Uses a `OnceLock` to ensure the cdylib is only built once per test run.
    fn hello_world_cdylib_path() -> PathBuf {
        HELLO_WORLD_CDYLIB_PATH
            .get_or_init(|| {
                let (target_dir, profile) = cargo_target_dir_and_profile();

                if let Some(path) = find_hello_world_cdylib(&target_dir, &profile) {
                    return path;
                }

                build_hello_world_cdylib(&target_dir, &profile);

                find_hello_world_cdylib(&target_dir, &profile)
                    .unwrap_or_else(|| panic!("hello-world cdylib not found after build"))
            })
            .clone()
    }

    /// Generates a unique temp config path for each test.
    ///
    /// Uses a counter and process ID to ensure uniqueness across concurrent
    /// test runs.
    fn temp_config_path() -> PathBuf {
        let counter = TEMP_CONFIG_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "cargo-operai-serve-config-{}-{counter}.toml",
            std::process::id()
        ))
    }

    /// Creates a temp config file that loads the given library.
    ///
    /// Handles Windows path escaping for TOML strings.
    fn write_config_for_library(path: &Path) -> PathBuf {
        let config_path = temp_config_path();
        let mut path_str = path.display().to_string();
        if std::path::MAIN_SEPARATOR == '\\' {
            path_str = path_str.replace('\\', "\\\\");
        }
        let contents = format!("[[tools]]\npath = \"{path_str}\"\n");
        std::fs::write(&config_path, contents).expect("write config");
        config_path
    }

    /// Attempts to connect to the gRPC server with retry logic.
    ///
    /// Retries up to 30 times with 50ms delays between attempts.
    async fn connect_with_retry(
        endpoint: &str,
    ) -> operai_runtime::proto::toolbox_client::ToolboxClient<tonic::transport::Channel> {
        let mut attempts = 0;
        loop {
            match operai_runtime::proto::toolbox_client::ToolboxClient::connect(
                endpoint.to_string(),
            )
            .await
            {
                Ok(client) => return client,
                Err(_) if attempts < 30 => {
                    attempts += 1;
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                }
                Err(e) => panic!("failed to connect to server: {e}"),
            }
        }
    }

    /// Integration test that verifies the serve command runs correctly.
    ///
    /// This test:
    /// 1. Builds a sample toolbox library
    /// 2. Creates a config referencing it
    /// 3. Starts the server on a random port
    /// 4. Connects and verifies tools are accessible
    /// 5. Shuts down the server cleanly
    #[tokio::test]
    async fn test_serve_runs_and_accepts_calls() -> Result<()> {
        let _lock = crate::testing::test_lock_async().await;

        let lib_path = hello_world_cdylib_path();
        let config_path = write_config_for_library(&lib_path);

        let port = TcpListener::bind("127.0.0.1:0")?.local_addr()?.port();
        let args = ServeArgs {
            config: Some(config_path),
            port,
        };

        let (tx, rx) = oneshot::channel::<()>();
        let server_handle = tokio::spawn(async move {
            run_with_shutdown(
                &args,
                async {
                    let _ = rx.await;
                },
                &operai_core::Config::empty(),
            )
            .await
        });

        let endpoint = format!("http://127.0.0.1:{port}");
        let mut client = connect_with_retry(&endpoint).await;

        let response = client
            .list_tools(operai_runtime::proto::ListToolsRequest {
                page_size: 1000,
                page_token: String::new(),
            })
            .await?
            .into_inner();

        assert!(
            response
                .tools
                .iter()
                .any(|tool| tool.name == "tools/hello-world.echo"),
            "expected hello-world tools to be listed"
        );

        let _ = tx.send(());
        server_handle.await.expect("server task")?;
        Ok(())
    }
}
