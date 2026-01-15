//! MCP (Model Context Protocol) server command implementation.
//!
//! This module provides the `mcp` subcommand which runs an MCP server that
//! exposes operai tools to MCP clients (such as Claude Desktop or other AI
//! assistants). The server can run in two modes:
//!
//! - **HTTP mode**: Exposes tools via a streaming HTTP endpoint on a
//!   configurable address/path
//! - **stdio mode**: Communicates via standard input/output for direct client
//!   integration
//!
//! The server supports optional semantic search capabilities when
//! `--searchable` is enabled, allowing clients to discover tools using natural
//! language queries.

use std::{future::Future, net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use console::style;
use operai_runtime::{McpService, RuntimeBuilder, SearchEmbedFuture, SearchEmbedder};
use rmcp::{
    service::ServiceExt,
    transport::{stdio, streamable_http_server::StreamableHttpServerConfig},
};
use tokio::signal;
use tracing::info;

use crate::embedding::EmbeddingGenerator;

/// Command-line arguments for the MCP server subcommand.
#[derive(Args)]
pub struct McpArgs {
    /// Path to the operai project config file (defaults to `operai.toml`).
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Address to bind the HTTP server to (defaults to `127.0.0.1:3333`).
    #[arg(short = 'a', long, default_value = "127.0.0.1:3333")]
    pub addr: String,

    /// HTTP path for the MCP endpoint (defaults to `/mcp`).
    #[arg(long, default_value = "/mcp")]
    pub path: String,

    /// Enable semantic search mode for tool discovery.
    ///
    /// When enabled, the server exposes a reduced set of search tools
    /// (`list_tool`, `find_tool`, `call_tool`) instead of exposing all
    /// registered tools directly.
    #[arg(long, default_value_t = false)]
    pub searchable: bool,

    /// Run in stdio mode instead of HTTP mode.
    ///
    /// In stdio mode, the server communicates via standard input/output,
    /// which is useful for direct integration with MCP clients.
    #[arg(long, default_value_t = false)]
    pub stdio: bool,
}

/// Runs the MCP server with the given configuration.
///
/// This function sets up a Ctrl+C signal handler and starts the server.
/// The server mode (HTTP or stdio) is determined by the `stdio` flag in `args`.
///
/// # Arguments
///
/// * `args` - Configuration for the MCP server
/// * `config` - Operai project config
pub async fn run(args: &McpArgs, config: &operai_core::Config) -> Result<()> {
    let shutdown = async {
        let _ = signal::ctrl_c().await;
        info!("Received shutdown signal");
    };
    run_with_shutdown(args, shutdown, config).await
}

/// Runs the MCP server with a custom shutdown future.
///
/// # Arguments
///
/// * `args` - Configuration for the MCP server
/// * `shutdown` - A future that completes when the server should shut down
/// * `config` - Operai project config
///
/// # Behavior
///
/// This function:
/// 1. Uses the provided config to initialize the runtime
/// 2. Optionally initializes a search embedder if `--searchable` is enabled
/// 3. Either starts an HTTP server or stdio server based on the `stdio` flag
/// 4. Waits for the shutdown signal
/// 5. Drains in-flight requests before returning
///
/// # Errors
///
/// Returns an error if:
/// - The config file cannot be loaded
/// - The runtime fails to initialize
/// - The search embedder fails to initialize (when `--searchable` is enabled)
/// - The server fails to bind to the specified address
/// - The server encounters an error during operation
async fn run_with_shutdown<F>(args: &McpArgs, shutdown: F, config: &operai_core::Config) -> Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let config_path = args
        .config
        .clone()
        .unwrap_or_else(|| PathBuf::from("operai.toml"));

    let local_runtime = RuntimeBuilder::new()
        .with_config_path(config_path)
        .build_local()
        .await
        .context("failed to initialize runtime")?;

    let search_embedder = if args.searchable {
        Some(
            std::sync::Arc::new(CliSearchEmbedder::new(
                EmbeddingGenerator::from_config(config)
                    .context("failed to initialize search embedder")?,
            )) as std::sync::Arc<dyn SearchEmbedder>
        )
    } else {
        None
    };

    if args.stdio {
        run_stdio(local_runtime, args.searchable, search_embedder).await?;
        return Ok(());
    }

    println!("{} Starting MCP server...", style("→").cyan());
    println!(
        "{} Loaded {} tool(s)",
        style("✓").green().bold(),
        local_runtime.registry().len()
    );

    let addr: SocketAddr = args
        .addr
        .parse()
        .with_context(|| format!("invalid --addr value: {}", args.addr))?;
    let path = normalize_path(&args.path);

    let mut service = McpService::from_runtime(local_runtime.clone()).searchable(args.searchable);
    if let Some(embedder) = search_embedder {
        service = service.with_search_embedder(embedder);
    }
    let service = service.streamable_http_service_with_config(StreamableHttpServerConfig {
        // Stateless mode keeps compatibility with MCP clients that don't send
        // the initialized notification after initialize.
        stateful_mode: false,
        ..Default::default()
    });
    let router = axum::Router::new().nest_service(path.as_str(), service);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("failed to bind MCP server on {addr}"))?;

    info!(address = %addr, path = %path, "Starting MCP server");

    println!(
        "{} MCP server running on http://{}{}",
        style("✓").green().bold(),
        addr,
        path
    );
    println!("Press Ctrl+C to stop\n");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown)
        .await
        .context("mcp server error")?;

    info!("Draining inflight requests");
    local_runtime.drain().await;

    info!("Operai MCP server stopped");
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
    fn embed_query(&self, query: &str) -> SearchEmbedFuture<'_> {
        let query = query.to_string();
        let generator = &self.generator;
        Box::pin(async move { generator.embed(&query).await.map_err(|err| err.to_string()) })
    }
}

/// Runs the MCP server in stdio mode.
///
/// In stdio mode, the server communicates via standard input/output using the
/// MCP stdio transport protocol. This is the standard mode for desktop-based
/// MCP clients (like Claude Desktop).
///
/// # Arguments
///
/// * `runtime` - The operai runtime containing registered tools
/// * `searchable` - Whether to enable search mode (exposes search tools instead
///   of all tools)
/// * `search_embedder` - Optional embedder for semantic search (required if
///   `searchable` is true)
///
/// # Behavior
///
/// The server will:
/// 1. Print startup messages to stderr
/// 2. Serve MCP requests via stdin/stdout
/// 3. Listen for Ctrl+C to initiate graceful shutdown
/// 4. Drain in-flight requests before exiting
///
/// # Errors
///
/// Returns an error if the stdio server fails to start or encounters a fatal
/// error.
async fn run_stdio(
    runtime: operai_runtime::LocalRuntime,
    searchable: bool,
    search_embedder: Option<std::sync::Arc<dyn SearchEmbedder>>,
) -> Result<()> {
    eprintln!("{} Starting MCP stdio server...", style("→").cyan());
    eprintln!(
        "{} Loaded {} tool(s)",
        style("✓").green().bold(),
        runtime.registry().len()
    );

    let mut service = McpService::from_runtime(runtime.clone()).searchable(searchable);
    if let Some(embedder) = search_embedder {
        service = service.with_search_embedder(embedder);
    }
    let (stdin, stdout) = stdio();

    let running = service
        .serve((stdin, stdout))
        .await
        .context("failed to start MCP stdio server")?;

    eprintln!("{} MCP stdio server running", style("✓").green().bold());
    eprintln!("Press Ctrl+C to stop\n");

    let cancel = running.cancellation_token();
    let mut waiting = Box::pin(running.waiting());

    tokio::select! {
        result = &mut waiting => {
            result.context("mcp stdio server exited")?;
        }
        _ = signal::ctrl_c() => {
            cancel.cancel();
            let _ = waiting.await;
        }
    }

    info!("Draining inflight requests");
    runtime.drain().await;
    info!("Operai MCP stdio server stopped");
    Ok(())
}

/// Normalizes an HTTP path to ensure it starts with `/`.
///
/// # Arguments
///
/// * `path` - The path string to normalize
///
/// # Returns
///
/// - If `path` is empty, returns `"/mcp"` (the default)
/// - If `path` already starts with `/`, returns it unchanged
/// - Otherwise, prepends `/` to the path
///
/// # Examples
///
/// ```
/// assert_eq!(normalize_path(""), "/mcp");
/// assert_eq!(normalize_path("/custom"), "/custom");
/// assert_eq!(normalize_path("custom"), "/custom");
/// ```
fn normalize_path(path: &str) -> String {
    if path.is_empty() {
        "/mcp".to_string()
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    /// Helper wrapper type for parsing [`McpArgs`] with clap.
    #[derive(Parser)]
    struct McpArgsCli {
        #[command(flatten)]
        mcp: McpArgs,
    }

    /// Verifies that [`McpArgs`] uses expected default values when no flags are
    /// provided.
    #[test]
    fn test_mcp_args_defaults() {
        let cli = McpArgsCli::try_parse_from(["test"]).expect("args should parse");
        assert_eq!(cli.mcp.addr, "127.0.0.1:3333");
        assert_eq!(cli.mcp.path, "/mcp");
        assert!(!cli.mcp.searchable);
        assert!(!cli.mcp.stdio);
        assert_eq!(cli.mcp.config, None);
    }

    /// Verifies that [`McpArgs`] correctly parses all supported flags.
    #[test]
    fn test_mcp_args_parse_flags() {
        let cli = McpArgsCli::try_parse_from([
            "test",
            "--config",
            "custom.toml",
            "--addr",
            "0.0.0.0:9000",
            "--path",
            "/custom",
            "--searchable",
            "--stdio",
        ])
        .expect("args should parse");
        assert_eq!(cli.mcp.config, Some(PathBuf::from("custom.toml")));
        assert_eq!(cli.mcp.addr, "0.0.0.0:9000");
        assert_eq!(cli.mcp.path, "/custom");
        assert!(cli.mcp.searchable);
        assert!(cli.mcp.stdio);
    }
}
