//! Core library for Operai Toolbox runtime.
//!
//! This crate provides:
//! - Dynamic loading of tool libraries (cdylib)
//! - Tool registry with lookup by qualified name
//! - Manifest parsing for tool configuration
//! - Semantic search via embeddings

mod loader;
mod manifest;
mod registry;

pub use loader::{LoadError, LoadedLibrary};
pub use manifest::{Manifest, ManifestError, ToolConfig};
pub use registry::{InflightRequestGuard, Registry, RegistryError, ToolHandle, ToolInfo};

pub mod policy;
// Re-export specific items for convenience, but the module itself is also
// public.
// Re-export session for top-level access if desired, but
// operai_core::policy::session is now valid.
pub use policy::{Effect, Policy, PolicyError, session};

// All tests are in their respective submodules:
// - loader::tests
// - manifest::tests
// - registry::tests
//
// This lib.rs only re-exports public types, so no additional tests needed here.
// See TESTING.md: "Don't test framework/library code" and avoid redundant
// coverage.
