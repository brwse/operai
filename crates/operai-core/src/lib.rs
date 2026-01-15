//! Core types and functionality for the Operai toolkit runtime.
//!
//! This crate provides the foundational infrastructure for dynamically loading,
//! managing, and executing tools through a plugin-based architecture. It handles
//! tool lifecycles, policy enforcement, and manifest-based configuration.
//!
//! # Key Components
//!
//! - **Tool Loading**: Dynamic library loading via [`ToolLibrary`] with ABI version checking
//! - **Tool Registry**: Centralized tool management through [`ToolRegistry`]
//! - **Policy System**: CEL-based policy evaluation for controlling tool execution
//! - **Manifests**: TOML-based configuration for tools and policies
//!
//! # Example
//!
//! ```ignore
//! use operai_core::{ToolRegistry, RuntimeContext};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let mut registry = ToolRegistry::new();
//! let runtime_ctx = RuntimeContext::new();
//!
//! // Load a tool library from a dynamic library
//! registry.load_library("path/to/tool.so", None, None, &runtime_ctx).await?;
//!
//! // List available tools
//! for tool in registry.list() {
//!     println!("Loaded: {}", tool.qualified_id);
//! }
//!
//! // Get a tool handle and invoke it
//! if let Some(handle) = registry.get("my_crate.my_tool") {
//!     // Use handle to call the tool
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Thread Safety
//!
//! The tool registry has two phases of operation:
//!
//! **Loading phase**: [`ToolRegistry::load_library`] requires `&mut self` and is not
//! thread-safe. Load all tools before wrapping the registry in `Arc`.
//!
//! **Execution phase**: Once wrapped in `std::sync::Arc`, the registry supports
//! concurrent queries ([`get`], [`list`], [`search`]). Tool handles use interior
//! `Arc` wrapping for safe concurrent invocation.

mod loader;
mod manifest;
mod tool;

/// Tool loading and lifecycle management.
///
/// Provides [`ToolLibrary`] for loading tools from dynamic libraries with
/// ABI validation and optional checksum verification.
pub use loader::{LoadError, ToolLibrary};

/// Manifest parsing and configuration.
///
/// Types for loading and validating TOML manifests that define tools,
/// policies, and configuration data.
pub use manifest::{Manifest, ManifestError, ToolConfig};

/// Tool registry and invocation.
///
/// Core runtime infrastructure including [`ToolRegistry`] for managing tools,
/// [`ToolHandle`] for invocation, and [`ToolInfo`] for metadata.
pub use tool::{InflightRequestGuard, RegistryError, ToolHandle, ToolInfo, ToolRegistry};

/// Policy evaluation and enforcement.
///
/// CEL-based policy system for controlling tool execution with conditional
/// effects and context management.
pub mod policy;

/// Policy-related types re-exported for convenience.
///
/// These are also available via the [`policy`] module, but are re-exported
/// at the crate root for easier access.
pub use policy::{Effect, Policy, PolicyError, session};

// All tests are in their respective submodules:
// - loader::tests
// - manifest::tests
// - registry::tests
//
// This lib.rs only re-exports public types, so no additional tests needed here.
// See TESTING.md: "Don't test framework/library code" and avoid redundant
// coverage.
