//! Runtime builder for local or remote execution.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use operai_abi::RuntimeContext;
#[cfg(feature = "static-link")]
use operai_abi::ToolModuleRef;
use operai_core::{
    Manifest, ToolRegistry,
    policy::session::{InMemoryPolicySessionStore, PolicyStore},
};
use tracing::{error, info, warn};

use crate::runtime::{LocalRuntime, RemoteRuntime, Runtime};

/// Errors that can occur while building a runtime.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeBuildError {
    /// Manifest loading or parsing failed.
    #[error("failed to load manifest: {0}")]
    Manifest(#[from] operai_core::ManifestError),
    /// Remote endpoint was not configured.
    #[error("remote endpoint is required")]
    MissingRemoteEndpoint,
    /// Remote connection failed.
    #[error("failed to connect to remote runtime: {0}")]
    Transport(#[from] tonic::transport::Error),
}

#[derive(Debug, Clone)]
enum RuntimeMode {
    Local,
    Remote { endpoint: String },
}

/// Configures and builds a runtime.
#[derive(Debug, Clone)]
pub struct RuntimeBuilder {
    manifest_path: PathBuf,
    runtime_ctx: RuntimeContext,
    mode: RuntimeMode,
    #[cfg(feature = "static-link")]
    static_tools: Vec<ToolModuleRef>,
}

impl RuntimeBuilder {
    /// Creates a builder with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            manifest_path: PathBuf::from("operai.toml"),
            runtime_ctx: RuntimeContext::new(),
            mode: RuntimeMode::Local,
            #[cfg(feature = "static-link")]
            static_tools: Vec::new(),
        }
    }

    /// Sets the manifest path used for local runtime initialization.
    #[must_use]
    pub fn with_manifest_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.manifest_path = path.into();
        self
    }

    /// Overrides the runtime context passed to tool initialization.
    #[must_use]
    pub fn with_runtime_context(mut self, ctx: RuntimeContext) -> Self {
        self.runtime_ctx = ctx;
        self
    }

    /// Builds a local runtime.
    pub async fn build_local(self) -> Result<LocalRuntime, RuntimeBuildError> {
        build_local_runtime(self).await
    }

    /// Builds a remote runtime.
    pub async fn build_remote(self) -> Result<RemoteRuntime, RuntimeBuildError> {
        let endpoint = match self.mode {
            RuntimeMode::Remote { endpoint } => endpoint,
            RuntimeMode::Local => return Err(RuntimeBuildError::MissingRemoteEndpoint),
        };

        let endpoint = normalize_endpoint(&endpoint);
        Ok(RemoteRuntime::connect(endpoint).await?)
    }

    /// Builds the runtime based on the configured mode.
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

    /// Configures the builder to create a local runtime.
    #[must_use]
    pub fn local(mut self) -> Self {
        self.mode = RuntimeMode::Local;
        self
    }

    /// Configures the builder to create a remote runtime.
    #[must_use]
    pub fn remote(mut self, endpoint: impl Into<String>) -> Self {
        self.mode = RuntimeMode::Remote {
            endpoint: endpoint.into(),
        };
        self
    }

    /// Registers a statically linked tool module.
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

async fn build_local_runtime(builder: RuntimeBuilder) -> Result<LocalRuntime, RuntimeBuildError> {
    let manifest_path = builder.manifest_path;
    let runtime_ctx = builder.runtime_ctx;

    let manifest = load_manifest_or_empty(&manifest_path)?;
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));

    let mut registry = ToolRegistry::new();

    for tool_config in manifest.enabled_tools() {
        let Some(path) = resolve_tool_path(tool_config, manifest_dir) else {
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

    match manifest.resolve_policies(&manifest_path) {
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
            warn!(error = %e, "Failed to resolve policies from manifest");
        }
    }

    Ok(LocalRuntime::with_context(
        registry,
        policy_store,
        runtime_ctx,
    ))
}

fn load_manifest_or_empty(manifest_path: &Path) -> Result<Manifest, RuntimeBuildError> {
    if manifest_path.exists() {
        Ok(Manifest::load(manifest_path)?)
    } else {
        warn!(
            path = %manifest_path.display(),
            "Manifest file not found, starting with empty registry"
        );
        Ok(Manifest::empty())
    }
}

fn resolve_tool_path(tool: &operai_core::ToolConfig, manifest_dir: &Path) -> Option<PathBuf> {
    if let Some(path) = &tool.path {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            Some(path)
        } else {
            Some(manifest_dir.join(path))
        }
    } else if let Some(pkg) = &tool.package {
        let lib_name = format!(
            "{}{}{}",
            std::env::consts::DLL_PREFIX,
            pkg.replace('-', "_"),
            std::env::consts::DLL_SUFFIX
        );
        Some(manifest_dir.join("target/release").join(lib_name))
    } else {
        None
    }
}

fn normalize_endpoint(endpoint: &str) -> String {
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        endpoint.to_string()
    } else {
        format!("http://{endpoint}")
    }
}

#[cfg(test)]
mod tests {
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
    static TEMP_MANIFEST_COUNTER: AtomicU64 = AtomicU64::new(0);

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

    fn temp_manifest_path() -> PathBuf {
        let counter = TEMP_MANIFEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "operai-runtime-manifest-{}-{counter}.toml",
            std::process::id()
        ))
    }

    fn write_manifest_for_library(path: &Path) -> PathBuf {
        let manifest_path = temp_manifest_path();
        let mut path_str = path.display().to_string();
        if std::path::MAIN_SEPARATOR == '\\' {
            path_str = path_str.replace('\\', "\\\\");
        }
        let contents = format!("[[tools]]\npath = \"{path_str}\"\n");
        std::fs::write(&manifest_path, contents).expect("write manifest");
        manifest_path
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
    async fn test_runtime_builder_loads_manifest_and_calls_tool() {
        let lib_path = hello_world_cdylib_path();
        let manifest_path = write_manifest_for_library(&lib_path);

        let runtime = RuntimeBuilder::new()
            .with_manifest_path(&manifest_path)
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
}
