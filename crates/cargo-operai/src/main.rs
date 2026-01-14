//! Cargo subcommand for Operai Tool SDK development.
//!
//! Usage:
//! ```bash
//! cargo operai new my-tool        # Create new tool project
//! cargo operai embed              # Generate embeddings
//! cargo operai build              # Build with embeddings
//! cargo operai serve              # Run local dev server
//! cargo operai mcp                # Run MCP server
//! cargo operai call <tool> <json> # Test a tool
//! cargo operai list               # List available tools
//! cargo operai describe <tool>    # Show tool details
//! ```

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod commands;
mod config;
mod embedding;

#[cfg(test)]
pub(crate) mod testing {
    use std::sync::OnceLock;

    use tokio::sync::Mutex;

    pub(crate) fn test_lock() -> tokio::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).blocking_lock()
    }

    pub(crate) async fn test_lock_async() -> tokio::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().await
    }
}

#[derive(Debug, Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
enum Cargo {
    /// Operai Tool SDK commands
    Operai(Operai),
}

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Operai {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new tool project
    New(commands::new::NewArgs),

    /// Generate embeddings for the current crate
    Embed(commands::embed::EmbedArgs),

    /// Build the tool (runs embed + cargo build --release)
    Build(commands::build::BuildArgs),

    /// Serve tools locally for development
    Serve(commands::serve::ServeArgs),

    /// Serve tools over MCP
    Mcp(commands::mcp::McpArgs),

    /// Call a tool for testing
    Call(commands::call::CallArgs),

    /// List available tools
    List(commands::list::ListArgs),

    /// Describe a specific tool
    Describe(commands::describe::DescribeArgs),
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::New(_) => f.debug_tuple("New").finish(),
            Self::Embed(_) => f.debug_tuple("Embed").finish(),
            Self::Build(_) => f.debug_tuple("Build").finish(),
            Self::Serve(_) => f.debug_tuple("Serve").finish(),
            Self::Mcp(_) => f.debug_tuple("Mcp").finish(),
            Self::Call(_) => f.debug_tuple("Call").finish(),
            Self::List(_) => f.debug_tuple("List").finish(),
            Self::Describe(_) => f.debug_tuple("Describe").finish(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("info".parse().context("failed to parse log directive")?),
        )
        .init();

    let Cargo::Operai(args) = Cargo::parse();

    match &args.command {
        Command::New(args) => commands::new::run(args),
        Command::Embed(args) => commands::embed::run(args).await,
        Command::Build(args) => commands::build::run(args).await,
        Command::Serve(args) => commands::serve::run(args).await,
        Command::Mcp(args) => commands::mcp::run(args).await,
        Command::Call(args) => commands::call::run(args).await,
        Command::List(args) => commands::list::run(args).await,
        Command::Describe(args) => commands::describe::run(args).await,
    }
}

#[cfg(test)]
mod tests {
    use clap::error::ErrorKind;

    use super::*;

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

        assert!(args.manifest.is_none());
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
    fn test_cli_embed_parses_optional_flags() -> Result<(), clap::Error> {
        let command = parse_command(&[
            "cargo",
            "operai",
            "embed",
            "--path",
            "my-crate",
            "-P",
            "openai",
            "--model",
            "text-embedding-3-small",
            "--output",
            "embedding.bin",
        ])?;

        let Command::Embed(args) = command else {
            panic!("expected Command::Embed");
        };

        assert_eq!(args.path, Some(std::path::PathBuf::from("my-crate")));
        assert_eq!(args.provider.as_deref(), Some("openai"));
        assert_eq!(args.model.as_deref(), Some("text-embedding-3-small"));
        assert_eq!(args.output, Some(std::path::PathBuf::from("embedding.bin")));
        Ok(())
    }

    #[test]
    fn test_cli_build_parses_path_skip_embed_provider_and_model() -> Result<(), clap::Error> {
        let command = parse_command(&[
            "cargo",
            "operai",
            "build",
            "--path",
            "my-crate",
            "--skip-embed",
            "-P",
            "fastembed",
            "--model",
            "nomic-embed-text-v1.5",
        ])?;

        let Command::Build(args) = command else {
            panic!("expected Command::Build");
        };

        assert_eq!(args.path, Some(std::path::PathBuf::from("my-crate")));
        assert!(args.skip_embed);
        assert_eq!(args.provider.as_deref(), Some("fastembed"));
        assert_eq!(args.model.as_deref(), Some("nomic-embed-text-v1.5"));
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
            ("cargo operai embed", "Embed"),
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
