//! Builder for constructing Operai runtime instances.
//!
//! This module provides [`RuntimeBuilder`], a fluent builder API for
//! configuring and constructing either local or remote runtime instances. The
//! builder handles:
//!
//! - **Project Config Loading**: Parsing tool configurations from `operai.toml`
//! - **Tool Registration**: Loading dynamic tool libraries and static tool
//!   modules
//! - **Policy Setup**: Resolving and registering policy enforcement rules
//! - **Runtime Mode**: Choosing between local execution or remote gRPC
//!   connections
//!
//! # Usage
//!
//! ```no_run
//! use operai_runtime::RuntimeBuilder;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Build a local runtime with custom project config path
//! let runtime = RuntimeBuilder::new()
//!     .with_config_path("custom/operai.toml")
//!     .build_local()
//!     .await?;
//!
//! // Build a remote runtime
//! let remote = RuntimeBuilder::new()
//!     .remote("localhost:50051")
//!     .build_remote()
//!     .await?;
//! # Ok(())
//! # }
//! ```

use std::{
    fmt,
    path::{Path, PathBuf},
    sync::Arc,
};

use operai_abi::RuntimeContext;
#[cfg(feature = "static-link")]
use operai_abi::ToolModuleRef;
use operai_core::{
    Config, ToolRegistry,
    policy::session::{InMemoryPolicySessionStore, PolicyStore},
};
use tracing::{error, info, warn};

use crate::runtime::{LocalRuntime, RemoteRuntime, Runtime};

/// Errors that can occur during runtime construction.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeBuildError {
    /// Failed to load or parse the config file.
    #[error("failed to load config: {0}")]
    Config(#[from] operai_core::ConfigError),

    /// Attempted to build a remote runtime without configuring an endpoint.
    #[error("remote endpoint is required")]
    MissingRemoteEndpoint,

    /// Failed to establish a connection to a remote runtime.
    #[error("failed to connect to remote runtime: {0}")]
    Transport(#[from] tonic::transport::Error),
}

#[derive(Debug, Clone)]
enum RuntimeMode {
    Local,
    Remote { endpoint: String },
}

/// Fluent builder for constructing [`Runtime`] instances.
///
/// `RuntimeBuilder` provides a configurable way to set up either local or
/// remote runtime instances. It supports:
///
/// - Custom config paths for tool configuration
/// - Runtime context injection for environment-specific settings
/// - Local tool library loading from dynamic libraries
/// - Static tool module registration (when `static-link` feature is enabled)
/// - Remote runtime connections via gRPC
///
/// # Default Configuration
///
/// - Config path: `operai.toml` in the current directory
/// - Runtime mode: Local execution
/// - Static tools: None (empty list)
///
/// # Example
///
/// ```no_run
/// # use operai_runtime::RuntimeBuilder;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let runtime = RuntimeBuilder::new()
///     .with_config_path("tools/operai.toml")
///     .local()
///     .build()
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct RuntimeBuilder {
    config_path: PathBuf,
    runtime_ctx: RuntimeContext,
    mode: RuntimeMode,
    #[cfg(feature = "static-link")]
    static_tools: Vec<ToolModuleRef>,
}

impl fmt::Debug for RuntimeBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug_struct = f.debug_struct("RuntimeBuilder");
        debug_struct
            .field("config_path", &self.config_path)
            .field("runtime_ctx", &self.runtime_ctx)
            .field("mode", &self.mode);
        #[cfg(feature = "static-link")]
        {
            debug_struct.field("static_tools", &self.static_tools.len());
        }
        debug_struct.finish()
    }
}

impl RuntimeBuilder {
    /// Creates a new builder with default configuration.
    ///
    /// # Default Values
    ///
    /// - Project config path: `operai.toml` (uses unified resolution if not
    ///   found)
    /// - Runtime context: Empty (newly created)
    /// - Mode: Local execution
    #[must_use]
    pub fn new() -> Self {
        Self {
            config_path: PathBuf::from("operai.toml"),
            runtime_ctx: RuntimeContext::new(),
            mode: RuntimeMode::Local,
            #[cfg(feature = "static-link")]
            static_tools: Vec::new(),
        }
    }

    /// Sets the path to the project config file (`operai.toml`).
    ///
    /// The project config file defines which tools should be loaded and any
    /// policies that should be enforced. If not set, the builder will use
    /// the unified resolution algorithm to find `operai.toml` in the
    /// current directory or any parent directories.
    ///
    /// # Parameters
    ///
    /// - `path`: Path to the project config file (relative or absolute)
    #[must_use]
    pub fn with_config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = path.into();
        self
    }

    /// Sets the runtime context for tool execution.
    ///
    /// The runtime context provides environment-specific configuration and
    /// capabilities that tools may access during execution.
    ///
    /// # Parameters
    ///
    /// - `ctx`: The runtime context to use
    #[must_use]
    pub fn with_runtime_context(mut self, ctx: RuntimeContext) -> Self {
        self.runtime_ctx = ctx;
        self
    }

    /// Builds a [`LocalRuntime`] instance.
    ///
    /// This method loads tools from the configured config, initializes
    /// the policy store, and constructs a local runtime that executes tools
    /// in-process. Tool loading failures are logged but do not prevent the
    /// runtime from being built.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeBuildError::Config`] if the config file cannot be
    /// loaded or parsed.
    pub async fn build_local(self) -> Result<LocalRuntime, RuntimeBuildError> {
        build_local_runtime(self).await
    }

    /// Builds a [`RemoteRuntime`] connected to a gRPC endpoint.
    ///
    /// The builder must be configured for remote mode using
    /// [`remote`](Self::remote) before calling this method. Tool execution
    /// is delegated to the remote server.
    ///
    /// # Errors
    ///
    /// - [`RuntimeBuildError::MissingRemoteEndpoint`] if the builder is in
    ///   local mode
    /// - [`RuntimeBuildError::Transport`] if the connection fails
    pub async fn build_remote(self) -> Result<RemoteRuntime, RuntimeBuildError> {
        let endpoint = match self.mode {
            RuntimeMode::Remote { endpoint } => endpoint,
            RuntimeMode::Local => return Err(RuntimeBuildError::MissingRemoteEndpoint),
        };

        let endpoint = normalize_endpoint(&endpoint);
        Ok(RemoteRuntime::connect(endpoint).await?)
    }

    /// Builds a [`Runtime`] enum based on the configured mode.
    ///
    /// This is a convenience method that returns either a local or remote
    /// runtime based on the builder's current mode configuration.
    ///
    /// # Errors
    ///
    /// - [`RuntimeBuildError::Config`] if loading the config fails (local mode)
    /// - [`RuntimeBuildError::MissingRemoteEndpoint`] if remote mode is not
    ///   configured
    /// - [`RuntimeBuildError::Transport`] if the remote connection fails
    pub async fn build(self) -> Result<Runtime, RuntimeBuildError> {
        match self.mode.clone() {
            RuntimeMode::Local => Ok(Runtime::Local(build_local_runtime(self).await?)),
            RuntimeMode::Remote { endpoint } => {
                let endpoint = normalize_endpoint(&endpoint);
                let runtime = RemoteRuntime::connect(endpoint).await?;
                Ok(Runtime::Remote(runtime))
            }
        }
    }

    /// Configures the builder for local execution mode.
    ///
    /// Tools will be loaded and executed in the current process. This is the
    /// default mode.
    #[must_use]
    pub fn local(mut self) -> Self {
        self.mode = RuntimeMode::Local;
        self
    }

    /// Configures the builder for remote execution mode.
    ///
    /// Tool execution will be delegated to a remote gRPC server. The endpoint
    /// can be specified with or without the `http://` prefix.
    ///
    /// # Parameters
    ///
    /// - `endpoint`: Remote server address (e.g., `localhost:50051` or `http://localhost:50051`)
    #[must_use]
    pub fn remote(mut self, endpoint: impl Into<String>) -> Self {
        self.mode = RuntimeMode::Remote {
            endpoint: endpoint.into(),
        };
        self
    }

    /// Registers a statically-linked tool module.
    ///
    /// This method is only available when the `static-link` feature is enabled.
    /// Static tools are compiled directly into the binary rather than loaded
    /// from dynamic libraries.
    ///
    /// # Parameters
    ///
    /// - `module`: Reference to a statically-linked tool module
    #[cfg(feature = "static-link")]
    #[must_use]
    pub fn with_static_tool(mut self, module: ToolModuleRef) -> Self {
        self.static_tools.push(module);
        self
    }
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds a local runtime from the builder configuration.
///
/// This internal function handles the core logic of constructing a local
/// runtime:
///
/// 1. Loads the project config from the configured path (or uses unified
///    resolution if not set)
/// 2. Creates a tool registry and loads all enabled tool libraries
/// 3. Registers any static tool modules
/// 4. Initializes the policy store and registers policies from the project
///    config
///
/// Tool loading failures are logged but do not prevent runtime construction.
/// Policy registration failures are similarly logged and skipped.
async fn build_local_runtime(builder: RuntimeBuilder) -> Result<LocalRuntime, RuntimeBuildError> {
    let config_path = builder.config_path;
    let runtime_ctx = builder.runtime_ctx;

    let config = load_config_or_empty(&config_path)?;
    let config_dir = config_path.parent().unwrap_or_else(|| Path::new("."));

    let mut registry = ToolRegistry::new();

    for tool_config in config.enabled_tools() {
        let Some(path) = resolve_tool_path(tool_config, config_dir) else {
            warn!("Tool config missing path and package, skipping.");
            continue;
        };

        info!(path = %path.display(), "Loading tool library");

        if let Err(e) = registry
            .load_library(
                &path,
                tool_config.checksum.as_deref(),
                Some(&tool_config.credentials),
                &runtime_ctx,
            )
            .await
        {
            error!(path = %path.display(), error = %e, "Failed to load tool library");
        } else {
            info!(path = %path.display(), "Loaded tool library");
        }
    }

    #[cfg(feature = "static-link")]
    for module in builder.static_tools {
        if let Err(e) = registry.register_module(module, None, &runtime_ctx).await {
            warn!(error = %e, "Failed to register static tool module");
        }
    }

    info!(tool_count = registry.len(), "Tool registry initialized");

    let registry = Arc::new(registry);

    let session_store = Arc::new(InMemoryPolicySessionStore::new());
    let policy_store = Arc::new(PolicyStore::new(session_store));

    match config.resolve_policies(&config_path) {
        Ok(policies) => {
            for policy in policies {
                let name = policy.name.clone();
                let version = policy.version.clone();
                match policy_store.register(policy) {
                    Ok(()) => {
                        info!(name = %name, version = %version, "Registered policy");
                    }
                    Err(e) => {
                        warn!(name = %name, error = %e, "Failed to register policy");
                    }
                }
            }
        }
        Err(e) => {
            warn!(error = %e, "Failed to resolve policies from config");
        }
    }

    Ok(LocalRuntime::with_context(
        registry,
        policy_store,
        runtime_ctx,
    ))
}

/// Loads a config from the given path, returning an empty config if not found.
///
/// This graceful degradation allows the runtime to start even without a config
/// file, useful for testing or purely static tool configurations.
///
/// # Parameters
///
/// - `config_path`: Path to the config file
///
/// # Returns
///
/// - `Ok(Config)` - The loaded config, or an empty one if the file doesn't
///   exist
fn load_config_or_empty(config_path: &Path) -> Result<Config, RuntimeBuildError> {
    if config_path.exists() {
        Ok(Config::load(config_path)?)
    } else {
        warn!(
            path = %config_path.display(),
            "Config file not found, starting with empty registry"
        );
        Ok(Config::empty())
    }
}

/// Resolves the filesystem path for a tool library.
///
/// This function implements the tool resolution strategy, which tries multiple
/// approaches in order:
///
/// 1. **Explicit path**: If `tool.path` is set, use it (absolute or relative to
///    config dir)
/// 2. **Name-based search**: If `tool.name` is set, search in standard
///    locations:
///    - `<config_dir>/target/release/`
///    - `/usr/local/lib/operai/`
///    - `<workspace_root>/target/release/` (if in a workspace)
///    - `~/.operai/tools/` (home directory)
///
/// # Parameters
///
/// - `tool`: Tool configuration containing either a path or name
/// - `config_dir`: Directory containing the config file (for resolving relative
///   paths)
///
/// # Returns
///
/// - `Some(PathBuf)` - Resolved path to the tool library
/// - `None` - No path or name specified in the tool config
fn resolve_tool_path(tool: &operai_core::ToolConfig, config_dir: &Path) -> Option<PathBuf> {
    // 1. Explicit path takes precedence
    if let Some(path) = &tool.path {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            return Some(path);
        }
        return Some(config_dir.join(path));
    }

    // 2. Name-based resolution - search in configured directories
    if let Some(name) = &tool.name {
        let lib_name = format!(
            "{}{}{}",
            std::env::consts::DLL_PREFIX,
            name.replace('-', "_"),
            std::env::consts::DLL_SUFFIX
        );

        // Collect all search paths as Option<PathBuf>, then filter and flatten
        let mut search_paths: Vec<Option<PathBuf>> = vec![
            // Standard target/release directory
            Some(config_dir.join("target/release")),
            // System-wide tools directory
            Some(PathBuf::from("/usr/local/lib/operai")),
        ];

        // Workspace target directory (if in workspace)
        let workspace_target = config_dir.join("..").join("..").join("target/release");
        if workspace_target.exists() {
            search_paths.push(Some(workspace_target));
        }

        // Global tools directory (e.g., ~/.operai/tools)
        if let Some(home) = dirs::home_dir() {
            search_paths.push(Some(home.join(".operai").join("tools")));
        }

        for search_path in search_paths.into_iter().flatten() {
            let full_path = search_path.join(&lib_name);
            if full_path.exists() {
                info!(
                    name = %name,
                    path = %full_path.display(),
                    "Resolved tool by name"
                );
                return Some(full_path);
            }
        }

        // If not found, return the default path for better error messages
        let default_path = config_dir.join("target/release").join(&lib_name);
        warn!(
            name = %name,
            attempted_path = %default_path.display(),
            "Tool not found in search paths, will attempt to load from default location"
        );
        return Some(default_path);
    }

    None
}

/// Normalizes a remote endpoint URL by ensuring it has a scheme.
///
/// If the endpoint already starts with `http://` or `https://`, it is returned
/// unchanged. Otherwise, `http://` is prepended.
///
/// # Parameters
///
/// - `endpoint`: The endpoint to normalize
///
/// # Returns
///
/// A URL with an `http://` or `https://` scheme
fn normalize_endpoint(endpoint: &str) -> String {
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        endpoint.to_string()
    } else {
        format!("http://{endpoint}")
    }
}

#[cfg(test)]
mod tests {
    /// Integration tests for the runtime builder.
    ///
    /// These tests build a hello-world tool library and verify that the builder
    /// can load tools from both explicit paths and name-based resolution.
    use std::{
        path::{Path, PathBuf},
        process::Command,
        sync::{
            OnceLock,
            atomic::{AtomicU64, Ordering},
        },
    };

    use super::*;
    use crate::{
        proto::{CallToolRequest, ListToolsRequest, call_tool_response},
        runtime::CallMetadata,
    };

    static HELLO_WORLD_CDYLIB_PATH: OnceLock<PathBuf> = OnceLock::new();
    static TEMP_CONFIG_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    fn cargo_target_dir_and_profile() -> (PathBuf, String) {
        let exe_path = std::env::current_exe().expect("test executable path should be available");
        let deps_dir = exe_path
            .parent()
            .expect("test executable should live in a deps directory");
        let profile_dir = deps_dir
            .parent()
            .expect("deps directory should have a profile directory parent");
        let profile = profile_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("debug")
            .to_string();
        let target_dir = profile_dir
            .parent()
            .expect("profile directory should have a target directory parent");
        (target_dir.to_path_buf(), profile)
    }

    fn expected_hello_world_cdylib_file_name() -> String {
        format!(
            "{}hello_world{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        )
    }

    fn find_hello_world_cdylib(target_dir: &Path, profile: &str) -> Option<PathBuf> {
        let file_name = expected_hello_world_cdylib_file_name();
        let profile_dir = target_dir.join(profile);

        let direct_path = profile_dir.join(&file_name);
        if direct_path.is_file() {
            return Some(direct_path);
        }

        let deps_dir = profile_dir.join("deps");
        let entries = std::fs::read_dir(deps_dir).ok()?;
        for entry in entries {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.file_name().and_then(|s| s.to_str()) == Some(file_name.as_str()) {
                return Some(path);
            }
        }

        None
    }

    fn build_hello_world_cdylib(target_dir: &Path, profile: &str) {
        let mut cmd = Command::new("cargo");
        cmd.current_dir(workspace_root());
        cmd.args(["build", "-p", "hello-world"]);
        if profile == "release" {
            cmd.arg("--release");
        }
        cmd.env("CARGO_TARGET_DIR", target_dir);

        let status = cmd.status().expect("cargo build should start");
        assert!(status.success(), "cargo build -p hello-world failed");
    }

    fn hello_world_cdylib_path() -> PathBuf {
        HELLO_WORLD_CDYLIB_PATH
            .get_or_init(|| {
                let (target_dir, profile) = cargo_target_dir_and_profile();

                if let Some(path) = find_hello_world_cdylib(&target_dir, &profile) {
                    return path;
                }

                build_hello_world_cdylib(&target_dir, &profile);

                find_hello_world_cdylib(&target_dir, &profile)
                    .unwrap_or_else(|| panic!("hello-world cdylib not found after build"))
            })
            .clone()
    }

    fn temp_config_path() -> PathBuf {
        let counter = TEMP_CONFIG_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "operai-runtime-config-{}-{counter}.toml",
            std::process::id()
        ))
    }

    fn write_config_for_library(path: &Path) -> PathBuf {
        let config_path = temp_config_path();
        let mut path_str = path.display().to_string();
        if std::path::MAIN_SEPARATOR == '\\' {
            path_str = path_str.replace('\\', "\\\\");
        }
        let contents = format!("[[tools]]\npath = \"{path_str}\"\n");
        std::fs::write(&config_path, contents).expect("write config");
        config_path
    }

    fn make_string_value(s: &str) -> prost_types::Value {
        prost_types::Value {
            kind: Some(prost_types::value::Kind::StringValue(s.to_string())),
        }
    }

    fn output_string(output: &prost_types::Struct, field: &str) -> String {
        let value = output
            .fields
            .get(field)
            .unwrap_or_else(|| panic!("missing output field `{field}`"));
        match &value.kind {
            Some(prost_types::value::Kind::StringValue(s)) => s.clone(),
            other => panic!("expected `{field}` to be a string, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_runtime_builder_loads_config_and_calls_tool() {
        let lib_path = hello_world_cdylib_path();
        let config_path = write_config_for_library(&lib_path);

        let runtime = RuntimeBuilder::new()
            .with_config_path(&config_path)
            .build_local()
            .await
            .expect("runtime should build");

        let list_response = runtime
            .list_tools(ListToolsRequest {
                page_size: 1000,
                page_token: String::new(),
            })
            .await
            .expect("list_tools should succeed");
        assert!(
            list_response
                .tools
                .iter()
                .any(|tool| tool.name == "tools/hello-world.echo"),
            "expected hello-world tools to be registered"
        );

        let input = prost_types::Struct {
            fields: [("message".to_string(), make_string_value("hi"))]
                .into_iter()
                .collect(),
        };
        let response = runtime
            .call_tool(
                CallToolRequest {
                    name: "tools/hello-world.echo".to_string(),
                    input: Some(input),
                },
                CallMetadata::default(),
            )
            .await
            .expect("call_tool should succeed");

        let Some(call_tool_response::Result::Output(output)) = response.result else {
            panic!("expected output result");
        };
        assert_eq!(output_string(&output, "echo"), "hi");
    }

    #[tokio::test]
    async fn test_runtime_builder_name_based_resolution() {
        let lib_path = hello_world_cdylib_path();

        // Create a config in temp directory that uses name-based resolution
        let config_path = temp_config_path();
        let config_dir = config_path.parent().unwrap_or_else(|| Path::new("."));
        let contents = "[[tools]]\nname = \"hello_world\"\nenabled = true\n".to_string();
        std::fs::write(&config_path, contents).expect("write config");

        // Create target/release structure in config dir for testing
        let target_dir = config_dir.join("target/release");
        std::fs::create_dir_all(&target_dir).expect("create target dir");

        // Copy library to test location
        let test_lib_dest = target_dir.join(format!(
            "{}hello_world{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        ));
        std::fs::copy(&lib_path, &test_lib_dest).expect("copy library");

        let runtime = RuntimeBuilder::new()
            .with_config_path(&config_path)
            .build_local()
            .await
            .expect("runtime should build with name-based resolution");

        let list_response = runtime
            .list_tools(ListToolsRequest {
                page_size: 1000,
                page_token: String::new(),
            })
            .await
            .expect("list_tools should succeed");
        assert!(
            list_response
                .tools
                .iter()
                .any(|tool| tool.name == "tools/hello-world.echo"),
            "expected hello-world tools to be registered via name-based resolution"
        );

        // Cleanup
        let _ = std::fs::remove_file(&test_lib_dest);
        let _ = std::fs::remove_dir_all(target_dir);
    }
}
