//! Manifest parsing and configuration for the Operai toolkit.
//!
//! This module provides functionality for loading and parsing TOML manifest files
//! that define tool configurations and policies. Manifests specify:
//!
//! - **Tools**: Dynamic libraries that can be loaded, including their paths, checksums,
//!   and enabled/disabled status
//! - **Policies**: Rules that govern tool execution, either defined inline or referenced
//!   via file paths
//! - **Config**: Arbitrary configuration data as TOML key-value pairs
//!
//! # Example
//!
//! ```toml
//! [[tools]]
//! path = "target/release/libhello.dylib"
//! enabled = true
//!
//! [[policies]]
//! name = "my-policy"
//! version = "1.0"
//! [[policies.effects]]
//! tool = "*"
//! stage = "after"
//! when = "true"
//! ```
//!
//! # Validation
//!
//! The manifest enforces that policies cannot specify both `path` (file reference)
//! and inline fields (`effects`, `context`) simultaneously. Policies must either
//! reference an external file or be fully defined inline.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::Policy;

/// Errors that can occur when loading or parsing a manifest file.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ManifestError {
    /// I/O error when reading the manifest file from disk.
    #[error("failed to read manifest: {0}")]
    Io(#[from] std::io::Error),

    /// TOML parsing error when the manifest file is malformed.
    #[error("failed to parse manifest: {0}")]
    Parse(#[from] toml::de::Error),

    /// Policy definition validation error.
    ///
    /// This occurs when a policy configuration is invalid, such as specifying
    /// both a file path and inline policy fields simultaneously.
    #[error("invalid policy definition: {0}")]
    Policy(String),
}

/// A manifest defining tool and policy configurations.
///
/// The manifest is the central configuration file for an Operai project, loaded
/// from a TOML file. It specifies which tools are available and which policies
/// govern their execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// List of tool configurations.
    ///
    /// Tools are dynamic libraries that can be loaded and invoked. Each tool
    /// can be enabled or disabled individually.
    #[serde(default)]
    pub tools: Vec<ToolConfig>,

    /// List of policy configurations.
    ///
    /// Policies define rules for tool execution, evaluated before and/or after
    /// tool invocations. Policies may be defined inline or referenced via file paths.
    #[serde(default)]
    pub policies: Vec<PolicyConfig>,

    /// Arbitrary configuration data.
    ///
    /// This can contain any TOML table with project-specific configuration
    /// that tools or policies may reference.
    pub config: Option<toml::Table>,
}

impl Manifest {
    /// Loads and parses a manifest file from the given path.
    ///
    /// This method reads the TOML file at the specified path, parses it into a
    /// `Manifest`, and validates policy configurations. Policy validation ensures
    /// that a policy cannot specify both a `path` field and inline fields
    /// (`effects` or `context`) simultaneously.
    ///
    /// # Parameters
    ///
    /// - `path`: Path to the TOML manifest file
    ///
    /// # Returns
    ///
    /// Returns `Ok(Manifest)` if the file is successfully loaded and validated.
    /// Returns `Err(ManifestError)` if:
    /// - The file cannot be read (I/O error)
    /// - The TOML is malformed (parse error)
    /// - A policy configuration is invalid (validation error)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use operai_core::Manifest;
    ///
    /// let manifest = Manifest::load("operai.toml").expect("Failed to load manifest");
    /// ```
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ManifestError> {
        let content = std::fs::read_to_string(&path)?;
        let manifest: Self = toml::from_str(&content)?;

        for policy_config in &manifest.policies {
            if policy_config.path.is_some()
                && (policy_config.effects.is_some() || policy_config.context.is_some())
            {
                return Err(ManifestError::Policy(format!(
                    "Policy '{}' cannot specify both `path` and inline fields (effects/context)",
                    policy_config.name.as_deref().unwrap_or("<unknown>")
                )));
            }
        }

        Ok(manifest)
    }

    /// Creates an empty manifest with no tools or policies.
    ///
    /// This is useful for testing or as a starting point for programmatically
    /// building manifest configurations.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            tools: Vec::new(),
            policies: Vec::new(),
            config: None,
        }
    }

    /// Returns an iterator over only the enabled tools in the manifest.
    ///
    /// This filters the `tools` list to return only tools where `enabled` is `true`.
    ///
    /// # Returns
    ///
    /// An iterator yielding references to `ToolConfig` items that are enabled.
    #[must_use = "iterator should be consumed to access enabled tools"]
    pub fn enabled_tools(&self) -> impl Iterator<Item = &ToolConfig> {
        self.tools.iter().filter(|t| t.enabled)
    }

    /// Resolves all policy configurations into concrete `Policy` instances.
    ///
    /// This method processes the policy configurations in the manifest:
    ///
    /// - For policies with a `path` field, loads the policy from the external file
    ///   (relative to the manifest directory)
    /// - For inline policies (with `name`, `version`, `context`, `effects` fields),
    ///   constructs a `Policy` directly from the configuration
    ///
    /// # Parameters
    ///
    /// - `manifest_path`: Path to the manifest file (used to resolve relative
    ///   policy file paths)
    ///
    /// # Returns
    ///
    /// Returns `Ok(Vec<Policy>)` containing all resolved policies.
    /// Returns `Err(ManifestError)` if:
    /// - An external policy file cannot be read
    /// - An external policy file fails to parse
    /// - An inline policy is missing required fields (e.g., `name`)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use operai_core::Manifest;
    /// use std::path::Path;
    ///
    /// # let manifest = Manifest::empty();
    /// let policies = manifest.resolve_policies(Path::new("operai.toml"))
    ///     .expect("Failed to resolve policies");
    /// ```
    pub fn resolve_policies(&self, manifest_path: &Path) -> Result<Vec<Policy>, ManifestError> {
        let root_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));

        let mut policies = Vec::new();

        for config in &self.policies {
            if let Some(rel_path) = &config.path {
                let policy_path = root_dir.join(rel_path);
                let content = std::fs::read_to_string(&policy_path)?;
                // Assume policy file is a TOML defining a single Policy or Policy fields
                let policy: Policy = toml::from_str(&content).map_err(ManifestError::Parse)?;
                policies.push(policy);
            } else {
                let name = config.name.clone().ok_or_else(|| {
                    ManifestError::Policy("Inline policy must have a name".to_string())
                })?;
                let version = config
                    .version
                    .clone()
                    .unwrap_or_else(|| "0.0.0".to_string());
                let context = config.context.clone().unwrap_or_default();
                let effects = config.effects.clone().unwrap_or_default();

                policies.push(Policy {
                    name,
                    version,
                    context,
                    effects,
                });
            }
        }

        Ok(policies)
    }
}

/// Configuration for a single tool in the manifest.
///
/// Tools are dynamic libraries that can be loaded by the Operai runtime.
/// This struct defines where to find the tool and how to load it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    /// Optional name identifying the tool.
    ///
    /// If not specified, the tool name may be derived from the library path
    /// or the tool's own metadata.
    pub name: Option<String>,

    /// Path to the tool's dynamic library file.
    ///
    /// This can be an absolute path or a path relative to the manifest directory.
    /// The file format is platform-dependent (e.g., `.dylib` on macOS, `.so` on Linux).
    pub path: Option<String>,

    /// Optional checksum for verifying the tool's integrity.
    ///
    /// This can be used to ensure the loaded library has not been modified.
    pub checksum: Option<String>,

    /// Whether the tool is currently enabled.
    ///
    /// Disabled tools are defined in the manifest but will not be loaded.
    /// Defaults to `true` if not specified in the TOML.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Credential configuration for the tool.
    ///
    /// A nested map where the outer key identifies a credential type or service,
    /// and the inner map contains key-value pairs for that credential.
    #[serde(default)]
    pub credentials: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
}

fn default_enabled() -> bool {
    true
}

/// Configuration for a policy in the manifest.
///
/// Policies can be defined in one of two mutually exclusive ways:
///
/// 1. **External reference**: Specify only `path` to load a policy from a separate file
/// 2. **Inline definition**: Specify `name`, and optionally `version`, `context`, and `effects`
///
/// The manifest validation enforces that these two approaches cannot be mixed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    /// Path to an external policy file (relative to the manifest directory).
    ///
    /// If specified, `name`, `version`, `context`, and `effects` must all be `None`.
    /// The external file should contain a complete policy definition in TOML format.
    pub path: Option<String>,

    /// Policy name (required for inline policies).
    ///
    /// Must be specified if `path` is `None`. This identifies the policy and
    /// is used in policy evaluation and error messages.
    pub name: Option<String>,

    /// Policy version (optional for inline policies).
    ///
    /// If not specified, defaults to "0.0.0".
    pub version: Option<String>,

    /// Initial context variables for the policy (optional for inline policies).
    ///
    /// A map of variable names to JSON values that will be available to
    /// CEL expressions in the policy's effects.
    #[serde(default)]
    pub context: Option<std::collections::HashMap<String, JsonValue>>,

    /// List of effects defining the policy's behavior (optional for inline policies).
    ///
    /// Effects are rules that are evaluated before or after tool execution,
    /// potentially blocking execution or modifying the policy context.
    #[serde(default)]
    pub effects: Option<Vec<crate::policy::Effect>>,
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    static TEMP_MANIFEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_temp_path(label: &str) -> PathBuf {
        let counter = TEMP_MANIFEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "operai-core-manifest-{label}-{}-{counter}.toml",
            std::process::id()
        ))
    }

    struct TempManifestFile {
        path: PathBuf,
    }

    impl TempManifestFile {
        fn new(contents: &str) -> Self {
            let path = unique_temp_path("temp");
            std::fs::write(&path, contents).expect("write temp manifest file");
            Self { path }
        }
    }

    impl Drop for TempManifestFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    // ==================== Trait Implementation Tests ====================

    #[test]
    fn test_manifest_clone_creates_independent_copy() {
        let original = Manifest {
            tools: vec![ToolConfig {
                name: None,
                path: Some("test.dylib".to_string()),
                checksum: None,
                enabled: true,
                credentials: HashMap::new(),
            }],
            policies: Vec::new(),
            config: None,
        };

        let cloned = original.clone();

        assert_eq!(cloned.tools.len(), 1);
        assert_eq!(cloned.tools[0].path, Some("test.dylib".to_string()));
    }

    #[test]
    fn test_deserialize_manifest_happy_path() {
        let toml = r#"
[config]
test_key = "test_val"

[[tools]]
path = "target/release/libhello.dylib"
enabled = true

[[policies]]
name = "inline-policy"
version = "1.0"
[[policies.effects]]
tool = "*"
stage = "after"
when = "true"
"#;

        let manifest: Manifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.tools.len(), 1);
        assert_eq!(manifest.policies.len(), 1);
        assert!(manifest.config.is_some());

        // Check inline policy
        let policies = manifest.resolve_policies(Path::new("dummy.toml")).unwrap();
        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].name, "inline-policy");
    }

    #[test]
    fn test_deserialize_tool_config_name_support() {
        let toml = r#"
[[tools]]
name = "my-tool"
enabled = true
"#;
        let manifest: Manifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.tools[0].name, Some("my-tool".to_string()));
        assert!(manifest.tools[0].path.is_none());
    }

    #[test]
    fn test_manifest_validation_rejects_ambiguous_policy() {
        let toml = r#"
[[policies]]
name = "bad"
path = "policy.toml"
[[policies.effects]]
tool = "*"
stage = "after"
when = "true"
"#;
        let file = TempManifestFile::new(toml);
        let res = Manifest::load(&file.path);

        assert!(res.is_err());
        match res.unwrap_err() {
            ManifestError::Policy(msg) => assert!(msg.contains("cannot specify both")),
            _ => panic!("Expected Policy error"),
        }
    }
}
