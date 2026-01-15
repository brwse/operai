//! Cargo custom subcommand for the Operai tool framework.
//!
//! This binary implements `cargo operai`, providing a CLI for:
//! - Creating new Operai tool projects (`new`)
//! - Building Operai tools (`build`)
//! - Running tool servers (`serve`)
//! - Running MCP servers (`mcp`)
//! - Calling tools remotely (`call`)
//! - Listing available tools (`list`)
//! - Describing tools (`describe`)
//!
//! # Command Structure
//!
//! The CLI follows Cargo's convention for custom subcommands, using a two-level
//! parsing structure:
//!
//! - `cargo operai <command>` - Top-level invocation
//! - Subcommands: `new`, `build`, `serve`, `mcp`, `call`, `list`, `describe`
//!
//! # Logging
//!
//! All commands use structured logging via `tracing` with output to stderr.
//! The default log level is `info`, configurable via the `RUST_LOG` environment
//! variable.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod commands;
mod embedding;

#[cfg(test)]
pub(crate) mod testing {
    use std::sync::OnceLock;

    use tokio::sync::Mutex;

    /// Acquires a synchronous test lock.
    ///
    /// This provides a static mutex for synchronizing tests that need to
    /// prevent concurrent execution (e.g., tests that modify shared state
    /// or use exclusive resources).
    ///
    /// # Returns
    ///
    /// A mutex guard that will release the lock when dropped.
    pub(crate) fn test_lock() -> tokio::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).blocking_lock()
    }

    /// Acquires an asynchronous test lock.
    ///
    /// This provides a static mutex for synchronizing async tests that need to
    /// prevent concurrent execution. Unlike [`test_lock`], this uses async locking.
    ///
    /// # Returns
    ///
    /// A mutex guard that will release the lock when dropped.
    pub(crate) async fn test_lock_async() -> tokio::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().await
    }
}

/// Top-level Cargo command parser.
///
/// This enum implements Cargo's convention for custom subcommands by matching
/// on the `cargo` binary name and extracting the `operai` subcommand.
#[derive(Debug, Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
enum Cargo {
    Operai(Operai),
}

/// Operai command-line interface.
///
/// This struct represents the `cargo operai` invocation and dispatches to
/// the appropriate subcommand handler.
#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Operai {
    /// The subcommand to execute.
    #[command(subcommand)]
    command: Command,
}

/// Available Operai subcommands.
///
/// Each variant corresponds to a distinct operation and contains the
/// arguments specific to that command.
#[derive(Subcommand)]
enum Command {
    /// Create a new Operai tool project.
    New(commands::new::NewArgs),

    /// Build an Operai tool.
    Build(commands::build::BuildArgs),

    /// Start a tool server.
    Serve(commands::serve::ServeArgs),

    /// Start an MCP server.
    Mcp(commands::mcp::McpArgs),

    /// Call a tool remotely.
    Call(commands::call::CallArgs),

    /// List available tools.
    List(commands::list::ListArgs),

    /// Describe a tool's schema.
    Describe(commands::describe::DescribeArgs),
}

/// Custom Debug implementation for Command.
///
/// Intentionally omits the inner arguments to provide cleaner logging output.
/// Only the variant name is displayed, not the full command arguments.
impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::New(_) => f.debug_tuple("New").finish(),
            Self::Build(_) => f.debug_tuple("Build").finish(),
            Self::Serve(_) => f.debug_tuple("Serve").finish(),
            Self::Mcp(_) => f.debug_tuple("Mcp").finish(),
            Self::Call(_) => f.debug_tuple("Call").finish(),
            Self::List(_) => f.debug_tuple("List").finish(),
            Self::Describe(_) => f.debug_tuple("Describe").finish(),
        }
    }
}

/// Entry point for the `cargo operai` CLI.
///
/// Initializes structured logging, loads the project config, and dispatches to
/// the appropriate command handler based on the parsed CLI arguments.
///
/// # Errors
///
/// Returns an error if:
/// - Logging initialization fails
/// - Config loading fails
/// - Command execution fails
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("info".parse().context("failed to parse log directive")?),
        )
        .with_writer(std::io::stderr)
        .init();

    let Cargo::Operai(args) = Cargo::parse();

    // Load project config once to share across subcommands
    let config = operai_core::Config::load("operai.toml")
        .unwrap_or_else(|_| operai_core::Config::empty());

    match &args.command {
        Command::New(args) => commands::new::run(args),
        Command::Build(args) => commands::build::run(args, &config).await,
        Command::Serve(args) => commands::serve::run(args).await,
        Command::Mcp(args) => commands::mcp::run(args, &config).await,
        Command::Call(args) => commands::call::run(args).await,
        Command::List(args) => commands::list::run(args).await,
        Command::Describe(args) => commands::describe::run(args).await,
    }
}

#[cfg(test)]
mod tests {
    use clap::error::ErrorKind;

    use super::*;

    /// Helper function to parse command-line arguments and extract the Command.
    ///
    /// This utility is used throughout the test suite to verify that CLI arguments
    /// are parsed correctly.
    ///
    /// # Arguments
    ///
    /// * `argv` - Command-line arguments (e.g., `&["cargo", "operai", "new", "my-tool"]`)
    ///
    /// # Returns
    ///
    /// The parsed [`Command`] enum variant.
    ///
    /// # Errors
    ///
    /// Returns [`clap::Error`] if the arguments fail to parse.
    fn parse_command(argv: &[&str]) -> Result<Command, clap::Error> {
        let Cargo::Operai(parsed) = Cargo::try_parse_from(argv.iter().copied())?;
        Ok(parsed.command)
    }

    #[test]
    fn test_cli_new_requires_name_argument() {
        let err = Cargo::try_parse_from(["cargo", "operai", "new"])
            .expect_err("expected clap parse error");
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn test_cli_parses_new_with_required_name() -> Result<(), clap::Error> {
        let command = parse_command(&["cargo", "operai", "new", "my-tool"])?;

        let Command::New(args) = command else {
            panic!("expected Command::New");
        };

        assert_eq!(args.name, "my-tool");
        assert!(!args.multi);
        assert!(args.output.is_none());
        Ok(())
    }

    #[test]
    fn test_cli_new_multi_flag_enables_multi_template() -> Result<(), clap::Error> {
        let command = parse_command(&["cargo", "operai", "new", "my-tool", "--multi"])?;

        let Command::New(args) = command else {
            panic!("expected Command::New");
        };

        assert!(args.multi);
        Ok(())
    }

    #[test]
    fn test_cli_requires_subcommand_after_operai() {
        let err =
            Cargo::try_parse_from(["cargo", "operai"]).expect_err("expected clap parse error");
        assert!(
            matches!(
                err.kind(),
                ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand | ErrorKind::MissingSubcommand
            ),
            "unexpected error kind: {:?}",
            err.kind()
        );
    }

    #[test]
    fn test_cli_rejects_unknown_subcommand() {
        let err = Cargo::try_parse_from(["cargo", "operai", "not-a-command"])
            .expect_err("expected clap parse error");
        assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
    }

    #[test]
    fn test_cli_call_requires_input_json_argument() {
        let err = Cargo::try_parse_from(["cargo", "operai", "call", "tool.id"])
            .expect_err("expected clap parse error");
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn test_cli_mcp_defaults() -> Result<(), clap::Error> {
        let command = parse_command(&["cargo", "operai", "mcp"])?;

        let Command::Mcp(args) = command else {
            panic!("expected Command::Mcp");
        };

        assert_eq!(args.addr, "127.0.0.1:3333");
        assert_eq!(args.path, "/mcp");
        Ok(())
    }

    #[test]
    fn test_cli_call_requires_tool_id_argument() {
        let err = Cargo::try_parse_from(["cargo", "operai", "call"])
            .expect_err("expected clap parse error");
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn test_cli_call_uses_default_server_when_not_provided() -> Result<(), clap::Error> {
        let command = parse_command(&["cargo", "operai", "call", "tool.id", "{}"])?;

        let Command::Call(args) = command else {
            panic!("expected Command::Call");
        };

        assert_eq!(args.server, "localhost:50051");
        Ok(())
    }

    #[test]
    fn test_cli_list_defaults_to_table_format_and_localhost_server() -> Result<(), clap::Error> {
        let command = parse_command(&["cargo", "operai", "list"])?;

        let Command::List(args) = command else {
            panic!("expected Command::List");
        };

        assert_eq!(args.server, "http://127.0.0.1:50051");
        assert_eq!(args.format, "table");
        Ok(())
    }

    #[test]
    fn test_cli_serve_defaults_to_port_50051() -> Result<(), clap::Error> {
        let command = parse_command(&["cargo", "operai", "serve"])?;

        let Command::Serve(args) = command else {
            panic!("expected Command::Serve");
        };

        assert!(args.config.is_none());
        assert_eq!(args.port, 50051);
        Ok(())
    }

    #[test]
    fn test_cli_describe_requires_tool_id_argument() {
        let err = Cargo::try_parse_from(["cargo", "operai", "describe"])
            .expect_err("expected clap parse error");
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn test_cli_describe_uses_default_server_when_not_provided() -> Result<(), clap::Error> {
        let command = parse_command(&["cargo", "operai", "describe", "tool.id"])?;

        let Command::Describe(args) = command else {
            panic!("expected Command::Describe");
        };

        assert_eq!(args.tool_id, "tool.id");
        assert_eq!(args.server, "http://localhost:50051");
        Ok(())
    }

    #[test]
    fn test_cli_build_parses_path_and_skip_embed() -> Result<(), clap::Error> {
        let command = parse_command(&[
            "cargo",
            "operai",
            "build",
            "--path",
            "my-crate",
            "--skip-embed",
        ])?;

        let Command::Build(args) = command else {
            panic!("expected Command::Build");
        };

        assert_eq!(args.path, Some(std::path::PathBuf::from("my-crate")));
        assert!(args.skip_embed);
        assert!(args.cargo_args.is_empty());
        Ok(())
    }

    #[test]
    fn test_cli_build_collects_trailing_cargo_args_after_double_dash() -> Result<(), clap::Error> {
        let command = parse_command(&[
            "cargo",
            "operai",
            "build",
            "--",
            "--features",
            "foo",
            "--locked",
        ])?;

        let Command::Build(args) = command else {
            panic!("expected Command::Build");
        };

        assert_eq!(args.cargo_args, ["--features", "foo", "--locked"]);
        Ok(())
    }

    #[test]
    fn test_command_debug_shows_variant_name_without_inner_args() -> Result<(), clap::Error> {
        // The Debug impl intentionally hides inner args for cleaner logging
        let test_cases = [
            ("cargo operai new my-tool", "New"),
            ("cargo operai build", "Build"),
            ("cargo operai serve", "Serve"),
            ("cargo operai call tool.id {}", "Call"),
            ("cargo operai list", "List"),
            ("cargo operai describe tool.id", "Describe"),
        ];

        for (argv, expected_variant) in test_cases {
            let command = parse_command(&argv.split_whitespace().collect::<Vec<_>>())?;
            let debug_output = format!("{command:?}");
            assert_eq!(debug_output, expected_variant, "for argv: {argv}");
        }

        Ok(())
    }
}
