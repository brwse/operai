//! `cargo operai mcp` command implementation.

use std::{future::Future, net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use console::style;
use operai_runtime::{McpService, RuntimeBuilder};
use rmcp::{service::ServiceExt, transport::stdio};
use tokio::signal;
use tracing::info;

/// Arguments for the `mcp` command.
#[derive(Args)]
pub struct McpArgs {
    /// Path to tools.toml manifest file.
    #[arg(short, long)]
    pub manifest: Option<PathBuf>,

    /// Address to bind the MCP server to.
    #[arg(short = 'a', long, default_value = "127.0.0.1:3333")]
    pub addr: String,

    /// HTTP path for MCP streamable transport.
    #[arg(long, default_value = "/mcp")]
    pub path: String,

    /// Enable searchable mode (exposes list/find/call tools only).
    #[arg(long, default_value_t = false)]
    pub searchable: bool,

    /// Serve MCP over stdio instead of HTTP.
    #[arg(long, default_value_t = false)]
    pub stdio: bool,
}

pub async fn run(args: &McpArgs) -> Result<()> {
    let shutdown = async {
        let _ = signal::ctrl_c().await;
        info!("Received shutdown signal");
    };
    run_with_shutdown(args, shutdown).await
}

async fn run_with_shutdown<F>(args: &McpArgs, shutdown: F) -> Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let manifest_path = args
        .manifest
        .clone()
        .unwrap_or_else(|| PathBuf::from("operai.toml"));

    let local_runtime = RuntimeBuilder::new()
        .with_manifest_path(manifest_path)
        .build_local()
        .await
        .context("failed to initialize runtime")?;

    if args.stdio {
        run_stdio(local_runtime, args.searchable).await?;
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

    let service = McpService::from_runtime(local_runtime.clone())
        .searchable(args.searchable)
        .streamable_http_service();
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

async fn run_stdio(runtime: operai_runtime::LocalRuntime, searchable: bool) -> Result<()> {
    eprintln!("{} Starting MCP stdio server...", style("→").cyan());
    eprintln!(
        "{} Loaded {} tool(s)",
        style("✓").green().bold(),
        runtime.registry().len()
    );

    let service = McpService::from_runtime(runtime.clone()).searchable(searchable);
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

    #[derive(Parser)]
    struct McpArgsCli {
        #[command(flatten)]
        mcp: McpArgs,
    }

    #[test]
    fn test_mcp_args_defaults() {
        let cli = McpArgsCli::try_parse_from(["test"]).expect("args should parse");
        assert_eq!(cli.mcp.addr, "127.0.0.1:3333");
        assert_eq!(cli.mcp.path, "/mcp");
        assert!(!cli.mcp.searchable);
        assert!(!cli.mcp.stdio);
        assert_eq!(cli.mcp.manifest, None);
    }

    #[test]
    fn test_mcp_args_parse_flags() {
        let cli = McpArgsCli::try_parse_from([
            "test",
            "--manifest",
            "custom.toml",
            "--addr",
            "0.0.0.0:9000",
            "--path",
            "/custom",
            "--searchable",
            "--stdio",
        ])
        .expect("args should parse");
        assert_eq!(cli.mcp.manifest, Some(PathBuf::from("custom.toml")));
        assert_eq!(cli.mcp.addr, "0.0.0.0:9000");
        assert_eq!(cli.mcp.path, "/custom");
        assert!(cli.mcp.searchable);
        assert!(cli.mcp.stdio);
    }
}
