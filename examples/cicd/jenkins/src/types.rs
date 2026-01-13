//! Type definitions for Jenkins API responses.

use operai::{JsonSchema, schemars};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Jenkins build result status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum BuildResult {
    Success,
    Failure,
    Unstable,
    Aborted,
    #[serde(rename = "NOT_BUILT")]
    NotBuilt,
}

/// Jenkins build action (contains parameters, causes, and other metadata)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum BuildAction {
    /// Parameters action
    Parameters {
        /// Action class name
        #[serde(rename = "_class")]
        class_name: String,
        /// Parameters list
        parameters: Vec<Value>,
    },
    /// Cause action
    Cause {
        /// Action class name
        #[serde(rename = "_class")]
        class_name: String,
        /// Causes list
        causes: Vec<Value>,
    },
    /// Generic action with unknown structure
    Other(Value),
}

/// Jenkins build status (whether build is in progress)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BuildStatus {
    /// Build number
    pub number: u32,
    /// Build display name (short form)
    #[serde(default, rename = "displayName")]
    pub display_name: Option<String>,
    /// Build full display name (includes job path)
    #[serde(default, rename = "fullDisplayName")]
    pub full_display_name: Option<String>,
    /// Build ID (often same as number but can be custom)
    #[serde(default)]
    pub id: Option<String>,
    /// Build URL
    pub url: String,
    /// Whether build is currently building
    pub building: bool,
    /// Build result (null if still building)
    #[serde(default)]
    pub result: Option<BuildResult>,
    /// Build duration in milliseconds
    pub duration: u64,
    /// Estimated duration in milliseconds (for running builds)
    #[serde(default, rename = "estimatedDuration")]
    pub estimated_duration: Option<u64>,
    /// Build timestamp (Unix epoch milliseconds)
    pub timestamp: u64,
    /// Build actions (parameters, causes, etc.)
    #[serde(default)]
    pub actions: Vec<BuildAction>,
}

/// Job parameter
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct JobParameter {
    /// Parameter name
    pub name: String,
    /// Parameter value
    pub value: String,
}
