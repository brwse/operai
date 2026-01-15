//! CLI command implementations for `cargo-operai`.
//!
//! This module organizes all top-level commands available in the `cargo operai` CLI.
//! Each subcommand is implemented in its own module and provides a distinct piece
//! of functionality:
//!
//! - **`new`**: Scaffold a new Operai tool or workspace project from templates
//! - **`build`**: Compile an Operai project and generate embeddings for tool discovery
//! - **`serve`**: Start a gRPC server hosting Operai tools
//! - **`mcp`**: Run a Model Context Protocol (MCP) server for AI assistant integration
//! - **`call`**: Invoke a specific tool on a running server with JSON input
//! - **`list`**: Display all available tools on a running server
//! - **`describe`**: Show detailed information about a specific tool
//!
//! # Command Structure
//!
//! Each command module exports:
//! - An `*Args` struct implementing `clap::Args` for command-line argument parsing
//! - A `run` function (typically `async`) with signature `fn run(&Args) -> Result<()>>`
//!
//! The main CLI dispatches to these `run` functions based on user input.

pub mod build;
pub mod call;
pub mod describe;
pub mod list;
pub mod mcp;
pub mod new;
pub mod serve;
