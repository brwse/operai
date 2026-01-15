//! # Operai Tool Framework
//!
//! This crate provides a framework for defining async tools that can be
//! dynamically loaded and invoked through an FFI boundary. Tools are defined
//! using the `#[tool]` attribute macro, which generates the necessary
//! boilerplate for registration, serialization, and schema generation.
//!
//! ## Defining a Tool
//!
//! Tools are async functions with the signature:
//!
//! ```ignore
//! #[tool]
//! async fn my_tool(ctx: Context, input: MyInput) -> Result<MyOutput> {
//!     // ...
//! }
//! ```
//!
//! Tool metadata is extracted from structured doc comments:
//!
//! ```ignore
//! /// # Tool Name (ID: custom_id)
//! ///
//! /// Description of what the tool does.
//!
//! /// ## Capabilities
//! /// - read
//! /// - write
//! ///
//! /// ## Tags
//! /// - utility
//! #[tool]
//! async fn my_tool(ctx: Context, input: MyInput) -> Result<MyOutput> {
//!     Ok(MyOutput { /* ... */ })
//! }
//! ```
//!
//! The `Result` type is re-exported from `anyhow` and expands to `Result<T,
//! anyhow::Error>`.
//!
//! ## Tool Module Entry Point
//!
//! At the end of your tool library, invoke the entrypoint macro:
//!
//! ```ignore
//! operai::generate_tool_entrypoint!();
//! ```
//!
//! This generates the FFI-compatible module interface that the runtime uses to
//! discover and invoke your tools.
//!
//! ## Context
//!
//! The `Context` parameter provides access to:
//! - `request_id()`, `session_id()`, `user_id()` - Request metadata
//! - `system_credential(name)` - System-level credentials
//! - `user_credential(name)` - User-provided credentials
//!
//! ## Credentials
//!
//! Define credentials using the `define_system_credential!` and
//! `define_user_credential!` macros:
//!
//! ```ignore
//! use operai::define_system_credential;
//!
//! define_system_credential!(ApiKey("api_key") {
//!     /// API key for authentication
//!     key: String,
//!     /// Optional endpoint override
//!     #[optional]
//!     endpoint: Option<String>,
//! });
//!
//! #[tool]
//! async fn my_tool(ctx: Context, input: MyInput) -> Result<MyOutput> {
//!     let api_key = ApiKey::get(&ctx)?;
//!     Ok(MyOutput { /* use api_key.key */ })
//! }
//! ```
//!
//! Credentials come in two namespaces:
//! - **System credentials**: Provider-level credentials configured by the
//!   operator
//! - **User credentials**: User-specific credentials for authentication
//!
//! ## Lifecycle Hooks
//!
//! Use `#[init]` and `#[shutdown]` to define lifecycle hooks:
//!
//! ```ignore
//! #[init]
//! async fn setup() -> Result<()> {
//!     // Initialize resources
//! }
//!
//! #[shutdown]
//! fn cleanup() {
//!     // Release resources
//! }
//! ```

// Allow proc-macro expansions within this crate to refer to it via `::operai`.
extern crate self as operai;

mod context;
mod credential;
mod entrypoint;

// Re-export abi_stable so the `export_root_module` proc macro can find
// `::abi_stable::` when the generate_tool_entrypoint! macro expands in
// dependent crates.
extern crate abi_stable as _;
pub use anyhow::{self, Result, bail, ensure};
pub use context::Context;
pub use credential::CredentialError;
pub use operai_macro::{define_system_credential, define_user_credential, init, shutdown, tool};
// Full schemars re-export required because JsonSchema derive macro generates
// code referencing `schemars::*` paths directly.
pub use schemars;
pub use schemars::JsonSchema;
pub use tracing::{Level, debug, error, info, span, trace, warn};

pub mod __private {
    pub use anyhow;
    pub use inventory;
    pub use operai_abi;
    pub use schemars;
    pub use serde;
    pub use serde_json;

    pub use crate::{
        context::Context,
        credential::{CredentialEntry, CredentialFieldSchema},
        entrypoint::{InitEntry, Sealed, ShutdownEntry, ToolEntry},
    };

    #[inline]
    #[must_use]
    pub const fn sealed() -> Sealed {
        Sealed(())
    }
}

#[macro_export]
macro_rules! generate_tool_entrypoint {
    () => {
        mod __operai_entrypoint {
            use ::operai::__private::operai_abi as abi;
            use ::std::sync::OnceLock;
            use abi::{
                abi_stable::{
                    export_root_module,
                    prefix_type::PrefixTypeTrait,
                    std_types::{ROption, RSlice, RStr, RVec},
                },
                async_ffi::{FfiFuture, FutureExt},
            };

            #[cfg(operai_embedding)]
            mod __operai_embedding {
                include!(concat!(env!("OUT_DIR"), "/embedding.rs"));
            }

            #[cfg(operai_embedding)]
            use __operai_embedding::EMBEDDING;

            #[cfg(not(operai_embedding))]
            const EMBEDDING: &[f32] = &[];

            static DESCRIPTORS: OnceLock<Vec<abi::ToolDescriptor>> = OnceLock::new();
            static CAPABILITIES: OnceLock<Vec<Vec<RStr<'static>>>> = OnceLock::new();
            static TAGS: OnceLock<Vec<Vec<RStr<'static>>>> = OnceLock::new();

            #[export_root_module]
            pub fn get_root_module() -> abi::ToolModuleRef {
                // Pre-initialize capability and tag arrays
                let capabilities = CAPABILITIES.get_or_init(|| {
                    ::operai::__private::inventory::iter::<::operai::__private::ToolEntry>()
                        .map(|entry| {
                            entry
                                .capabilities
                                .iter()
                                .map(|s| RStr::from_str(s))
                                .collect()
                        })
                        .collect()
                });
                let tags = TAGS.get_or_init(|| {
                    ::operai::__private::inventory::iter::<::operai::__private::ToolEntry>()
                        .map(|entry| entry.tags.iter().map(|s| RStr::from_str(s)).collect())
                        .collect()
                });

                let descriptors = DESCRIPTORS.get_or_init(|| {
                    ::operai::__private::inventory::iter::<::operai::__private::ToolEntry>()
                        .enumerate()
                        .map(|(i, entry)| {
                            let cred_schema: ::std::collections::HashMap<&str, _> =
                                ::operai::__private::inventory::iter::<
                                    ::operai::__private::CredentialEntry,
                                >()
                                .map(|c| (c.name, c))
                                .collect();
                            let cred_schema_json =
                                ::operai::__private::serde_json::to_string(&cred_schema)
                                    .expect("credential schema should be serializable");

                            abi::ToolDescriptor {
                                id: RStr::from_str(entry.id),
                                name: RStr::from_str(entry.name),
                                description: RStr::from_str(entry.description),
                                input_schema: RStr::from_str(Box::leak(
                                    (entry.input_schema_fn)().into_boxed_str(),
                                )),
                                output_schema: RStr::from_str(Box::leak(
                                    (entry.output_schema_fn)().into_boxed_str(),
                                )),
                                credential_schema: if cred_schema.is_empty() {
                                    ROption::RNone
                                } else {
                                    ROption::RSome(RStr::from_str(Box::leak(
                                        cred_schema_json.into_boxed_str(),
                                    )))
                                },
                                capabilities: RSlice::from_slice(&capabilities[i]),
                                tags: RSlice::from_slice(&tags[i]),
                                embedding: RSlice::from_slice(EMBEDDING),
                            }
                        })
                        .collect()
                });

                abi::ToolModule {
                    meta: abi::ToolMeta::new(
                        abi::TOOL_ABI_VERSION,
                        RStr::from_str(env!("CARGO_PKG_NAME")),
                        RStr::from_str(env!("CARGO_PKG_VERSION")),
                    ),
                    descriptors: RSlice::from_slice(descriptors),
                    init,
                    call,
                    shutdown,
                }
                .leak_into_prefix()
            }

            extern "C" fn init(_args: abi::InitArgs) -> FfiFuture<abi::ToolResult> {
                async {
                    for entry in
                        ::operai::__private::inventory::iter::<::operai::__private::InitEntry>()
                    {
                        if let Err(e) = (entry.handler)().await {
                            ::operai::error!("init hook failed: {e:?}");
                            return abi::ToolResult::InitFailed;
                        }
                    }
                    abi::ToolResult::Ok
                }
                .into_ffi()
            }

            extern "C" fn call(args: abi::CallArgs<'_>) -> FfiFuture<abi::CallResult> {
                let tool_id_str = args.tool_id.as_str();

                let handler =
                    ::operai::__private::inventory::iter::<::operai::__private::ToolEntry>()
                        .find(|e| e.id == tool_id_str);

                let handler = match handler {
                    Some(h) => h.handler,
                    None => {
                        return async {
                            abi::CallResult::error(abi::ToolResult::NotFound, "tool not found")
                        }
                        .into_ffi();
                    }
                };

                let ctx = ::operai::__private::Context::__from_call_context(&args.context);
                let input_bytes = args.input.as_slice().to_vec();
                let future = (handler)(ctx, input_bytes);

                async move {
                    match future.await {
                        Ok(output_bytes) => abi::CallResult::ok(RVec::from(output_bytes)),
                        Err(e) => abi::CallResult::error(abi::ToolResult::Error, &e.to_string()),
                    }
                }
                .into_ffi()
            }

            extern "C" fn shutdown() {
                for entry in
                    ::operai::__private::inventory::iter::<::operai::__private::ShutdownEntry>()
                {
                    (entry.handler)();
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use tokio::sync::Mutex as AsyncMutex;

    use super::*;

    // Shared lock for tests that modify global state (init/shutdown counters)
    static TEST_LOCK: Mutex<()> = Mutex::new(());
    static ASYNC_TEST_LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();

    async fn test_lock_async() -> tokio::sync::MutexGuard<'static, ()> {
        ASYNC_TEST_LOCK
            .get_or_init(|| AsyncMutex::new(()))
            .lock()
            .await
    }

    mod test_tool_library {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

        use serde::{Deserialize, Serialize};

        use super::{Context, JsonSchema, Result, bail, ensure, init, shutdown, tool};

        static INIT_SHOULD_FAIL: AtomicBool = AtomicBool::new(false);
        static INIT_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);
        static SHUTDOWN_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

        #[derive(Debug, Deserialize, JsonSchema)]
        struct GreetInput {
            name: String,
        }

        #[derive(Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
        pub(super) struct GreetOutput {
            pub(super) message: String,
            pub(super) user_id: String,
        }

        /// # Greet (ID: greet)
        ///
        /// Greets a user.
        ///
        /// ## Capabilities
        /// - read
        ///
        /// ## Tags
        /// - greeting
        #[tool]
        async fn greet(ctx: Context, input: GreetInput) -> Result<GreetOutput> {
            Ok(GreetOutput {
                message: format!("Hello, {}!", input.name),
                user_id: ctx.user_id().to_string(),
            })
        }

        #[derive(Debug, Deserialize, JsonSchema)]
        struct FailInput {}

        #[derive(Debug, Serialize, JsonSchema)]
        struct FailOutput {
            message: String,
        }

        /// # Fail (ID: fail)
        ///
        /// Always fails.
        #[tool]
        async fn fail(_ctx: Context, _input: FailInput) -> Result<FailOutput> {
            bail!("boom");
        }

        #[init]
        async fn setup() -> Result<()> {
            INIT_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
            ensure!(!INIT_SHOULD_FAIL.load(Ordering::SeqCst), "init failed");
            Ok(())
        }

        #[shutdown]
        fn cleanup() {
            SHUTDOWN_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
        }

        crate::generate_tool_entrypoint!();

        pub use __operai_entrypoint::get_root_module;

        pub fn set_init_should_fail(should_fail: bool) {
            INIT_SHOULD_FAIL.store(should_fail, Ordering::SeqCst);
        }

        pub fn init_call_count() -> usize {
            INIT_CALL_COUNT.load(Ordering::SeqCst)
        }

        pub fn reset_init_call_count() {
            INIT_CALL_COUNT.store(0, Ordering::SeqCst);
        }

        pub fn shutdown_call_count() -> usize {
            SHUTDOWN_CALL_COUNT.load(Ordering::SeqCst)
        }

        pub fn reset_shutdown_call_count() {
            SHUTDOWN_CALL_COUNT.store(0, Ordering::SeqCst);
        }
    }

    fn find_descriptor<'a>(
        module: &'a operai_abi::ToolModuleRef,
        id: &str,
    ) -> &'a operai_abi::ToolDescriptor {
        module
            .descriptors_iter()
            .find(|d| d.id.as_str() == id)
            .unwrap_or_else(|| panic!("missing descriptor for tool id: {id}"))
    }

    fn schema_contains_property(schema: &serde_json::Value, property: &str) -> bool {
        match schema {
            serde_json::Value::Object(map) => {
                if let Some(serde_json::Value::Object(props)) = map.get("properties")
                    && props.contains_key(property)
                {
                    return true;
                }

                map.values().any(|v| schema_contains_property(v, property))
            }
            serde_json::Value::Array(values) => {
                values.iter().any(|v| schema_contains_property(v, property))
            }
            _ => false,
        }
    }

    fn call_args<'a>(
        request_id: &'a str,
        session_id: &'a str,
        user_id: &'a str,
        tool_id: &'a str,
        input_json: &'a [u8],
    ) -> operai_abi::CallArgs<'a> {
        use operai_abi::{
            CallContext,
            abi_stable::std_types::{RSlice, RStr},
        };

        let call_ctx = CallContext {
            request_id: RStr::from_str(request_id),
            session_id: RStr::from_str(session_id),
            user_id: RStr::from_str(user_id),
            user_credentials: RSlice::from_slice(&[]),
            system_credentials: RSlice::from_slice(&[]),
        };

        operai_abi::CallArgs::new(
            call_ctx,
            RStr::from_str(tool_id),
            RSlice::from_slice(input_json),
        )
    }

    // ==========================================================================
    // schema_contains_property helper tests
    // ==========================================================================

    #[test]
    fn test_schema_contains_property_finds_direct_property() {
        // Arrange
        let schema = serde_json::json!({
            "properties": {
                "name": { "type": "string" }
            }
        });

        // Act & Assert
        assert!(schema_contains_property(&schema, "name"));
        assert!(!schema_contains_property(&schema, "missing"));
    }

    #[test]
    fn test_schema_contains_property_finds_nested_property() {
        // Arrange
        let schema = serde_json::json!({
            "definitions": {
                "Address": {
                    "properties": {
                        "street": { "type": "string" }
                    }
                }
            }
        });

        // Act & Assert
        assert!(schema_contains_property(&schema, "street"));
    }

    #[test]
    fn test_schema_contains_property_returns_false_for_empty_schema() {
        // Arrange
        let schema = serde_json::json!({});

        // Act & Assert
        assert!(!schema_contains_property(&schema, "any"));
    }

    #[test]
    fn test_schema_contains_property_returns_false_for_non_object_schema() {
        // Arrange
        let string_schema = serde_json::json!("not an object");
        let number_schema = serde_json::json!(42);
        let null_schema = serde_json::Value::Null;

        // Act & Assert
        assert!(!schema_contains_property(&string_schema, "any"));
        assert!(!schema_contains_property(&number_schema, "any"));
        assert!(!schema_contains_property(&null_schema, "any"));
    }

    #[test]
    fn test_schema_contains_property_finds_property_in_array() {
        // Arrange
        let schema = serde_json::json!([
            { "properties": { "a": {} } },
            { "properties": { "b": {} } }
        ]);

        // Act & Assert
        assert!(schema_contains_property(&schema, "a"));
        assert!(schema_contains_property(&schema, "b"));
        assert!(!schema_contains_property(&schema, "c"));
    }

    // ==========================================================================
    // Module metadata tests
    // ==========================================================================

    #[test]
    fn test_module_meta_contains_abi_version_and_crate_info() {
        // Arrange
        let module = test_tool_library::get_root_module();

        // Act
        let meta = module.meta();

        // Assert
        assert_eq!(meta.abi_version, operai_abi::TOOL_ABI_VERSION);
        assert_eq!(meta.crate_name.as_str(), env!("CARGO_PKG_NAME"));
        assert_eq!(meta.crate_version.as_str(), env!("CARGO_PKG_VERSION"));
    }

    // ==========================================================================
    // Tool descriptor tests
    // ==========================================================================

    #[test]
    fn test_tool_descriptor_contains_metadata_and_valid_schemas() {
        // Arrange
        let module = test_tool_library::get_root_module();

        // Act
        let greet = find_descriptor(&module, "greet");
        let input_schema: serde_json::Value =
            serde_json::from_str(greet.input_schema.as_str()).unwrap();
        let output_schema: serde_json::Value =
            serde_json::from_str(greet.output_schema.as_str()).unwrap();

        // Assert
        assert_eq!(greet.name.as_str(), "Greet");
        assert_eq!(greet.description.as_str(), "Greets a user.");

        let capabilities: Vec<&str> = greet
            .capabilities
            .as_slice()
            .iter()
            .map(operai_abi::abi_stable::std_types::RStr::as_str)
            .collect();
        assert_eq!(capabilities, vec!["read"]);

        let tags: Vec<&str> = greet
            .tags
            .as_slice()
            .iter()
            .map(operai_abi::abi_stable::std_types::RStr::as_str)
            .collect();
        assert_eq!(tags, vec!["greeting"]);

        assert!(schema_contains_property(&input_schema, "name"));
        assert!(schema_contains_property(&output_schema, "message"));
        assert!(schema_contains_property(&output_schema, "user_id"));

        assert!(matches!(
            greet.credential_schema,
            operai_abi::abi_stable::std_types::ROption::RNone
        ));
    }

    #[test]
    fn test_tool_descriptor_with_empty_capabilities_and_tags() {
        // Arrange
        let module = test_tool_library::get_root_module();

        // Act
        let fail_tool = find_descriptor(&module, "fail");

        // Assert
        assert!(fail_tool.capabilities.as_slice().is_empty());
        assert!(fail_tool.tags.as_slice().is_empty());
    }

    #[test]
    fn test_all_registered_tools_are_discoverable() {
        // Arrange
        let module = test_tool_library::get_root_module();

        // Act
        let tool_ids: Vec<&str> = module.descriptors_iter().map(|d| d.id.as_str()).collect();

        // Assert
        assert!(
            tool_ids.contains(&"greet"),
            "greet tool should be registered"
        );
        assert!(tool_ids.contains(&"fail"), "fail tool should be registered");
    }

    // ==========================================================================
    // Tool call tests
    // ==========================================================================

    #[tokio::test]
    async fn test_call_with_valid_input_returns_json_output() {
        // Arrange
        let module = test_tool_library::get_root_module();
        let input = serde_json::json!({ "name": "Alice" });
        let input_json = serde_json::to_vec(&input).unwrap();

        let args = call_args("req-123", "sess-456", "user-789", "greet", &input_json);

        // Act
        let result = (module.call())(args).await;

        // Assert
        assert_eq!(result.result, operai_abi::ToolResult::Ok);
        let output: test_tool_library::GreetOutput =
            serde_json::from_slice(result.output.as_slice()).unwrap();
        assert_eq!(
            output,
            test_tool_library::GreetOutput {
                message: "Hello, Alice!".to_string(),
                user_id: "user-789".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn test_call_with_unknown_tool_id_returns_not_found() {
        // Arrange
        let module = test_tool_library::get_root_module();
        let input_json = b"{}";
        let args = call_args(
            "req-123",
            "sess-456",
            "user-789",
            "does_not_exist",
            input_json,
        );

        // Act
        let result = (module.call())(args).await;

        // Assert
        assert_eq!(result.result, operai_abi::ToolResult::NotFound);
        assert_eq!(result.output.as_slice(), b"tool not found");
    }

    #[tokio::test]
    async fn test_call_with_missing_required_field_returns_error() {
        // Arrange
        let module = test_tool_library::get_root_module();
        let input_json = b"{}";
        let args = call_args("req-123", "sess-456", "user-789", "greet", input_json);

        // Act
        let result = (module.call())(args).await;

        // Assert
        assert_eq!(result.result, operai_abi::ToolResult::Error);
        let message = std::str::from_utf8(result.output.as_slice()).unwrap();
        assert!(message.contains("missing field `name`"));
    }

    #[tokio::test]
    async fn test_call_propagates_tool_error_message() {
        // Arrange
        let module = test_tool_library::get_root_module();
        let input_json = b"{}";
        let args = call_args("req-123", "sess-456", "user-789", "fail", input_json);

        // Act
        let result = (module.call())(args).await;

        // Assert
        assert_eq!(result.result, operai_abi::ToolResult::Error);
        assert_eq!(result.output.as_slice(), b"boom");
    }

    #[tokio::test]
    async fn test_call_with_malformed_json_returns_parse_error() {
        // Arrange
        let module = test_tool_library::get_root_module();
        let input_json = b"{ invalid json }";
        let args = call_args("req-123", "sess-456", "user-789", "greet", input_json);

        // Act
        let result = (module.call())(args).await;

        // Assert
        assert_eq!(result.result, operai_abi::ToolResult::Error);
        let message = std::str::from_utf8(result.output.as_slice()).unwrap();
        assert!(
            !message.is_empty(),
            "error message should describe parsing failure"
        );
    }

    #[tokio::test]
    async fn test_call_with_empty_json_object_for_no_required_fields_succeeds() {
        // Arrange
        let module = test_tool_library::get_root_module();
        let input_json = b"{}";
        // The 'fail' tool has FailInput with no required fields
        let args = call_args("req-123", "sess-456", "user-789", "fail", input_json);

        // Act
        let result = (module.call())(args).await;

        // Assert - tool should parse input successfully, then fail with "boom"
        assert_eq!(result.result, operai_abi::ToolResult::Error);
        assert_eq!(result.output.as_slice(), b"boom");
    }

    // ==========================================================================
    // Init lifecycle tests
    // ==========================================================================

    #[tokio::test]
    async fn test_init_returns_ok_when_hook_succeeds() {
        use operai_abi::{InitArgs, RuntimeContext, ToolResult};

        // Arrange
        let _guard = test_lock_async().await;
        let module = test_tool_library::get_root_module();
        test_tool_library::reset_init_call_count();
        test_tool_library::set_init_should_fail(false);

        // Act
        let result = (module.init())(InitArgs::new(RuntimeContext::new())).await;

        // Assert
        assert_eq!(result, ToolResult::Ok);
        assert_eq!(test_tool_library::init_call_count(), 1);
    }

    #[tokio::test]
    async fn test_init_returns_init_failed_when_hook_fails() {
        use operai_abi::{InitArgs, RuntimeContext, ToolResult};

        // Arrange
        let _guard = test_lock_async().await;
        let module = test_tool_library::get_root_module();
        test_tool_library::reset_init_call_count();
        test_tool_library::set_init_should_fail(true);

        // Act
        let result = (module.init())(InitArgs::new(RuntimeContext::new())).await;

        // Assert
        assert_eq!(result, ToolResult::InitFailed);
        assert_eq!(test_tool_library::init_call_count(), 1);
    }

    // ==========================================================================
    // Shutdown lifecycle tests
    // ==========================================================================

    #[test]
    fn test_shutdown_invokes_registered_hooks() {
        // Arrange
        let _guard = TEST_LOCK.lock().unwrap();
        let module = test_tool_library::get_root_module();
        test_tool_library::reset_shutdown_call_count();

        // Act
        (module.shutdown())();

        // Assert
        assert_eq!(test_tool_library::shutdown_call_count(), 1);
    }

    // ==========================================================================
    // GreetOutput serialization roundtrip test
    // ==========================================================================

    #[test]
    fn test_greet_output_serialization_roundtrip() {
        // Arrange
        let output = test_tool_library::GreetOutput {
            message: "Hello, World!".to_string(),
            user_id: "user-123".to_string(),
        };

        // Act
        let json = serde_json::to_string(&output).unwrap();
        let parsed: test_tool_library::GreetOutput = serde_json::from_str(&json).unwrap();

        // Assert
        assert_eq!(output, parsed);
    }
}
