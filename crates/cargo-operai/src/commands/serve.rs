//! `cargo operai serve` command implementation.

use std::{net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use console::style;
use operai_runtime::{RuntimeBuilder, proto, transports};
use tokio::signal;
use tonic::transport::Server;
use tonic_health::ServingStatus;
use tracing::info;

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
    let shutdown = async {
        let _ = signal::ctrl_c().await;
        info!("Received shutdown signal");
    };
    run_with_shutdown(args, shutdown).await
}

async fn run_with_shutdown<F>(args: &ServeArgs, shutdown: F) -> Result<()>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    println!("{} Starting local toolbox server...", style("→").cyan());

    let manifest_path = args
        .manifest
        .clone()
        .unwrap_or_else(|| PathBuf::from("operai.toml"));

    let local_runtime = RuntimeBuilder::new()
        .with_manifest_path(manifest_path)
        .build_local()
        .await
        .context("failed to initialize runtime")?;

    println!(
        "{} Loaded {} tool(s)",
        style("✓").green().bold(),
        local_runtime.registry().len()
    );

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
    static TEMP_MANIFEST_COUNTER: AtomicU64 = AtomicU64::new(0);

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

    fn temp_manifest_path() -> PathBuf {
        let counter = TEMP_MANIFEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "cargo-operai-serve-manifest-{}-{counter}.toml",
            std::process::id()
        ))
    }

    fn write_manifest_for_library(path: &Path) -> PathBuf {
        let manifest_path = temp_manifest_path();
        let mut path_str = path.display().to_string();
        if std::path::MAIN_SEPARATOR == '\\' {
            path_str = path_str.replace('\\', "\\\\");
        }
        let contents = format!("[[tools]]\npath = \"{path_str}\"\n");
        std::fs::write(&manifest_path, contents).expect("write manifest");
        manifest_path
    }

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

    #[tokio::test]
    async fn test_serve_runs_and_accepts_calls() -> Result<()> {
        let _lock = crate::testing::test_lock_async().await;

        let lib_path = hello_world_cdylib_path();
        let manifest_path = write_manifest_for_library(&lib_path);

        let port = TcpListener::bind("127.0.0.1:0")?.local_addr()?.port();
        let args = ServeArgs {
            manifest: Some(manifest_path),
            port,
        };

        let (tx, rx) = oneshot::channel::<()>();
        let server_handle = tokio::spawn(async move {
            run_with_shutdown(&args, async {
                let _ = rx.await;
            })
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
