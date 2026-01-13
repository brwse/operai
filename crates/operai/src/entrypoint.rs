//! Tool registration and entrypoint types.

use std::{future::Future, pin::Pin};

use crate::Context;

/// Async handler function type for tools.
pub type ToolHandlerFn =
    fn(Context, Vec<u8>) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<u8>>> + Send + 'static>>;

/// Async init function type.
pub type InitFn = fn() -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>;

/// Sync shutdown function type.
pub type ShutdownFn = fn();

/// Sealed token that prevents external construction of [`ToolEntry`].
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct Sealed(pub(crate) ());

/// Registry entry for a tool, constructed only by the `#[tool]` macro.
#[derive(Debug)]
pub struct ToolEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub capabilities: &'static [&'static str],
    pub tags: &'static [&'static str],
    pub input_schema_fn: fn() -> String,
    pub output_schema_fn: fn() -> String,
    pub handler: ToolHandlerFn,
    #[doc(hidden)]
    pub __sealed: Sealed,
}

inventory::collect!(ToolEntry);

/// Registry entry for an init function, constructed only by the `#[init]`
/// macro.
#[derive(Debug)]
pub struct InitEntry {
    /// Used for debugging and logging.
    pub name: &'static str,
    pub handler: InitFn,
    #[doc(hidden)]
    pub __sealed: Sealed,
}

inventory::collect!(InitEntry);

/// Registry entry for a shutdown function, constructed only by the
/// `#[shutdown]` macro.
#[derive(Debug)]
pub struct ShutdownEntry {
    /// Used for debugging and logging.
    pub name: &'static str,
    pub handler: ShutdownFn,
    #[doc(hidden)]
    pub __sealed: Sealed,
}

inventory::collect!(ShutdownEntry);

#[cfg(test)]
mod tests {
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
