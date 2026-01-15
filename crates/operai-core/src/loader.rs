//! Dynamic loader for Operai tool libraries.
//!
//! This module provides functionality for dynamically loading tool libraries from
//! shared object files (.so, .dll, .dylib) using the Operai ABI. It handles:
//!
//! - Library loading with optional SHA256 checksum verification
//! - ABI version compatibility checking
//! - Tool library initialization and shutdown lifecycle management
//! - Safe cleanup via Drop implementation
//!
//! # Tool Library Lifecycle
//!
//! Each loaded library follows this lifecycle:
//!
//! 1. **Load**: The shared object file is loaded from disk via [`ToolLibrary::load`]
//! 2. **Verify**: Optional checksum verification ensures integrity
//! 3. **Validate**: ABI version is checked for compatibility
//! 4. **Initialize**: [`ToolLibrary::init`] calls the tool's init function
//! 5. **Use**: Tool functions can be invoked via the module reference
//! 6. **Shutdown**: [`ToolLibrary::shutdown`] or Drop cleanup calls the tool's shutdown function
//!
//! # Thread Safety
//!
//! - [`ToolLibrary`] is `Send` and `Sync`, allowing safe cross-thread usage
//! - Shutdown is idempotent and uses atomic operations for thread safety
//! - The underlying ABI may allow concurrent tool calls (see [`operai_abi`])

use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use abi_stable::library::RootModule;
use operai_abi::{InitArgs, RuntimeContext, TOOL_ABI_VERSION, ToolModuleRef, ToolResult};
use tracing::{debug, error, info};

/// Errors that can occur during tool library loading.
///
/// This enum represents all failure modes that can occur when loading,
/// validating, and initializing a tool library from a shared object file.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// The shared object file could not be loaded.
    ///
    /// This can occur due to:
    /// - File not found
    /// - Missing system dependencies
    /// - Incompatible binary format
    /// - Permission issues
    #[error("failed to load library: {0}")]
    LibraryLoad(String),

    /// The tool library's ABI version does not match the runtime's expected version.
    ///
    /// This occurs when the library was compiled against a different version of the
    /// Operai ABI. The library must be recompiled with a compatible version.
    #[error("ABI version mismatch: expected {expected}, got {actual}")]
    AbiMismatch { expected: u32, actual: u32 },

    /// The tool library's initialization function returned an error.
    ///
    /// This occurs when the tool's `init` function returns anything other than
    /// [`ToolResult::Ok`], indicating the tool failed to initialize its resources.
    #[error("tool initialization failed")]
    InitFailed,

    /// The provided path is not valid UTF-8.
    ///
    /// The loader requires valid UTF-8 paths for logging and error reporting.
    #[error("invalid path: {0}")]
    InvalidPath(String),

    /// SHA256 checksum verification failed.
    ///
    /// This occurs when the computed checksum of the library file does not match
    /// the expected checksum, indicating the file may be corrupted or tampered with.
    #[error("checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
}

/// A dynamically loaded tool library.
///
/// This type manages the lifecycle of a loaded tool library, including initialization,
/// shutdown, and safe cleanup. It provides access to the underlying [`ToolModuleRef`]
/// for invoking tool functions.
///
/// # Safety and Cleanup
///
/// - Shutdown is idempotent: calling [`ToolLibrary::shutdown`] multiple times is safe
/// - [`Drop`] implementation ensures cleanup even if shutdown is not explicitly called
/// - Uses atomic operations for thread-safe shutdown state tracking
///
/// # Example
///
/// ```no_run
/// # use operai_core::ToolLibrary;
/// # use operai_abi::RuntimeContext;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let library = ToolLibrary::load("./tools/my_tool.so", None)?;
///
/// let ctx = RuntimeContext::new();
/// library.init(&ctx).await?;
///
/// // Use the library...
/// let module = library.module();
/// // ... invoke tool functions via module ...
///
/// // Explicit shutdown (optional, Drop will also handle it)
/// library.shutdown();
/// # Ok(())
/// # }
/// ```
pub struct ToolLibrary {
    module: ToolModuleRef,
    path: String,
    shutdown_called: AtomicBool,
}

impl ToolLibrary {
    /// Loads a tool library from a shared object file.
    ///
    /// This function performs the following steps:
    ///
    /// 1. Validates the path is valid UTF-8
    /// 2. Optionally verifies SHA256 checksum if provided
    /// 3. Loads the shared object file using `abi_stable`
    /// 4. Validates ABI version compatibility
    /// 5. Returns a [`ToolLibrary` instance if all checks pass
    ///
    /// # Parameters
    ///
    /// - `path`: Path to the shared object file (.so, .dll, .dylib)
    /// - `checksum`: Optional SHA256 checksum to verify library integrity.
    ///   If provided, the file's SHA256 digest must match exactly.
    ///
    /// # Errors
    ///
    /// Returns [`LoadError`] if:
    /// - Path is not valid UTF-8
    /// - File cannot be read or loaded
    /// - Checksum verification fails
    /// - ABI version mismatch is detected
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use operai_core::ToolLibrary;
    /// let library = ToolLibrary::load("./tools/tool.so", Some("abc123..."))
    ///     .expect("Failed to load library");
    /// ```
    pub fn load(path: impl AsRef<Path>, checksum: Option<&str>) -> Result<Self, LoadError> {
        let path = path.as_ref();
        let path_str = path
            .to_str()
            .ok_or_else(|| LoadError::InvalidPath(path.display().to_string()))?;

        info!(path = %path_str, "Loading tool library");

        if let Some(expected_checksum) = checksum {
            let file_bytes =
                std::fs::read(path).map_err(|e| LoadError::LibraryLoad(e.to_string()))?;
            let digest = sha256::digest(&file_bytes[..]);
            if digest != expected_checksum {
                return Err(LoadError::ChecksumMismatch {
                    expected: expected_checksum.to_string(),
                    actual: digest,
                });
            }
            debug!(path = %path_str, checksum = %digest, "Checksum verified");
        }

        let module = ToolModuleRef::load_from_file(path)
            .map_err(|e| LoadError::LibraryLoad(e.to_string()))?;

        let meta = module.meta();
        if meta.abi_version != TOOL_ABI_VERSION {
            return Err(LoadError::AbiMismatch {
                expected: TOOL_ABI_VERSION,
                actual: meta.abi_version,
            });
        }

        let crate_name = meta.crate_name.as_str();
        let crate_version = meta.crate_version.as_str();
        let tool_count = module.descriptors().len();

        debug!(
            crate_name = %crate_name,
            crate_version = %crate_version,
            tool_count = tool_count,
            "Library loaded successfully"
        );

        Ok(Self {
            module,
            path: path_str.to_string(),
            shutdown_called: AtomicBool::new(false),
        })
    }

    /// Initializes the tool library with the provided runtime context.
    ///
    /// This function calls the tool library's `init` function, passing the runtime
    /// context that contains configuration and resources needed by the tool.
    ///
    /// # Parameters
    ///
    /// - `ctx`: Runtime context containing configuration and state for the tool
    ///
    /// # Errors
    ///
    /// Returns [`LoadError::InitFailed`] if the tool's init function returns
    /// anything other than [`ToolResult::Ok`].
    pub async fn init(&self, ctx: &RuntimeContext) -> Result<(), LoadError> {
        debug!(path = %self.path, "Initializing tool library");

        let args = InitArgs::new(*ctx);
        let init_fn = self.module.init();
        let result = init_fn(args).await;

        if result == ToolResult::Ok {
            info!(path = %self.path, "Tool library initialized");
            Ok(())
        } else {
            error!(path = %self.path, result = ?result, "Tool initialization failed");
            Err(LoadError::InitFailed)
        }
    }

    /// Shuts down the tool library, releasing its resources.
    ///
    /// This function calls the tool library's `shutdown` function to allow it to
    /// release resources and perform cleanup operations.
    ///
    /// # Idempotency
    ///
    /// This function is idempotent: calling it multiple times is safe and will
    /// only invoke the underlying shutdown function once. Subsequent calls will
    /// return immediately without doing anything.
    ///
    /// # Thread Safety
    ///
    /// Uses atomic operations to ensure thread-safe shutdown even when called
    /// from multiple threads concurrently.
    pub fn shutdown(&self) {
        if self.shutdown_called.swap(true, Ordering::SeqCst) {
            return;
        }

        debug!(path = %self.path, "Shutting down tool library");
        let shutdown_fn = self.module.shutdown();
        shutdown_fn();
    }

    /// Returns a reference to the underlying tool module.
    ///
    /// This provides access to the [`ToolModuleRef`] which can be used to:
    /// - Get tool descriptors via [`ToolModuleRef::descriptors()`]
    /// - Invoke tool functions via [`ToolModuleRef::call()`]
    /// - Access metadata via [`ToolModuleRef::meta()`]
    #[must_use]
    pub fn module(&self) -> ToolModuleRef {
        self.module
    }

    /// Returns the path to the loaded library file.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the crate name of the loaded library.
    #[must_use]
    pub fn crate_name(&self) -> &str {
        self.module.meta().crate_name.as_str()
    }

    /// Returns the version of the loaded library.
    #[must_use]
    pub fn crate_version(&self) -> &str {
        self.module.meta().crate_version.as_str()
    }
}

impl Drop for ToolLibrary {
    fn drop(&mut self) {
        // Automatically call shutdown when the library is dropped.
        // This ensures cleanup happens even if shutdown() was not explicitly called.
        // The shutdown() function is idempotent, so this is safe even if
        // shutdown() was already called manually.
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use abi_stable::{
        prefix_type::{PrefixRefTrait, WithMetadata},
        std_types::{RSlice, RStr},
    };
    use async_ffi::FfiFuture;
    use operai_abi::{CallArgs, CallResult, ToolMeta, ToolModule};

    use super::*;

    extern "C" fn dummy_call(_args: CallArgs<'_>) -> FfiFuture<CallResult> {
        FfiFuture::new(async { CallResult::error(ToolResult::NotFound, "not used") })
    }

    fn test_tool_module_ref(
        abi_version: u32,
        crate_name: &'static str,
        crate_version: &'static str,
        init: operai_abi::ToolInitFn,
        shutdown: operai_abi::ToolShutdownFn,
    ) -> ToolModuleRef {
        let module = ToolModule {
            meta: ToolMeta::new(
                abi_version,
                RStr::from_str(crate_name),
                RStr::from_str(crate_version),
            ),
            descriptors: RSlice::from_slice(&[]),
            init,
            call: dummy_call,
            shutdown,
        };

        let with_metadata: &'static WithMetadata<ToolModule> =
            Box::leak(Box::new(WithMetadata::new(module)));
        ToolModuleRef::from_prefix_ref(with_metadata.static_as_prefix())
    }

    #[test]
    fn test_load_error_display_messages() {
        // Arrange
        let library_load = LoadError::LibraryLoad("boom".to_string());
        let abi_mismatch = LoadError::AbiMismatch {
            expected: 1,
            actual: 2,
        };
        let init_failed = LoadError::InitFailed;
        let invalid_path = LoadError::InvalidPath("bad-path".to_string());

        // Act & Assert
        assert_eq!(library_load.to_string(), "failed to load library: boom");
        assert_eq!(
            abi_mismatch.to_string(),
            "ABI version mismatch: expected 1, got 2"
        );
        assert_eq!(init_failed.to_string(), "tool initialization failed");
        assert_eq!(invalid_path.to_string(), "invalid path: bad-path");
    }

    #[test]
    fn test_load_error_debug_includes_variant_and_values() {
        let abi_mismatch = LoadError::AbiMismatch {
            expected: 1,
            actual: 2,
        };

        let debug_str = format!("{abi_mismatch:?}");

        assert!(debug_str.contains("AbiMismatch"));
        assert!(debug_str.contains("expected"));
        assert!(debug_str.contains("actual"));
    }

    #[test]
    fn test_load_nonexistent_file_returns_library_load_error() {
        let result = ToolLibrary::load("/nonexistent/path/to/library.so", None);

        let Err(err) = result else {
            panic!("expected loading nonexistent file to fail");
        };
        assert!(matches!(err, LoadError::LibraryLoad(_)));
    }

    #[tokio::test]
    async fn test_loaded_library_init_when_tool_init_returns_ok_returns_ok() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Arrange - use test-local static to avoid interference
        static INIT_OK_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

        extern "C" fn init_ok(_args: InitArgs) -> FfiFuture<ToolResult> {
            INIT_OK_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
            FfiFuture::new(async { ToolResult::Ok })
        }

        extern "C" fn shutdown_noop() {}

        let initial_count = INIT_OK_CALL_COUNT.load(Ordering::SeqCst);

        let module = test_tool_module_ref(
            TOOL_ABI_VERSION,
            "test-crate",
            "0.0.0",
            init_ok,
            shutdown_noop,
        );
        let library = ToolLibrary {
            module,
            path: "test-path".to_string(),
            shutdown_called: AtomicBool::new(false),
        };
        let runtime_ctx = RuntimeContext::new();

        // Act
        let result = library.init(&runtime_ctx).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(INIT_OK_CALL_COUNT.load(Ordering::SeqCst), initial_count + 1);
        assert_eq!(library.path(), "test-path");
        assert_eq!(library.crate_name(), "test-crate");
        assert_eq!(library.crate_version(), "0.0.0");
    }

    #[tokio::test]
    async fn test_loaded_library_init_when_tool_init_returns_error_returns_init_failed() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Arrange - use unique static name to avoid interference
        static INIT_ERROR_CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

        extern "C" fn init_error(_args: InitArgs) -> FfiFuture<ToolResult> {
            INIT_ERROR_CALL_COUNT.fetch_add(1, Ordering::SeqCst);
            FfiFuture::new(async { ToolResult::Error })
        }

        extern "C" fn shutdown_noop() {}

        let initial_count = INIT_ERROR_CALL_COUNT.load(Ordering::SeqCst);

        let module = test_tool_module_ref(
            TOOL_ABI_VERSION,
            "test-crate",
            "0.0.0",
            init_error,
            shutdown_noop,
        );
        let library = ToolLibrary {
            module,
            path: "test-path".to_string(),
            shutdown_called: AtomicBool::new(false),
        };
        let runtime_ctx = RuntimeContext::new();

        // Act
        let result = library.init(&runtime_ctx).await;

        // Assert
        let err = match result {
            Ok(()) => panic!("expected init error to return an error"),
            Err(err) => err,
        };
        assert!(matches!(err, LoadError::InitFailed));
        assert_eq!(
            INIT_ERROR_CALL_COUNT.load(Ordering::SeqCst),
            initial_count + 1
        );
    }

    #[test]
    fn test_loaded_library_shutdown_is_idempotent_and_drop_does_not_double_call() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Arrange - unique static to avoid interference with other shutdown tests
        static SHUTDOWN_IDEMPOTENT_COUNT: AtomicUsize = AtomicUsize::new(0);

        extern "C" fn init_ok(_args: InitArgs) -> FfiFuture<ToolResult> {
            FfiFuture::new(async { ToolResult::Ok })
        }

        extern "C" fn shutdown_counting() {
            SHUTDOWN_IDEMPOTENT_COUNT.fetch_add(1, Ordering::SeqCst);
        }

        let initial_count = SHUTDOWN_IDEMPOTENT_COUNT.load(Ordering::SeqCst);

        let module = test_tool_module_ref(
            TOOL_ABI_VERSION,
            "test-crate",
            "0.0.0",
            init_ok,
            shutdown_counting,
        );

        // Act
        {
            let library = ToolLibrary {
                module,
                path: "test-path".to_string(),
                shutdown_called: AtomicBool::new(false),
            };
            library.shutdown();
            library.shutdown();
        }

        // Assert - only one call despite calling shutdown() twice plus drop
        assert_eq!(
            SHUTDOWN_IDEMPOTENT_COUNT.load(Ordering::SeqCst),
            initial_count + 1
        );
    }

    #[test]
    fn test_loaded_library_drop_invokes_shutdown_if_not_called_explicitly() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Arrange - unique static to avoid interference with other shutdown tests
        static SHUTDOWN_DROP_COUNT: AtomicUsize = AtomicUsize::new(0);

        extern "C" fn init_ok(_args: InitArgs) -> FfiFuture<ToolResult> {
            FfiFuture::new(async { ToolResult::Ok })
        }

        extern "C" fn shutdown_counting() {
            SHUTDOWN_DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }

        let initial_count = SHUTDOWN_DROP_COUNT.load(Ordering::SeqCst);

        let module = test_tool_module_ref(
            TOOL_ABI_VERSION,
            "test-crate",
            "0.0.0",
            init_ok,
            shutdown_counting,
        );

        // Act - let drop handle shutdown
        {
            let _library = ToolLibrary {
                module,
                path: "test-path".to_string(),
                shutdown_called: AtomicBool::new(false),
            };
        }

        // Assert - drop called shutdown exactly once
        assert_eq!(
            SHUTDOWN_DROP_COUNT.load(Ordering::SeqCst),
            initial_count + 1
        );
    }

    #[test]
    fn test_loaded_library_module_returns_the_module_ref() {
        extern "C" fn init_ok(_args: InitArgs) -> FfiFuture<ToolResult> {
            FfiFuture::new(async { ToolResult::Ok })
        }

        extern "C" fn shutdown_noop() {}

        let module = test_tool_module_ref(
            TOOL_ABI_VERSION,
            "module-test-crate",
            "1.2.3",
            init_ok,
            shutdown_noop,
        );
        let library = ToolLibrary {
            module,
            path: "test-path".to_string(),
            shutdown_called: AtomicBool::new(false),
        };

        // Verify module() returns a working reference by checking metadata
        let returned_module = library.module();
        assert_eq!(
            returned_module.meta().crate_name.as_str(),
            "module-test-crate"
        );
        assert_eq!(returned_module.meta().crate_version.as_str(), "1.2.3");
    }

    #[test]
    fn test_load_error_is_send_and_sync() {
        // Errors should be thread-safe for use across async boundaries
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LoadError>();
    }

    #[test]
    fn test_load_error_implements_std_error() {
        // Verify LoadError implements std::error::Error
        fn assert_error<T: std::error::Error>() {}
        assert_error::<LoadError>();
    }
}
