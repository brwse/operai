//! Transport layer implementations for serving tools over different protocols.
//!
//! This module provides transport-layer implementations that expose the tool
//! runtime through various protocols. Each transport adapts the core
//! [`LocalRuntime`](crate::runtime::LocalRuntime) to a specific wire protocol,
//! handling serialization, authentication, and protocol-specific conventions.
//!
//! # Available Transports
//!
//! - `grpc` - gRPC/HTTP2 transport using Tonic, providing high-performance RPC
//!   access
//! - `mcp` - Model Context Protocol (MCP) transport for HTTP-based tool serving
//!   (feature-gated, requires `mcp` feature)
//!
//! # Architecture
//!
//! All transports follow a similar pattern:
//! 1. Wrap a [`LocalRuntime`] instance
//! 2. Extract authentication/authorization metadata from protocol-specific
//!    headers
//! 3. Convert protocol requests to internal runtime requests
//! 4. Return protocol-formatted responses
//!
//! # Example (gRPC)
//!
//! ```no_run
//! use operai_runtime::transports::grpc::ToolboxService;
//! # use operai_core::{ToolRegistry, policy::session::PolicyStore};
//! # use std::sync::Arc;
//!
//! # let registry = Arc::new(ToolRegistry::new());
//! # let policy_store = Arc::new(PolicyStore::new(
//! #     Arc::new(operai_core::policy::session::InMemoryPolicySessionStore::new())
//! # ));
//! let service = ToolboxService::new(registry, policy_store);
//! // Use service with tonic server...
//! ```

/// gRPC transport implementation using Tonic.
///
/// Provides a [`ToolboxService`](grpc::ToolboxService) that implements the gRPC
/// `Toolbox` service protocol, exposing tools via gRPC/HTTP2.
pub mod grpc;

/// Model Context Protocol (MCP) transport implementation.
///
/// Provides `McpService` that implements the MCP protocol
/// for HTTP-based tool serving with streaming support. Requires the `mcp`
/// feature.
#[cfg(feature = "mcp")]
pub mod mcp;
