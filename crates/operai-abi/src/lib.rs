//! Stable ABI types for Operai Tool runtime.
//!
//! This crate defines the FFI boundary between the Operai Toolbox runtime and
//! dynamically loaded tool libraries (cdylib). All types use `abi_stable` for
//! guaranteed ABI stability across Rust compiler versions.
//!
//! # Ownership Philosophy
//!
//! Types in this crate follow the principle: **borrow as much as possible until
//! cloning is needed** (e.g., for async futures).
//!
//! - **`'static` lifetime types** ([`ToolDescriptor`], [`ToolMeta`]): Borrow
//!   from the loaded library. The data lives as long as the library is loaded.
//!
//! - **Per-call types with `'a` lifetime** ([`CallContext`], [`CallArgs`]):
//!   Borrow from the caller's stack. Valid only for the duration of the
//!   synchronous FFI call.
//!
//! - **Async return types** ([`FfiFuture`]): FFI-safe futures returned from
//!   tool operations. The caller awaits these to get the result.
//!
//! - **SDK types** (`Context` in operai): Owned for user ergonomics. The SDK
//!   clones data from the FFI types when crossing the async boundary.
//!
//! # Safety
//!
//! This crate uses `abi_stable` to provide safe FFI types. Tool authors should
//! use the `operai` SDK which provides additional abstractions.

pub use abi_stable;
use abi_stable::{
    StableAbi, declare_root_module_statics,
    library::RootModule,
    package_version_strings,
    sabi_types::VersionStrings,
    std_types::{ROption, RSlice, RStr, RVec},
};
pub use async_ffi;
use async_ffi::FfiFuture;

/// Current ABI version. Incremented when breaking changes are made.
/// The runtime checks this to ensure compatibility with loaded tools.
pub const TOOL_ABI_VERSION: u32 = 1;

/// Result codes for tool operations.
#[repr(u8)]
#[derive(StableAbi, Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ToolResult {
    /// Operation completed successfully.
    Ok = 0,
    /// Generic error occurred.
    Error = 1,
    /// Tool with the specified ID was not found.
    NotFound = 2,
    /// Invalid input was provided.
    InvalidInput = 3,
    /// ABI version mismatch between runtime and tool.
    AbiMismatch = 4,
    /// Tool initialization failed.
    InitFailed = 5,
    /// Credential not found or invalid.
    CredentialError = 6,
}

/// Metadata about the tool crate.
#[repr(C)]
#[derive(StableAbi, Debug, Clone, Copy)]
pub struct ToolMeta {
    /// ABI version this tool was compiled with.
    pub abi_version: u32,
    /// Crate name (e.g., "hello-world").
    pub crate_name: RStr<'static>,
    /// Crate version (e.g., "0.1.0").
    pub crate_version: RStr<'static>,
}

impl ToolMeta {
    #[must_use]
    pub const fn new(
        abi_version: u32,
        crate_name: RStr<'static>,
        crate_version: RStr<'static>,
    ) -> Self {
        Self {
            abi_version,
            crate_name,
            crate_version,
        }
    }
}

/// Descriptor for a single tool within a crate.
///
/// A crate may contain multiple tools, each with its own descriptor.
/// The qualified name is "crate-name.tool-id" (e.g., "hello-world.greet").
#[repr(C)]
#[derive(StableAbi, Debug, Clone)]
pub struct ToolDescriptor {
    /// Unique identifier within the crate (e.g., "greet").
    pub id: RStr<'static>,
    /// Human-readable display name (e.g., "Say Hello!").
    pub name: RStr<'static>,
    /// Human-readable description of what this tool does.
    pub description: RStr<'static>,
    /// JSON Schema defining valid input.
    pub input_schema: RStr<'static>,
    /// JSON Schema defining output format.
    pub output_schema: RStr<'static>,
    /// JSON Schema for required credentials, if any.
    pub credential_schema: ROption<RStr<'static>>,
    /// Required capabilities (e.g., "read", "write").
    pub capabilities: RSlice<'static, RStr<'static>>,
    /// Tags for categorization and discovery.
    pub tags: RSlice<'static, RStr<'static>>,
    /// Pre-computed embedding for semantic search.
    pub embedding: RSlice<'static, f32>,
}

/// Runtime context passed to tools during initialization.
///
/// Reserved for future use. May contain runtime configuration or
/// shared resources in future versions.
#[repr(C)]
#[derive(StableAbi, Debug, Clone, Copy, Default)]
pub struct RuntimeContext {
    /// Reserved field to ensure the struct is not zero-sized.
    /// Zero-sized types have special ABI considerations.
    reserved: u8,
}

impl RuntimeContext {
    #[must_use]
    pub const fn new() -> Self {
        Self { reserved: 0 }
    }
}

/// Per-request context passed to tools during invocation.
///
/// All data is borrowed from the runtime's call stack and valid only
/// for the duration of the synchronous FFI call.
#[repr(C)]
#[derive(StableAbi, Debug, Clone, Copy)]
pub struct CallContext<'a> {
    /// For request tracing and correlation.
    pub request_id: RStr<'a>,
    /// For stateful multi-turn interactions.
    pub session_id: RStr<'a>,
    /// Identifier for the calling user.
    pub user_id: RStr<'a>,
    /// Bincode-encoded user credentials.
    pub user_credentials: RSlice<'a, u8>,
    /// Bincode-encoded system credentials.
    pub system_credentials: RSlice<'a, u8>,
}

/// Result of an async tool call operation.
#[repr(C)]
#[derive(StableAbi, Debug, Clone)]
pub struct CallResult {
    pub result: ToolResult,
    /// Output data, or error message on failure.
    pub output: RVec<u8>,
}

impl CallResult {
    #[must_use]
    pub fn ok(output: RVec<u8>) -> Self {
        Self {
            result: ToolResult::Ok,
            output,
        }
    }

    #[must_use]
    pub fn error(result: ToolResult, message: &str) -> Self {
        Self {
            result,
            output: RVec::from_slice(message.as_bytes()),
        }
    }
}

/// Arguments for [`ToolInitFn`].
#[repr(C)]
#[derive(StableAbi, Clone, Copy)]
pub struct InitArgs {
    pub ctx: RuntimeContext,
}

impl InitArgs {
    #[must_use]
    pub const fn new(ctx: RuntimeContext) -> Self {
        Self { ctx }
    }
}

/// Arguments for [`ToolCallFn`].
#[repr(C)]
#[derive(StableAbi)]
pub struct CallArgs<'a> {
    pub context: CallContext<'a>,
    /// Which tool to invoke (e.g., "greet").
    pub tool_id: RStr<'a>,
    /// JSON-encoded input data.
    pub input: RSlice<'a, u8>,
}

impl<'a> CallArgs<'a> {
    #[must_use]
    pub const fn new(context: CallContext<'a>, tool_id: RStr<'a>, input: RSlice<'a, u8>) -> Self {
        Self {
            context,
            tool_id,
            input,
        }
    }
}

/// Function signature for tool initialization.
pub type ToolInitFn = extern "C" fn(args: InitArgs) -> FfiFuture<ToolResult>;

/// Function signature for tool invocation.
pub type ToolCallFn = extern "C" fn(args: CallArgs<'_>) -> FfiFuture<CallResult>;

/// Function signature for tool shutdown.
pub type ToolShutdownFn = extern "C" fn();

/// Tool module - the main interface exposed by tool libraries.
///
/// This is a prefix type that allows adding new fields in future versions
/// while maintaining backward compatibility.
#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix(prefix_ref = ToolModuleRef)))]
#[sabi(missing_field(panic))]
pub struct ToolModule {
    /// Metadata about the tool crate.
    pub meta: ToolMeta,

    /// Array of tool descriptors.
    pub descriptors: RSlice<'static, ToolDescriptor>,

    /// Called once after loading to initialize the tool library.
    #[sabi(unsafe_opaque_field)]
    pub init: ToolInitFn,

    /// Invokes a tool by ID.
    #[sabi(unsafe_opaque_field)]
    pub call: ToolCallFn,

    /// Called once before unloading to clean up resources.
    #[sabi(last_prefix_field)]
    pub shutdown: ToolShutdownFn,
}

impl RootModule for ToolModuleRef {
    declare_root_module_statics! { ToolModuleRef }

    const BASE_NAME: &'static str = "operai";
    const NAME: &'static str = "Operai Tool Module";
    const VERSION_STRINGS: VersionStrings = package_version_strings!();
}

impl ToolModuleRef {
    pub fn descriptors_iter(&self) -> impl Iterator<Item = &ToolDescriptor> {
        self.descriptors().as_slice().iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_result_discriminants_are_stable() {
        assert_eq!(ToolResult::Ok as u8, 0);
        assert_eq!(ToolResult::Error as u8, 1);
        assert_eq!(ToolResult::NotFound as u8, 2);
        assert_eq!(ToolResult::InvalidInput as u8, 3);
        assert_eq!(ToolResult::AbiMismatch as u8, 4);
        assert_eq!(ToolResult::InitFailed as u8, 5);
        assert_eq!(ToolResult::CredentialError as u8, 6);
    }

    #[test]
    fn test_call_result_ok_sets_success_and_preserves_output() {
        let output_bytes = vec![0_u8, 1, 2, 3];
        let output = RVec::from(output_bytes.clone());

        let call_result = CallResult::ok(output);

        assert_eq!(call_result.result, ToolResult::Ok);
        assert_eq!(call_result.output.as_slice(), output_bytes.as_slice());
    }

    #[test]
    fn test_call_result_error_sets_result_and_encodes_message() {
        let result_code = ToolResult::InvalidInput;
        let message = "invalid input: missing field";

        let call_result = CallResult::error(result_code, message);

        assert_eq!(call_result.result, result_code);
        assert_eq!(call_result.output.as_slice(), message.as_bytes());
    }

    #[test]
    fn test_call_result_error_with_empty_message_returns_empty_output() {
        let call_result = CallResult::error(ToolResult::Error, "");

        assert!(call_result.output.as_slice().is_empty());
    }

    #[test]
    fn test_runtime_context_default_and_new_use_zero_reserved() {
        let new_context = RuntimeContext::new();
        let default_context = RuntimeContext::default();

        assert!(std::mem::size_of::<RuntimeContext>() > 0);
        assert_eq!(new_context.reserved, 0);
        assert_eq!(default_context.reserved, 0);
    }

    #[test]
    fn test_tool_meta_new_preserves_all_fields() {
        let meta = ToolMeta::new(TOOL_ABI_VERSION, RStr::from("my-tool"), RStr::from("1.2.3"));

        assert_eq!(meta.abi_version, TOOL_ABI_VERSION);
        assert_eq!(meta.crate_name.as_str(), "my-tool");
        assert_eq!(meta.crate_version.as_str(), "1.2.3");
    }

    #[test]
    fn test_init_args_new_preserves_context() {
        let ctx = RuntimeContext::new();
        let args = InitArgs::new(ctx);

        assert_eq!(args.ctx.reserved, ctx.reserved);
    }

    #[test]
    fn test_call_args_new_preserves_all_fields() {
        let context = CallContext {
            request_id: RStr::from("req-123"),
            session_id: RStr::from("sess-456"),
            user_id: RStr::from("user-789"),
            user_credentials: RSlice::from_slice(&[1, 2, 3]),
            system_credentials: RSlice::from_slice(&[4, 5, 6]),
        };
        let tool_id = RStr::from("greet");
        let input = RSlice::from_slice(b"{\"name\":\"world\"}");

        let args = CallArgs::new(context, tool_id, input);

        assert_eq!(args.context.request_id.as_str(), "req-123");
        assert_eq!(args.context.session_id.as_str(), "sess-456");
        assert_eq!(args.context.user_id.as_str(), "user-789");
        assert_eq!(args.context.user_credentials.as_slice(), &[1, 2, 3]);
        assert_eq!(args.context.system_credentials.as_slice(), &[4, 5, 6]);
        assert_eq!(args.tool_id.as_str(), "greet");
        assert_eq!(args.input.as_slice(), b"{\"name\":\"world\"}");
    }

    #[test]
    fn test_call_result_ok_with_empty_output() {
        let call_result = CallResult::ok(RVec::new());

        assert_eq!(call_result.result, ToolResult::Ok);
        assert!(call_result.output.as_slice().is_empty());
    }

    #[test]
    fn test_tool_result_is_copy() {
        let original = ToolResult::NotFound;
        let copied = original;

        // Both should be usable after copy (not moved)
        assert_eq!(original, copied);
        assert_eq!(original, ToolResult::NotFound);
    }

    #[test]
    fn test_tool_result_equality() {
        assert_eq!(ToolResult::Ok, ToolResult::Ok);
        assert_ne!(ToolResult::Ok, ToolResult::Error);
        assert_ne!(ToolResult::InvalidInput, ToolResult::AbiMismatch);
    }

    // ABI stability tests - these catch accidental layout changes
    #[test]
    fn test_runtime_context_is_not_zero_sized() {
        // Zero-sized types have special ABI considerations that could cause issues
        assert!(
            std::mem::size_of::<RuntimeContext>() > 0,
            "RuntimeContext must not be zero-sized for ABI stability"
        );
    }

    #[test]
    fn test_tool_result_size_is_one_byte() {
        // repr(u8) should make this exactly 1 byte
        assert_eq!(
            std::mem::size_of::<ToolResult>(),
            1,
            "ToolResult should be 1 byte (repr(u8))"
        );
    }

    #[test]
    fn test_tool_result_debug_format_contains_variant_name() {
        assert!(format!("{:?}", ToolResult::Ok).contains("Ok"));
        assert!(format!("{:?}", ToolResult::CredentialError).contains("CredentialError"));
    }

    #[test]
    fn test_call_result_clone_creates_independent_copy() {
        let original = CallResult::ok(RVec::from(vec![1_u8, 2, 3]));
        let cloned = original.clone();

        assert_eq!(cloned.result, original.result);
        assert_eq!(cloned.output.as_slice(), original.output.as_slice());
    }

    #[test]
    fn test_tool_meta_is_copy() {
        let original = ToolMeta::new(TOOL_ABI_VERSION, RStr::from("test"), RStr::from("1.0.0"));
        let copied = original;

        // Both should be usable after copy
        assert_eq!(original.crate_name.as_str(), copied.crate_name.as_str());
    }

    #[test]
    fn test_runtime_context_is_copy() {
        let original = RuntimeContext::new();
        let copied = original;

        // Both should be usable after copy
        assert_eq!(original.reserved, copied.reserved);
    }

    #[test]
    fn test_call_context_is_copy() {
        let original = CallContext {
            request_id: RStr::from("req-123"),
            session_id: RStr::from("sess-456"),
            user_id: RStr::from("user-789"),
            user_credentials: RSlice::from_slice(&[1, 2, 3]),
            system_credentials: RSlice::from_slice(&[4, 5, 6]),
        };
        let copied = original;

        // Both should be usable after copy (not moved)
        assert_eq!(original.request_id.as_str(), copied.request_id.as_str());
        assert_eq!(original.user_id.as_str(), copied.user_id.as_str());
    }

    #[test]
    fn test_init_args_is_copy() {
        let original = InitArgs::new(RuntimeContext::new());
        let copied = original;

        // Both should be usable after copy
        assert_eq!(original.ctx.reserved, copied.ctx.reserved);
    }

    #[test]
    fn test_abi_version_is_documented_value() {
        // This test documents the current ABI version and will fail
        // if it changes unexpectedly, prompting review of compatibility
        assert_eq!(TOOL_ABI_VERSION, 1);
    }

    #[test]
    fn test_call_context_with_empty_identifiers() {
        let context = CallContext {
            request_id: RStr::from(""),
            session_id: RStr::from(""),
            user_id: RStr::from(""),
            user_credentials: RSlice::from_slice(&[]),
            system_credentials: RSlice::from_slice(&[]),
        };

        assert!(context.request_id.as_str().is_empty());
        assert!(context.session_id.as_str().is_empty());
        assert!(context.user_id.as_str().is_empty());
        assert!(context.user_credentials.as_slice().is_empty());
        assert!(context.system_credentials.as_slice().is_empty());
    }

    #[test]
    fn test_call_args_with_empty_input() {
        let context = CallContext {
            request_id: RStr::from("req"),
            session_id: RStr::from("sess"),
            user_id: RStr::from("user"),
            user_credentials: RSlice::from_slice(&[]),
            system_credentials: RSlice::from_slice(&[]),
        };
        let args = CallArgs::new(context, RStr::from("tool"), RSlice::from_slice(&[]));

        assert!(args.input.as_slice().is_empty());
    }

    #[test]
    fn test_tool_descriptor_clone_creates_independent_copy() {
        let descriptor = ToolDescriptor {
            id: RStr::from("test-id"),
            name: RStr::from("Test Tool"),
            description: RStr::from("A test tool"),
            input_schema: RStr::from("{}"),
            output_schema: RStr::from("{}"),
            credential_schema: ROption::RNone,
            capabilities: RSlice::from_slice(&[]),
            tags: RSlice::from_slice(&[]),
            embedding: RSlice::from_slice(&[]),
        };

        let cloned = descriptor.clone();

        assert_eq!(cloned.id.as_str(), descriptor.id.as_str());
        assert_eq!(cloned.name.as_str(), descriptor.name.as_str());
        assert_eq!(cloned.description.as_str(), descriptor.description.as_str());
    }

    #[test]
    fn test_tool_descriptor_with_credential_schema() {
        let schema = RStr::from(r#"{"type":"object"}"#);
        let descriptor = ToolDescriptor {
            id: RStr::from("secure-tool"),
            name: RStr::from("Secure Tool"),
            description: RStr::from("Requires credentials"),
            input_schema: RStr::from("{}"),
            output_schema: RStr::from("{}"),
            credential_schema: ROption::RSome(schema),
            capabilities: RSlice::from_slice(&[]),
            tags: RSlice::from_slice(&[]),
            embedding: RSlice::from_slice(&[]),
        };

        match descriptor.credential_schema {
            ROption::RSome(s) => assert_eq!(s.as_str(), r#"{"type":"object"}"#),
            ROption::RNone => panic!("Expected credential schema to be present"),
        }
    }

    #[test]
    fn test_call_result_error_with_all_result_codes() {
        // Ensure all error codes work with CallResult::error
        let codes = [
            ToolResult::Error,
            ToolResult::NotFound,
            ToolResult::InvalidInput,
            ToolResult::AbiMismatch,
            ToolResult::InitFailed,
            ToolResult::CredentialError,
        ];

        for code in codes {
            let result = CallResult::error(code, "test message");
            assert_eq!(result.result, code);
        }
    }
}
