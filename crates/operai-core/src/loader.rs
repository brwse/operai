//! Dynamic library loading for tool crates.

use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use abi_stable::library::RootModule;
use operai_abi::{InitArgs, RuntimeContext, TOOL_ABI_VERSION, ToolModuleRef, ToolResult};
use tracing::{debug, error, info};

/// Errors that can occur when loading a tool library.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    /// Failed to load the dynamic library.
    #[error("failed to load library: {0}")]
    LibraryLoad(String),

    /// ABI version mismatch.
    #[error("ABI version mismatch: expected {expected}, got {actual}")]
    AbiMismatch { expected: u32, actual: u32 },

    /// Tool initialization failed.
    #[error("tool initialization failed")]
    InitFailed,

    /// Invalid path.
    #[error("invalid path: {0}")]
    InvalidPath(String),

    /// Checksum mismatch.
    #[error("checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
}

/// A loaded tool library.
///
/// This struct owns the loaded library and provides access to its module.
/// The library is unloaded when this struct is dropped.
pub struct ToolLibrary {
    /// Managed by `abi_stable` - automatically handles library unloading.
    module: ToolModuleRef,
    path: String,
    shutdown_called: AtomicBool,
}

impl ToolLibrary {
    /// Loads a tool library from the specified path.
    ///
    /// This function uses `abi_stable`'s `RootModule` loading which provides
    /// automatic ABI version checking and safe type handling.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The library cannot be loaded
    /// - The checksum does not match
    /// - The ABI version doesn't match
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

    /// Initializes the tool library with the runtime context.
    ///
    /// Must be called before invoking any tools.
    ///
    /// # Errors
    ///
    /// Returns an error if initialization fails.
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

    /// Shuts down the tool library.
    ///
    /// Should be called before unloading the library.
    pub fn shutdown(&self) {
        if self.shutdown_called.swap(true, Ordering::SeqCst) {
            return;
        }

        debug!(path = %self.path, "Shutting down tool library");
        let shutdown_fn = self.module.shutdown();
        shutdown_fn();
    }

    #[must_use]
    pub fn module(&self) -> ToolModuleRef {
        self.module
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn crate_name(&self) -> &str {
        self.module.meta().crate_name.as_str()
    }

    #[must_use]
    pub fn crate_version(&self) -> &str {
        self.module.meta().crate_version.as_str()
    }
}

impl Drop for ToolLibrary {
    fn drop(&mut self) {
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
