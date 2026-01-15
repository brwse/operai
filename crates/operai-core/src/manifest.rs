//! Manifest parsing for tool configuration.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::Policy;

/// Errors that can occur when parsing a manifest.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ManifestError {
    /// Failed to read the manifest file.
    #[error("failed to read manifest: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to parse the manifest.
    #[error("failed to parse manifest: {0}")]
    Parse(#[from] toml::de::Error),

    /// Policy definition error.
    #[error("invalid policy definition: {0}")]
    Policy(String),
}

/// Tool manifest configuration.
///
/// The manifest is a TOML file that lists tool libraries to load and policies
/// to enforce.
///
/// # Example
///
/// ```toml
/// [config]
/// embedding_model = "fastembed"
///
/// [[tools]]
/// name = "hello-world"
/// enabled = true
///
/// [[policies]]
/// name = "audit-logging"
/// version = "1.0"
/// [[policies.effects]]
/// tool = "*"
/// stage = "after"
/// when = "true"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub tools: Vec<ToolConfig>,

    #[serde(default)]
    pub policies: Vec<PolicyConfig>,

    /// Project configuration (build settings, etc).
    /// Kept generic here as it's primarily used by `cargo-operai`.
    pub config: Option<toml::Table>,
}

impl Manifest {
    /// Loads a manifest from the specified path.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ManifestError> {
        let content = std::fs::read_to_string(&path)?;
        let manifest: Self = toml::from_str(&content)?;

        // Validate policies
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

    #[must_use]
    pub fn empty() -> Self {
        Self {
            tools: Vec::new(),
            policies: Vec::new(),
            config: None,
        }
    }

    #[must_use = "iterator should be consumed to access enabled tools"]
    pub fn enabled_tools(&self) -> impl Iterator<Item = &ToolConfig> {
        self.tools.iter().filter(|t| t.enabled)
    }

    /// Resolves all policies, loading file-based policies relative to the
    /// manifest path.
    ///
    /// # Errors
    ///
    /// Returns an error if a policy file cannot be read or parsed.
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
                // Inline policy
                // We need to convert PolicyConfig to Policy.
                // Since PolicyConfig is a superset/subset, we might need manual conversion
                // or ensure PolicyConfig can be converted.
                // For now, let's assume inline definition MUST have name/version.
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

/// Configuration for a single tool library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    /// Name of the tool for name-based resolution.
    /// If `path` is not specified, the tool will be auto-resolved from
    /// configured search directories using this name.
    pub name: Option<String>,

    /// Path to the dynamic library.
    /// If specified, this takes precedence over name-based resolution.
    pub path: Option<String>,

    /// Checksum of the tool binary for verification (SHA256).
    pub checksum: Option<String>,

    /// Defaults to `true` if not specified in the manifest.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// System credentials injected when loading this tool.
    #[serde(default)]
    pub credentials: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
}

fn default_enabled() -> bool {
    true
}

/// Configuration for a policy.
/// Can be inline or file-based.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    /// Path to policy file (relative to manifest).
    pub path: Option<String>,

    // Inline fields (Optional if path is set)
    pub name: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub context: Option<std::collections::HashMap<String, JsonValue>>,
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
