//! # Operai Tool ABI
//!
//! This crate defines the stable Application Binary Interface (ABI) for Operai
//! tools. It enables dynamic loading and invocation of tool libraries across
//! different versions of the Operai runtime.
//!
//! ## Architecture
//!
//! The ABI uses [`abi_stable`] to provide cross-version compatibility, allowing
//! tools compiled against one version of the ABI to work with runtimes built
//! against another version (within the same ABI major version).
//!
//! ## Tool Lifecycle
//!
//! Every tool library must implement the [`ToolModule`] interface, which
//! defines three lifecycle operations:
//!
//! 1. **Initialization**: `init()` is called once when the library is loaded
//! 2. **Invocation**: `call()` is called for each tool execution request
//! 3. **Shutdown**: `shutdown()` is called when the library is unloaded
//!
//! ## Thread Safety
//!
//! - The runtime guarantees that `init()` and `shutdown()` are called from a
//!   single thread
//! - Multiple `call()` invocations may occur concurrently on different threads
//! - Tool implementations must ensure their `call()` function is thread-safe
//!
//! ## ABI Versioning
//!
//! The current ABI version is defined by [`TOOL_ABI_VERSION`). All tool
//! libraries must export a [`ToolMeta`] struct with the matching `abi_version`
//! to be loaded.
//!
//! ## Example
//!
//! ```no_run
//! # #![allow(dead_code)]
//! use abi_stable::std_types::RVec;
//! use async_ffi::FfiFuture;
//! use operai_abi::*;
//!
//! pub extern "C" fn init(args: InitArgs) -> FfiFuture<ToolResult> {
//!     // Initialize tool resources
//!     FfiFuture::new(async { ToolResult::Ok })
//! }
//!
//! pub extern "C" fn call(args: CallArgs<'_>) -> FfiFuture<CallResult> {
//!     // Handle tool invocation
//!     FfiFuture::new(async { CallResult::ok(RVec::from_slice(b"result")) })
//! }
//!
//! pub extern "C" fn shutdown() {
//!     // Cleanup resources
//! }
//! ```

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

/// Current ABI version for Operai tools.
///
/// Tool libraries must export a [`ToolMeta`] struct with this `abi_version`
/// to be compatible with the current runtime. Mismatches will result in
/// load failures with [`ToolResult::AbiMismatch`].
pub const TOOL_ABI_VERSION: u32 = 1;

/// Result codes for tool operations.
///
/// These codes are used across the ABI to indicate success or failure of
/// initialization, calls, and other operations. The enum is marked as
/// `#[non_exhaustive]` to allow adding new error codes in future ABI versions
/// without breaking compatibility.
///
/// # Representation
///
/// The enum uses `#[repr(u8)]` for a stable wire format, ensuring it can be
/// safely passed across FFI boundaries. Each variant has an explicit
/// discriminant to maintain binary compatibility across ABI versions.
#[repr(u8)]
#[derive(StableAbi, Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ToolResult {
    /// Operation completed successfully.
    Ok = 0,
    /// General error (use specific error codes when available).
    Error = 1,
    /// Tool or resource not found.
    NotFound = 2,
    /// Input validation failed.
    InvalidInput = 3,
    /// ABI version mismatch between tool and runtime.
    AbiMismatch = 4,
    /// Tool initialization failed.
    InitFailed = 5,
    /// Credential validation or retrieval failed.
    CredentialError = 6,
}

/// Metadata about a tool library.
///
/// This struct is exported by every tool library and contains version
/// information used by the runtime to verify compatibility before loading the
/// library.
#[repr(C)]
#[derive(StableAbi, Debug, Clone, Copy)]
pub struct ToolMeta {
    /// ABI version this library was built against.
    /// Must match [`TOOL_ABI_VERSION`] for successful loading.
    pub abi_version: u32,
    /// Crate name of the tool library (e.g., "my-tool").
    pub crate_name: RStr<'static>,
    /// SemVer version of the tool library (e.g., "1.2.3").
    pub crate_version: RStr<'static>,
}

impl ToolMeta {
    /// Creates a new `ToolMeta` instance.
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

/// Describes a tool's interface and capabilities.
///
/// Each tool in a library must have a corresponding `ToolDescriptor` that
/// describes its inputs, outputs, and metadata. This information is used by the
/// runtime for validation, discovery, and tool selection.
#[repr(C)]
#[derive(StableAbi, Debug, Clone)]
pub struct ToolDescriptor {
    /// Unique identifier for this tool (e.g., "greet").
    /// Combined with the crate name to form a qualified ID:
    /// `{crate_name}.{id}`.
    pub id: RStr<'static>,
    /// Human-readable name of the tool.
    pub name: RStr<'static>,
    /// Description of what the tool does.
    pub description: RStr<'static>,
    /// JSON Schema describing the tool's input format.
    pub input_schema: RStr<'static>,
    /// JSON Schema describing the tool's output format.
    pub output_schema: RStr<'static>,
    /// Optional JSON Schema for credential validation.
    /// If `None`, the tool does not require credentials.
    pub credential_schema: ROption<RStr<'static>>,
    /// List of capabilities the tool provides (e.g., "read", "write").
    pub capabilities: RSlice<'static, RStr<'static>>,
    /// Tags for categorization and discovery (e.g., "utility", "ai").
    pub tags: RSlice<'static, RStr<'static>>,
    /// Embedding vector for semantic search and tool matching.
    /// Empty slice if no embedding is available.
    pub embedding: RSlice<'static, f32>,
}

/// Context provided during tool initialization.
///
/// This struct is reserved for future use. Currently, it contains no fields
/// but maintains a non-zero size for ABI stability.
#[repr(C)]
#[derive(StableAbi, Debug, Clone, Copy, Default)]
pub struct RuntimeContext {
    /// Reserved for future use.
    reserved: u8,
}

impl RuntimeContext {
    /// Creates a new `RuntimeContext`.
    #[must_use]
    pub const fn new() -> Self {
        Self { reserved: 0 }
    }
}

/// Context passed to each tool invocation.
///
/// Contains request metadata and credentials for a single tool call.
/// The same context is provided to all tools invoked during a single request.
#[repr(C)]
#[derive(StableAbi, Debug, Clone, Copy)]
pub struct CallContext<'a> {
    /// Unique identifier for this request (for tracing/logging).
    pub request_id: RStr<'a>,
    /// Session identifier for grouping related requests.
    pub session_id: RStr<'a>,
    /// User-specific credentials as serialized bytes.
    /// Format: binary-serialized `HashMap<String, HashMap<String, String>>`.
    pub user_credentials: RSlice<'a, u8>,
    /// System credentials for this tool as serialized bytes.
    /// Format: binary-serialized credentials specific to the tool.
    pub system_credentials: RSlice<'a, u8>,
}

/// Result returned by a tool invocation.
///
/// Contains both the status code and output data. The output field contains
/// the serialized response data on success, or an error message on failure.
#[repr(C)]
#[derive(StableAbi, Debug, Clone)]
pub struct CallResult {
    /// Result code indicating success or failure type.
    pub result: ToolResult,
    /// Output data or error message as bytes.
    /// On success: serialized output according to the tool's output schema.
    /// On error: UTF-8 encoded error message.
    pub output: RVec<u8>,
}

impl CallResult {
    /// Creates a successful result with output data.
    #[must_use]
    pub fn ok(output: RVec<u8>) -> Self {
        Self {
            result: ToolResult::Ok,
            output,
        }
    }

    /// Creates an error result with a message.
    ///
    /// The message will be UTF-8 encoded into the output field.
    /// The `result` parameter should be a specific error code (not
    /// `ToolResult::Ok`).
    #[must_use]
    pub fn error(result: ToolResult, message: &str) -> Self {
        Self {
            result,
            output: RVec::from_slice(message.as_bytes()),
        }
    }
}

/// Arguments passed to the tool initialization function.
#[repr(C)]
#[derive(StableAbi, Clone, Copy)]
pub struct InitArgs {
    /// Runtime context (currently empty, reserved for future use).
    pub ctx: RuntimeContext,
}

impl InitArgs {
    /// Creates new initialization arguments.
    #[must_use]
    pub const fn new(ctx: RuntimeContext) -> Self {
        Self { ctx }
    }
}

/// Arguments passed to each tool invocation.
///
/// Contains the call context, tool identifier, and input data.
/// The tool ID allows a single library to export multiple tools.
#[repr(C)]
#[derive(StableAbi)]
pub struct CallArgs<'a> {
    /// Request context with metadata and credentials.
    pub context: CallContext<'a>,
    /// Identifier of the tool being invoked (matches a `ToolDescriptor::id`).
    pub tool_id: RStr<'a>,
    /// Input data as serialized bytes (according to the tool's input schema).
    pub input: RSlice<'a, u8>,
}

impl<'a> CallArgs<'a> {
    /// Creates new call arguments.
    #[must_use]
    pub const fn new(context: CallContext<'a>, tool_id: RStr<'a>, input: RSlice<'a, u8>) -> Self {
        Self {
            context,
            tool_id,
            input,
        }
    }
}

/// Function pointer type for tool initialization.
///
/// Called once when the library is loaded. Must return `ToolResult::Ok` for
/// successful initialization. Any other return value will prevent the library
/// from being used.
pub type ToolInitFn = extern "C" fn(args: InitArgs) -> FfiFuture<ToolResult>;

/// Function pointer type for tool invocation.
///
/// Called for each tool execution request. The function must deserialize
/// the input, process the request, and return a `CallResult`.
pub type ToolCallFn = extern "C" fn(args: CallArgs<'_>) -> FfiFuture<CallResult>;

/// Function pointer type for tool cleanup.
///
/// Called when the library is being unloaded. This function should release
/// any resources acquired during initialization.
pub type ToolShutdownFn = extern "C" fn();

/// Root module exported by tool libraries.
///
/// Every Operai tool library must export a `ToolModule` as its root module.
/// This struct contains metadata, descriptors for all tools in the library,
/// and function pointers for the lifecycle operations.
///
/// The struct uses `abi_stable` attributes to enable versioning:
/// - `#[sabi(kind(Prefix))]`: Allows adding new fields in future versions
/// - `#[sabi(missing_field(panic))]`: Panics if a required field is missing
/// - `#[sabi(unsafe_opaque_field)]`: Marks function pointers as opaque
/// - `#[sabi(last_prefix_field)]`: Marks the last field that can be added in
///   the current version
#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix(prefix_ref = ToolModuleRef)))]
#[sabi(missing_field(panic))]
pub struct ToolModule {
    /// Metadata about the library.
    pub meta: ToolMeta,

    /// Descriptors for all tools exported by this library.
    pub descriptors: RSlice<'static, ToolDescriptor>,

    /// Initialization function pointer.
    #[sabi(unsafe_opaque_field)]
    pub init: ToolInitFn,

    /// Tool invocation function pointer.
    #[sabi(unsafe_opaque_field)]
    pub call: ToolCallFn,

    /// Cleanup function pointer.
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
    /// Returns an iterator over all tool descriptors in this module.
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
            user_credentials: RSlice::from_slice(&[1, 2, 3]),
            system_credentials: RSlice::from_slice(&[4, 5, 6]),
        };
        let tool_id = RStr::from("greet");
        let input = RSlice::from_slice(b"{\"name\":\"world\"}");

        let args = CallArgs::new(context, tool_id, input);

        assert_eq!(args.context.request_id.as_str(), "req-123");
        assert_eq!(args.context.session_id.as_str(), "sess-456");
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
            user_credentials: RSlice::from_slice(&[1, 2, 3]),
            system_credentials: RSlice::from_slice(&[4, 5, 6]),
        };
        let copied = original;

        // Both should be usable after copy (not moved)
        assert_eq!(original.request_id.as_str(), copied.request_id.as_str());
        assert_eq!(original.session_id.as_str(), copied.session_id.as_str());
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
            user_credentials: RSlice::from_slice(&[]),
            system_credentials: RSlice::from_slice(&[]),
        };

        assert!(context.request_id.as_str().is_empty());
        assert!(context.session_id.as_str().is_empty());
        assert!(context.user_credentials.as_slice().is_empty());
        assert!(context.system_credentials.as_slice().is_empty());
    }

    #[test]
    fn test_call_args_with_empty_input() {
        let context = CallContext {
            request_id: RStr::from("req"),
            session_id: RStr::from("sess"),
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
