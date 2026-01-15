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

use std::{collections::HashMap, sync::Arc};

use cel_interpreter::{
    Context, ParseErrors, Program, Value,
    objects::{Key, Map as CelMap},
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;
use tracing::{debug, instrument};

pub mod session;
use session::PolicySession;

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
    /// Compiled tool pattern for matching.
    pub tool_pattern: CompiledPattern,
    /// Compiled CEL condition program.
    pub condition: Program,
    /// Compiled CEL update expressions (variable name -> program).
    pub updates: HashMap<String, Program>,
}

/// A precompiled tool pattern for efficient matching.
#[derive(Debug, Clone)]
pub struct CompiledPattern {
    segments: Vec<PatternSegment>,
}

/// A segment in a compiled pattern.
#[derive(Debug, Clone)]
enum PatternSegment {
    /// Matches exactly this literal string.
    Literal(String),
    /// Matches any single segment (*).
    Single,
    /// Matches zero or more segments (**).
    Multi,
    /// Matches a segment with ? wildcards (pattern chars).
    Wildcard(Vec<char>),
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
                tool_pattern: CompiledPattern::new(&effect.tool),
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
    /// Initializes the session context with the policy's default values.
    ///
    /// Existing session values take precedence over policy defaults.
    /// Returns `true` if any new values were added.
    fn initialize_context(&self, state: &mut PolicySession) -> bool {
        let mut modified = false;
        for (key, value) in &self.original.context {
            if !state.context.contains_key(key) {
                state.context.insert(key.clone(), value.clone());
                modified = true;
            }
        }
        modified
    }

    /// Evaluates all `Before` stage effects for a tool invocation.
    ///
    /// This is called before tool execution to enforce guards and optionally
    /// modify the policy context. Effects are evaluated in order; if any guard
    /// fails with a `fail_message`, execution is blocked.
    ///
    /// # CEL Context Variables
    ///
    /// - `context`: Policy session context (`HashMap`)
    /// - `input`: Tool input JSON
    /// - `tool`: Tool name string
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
    #[instrument(skip(self, state, input), fields(policy = %self.original.name, tool = %tool_name))]
    pub fn evaluate_pre_effects(
        &self,
        state: &mut PolicySession,
        tool_name: &str,
        input: &JsonValue,
    ) -> Result<bool, PolicyError> {
        debug!("Evaluating pre-effects");
        // Initialize session context with policy defaults (existing values take
        // precedence)
        let initialized = self.initialize_context(state);
        let mut cel_ctx = build_base_context(&state.context, input, tool_name);

        let mut modified = initialized;

        for effect in &self.effects {
            if effect.original.stage == PolicyStage::Before
                && effect.tool_pattern.matches(tool_name)
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
    /// - `input`: Tool input JSON
    /// - `tool`: Tool name string
    /// - `output`: Tool output JSON (null if error)
    /// - `error`: Error message string (only present if tool failed)
    /// - `success`: Boolean indicating if tool execution succeeded
    ///
    /// # Errors
    ///
    /// Returns `PolicyError::EvalError` if CEL evaluation fails.
    #[instrument(skip(self, state, input, output), fields(policy = %self.original.name, tool = %tool))]
    pub fn evaluate_post_effects(
        &self,
        state: &mut PolicySession,
        tool: &str,
        input: &JsonValue,
        output: Result<&JsonValue, &str>,
    ) -> Result<(), PolicyError> {
        debug!("Evaluating post-effects");
        // Initialize session context with policy defaults (existing values take
        // precedence)
        self.initialize_context(state);
        let mut cel_ctx = build_base_context(&state.context, input, tool);

        match output {
            Ok(val) => {
                cel_ctx.add_variable("output", to_cel_json(val));
                cel_ctx.add_variable("success", Value::Bool(true));
            }
            Err(e) => {
                cel_ctx.add_variable("output", Value::Null);
                cel_ctx.add_variable("error", Value::String(Arc::new(e.to_string())));
                cel_ctx.add_variable("success", Value::Bool(false));
            }
        }

        for effect in &self.effects {
            if effect.original.stage == PolicyStage::After && effect.tool_pattern.matches(tool) {
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

        Ok(())
    }
}

impl CompiledPattern {
    /// Compiles a pattern string into a `CompiledPattern`.
    pub fn new(pattern: &str) -> Self {
        let segments = pattern
            .split('.')
            .map(|s| {
                if s == "**" {
                    PatternSegment::Multi
                } else if s == "*" {
                    PatternSegment::Single
                } else if s.contains('?') {
                    PatternSegment::Wildcard(s.chars().collect())
                } else {
                    PatternSegment::Literal(s.to_string())
                }
            })
            .collect();
        Self { segments }
    }

    /// Checks if the pattern matches the given tool name.
    pub fn matches(&self, tool_id: &str) -> bool {
        let tool_parts: Vec<&str> = tool_id.split('.').collect();
        Self::matches_segments(&self.segments, &tool_parts)
    }

    fn matches_segments(pattern: &[PatternSegment], tool: &[&str]) -> bool {
        match (pattern.first(), tool.first()) {
            (None, None) => true,
            (None, Some(_)) => false,
            (Some(_), None) => pattern.iter().all(|p| matches!(p, PatternSegment::Multi)),
            (Some(p), Some(&t)) => match p {
                PatternSegment::Multi => {
                    // ** can match zero or more segments
                    Self::matches_segments(&pattern[1..], tool)
                        || Self::matches_segments(pattern, &tool[1..])
                }
                PatternSegment::Single => Self::matches_segments(&pattern[1..], &tool[1..]),
                PatternSegment::Literal(lit) if lit == t => {
                    Self::matches_segments(&pattern[1..], &tool[1..])
                }
                PatternSegment::Wildcard(chars) if Self::matches_wildcard(chars, t) => {
                    Self::matches_segments(&pattern[1..], &tool[1..])
                }
                _ => false,
            },
        }
    }

    fn matches_wildcard(pattern: &[char], segment: &str) -> bool {
        let s_chars: Vec<char> = segment.chars().collect();
        if pattern.len() != s_chars.len() {
            return false;
        }
        pattern
            .iter()
            .zip(s_chars.iter())
            .all(|(p, s)| *p == '?' || p == s)
    }
}

#[cfg(test)]
fn matches_tool_pattern(pattern: &str, tool_id: &str) -> bool {
    CompiledPattern::new(pattern).matches(tool_id)
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

/// Builds the base CEL context with common variables.
fn build_base_context(
    context: &HashMap<String, JsonValue>,
    input: &JsonValue,
    tool: &str,
) -> Context<'static> {
    let mut cel_ctx = Context::default();
    cel_ctx.add_variable("context", to_cel_value(context));
    cel_ctx.add_variable("input", to_cel_json(input));
    cel_ctx.add_variable("tool", Value::String(Arc::new(tool.to_string())));
    cel_ctx
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
                condition: "success".into(),
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
    }
    #[test]
    fn test_matches_tool_pattern_glob() {
        // Exact match
        assert!(matches_tool_pattern("foo.bar", "foo.bar"));
        assert!(!matches_tool_pattern("foo.bar", "foo.baz"));

        // Single wildcard (*) matches exactly one segment
        assert!(matches_tool_pattern("foo.*", "foo.bar"));
        assert!(!matches_tool_pattern("foo.*", "foo.bar.baz"));
        assert!(!matches_tool_pattern("foo.*", "foobar"));
        assert!(!matches_tool_pattern("group.*", "groupie.thing"));

        // Double wildcard (**) matches zero or more segments
        assert!(matches_tool_pattern("foo.**", "foo"));
        assert!(matches_tool_pattern("foo.**", "foo.bar"));
        assert!(matches_tool_pattern("foo.**", "foo.bar.baz"));
        assert!(matches_tool_pattern("**", "anything.at.all"));
        assert!(matches_tool_pattern("foo.**.baz", "foo.baz"));
        assert!(matches_tool_pattern("foo.**.baz", "foo.bar.baz"));
        assert!(matches_tool_pattern("foo.**.baz", "foo.bar.qux.baz"));

        // Single char wildcard (?)
        assert!(matches_tool_pattern("foo.b?r", "foo.bar"));
        assert!(matches_tool_pattern("foo.b?r", "foo.bxr"));
        assert!(!matches_tool_pattern("foo.b?r", "foo.baar"));

        // Mixed patterns
        assert!(matches_tool_pattern("*.bar", "foo.bar"));
        assert!(matches_tool_pattern("**.bar", "foo.baz.bar"));
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

        // Check session was saved
        let session = store.load(session_id).await.unwrap();
        assert_eq!(session.version, 1, "Version should be 1");
    }
}
