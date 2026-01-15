//! Plugin entrypoint registration and discovery.
//!
//! This module provides the registry system for Opera plugins using the [`inventory`] crate.
//! Plugins register their tools, init handlers, and shutdown handlers through static entries
//! that are collected into global inventories at compile time.
//!
//! # Entry Types
//!
//! - [`ToolEntry`] - Registers a tool with its metadata, schema functions, and async handler
//! - [`InitEntry`] - Registers an initialization handler that runs when the plugin loads
//! - [`ShutdownEntry`] - Registers a cleanup handler that runs when the plugin unloads
//!
//! # Sealed Pattern
//!
//! All entry types include a `__sealed: Sealed` field to prevent direct construction outside
//! of the generated code (typically from the `operai-macro` crate). This ensures entries
//! are only created through the procedural macros that validate the registration.
//!
//! # Example
//!
//! Tools are defined using the `#[tool]` attribute macro from the `operai-macro` crate:
//!
//! ```ignore
//! use operai::Context;
//!
//! /// # My Tool
//! ///
//! /// Does something useful.
//! #[operai::tool]
//! async fn my_tool(ctx: Context, input: MyInput) -> operai::Result<MyOutput> {
//!     // Tool implementation
//!     Ok(MyOutput)
//! }
//! ```

use std::{future::Future, pin::Pin};

use crate::Context;

/// Async handler function for a tool invocation.
///
/// This function type is called by the runtime when a tool is invoked.
/// It receives the call context and raw input bytes, and returns raw output bytes.
pub type ToolHandlerFn =
    fn(Context, Vec<u8>) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<u8>>> + Send + 'static>>;

/// Async initialization handler function.
///
/// This function type is called when the plugin is first loaded to perform
/// any necessary setup, such as establishing connections or initializing resources.
pub type InitFn = fn() -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>;

/// Synchronous shutdown handler function.
///
/// This function type is called when the plugin is unloaded to perform cleanup,
/// such as closing connections or releasing resources.
pub type ShutdownFn = fn();

/// Marker type to prevent direct construction of entry types.
///
/// The sealed pattern ensures that entries like [`ToolEntry`] can only be created
/// through the procedural macros in the `operai-macro` crate, which validate
/// that all required fields are properly set.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct Sealed(pub(crate) ());

/// Registration entry for a tool in the plugin.
///
/// `ToolEntry` represents a tool that can be invoked through the Opera runtime.
/// Entries are typically created by the `#[operai_tool]` procedural macro and
/// automatically submitted to the global inventory via `inventory::submit!`.
///
/// # Fields
///
/// - `id` - Unique identifier for the tool (e.g., "com.example.tool")
/// - `name` - Human-readable display name
/// - `description` - Detailed description of what the tool does
/// - `capabilities` - Optional list of capability strings the tool provides
/// - `tags` - Optional list of tags for categorization and discovery
/// - `input_schema_fn` - Function that returns JSON Schema for tool input
/// - `output_schema_fn` - Function that returns JSON Schema for tool output
/// - `handler` - Async function that handles tool invocations
/// - `__sealed` - Prevents manual construction (use the macro instead)
#[derive(Debug)]
pub struct ToolEntry {
    /// Unique identifier for this tool (e.g., "com.example.my_tool")
    pub id: &'static str,
    /// Human-readable display name
    pub name: &'static str,
    /// Detailed description of the tool's purpose and behavior
    pub description: &'static str,
    /// Optional list of capability strings describing what the tool can do
    pub capabilities: &'static [&'static str],
    /// Optional list of tags for categorization and discovery
    pub tags: &'static [&'static str],
    /// Returns the JSON Schema for this tool's input
    pub input_schema_fn: fn() -> String,
    /// Returns the JSON Schema for this tool's output
    pub output_schema_fn: fn() -> String,
    /// Async handler function called when the tool is invoked
    pub handler: ToolHandlerFn,
    /// Sealed field to prevent manual construction
    #[doc(hidden)]
    pub __sealed: Sealed,
}

inventory::collect!(ToolEntry);

/// Registration entry for a plugin initialization handler.
///
/// `InitEntry` registers an async initialization function that runs when the plugin
/// is first loaded. This is useful for establishing connections, allocating resources,
/// or performing other one-time setup.
///
/// Init handlers are called in the order they were submitted to the inventory.
#[derive(Debug)]
pub struct InitEntry {
    /// Identifier for this init handler (e.g., "database_init")
    pub name: &'static str,
    /// Async initialization function to run on plugin load
    pub handler: InitFn,
    /// Sealed field to prevent manual construction
    #[doc(hidden)]
    pub __sealed: Sealed,
}

inventory::collect!(InitEntry);

/// Registration entry for a plugin shutdown handler.
///
/// `ShutdownEntry` registers a synchronous cleanup function that runs when the plugin
/// is unloaded. This is useful for closing connections, releasing resources, or
/// performing other cleanup operations.
///
/// Shutdown handlers are called in the order they were submitted to the inventory.
/// They run synchronously and cannot be async.
#[derive(Debug)]
pub struct ShutdownEntry {
    /// Identifier for this shutdown handler (e.g., "database_cleanup")
    pub name: &'static str,
    /// Synchronous cleanup function to run on plugin unload
    pub handler: ShutdownFn,
    /// Sealed field to prevent manual construction
    #[doc(hidden)]
    pub __sealed: Sealed,
}

inventory::collect!(ShutdownEntry);

#[cfg(test)]
mod tests {
    //! Unit tests for entrypoint registration and discovery.
    //!
    //! These tests verify that:
    //! - Entry types can be submitted and discovered via inventory
    //! - Handlers can be invoked and return expected results
    //! - The sealed pattern prevents unauthorized construction
    //! - Schema functions return valid JSON
    //! - Futures are Send and can be spawned
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    };

    use super::*;

    const TEST_TOOL_ID: &str = "operai.test.entrypoint.tool";
    const TEST_INIT_NAME: &str = "operai.test.entrypoint.init";
    const TEST_SHUTDOWN_NAME: &str = "operai.test.entrypoint.shutdown";

    // Shared lock for tests that modify global state (SHUTDOWN_CALLED)
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    static SHUTDOWN_CALLED: AtomicBool = AtomicBool::new(false);

    fn test_tool_handler(
        ctx: Context,
        input: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<u8>>> + Send + 'static>> {
        Box::pin(async move {
            let mut output = ctx.request_id().as_bytes().to_vec();
            output.push(b':');
            output.extend_from_slice(&input);
            Ok(output)
        })
    }

    fn test_init_handler() -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>> {
        Box::pin(async { Ok(()) })
    }

    fn test_shutdown_handler() {
        SHUTDOWN_CALLED.store(true, Ordering::SeqCst);
    }

    inventory::submit! {
        ToolEntry {
            id: TEST_TOOL_ID,
            name: "Test Tool",
            description: "Tool entrypoint test tool",
            capabilities: &[],
            tags: &["test"],
            input_schema_fn: || "{\"type\":\"object\"}".to_string(),
            output_schema_fn: || "{\"type\":\"object\"}".to_string(),
            handler: test_tool_handler,
            __sealed: Sealed(()),
        }
    }

    inventory::submit! {
        InitEntry {
            name: TEST_INIT_NAME,
            handler: test_init_handler,
            __sealed: Sealed(()),
        }
    }

    inventory::submit! {
        ShutdownEntry {
            name: TEST_SHUTDOWN_NAME,
            handler: test_shutdown_handler,
            __sealed: Sealed(()),
        }
    }

    #[test]
    fn test_sealed_is_copy_clone_and_debug() {
        fn assert_clone<T: Clone>() {}

        // Arrange
        let sealed = Sealed(());

        // Act
        let copied = sealed;
        let also_copied = sealed;
        assert_clone::<Sealed>();
        let debug = format!("{sealed:?}");

        // Assert
        assert_eq!(debug, "Sealed(())");
        let _ = (copied, also_copied);
    }

    #[tokio::test]
    async fn test_tool_entry_submitted_to_inventory_is_discoverable_and_callable() {
        // Arrange
        let tool_entry = inventory::iter::<ToolEntry>()
            .find(|entry| entry.id == TEST_TOOL_ID)
            .expect("test tool entry must be registered via inventory");
        let ctx = Context::with_metadata("req-123", "sess-456", "user-789");
        let input = b"hello".to_vec();

        // Act
        let future = (tool_entry.handler)(ctx, input);
        let output = tokio::spawn(future)
            .await
            .expect("task should join")
            .expect("tool handler should succeed");

        // Assert - verify all ToolEntry fields are accessible and correct
        assert_eq!(tool_entry.id, TEST_TOOL_ID);
        assert_eq!(tool_entry.name, "Test Tool");
        assert_eq!(tool_entry.description, "Tool entrypoint test tool");
        assert_eq!(tool_entry.capabilities, &[] as &[&str]);
        assert_eq!(tool_entry.tags, &["test"]);
        assert_eq!((tool_entry.input_schema_fn)(), "{\"type\":\"object\"}");
        assert_eq!((tool_entry.output_schema_fn)(), "{\"type\":\"object\"}");
        assert_eq!(output, b"req-123:hello".to_vec());
    }

    #[tokio::test]
    async fn test_tool_handler_with_empty_input_returns_request_id_prefix() {
        // Arrange
        let tool_entry = inventory::iter::<ToolEntry>()
            .find(|entry| entry.id == TEST_TOOL_ID)
            .expect("test tool entry must be registered via inventory");
        let ctx = Context::with_metadata("empty-test", "sess", "user");
        let input = Vec::new();

        // Act
        let future = (tool_entry.handler)(ctx, input);
        let output = future.await.expect("handler should succeed");

        // Assert - empty input should still produce request_id prefix with colon
        assert_eq!(output, b"empty-test:".to_vec());
    }

    #[tokio::test]
    async fn test_init_entry_submitted_to_inventory_is_discoverable_and_callable() {
        // Arrange
        let init_entry = inventory::iter::<InitEntry>()
            .find(|entry| entry.name == TEST_INIT_NAME)
            .expect("test init entry must be registered via inventory");

        // Act
        let result = (init_entry.handler)().await;

        // Assert
        assert!(result.is_ok());
    }

    #[test]
    fn test_shutdown_entry_submitted_to_inventory_is_discoverable_and_callable() {
        // Arrange
        let _guard = TEST_LOCK.lock().unwrap();
        SHUTDOWN_CALLED.store(false, Ordering::SeqCst);
        let shutdown_entry = inventory::iter::<ShutdownEntry>()
            .find(|entry| entry.name == TEST_SHUTDOWN_NAME)
            .expect("test shutdown entry must be registered via inventory");

        // Act
        (shutdown_entry.handler)();

        // Assert
        assert!(SHUTDOWN_CALLED.load(Ordering::SeqCst));
    }

    #[test]
    fn test_tool_entry_capabilities_and_tags_slices_are_accessible() {
        // Arrange & Act
        let tool_entry = inventory::iter::<ToolEntry>()
            .find(|entry| entry.id == TEST_TOOL_ID)
            .expect("test tool entry must be registered");

        // Assert - verify slices are accessible (capabilities empty, tags has 1 item)
        assert!(tool_entry.capabilities.is_empty());
        assert_eq!(tool_entry.tags, &["test"]);
    }

    #[tokio::test]
    async fn test_tool_handler_is_send_and_can_be_spawned() {
        // Arrange
        let tool_entry = inventory::iter::<ToolEntry>()
            .find(|entry| entry.id == TEST_TOOL_ID)
            .expect("test tool entry must be registered");
        let ctx = Context::with_metadata("spawn-test", "sess", "user");
        let input = b"test".to_vec();

        // Act - verify the future is Send by spawning it
        let future = (tool_entry.handler)(ctx, input);
        let handle = tokio::spawn(future);
        let result = handle.await;

        // Assert
        assert!(result.is_ok());
        assert!(result.unwrap().is_ok());
    }

    #[tokio::test]
    async fn test_init_handler_is_send_and_can_be_spawned() {
        // Arrange
        let init_entry = inventory::iter::<InitEntry>()
            .find(|entry| entry.name == TEST_INIT_NAME)
            .expect("test init entry must be registered");

        // Act - verify the future is Send by spawning it
        let future = (init_entry.handler)();
        let handle = tokio::spawn(future);
        let result = handle.await;

        // Assert
        assert!(result.is_ok());
        assert!(result.unwrap().is_ok());
    }

    #[test]
    fn test_schema_functions_return_valid_json() {
        // Arrange
        let tool_entry = inventory::iter::<ToolEntry>()
            .find(|entry| entry.id == TEST_TOOL_ID)
            .expect("test tool entry must be registered");

        // Act
        let input_schema = (tool_entry.input_schema_fn)();
        let output_schema = (tool_entry.output_schema_fn)();

        // Assert - verify schemas are valid JSON
        let input_parsed: serde_json::Value =
            serde_json::from_str(&input_schema).expect("input schema should be valid JSON");
        let output_parsed: serde_json::Value =
            serde_json::from_str(&output_schema).expect("output schema should be valid JSON");

        assert_eq!(input_parsed["type"], "object");
        assert_eq!(output_parsed["type"], "object");
    }

    #[test]
    fn test_entry_types_implement_debug() {
        // Arrange
        let tool_entry = inventory::iter::<ToolEntry>()
            .find(|entry| entry.id == TEST_TOOL_ID)
            .expect("test tool entry must be registered");
        let init_entry = inventory::iter::<InitEntry>()
            .find(|entry| entry.name == TEST_INIT_NAME)
            .expect("test init entry must be registered");
        let shutdown_entry = inventory::iter::<ShutdownEntry>()
            .find(|entry| entry.name == TEST_SHUTDOWN_NAME)
            .expect("test shutdown entry must be registered");

        // Act - verify Debug is implemented and produces non-empty output
        let tool_debug = format!("{tool_entry:?}");
        let init_debug = format!("{init_entry:?}");
        let shutdown_debug = format!("{shutdown_entry:?}");

        // Assert - verify debug output contains type names and key fields
        assert!(tool_debug.contains("ToolEntry"));
        assert!(tool_debug.contains(TEST_TOOL_ID));
        assert!(init_debug.contains("InitEntry"));
        assert!(init_debug.contains(TEST_INIT_NAME));
        assert!(shutdown_debug.contains("ShutdownEntry"));
        assert!(shutdown_debug.contains(TEST_SHUTDOWN_NAME));
    }
}
