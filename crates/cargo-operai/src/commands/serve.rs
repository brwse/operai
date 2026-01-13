//! `cargo operai serve` command implementation.

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use clap::Args;
use console::style;
use operai_abi::RuntimeContext;
use operai_core::{Manifest, Registry};
use operai_runtime::{proto, service};
use tokio::signal;
use tonic::transport::Server;
use tonic_health::ServingStatus;
use tracing::{error, info, warn};

/// Arguments for the `serve` command.
#[derive(Args)]
pub struct ServeArgs {
    /// Path to tools.toml manifest file.
    #[arg(short, long)]
    pub manifest: Option<PathBuf>,

    /// Port to serve on.
    #[arg(short, long, default_value = "50051")]
    pub port: u16,
}

pub async fn run(args: &ServeArgs) -> Result<()> {
    println!("{} Starting local toolbox server...", style("→").cyan());

    let manifest_path = args
        .manifest
        .clone()
        .unwrap_or_else(|| PathBuf::from("operai.toml"));
    let manifest = load_manifest_or_empty(&manifest_path)?;

    let mut registry = Registry::new();
    let runtime_ctx = RuntimeContext::new();

    for tool_config in manifest.enabled_tools() {
        let path_buf;
        let path = if let Some(p) = &tool_config.path {
            p
        } else if let Some(pkg) = &tool_config.package {
            // Simple resolution: assume target/release/lib{package}.dylib
            // In a real implementation, we'd check target/debug if debug profile, and
            // handle OS extensions.
            let lib_name = format!("lib{}.dylib", pkg.replace('-', "_"));
            path_buf = PathBuf::from("target/release").join(lib_name);
            path_buf.to_str().unwrap()
        } else {
            warn!("Tool config missing path and package, skipping.");
            continue;
        };

        info!(path = %path, "Loading tool library");

        if let Err(e) = registry
            .load_library(
                path,
                tool_config.checksum.as_deref(),
                Some(&tool_config.credentials),
                &runtime_ctx,
            )
            .await
        {
            error!(
                path = %path,
                error = %e,
                "Failed to load tool library"
            );
            println!("{} Failed to load tool: {}", style("x").red().bold(), path);
        } else {
            println!(
                "{} Loaded tool library: {}",
                style("✓").green().bold(),
                path
            );
        }
    }

    info!(tool_count = registry.len(), "Tool registry initialized");

    let registry = Arc::new(registry);

    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_service_status("", ServingStatus::Serving)
        .await;
    health_reporter
        .set_service_status("brwse.toolbox.v1alpha1.Toolbox", ServingStatus::Serving)
        .await;

    let session_store = Arc::new(operai_core::policy::session::InMemoryPolicySessionStore::new());
    let policy_store = Arc::new(operai_core::policy::session::PolicyStore::new(
        session_store,
    ));

    // Load policies from manifest
    match manifest.resolve_policies(&manifest_path) {
        Ok(policies) => {
            for policy in policies {
                info!(
                    name = ?policy.name,
                    version = ?policy.version,
                    "Registered policy"
                );
                println!(
                    "{} Registered policy: {} ({})",
                    style("✓").green().bold(),
                    policy.name,
                    policy.version
                );
                let _ = policy_store.register(policy);
            }
        }
        Err(e) => {
            warn!(error = %e, "Failed to resolve policies from manifest");
            println!(
                "{} Failed to load policies: {}",
                style("!").yellow().bold(),
                e
            );
        }
    }

    let toolbox_service = service::ToolboxService::new(Arc::clone(&registry), policy_store);

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
        .serve_with_shutdown(addr, async {
            let _ = signal::ctrl_c().await;
            info!("Received shutdown signal");
        })
        .await
        .context("server error")?;

    // Wait for in-flight tool invocations to complete before exiting.
    info!("Draining inflight requests");
    registry.drain().await;

    info!("Operai Toolbox stopped");
    Ok(())
}

fn load_manifest_or_empty(manifest_path: &Path) -> Result<Manifest> {
    if manifest_path.exists() {
        Manifest::load(manifest_path).context("failed to load manifest")
    } else {
        // If the user explicitly provided a manifest path that doesn't exist, that's an
        // error. But the previous behavior (implied by existing code) was
        // slightly more lenient or different. However, here if we use default
        // "tools.toml" and it's missing, we warn. If user supplied --manifest
        // override, we should probably error if missing? For consistency with
        // previous binary:
        warn!(
            path = %manifest_path.display(),
            "Manifest file not found, starting with empty registry"
        );
        Ok(Manifest::empty())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct ServeArgsCli {
        #[command(flatten)]
        serve: ServeArgs,
    }

    #[test]
    fn test_serve_args_defaults_port_to_50051() {
        let cli = ServeArgsCli::try_parse_from(["test"]).expect("args should parse");
        assert_eq!(cli.serve.port, 50051);
        assert_eq!(cli.serve.manifest, None);
    }

    #[test]
    fn test_serve_args_parses_manifest_and_port_flags() {
        let cli =
            ServeArgsCli::try_parse_from(["test", "--manifest", "custom.toml", "--port", "123"])
                .expect("args should parse");
        assert_eq!(cli.serve.port, 123);
        assert_eq!(cli.serve.manifest, Some(PathBuf::from("custom.toml")));
    }

    #[test]
    fn test_serve_args_parses_short_flags() {
        let cli = ServeArgsCli::try_parse_from(["test", "-m", "short.toml", "-p", "8080"])
            .expect("short flags should parse");
        assert_eq!(cli.serve.port, 8080);
        assert_eq!(cli.serve.manifest, Some(PathBuf::from("short.toml")));
    }
}
