//! Policy evaluation and enforcement using Common Expression Language (CEL).
//!
//! This module provides a policy system for controlling tool execution through
//! conditional effects that can modify execution context and enforce guards.
//! Policies are defined using CEL expressions and can be evaluated before or
//! after tool execution.
//!
//! # Key Concepts
//!
//! - **Policy**: A collection of effects with shared context
//! - **Effect**: A conditional action that applies to specific tools at a
//!   specific stage
//! - **Stage**: Before or after tool execution
//! - **Compilation**: CEL expressions are compiled to Programs for efficient
//!   evaluation
//! - **Session**: Maintains context and history across policy evaluations
//!
//! # Example
//!
//! ```ignore
//! let policy = Policy {
//!     name: "safety_checks".into(),
//!     version: "1.0.0".into(),
//!     context: HashMap::new(),
//!     effects: vec![Effect {
//!         tool: "dangerous.*".into(),
//!         stage: PolicyStage::Before,
//!         condition: "context.safe_mode == true".into(),
//!         fail_message: Some("Operation blocked: safe mode enabled".into()),
//!         updates: HashMap::new(),
//!     }],
//! };
//!
//! let compiled = policy.compile()?;
//! let mut session = PolicySession::default();
//! compiled.evaluate_pre_effects(&mut session, "dangerous.nuke", &input)?;
//! ```

use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use cel_interpreter::{
    Context, ParseErrors, Program, Value,
    objects::{Key, Map as CelMap},
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;

pub mod session;
use session::{HistoryEvent, PolicySession};

/// When a policy effect should be evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PolicyStage {
    /// Evaluate before tool execution.
    /// Can block execution via `fail_message` or modify input via `updates`.
    Before,
    /// Evaluate after tool execution.
    /// Can modify context based on tool output via `updates`.
    #[default]
    After,
}

/// A policy definition containing conditional effects for controlling tool
/// execution.
///
/// Policies are evaluated against tool invocations to enforce guards and
/// modify execution context. They use CEL (Common Expression Language) for
/// flexible, composable condition expressions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    /// Unique identifier for this policy.
    pub name: String,
    /// Version string for policy tracking and compatibility.
    pub version: String,

    /// Initial context variables available to all CEL expressions in this
    /// policy.
    #[serde(default)]
    pub context: HashMap<String, JsonValue>,

    /// Effects to evaluate when tools are invoked.
    #[serde(default)]
    pub effects: Vec<Effect>,
}

/// A conditional effect that can modify execution or block tool invocations.
///
/// Effects are evaluated in the context of a specific tool invocation and can
/// update policy context or enforce guards based on CEL conditions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Effect {
    /// Tool name pattern this effect applies to.
    /// Supports glob patterns: "*" (all tools), "group.*" (tools in a group).
    pub tool: String,

    /// When this effect should be evaluated.
    #[serde(default)]
    pub stage: PolicyStage,

    /// CEL condition expression that must evaluate to `true` for this effect to
    /// apply.
    #[serde(rename = "when")]
    pub condition: String,

    /// Error message to return if the condition fails in `Before` stage.
    /// If `None`, a failed condition simply skips the effect.
    pub fail_message: Option<String>,

    /// Context variables to update when the condition succeeds.
    /// Keys are variable names, values are CEL expressions.
    #[serde(rename = "set", default)]
    pub updates: HashMap<String, String>,
}

/// Errors that can occur during policy evaluation.
#[derive(Debug, Error)]
pub enum PolicyError {
    /// A guard condition failed and tool execution was blocked.
    #[error("Guard failed: {0}")]
    GuardFailed(String),

    /// CEL parsing or syntax error.
    #[error("CEL error: {0}")]
    CelError(String),

    /// Runtime error during CEL expression evaluation.
    #[error("Evaluation error: {0}")]
    EvalError(String),

    /// Error compiling CEL expressions to Programs.
    #[error("Compilation error: {0}")]
    CompilationError(String),

    /// Session conflict during optimistic concurrency control (internal use).
    #[error("Session conflict")]
    SessionConflict,
}

/// A compiled effect ready for evaluation.
///
/// Contains the original effect definition alongside compiled CEL programs
/// for efficient runtime evaluation.
#[derive(Debug)]
pub struct CompiledEffect {
    /// The original effect definition.
    pub original: Effect,
    /// Compiled CEL condition program.
    pub condition: Program,
    /// Compiled CEL update expressions (variable name -> program).
    pub updates: HashMap<String, Program>,
}

/// A compiled policy ready for evaluation.
///
/// Created by compiling a `Policy` to validate CEL expressions and prepare
/// them for efficient execution.
#[derive(Debug)]
pub struct CompiledPolicy {
    /// The original policy definition.
    pub original: Policy,
    /// Compiled effects for evaluation.
    pub effects: Vec<CompiledEffect>,
}

impl From<ParseErrors> for PolicyError {
    fn from(e: ParseErrors) -> Self {
        Self::CelError(e.to_string())
    }
}

impl Policy {
    /// Compiles all CEL expressions in this policy for efficient evaluation.
    ///
    /// Validates that all condition and update expressions are valid CEL and
    /// compiles them to `Program` objects for runtime execution.
    ///
    /// # Errors
    ///
    /// Returns `PolicyError::CompilationError` if any CEL expression fails to
    /// parse.
    pub fn compile(self) -> Result<CompiledPolicy, PolicyError> {
        let mut compiled_effects = Vec::new();
        for effect in &self.effects {
            let condition = Program::compile(&effect.condition)
                .map_err(|e| PolicyError::CompilationError(format!("Condition error: {e}")))?;

            let mut updates = HashMap::new();
            for (key, expr) in &effect.updates {
                let prog = Program::compile(expr).map_err(|e| {
                    PolicyError::CompilationError(format!("Update error for {key}: {e}"))
                })?;
                updates.insert(key.clone(), prog);
            }

            compiled_effects.push(CompiledEffect {
                original: effect.clone(),
                condition,
                updates,
            });
        }

        Ok(CompiledPolicy {
            original: self,
            effects: compiled_effects,
        })
    }
}

impl CompiledPolicy {
    /// Evaluates all `Before` stage effects for a tool invocation.
    ///
    /// This is called before tool execution to enforce guards and optionally
    /// modify the policy context. Effects are evaluated in order; if any guard
    /// fails with a `fail_message`, execution is blocked.
    ///
    /// # CEL Context Variables
    ///
    /// - `context`: Policy session context (`HashMap`)
    /// - `history`: Recent tool execution history (last 5 events)
    /// - `input`: Tool input JSON
    /// - `tool`: Map with `name` key containing the tool name
    ///
    /// # Returns
    ///
    /// - `Ok(true)`: Context was modified and should be persisted
    /// - `Ok(false)`: No changes to context
    /// - `Err(PolicyError::GuardFailed(msg))`: Execution blocked
    ///
    /// # Errors
    ///
    /// - Returns `PolicyError::EvalError` if CEL evaluation fails
    /// - Returns `PolicyError::GuardFailed` if a guard condition fails with a
    ///   fail message
    pub fn evaluate_pre_effects(
        &self,
        state: &mut PolicySession,
        tool_name: &str,
        input: &JsonValue,
    ) -> Result<bool, PolicyError> {
        let mut cel_ctx = Context::default();
        cel_ctx.add_variable("context", to_cel_value(&state.context));
        cel_ctx.add_variable("history", to_cel_history(&state.history));
        cel_ctx.add_variable("input", to_cel_json(input));
        let mut tool_map = HashMap::new();
        tool_map.insert("name".into(), tool_name.into());
        cel_ctx.add_variable(
            "tool",
            Value::Map(CelMap {
                map: Arc::new(tool_map),
            }),
        );

        let mut modified = false;

        for effect in &self.effects {
            if effect.original.stage == PolicyStage::Before
                && matches_tool_pattern(&effect.original.tool, tool_name)
            {
                let result = effect
                    .condition
                    .execute(&cel_ctx)
                    .map_err(|e| PolicyError::EvalError(e.to_string()))?;

                let Value::Bool(condition_met) = result else {
                    return Err(PolicyError::EvalError(
                        "Effect condition must return boolean".into(),
                    ));
                };

                if !condition_met {
                    if let Some(msg) = &effect.original.fail_message {
                        return Err(PolicyError::GuardFailed(msg.clone()));
                    }
                    continue;
                }

                if !effect.updates.is_empty() {
                    for (key, expr_prog) in &effect.updates {
                        let new_val_cel = expr_prog.execute(&cel_ctx).map_err(|e| {
                            PolicyError::EvalError(format!("Effect update error for {key}: {e}"))
                        })?;

                        let new_val_json = cel_to_json(new_val_cel);
                        state.context.insert(key.clone(), new_val_json);

                        cel_ctx.add_variable("context", to_cel_value(&state.context));
                    }
                    modified = true;
                }
            }
        }
        Ok(modified)
    }

    /// Evaluates all `After` stage effects for a tool invocation.
    ///
    /// This is called after tool execution to update policy context based on
    /// the tool's result. Always adds a history event for the invocation.
    ///
    /// # CEL Context Variables
    ///
    /// - `context`: Policy session context (`HashMap`)
    /// - `history`: Recent tool execution history (last 5 events)
    /// - `input`: Tool input JSON
    /// - `tool`: Map with `name` key containing the tool name
    /// - `output`: Tool output JSON (null if error)
    /// - `error`: Error message string (only present if tool failed)
    /// - `result`: Map with `is_ok` boolean key
    ///
    /// # Errors
    ///
    /// Returns `PolicyError::EvalError` if CEL evaluation fails.
    pub fn evaluate_post_effects(
        &self,
        state: &mut PolicySession,
        tool_name: &str,
        input: &JsonValue,
        output: Result<&JsonValue, &str>,
    ) -> Result<(), PolicyError> {
        let mut cel_ctx = Context::default();
        cel_ctx.add_variable("context", to_cel_value(&state.context));
        cel_ctx.add_variable("history", to_cel_history(&state.history));
        cel_ctx.add_variable("input", to_cel_json(input));

        let mut tool_map = HashMap::new();
        tool_map.insert("name".into(), tool_name.into());
        cel_ctx.add_variable(
            "tool",
            Value::Map(CelMap {
                map: Arc::new(tool_map),
            }),
        );

        match output {
            Ok(val) => {
                cel_ctx.add_variable("output", to_cel_json(val));
                cel_ctx.add_variable(
                    "result",
                    Value::Map(CelMap {
                        map: Arc::new(HashMap::from([("is_ok".into(), true.into())])),
                    }),
                );
            }
            Err(e) => {
                cel_ctx.add_variable("output", Value::Null);
                cel_ctx.add_variable("error", Value::String(Arc::new(e.to_string())));
                cel_ctx.add_variable(
                    "result",
                    Value::Map(CelMap {
                        map: Arc::new(HashMap::from([("is_ok".into(), false.into())])),
                    }),
                );
            }
        }

        for effect in &self.effects {
            if effect.original.stage == PolicyStage::After
                && matches_tool_pattern(&effect.original.tool, tool_name)
            {
                let result = effect
                    .condition
                    .execute(&cel_ctx)
                    .map_err(|e| PolicyError::EvalError(e.to_string()))?;

                if matches!(result, Value::Bool(true)) {
                    for (key, expr_prog) in &effect.updates {
                        let new_val_cel = expr_prog.execute(&cel_ctx).map_err(|e| {
                            PolicyError::EvalError(format!("Effect update error for {key}: {e}"))
                        })?;

                        let new_val_json = cel_to_json(new_val_cel);
                        state.context.insert(key.clone(), new_val_json);

                        // Update context for subsequent effects
                        cel_ctx.add_variable("context", to_cel_value(&state.context));
                    }
                }
            }
        }

        state.history.push(HistoryEvent {
            tool: tool_name.to_string(),
            input: input.clone(),
            success: output.is_ok(),
            output: output.ok().cloned(),
            error: output.err().map(ToString::to_string),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        });

        Ok(())
    }
}

/// Checks if a tool name matches a pattern.
///
/// # Patterns
///
/// - `"*"`: Matches all tools
/// - `"group.*"`: Matches `group` and any tool with a `group.` prefix (e.g.,
///   `group.subtool`)
/// - `"exact_name"`: Matches only the exact tool name
fn matches_tool_pattern(pattern: &str, tool_id: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix(".*") {
        return tool_id == prefix || tool_id.starts_with(&format!("{prefix}."));
    }
    pattern == tool_id
}

/// Converts a `serde_json::Value` to a CEL `Value`.
fn to_cel_json(v: &JsonValue) -> Value {
    match v {
        JsonValue::Null => Value::Null,
        JsonValue::Bool(b) => Value::Bool(*b),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            }
            // CEL doesn't have float? It does (Double)
            else {
                Value::Int(0)
            } // Fallback
        }
        JsonValue::String(s) => Value::String(Arc::new(s.clone())),
        JsonValue::Array(arr) => Value::List(Arc::new(arr.iter().map(to_cel_json).collect())),
        JsonValue::Object(map) => {
            let mut m = HashMap::new();
            for (k, v) in map {
                m.insert(k.clone().into(), to_cel_json(v));
            }
            Value::Map(CelMap { map: Arc::new(m) })
        }
    }
}

/// Converts a JSON `HashMap` to a CEL Map value.
fn to_cel_value(map: &HashMap<String, JsonValue>) -> Value {
    let mut m = HashMap::new();
    for (k, v) in map {
        m.insert(k.clone().into(), to_cel_json(v));
    }
    Value::Map(CelMap { map: Arc::new(m) })
}

/// Converts a CEL `Value` back to a `serde_json::Value`.
fn cel_to_json(v: Value) -> JsonValue {
    match v {
        Value::Int(i) => JsonValue::Number(i.into()),
        Value::UInt(u) => JsonValue::Number(u.into()),
        Value::Float(f) => JsonValue::Number(serde_json::Number::from_f64(f).unwrap_or(0.into())),
        Value::String(s) => JsonValue::String(s.to_string()),
        Value::Bool(b) => JsonValue::Bool(b),
        Value::Null => JsonValue::Null,
        Value::List(l) => JsonValue::Array(l.iter().map(|v| cel_to_json(v.clone())).collect()),
        Value::Map(m) => {
            let mut map = serde_json::Map::new();
            for (k, v) in m.map.iter() {
                if let Key::String(s) = k {
                    map.insert(s.to_string(), cel_to_json(v.clone()));
                }
            }
            JsonValue::Object(map)
        }
        _ => JsonValue::String(format!("{v:?}")),
    }
}

/// Maximum number of history events exposed to CEL expressions.
const MAX_HISTORY_ITEMS: usize = 5;

/// Converts history events to a CEL list value.
///
/// Returns the last `MAX_HISTORY_ITEMS` events (most recent).
fn to_cel_history(hist: &[HistoryEvent]) -> Value {
    let start = hist.len().saturating_sub(MAX_HISTORY_ITEMS);
    let list: Vec<Value> = hist[start..]
        .iter()
        .map(|e| {
            let mut m = HashMap::new();
            m.insert("tool".into(), Value::String(Arc::new(e.tool.clone())));
            m.insert("success".into(), Value::Bool(e.success));
            Value::Map(CelMap { map: Arc::new(m) })
        })
        .collect();

    Value::List(Arc::new(list))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::policy::session::PolicySessionStore;

    #[test]
    fn test_guard_blocks_execution() {
        let policy = Policy {
            name: "test".into(),
            version: "1".into(),
            context: HashMap::new(),
            effects: vec![Effect {
                tool: "dangerous.*".into(),
                stage: PolicyStage::Before,
                condition: "context.safe_mode == true".into(),
                fail_message: Some("Safety first!".into()),
                updates: HashMap::new(),
            }],
        };

        let mut state = PolicySession::default();
        state.context.insert("safe_mode".into(), json!(false));

        let compiled_policy = policy.compile().expect("compilation failed");
        let res = compiled_policy.evaluate_pre_effects(&mut state, "dangerous.nuke", &json!({}));
        assert!(res.is_err());
        match res {
            Err(PolicyError::GuardFailed(msg)) => assert_eq!(msg, "Safety first!"),
            _ => panic!("Wrong error"),
        }

        // Allow
        state.context.insert("safe_mode".into(), json!(true));
        assert!(
            compiled_policy
                .evaluate_pre_effects(&mut state, "dangerous.nuke", &json!({}))
                .is_ok()
        );
    }

    #[test]
    fn test_effect_updates_context() {
        let policy = Policy {
            name: "test".into(),
            version: "1".into(),
            context: HashMap::new(),
            effects: vec![Effect {
                tool: "git.commit".into(),
                stage: PolicyStage::After,
                condition: "result.is_ok".into(),
                fail_message: None,
                updates: HashMap::from([
                    ("last_hash".into(), "output.hash".into()),
                    ("commit_count".into(), "context.commit_count + 1".into()),
                ]),
            }],
        };

        let mut state = PolicySession::default();
        state.context.insert("commit_count".into(), json!(0));

        let input = json!({});
        let output = json!({"hash": "abc-123"});

        let compiled_policy = policy.compile().expect("compilation failed");
        compiled_policy
            .evaluate_post_effects(&mut state, "git.commit", &input, Ok(&output))
            .unwrap();

        assert_eq!(state.context.get("last_hash"), Some(&json!("abc-123")));
        assert_eq!(state.context.get("commit_count"), Some(&json!(1)));
        assert_eq!(state.history.len(), 1);
    }
    #[test]
    fn test_matches_tool_pattern_correctness() {
        // "group.*" should match "group.thing"
        assert!(matches_tool_pattern("group.*", "group.thing"));

        // "group.*" should NOT match "groupie.thing"
        assert!(
            !matches_tool_pattern("group.*", "groupie.thing"),
            "Bug 1: group.* matched groupie.thing (prefix issue)"
        );
    }

    #[tokio::test]
    async fn test_post_effects_persistence_and_conflict() {
        use std::sync::Arc;

        use crate::policy::session::{InMemoryPolicySessionStore, PolicyStore};

        let store = Arc::new(InMemoryPolicySessionStore::new());
        let policy_store = PolicyStore::new(store.clone());

        // Define a policy with NO effects to test implicit history saving (Bug 2)
        let policy = Policy {
            name: "test_policy".into(),
            version: "1".into(),
            context: HashMap::new(),
            effects: vec![],
        };
        policy_store.register(policy).expect("registration failed");

        let session_id = "test_session";

        // 1. Test: Implicit History Save (Bug 2)
        // This invokes evaluate_post_effects. Even with no policy effects,
        // it should save the history event.
        let res = policy_store
            .evaluate_post_effects(session_id, "some.tool", &json!({}), Ok(&json!({})))
            .await;
        assert!(res.is_ok(), "evaluate_post_effects failed: {:?}", res.err());

        // Check if history was saved
        let session = store.load(session_id).await.unwrap();
        assert_eq!(
            session.history.len(),
            1,
            "Bug 2: History was not saved when no policy effects triggered"
        );
        assert_eq!(session.version, 1, "Version should be 1");
    }

    #[test]
    fn test_history_truncation() {
        let mut history = Vec::new();
        for i in 0..60 {
            history.push(HistoryEvent {
                tool: format!("tool_{i}"),
                input: json!({}),
                success: true,
                output: None,
                error: None,
                timestamp: 0,
            });
        }

        let cel_val = to_cel_history(&history);
        match cel_val {
            Value::List(list) => {
                assert_eq!(list.len(), 5);
                // Verify it's the *last* 5 items (55 to 59)
                let first_item = &list[0];
                match first_item {
                    Value::Map(m) => {
                        let tool_key = Key::String(Arc::new("tool".to_string()));
                        let tool_val = m.map.get(&tool_key).expect("tool key missing");
                        if let Value::String(s) = tool_val {
                            assert_eq!(s.as_str(), "tool_55");
                        } else {
                            panic!("tool is not string");
                        }
                    }
                    _ => panic!("item is not map"),
                }
            }
            _ => panic!("Expected CEL List"),
        }
    }
}
