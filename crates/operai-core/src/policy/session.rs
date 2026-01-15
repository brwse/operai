//! Session management for policy evaluation with optimistic concurrency
//! control.
//!
//! This module provides the storage and runtime infrastructure for maintaining
//! policy evaluation state across multiple tool executions. Sessions track
//! context variables, enabling policies to make decisions
//! based on prior operations.
//!
//! # Key Concepts
//!
//! - **Session Version**: Incremented on each save to detect concurrent
//!   modifications
//! - **Optimistic Concurrency Control (OCC)**: Conflicts are detected via
//!   version checks and resolved through retry loops
//! - **History**: Chronological record of tool executions for policy evaluation
//!
//! # Concurrency Model
//!
//! The session store uses optimistic locking:
//! 1. Load session at version V
//! 2. Modify session (keeping version V)
//! 3. Save with version check (V == `V_current`)
//! 4. On conflict, retry from step 1
//!
//! # Example
//!
//! ```ignore
//! let store = Arc::new(InMemoryPolicySessionStore::new());
//! let policy_store = PolicyStore::new(store.clone());
//!
//! // Register a policy
//! policy_store.register(policy)?;
//!
//! // Evaluate pre-effects (before tool execution)
//! policy_store.evaluate_pre_effects("session_id", "tool_name", &input).await?;
//!
//! // Execute tool...
//!
//! // Evaluate post-effects (after tool execution)
//! policy_store.evaluate_post_effects("session_id", "tool_name", &input, Ok(&output)).await?;
//! ```

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use async_trait::async_trait;
use backon::{ExponentialBuilder, Retryable};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;
use tracing::{debug, instrument, warn};

use super::{CompiledPolicy, Policy};

/// A policy evaluation session that maintains state across tool executions.
///
/// Sessions are versioned to detect concurrent modifications using optimistic
/// concurrency control. Each successful save increments the version, and
/// conflicting saves are rejected.
///
/// # Fields
///
/// - `version`: Monotonically increasing version number for OCC
/// - `context`: Arbitrary key-value pairs accessible in CEL expressions
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PolicySession {
    /// Version number for optimistic concurrency control.
    ///
    /// Incremented on each successful save. Used to detect and reject
    /// conflicting updates from concurrent operations.
    #[serde(default)]
    pub version: u64,
    /// Context variables accessible to policy expressions.
    ///
    /// This map is exposed as the `context` variable in CEL conditions and
    /// effect updates. Policies can read and modify these values.
    pub context: HashMap<String, JsonValue>,
}

/// Errors that can occur during session storage operations.
#[derive(Debug, Error)]
pub enum SessionError {
    /// Optimistic concurrency control conflict detected.
    ///
    /// This occurs when attempting to save a session with a version number
    /// that doesn't match the current stored version, indicating the session
    /// was modified by another operation after it was loaded.
    #[error("Session conflict: expected version {expected}, found {found}")]
    Conflict { expected: u64, found: u64 },
    /// Underlying storage backend error.
    #[error("Storage error: {0}")]
    Storage(String),
    /// Internal lock poisoned, indicating concurrent access failure.
    ///
    /// This is a fatal error indicating the internal synchronization primitive
    /// was corrupted. Usually caused by a thread panicking while holding the
    /// lock.
    #[error("Lock poisoned")]
    LockPoisoned,
}

use crate::PolicyError;

/// Async storage interface for policy sessions.
///
/// Implementations must be thread-safe (`Send + Sync`) to support concurrent
/// policy evaluations. The trait uses optimistic concurrency control: callers
/// should handle `SessionError::Conflict` by reloading the session and
/// retrying.
///
/// # Required Methods
///
/// - [`Self::load`]: Retrieve a session by ID
/// - [`Self::save`]: Persist a session with version checking
#[async_trait]
pub trait PolicySessionStore: std::fmt::Debug + Send + Sync {
    /// Load a session from storage.
    ///
    /// Returns a default (empty) session if the ID doesn't exist, allowing
    /// lazy session creation.
    async fn load(&self, session_id: &str) -> Result<PolicySession, SessionError>;

    /// Save a session to storage.
    ///
    /// Implementations must perform optimistic concurrency control by verifying
    /// the session version matches the stored version before saving. If
    /// versions don't match, this must return `SessionError::Conflict` to
    /// signal the caller to retry.
    async fn save(&self, session_id: &str, session: &PolicySession) -> Result<(), SessionError>;
}

/// In-memory implementation of [`PolicySessionStore`].
///
/// This implementation stores sessions in a `HashMap` protected by a `RwLock`.
/// It's primarily useful for testing and single-process scenarios. For
/// production use with multiple processes or persistence requirements,
/// implement [`PolicySessionStore`] with a proper backend (database, file
/// system, etc.).
///
/// # Concurrency
///
/// Uses a read-write lock to allow concurrent reads while writes are exclusive.
/// The lock can be poisoned if a thread panics while holding it, which will
/// result in `SessionError::LockPoisoned` on subsequent operations.
#[derive(Debug, Default)]
pub struct InMemoryPolicySessionStore {
    sessions: RwLock<HashMap<String, PolicySession>>,
}

impl InMemoryPolicySessionStore {
    /// Create a new empty in-memory session store.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl PolicySessionStore for InMemoryPolicySessionStore {
    #[instrument(skip(self), fields(session_id = %session_id))]
    async fn load(&self, session_id: &str) -> Result<PolicySession, SessionError> {
        let map = self
            .sessions
            .read()
            .map_err(|_| SessionError::LockPoisoned)?;
        Ok(map.get(session_id).cloned().unwrap_or_default())
    }

    #[instrument(skip(self, session), fields(session_id = %session_id, version = session.version))]
    async fn save(&self, session_id: &str, session: &PolicySession) -> Result<(), SessionError> {
        let mut map = self
            .sessions
            .write()
            .map_err(|_| SessionError::LockPoisoned)?;
        let current = map.entry(session_id.to_string()).or_default();

        // Optimistic concurrency control: verify version before updating
        if current.version != session.version {
            return Err(SessionError::Conflict {
                expected: current.version,
                found: session.version,
            });
        }

        // Increment version for the next save
        let mut new_session = session.clone();
        new_session.version += 1;
        *current = new_session;
        Ok(())
    }
}

/// Registry for compiled policies with session-aware evaluation.
///
/// The `PolicyStore` manages a collection of compiled policies and provides
/// methods to evaluate them across sessions with automatic retry logic for
/// handling concurrent modifications.
///
/// # Policy Evaluation
///
/// Policies are evaluated in two stages:
/// - **Pre-effects**: Before tool execution, can block execution or modify
///   context
/// - **Post-effects**: After tool execution, can modify context
///
/// # Concurrency
///
/// Uses optimistic concurrency control with retry loops (up to 3 attempts) to
/// handle conflicts from concurrent policy evaluations.
#[derive(Debug)]
pub struct PolicyStore {
    /// Compiled policies indexed by name.
    policies: RwLock<HashMap<String, CompiledPolicy>>,
    /// Session storage backend.
    store: Arc<dyn PolicySessionStore + Send + Sync>,
}

impl PolicyStore {
    /// Create a new policy store with the given session storage backend.
    pub fn new(store: Arc<dyn PolicySessionStore + Send + Sync>) -> Self {
        Self {
            policies: RwLock::new(HashMap::new()),
            store,
        }
    }

    /// Register a policy for evaluation.
    ///
    /// The policy is compiled and stored by name. If a policy with the same
    /// name already exists, it will be replaced.
    ///
    /// # Errors
    ///
    /// Returns `Err(PolicyError)` if the policy fails to compile or if the
    /// policy lock is poisoned.
    #[instrument(skip(self, policy), fields(policy_name = %policy.name))]
    pub fn register(&self, policy: Policy) -> Result<(), PolicyError> {
        debug!("Registering policy");
        let compiled = policy.compile()?;
        let mut map = self
            .policies
            .write()
            .map_err(|_| PolicyError::EvalError("policy lock poisoned".into()))?;
        map.insert(compiled.original.name.clone(), compiled);
        Ok(())
    }

    /// Retrieve a registered policy by name.
    ///
    /// Returns `None` if the policy doesn't exist or if the lock is poisoned.
    pub fn get(&self, name: &str) -> Option<Policy> {
        self.policies
            .read()
            .ok()?
            .get(name)
            .map(|cp| cp.original.clone())
    }

    /// Evaluate pre-effects for all registered policies.
    ///
    /// This method evaluates the "before" stage of all policies that match the
    /// given tool. It handles concurrent modifications via optimistic
    /// concurrency control, retrying up to 3 times on conflict.
    ///
    /// # Behavior
    ///
    /// - Loads the session from storage
    /// - Evaluates all matching pre-effects
    /// - Saves the session if any policies modified it
    /// - Returns early if no modifications were made (optimization)
    ///
    /// # Errors
    ///
    /// Returns `PolicyError::GuardFailed` if any policy's guard condition
    /// fails. Returns `PolicyError::EvalError` if session operations fail
    /// after retries.
    ///
    /// # Panics
    ///
    /// Panics if the policy lock is poisoned (indicating a previous writer
    /// thread panicked while holding the read lock).
    #[instrument(skip(self, input), fields(session_id = %session_id, tool = %tool))]
    pub async fn evaluate_pre_effects(
        &self,
        session_id: &str,
        tool: &str,
        input: &JsonValue,
    ) -> Result<(), PolicyError> {
        let operation = || async {
            let mut session = self
                .store
                .load(session_id)
                .await
                .map_err(|e| PolicyError::EvalError(format!("Failed to load session: {e}")))?;

            let mut any_modified = false;
            {
                let policies = self.policies.read().expect("lock poisoned");
                for policy in policies.values() {
                    if policy.evaluate_pre_effects(&mut session, tool, input)? {
                        any_modified = true;
                    }
                }
            }

            if !any_modified {
                // No changes, no need to save or check conflicts.
                return Ok(());
            }

            self.store.save(session_id, &session).await.map_err(|e| {
                if matches!(e, SessionError::Conflict { .. }) {
                    PolicyError::SessionConflict
                } else {
                    PolicyError::EvalError(format!("Failed to save session: {e}"))
                }
            })
        };

        operation
            .retry(
                ExponentialBuilder::default()
                    .with_min_delay(Duration::from_millis(10))
                    .with_max_delay(Duration::from_millis(100))
                    .with_max_times(3)
                    .with_jitter(),
            )
            .when(|e| matches!(e, PolicyError::SessionConflict))
            .await
            .map_err(|e| match e {
                PolicyError::SessionConflict => PolicyError::EvalError(
                    "Failed to reserve session after retries due to conflicts".into(),
                ),
                other => other,
            })
    }

    /// Evaluate post-effects for all registered policies.
    ///
    /// This method evaluates the "after" stage of all policies that match the
    /// given tool.
    ///
    /// # Behavior
    ///
    /// - Loads the session from storage
    /// - Evaluates all matching post-effects
    /// - Saves the session if context was modified
    /// - Retries up to 3 times on conflict
    ///
    /// # Panics
    ///
    /// Panics if the policy lock is poisoned (indicating a previous writer
    /// thread panicked while holding the read lock).
    ///
    /// # Errors
    ///
    /// Returns `PolicyError::EvalError` if session operations fail after
    /// retries.
    #[instrument(skip(self, input, output), fields(session_id = %session_id, tool = %tool))]
    pub async fn evaluate_post_effects(
        &self,
        session_id: &str,
        tool: &str,
        input: &JsonValue,
        output: Result<&JsonValue, &str>,
    ) -> Result<(), PolicyError> {
        let operation = || async {
            let mut session = self
                .store
                .load(session_id)
                .await
                .map_err(|e| PolicyError::EvalError(format!("Failed to load session: {e}")))?;

            {
                let policies = self.policies.read().expect("lock poisoned");
                for policy in policies.values() {
                    policy.evaluate_post_effects(&mut session, tool, input, output)?;
                }
            }

            self.store.save(session_id, &session).await.map_err(|e| {
                if matches!(e, SessionError::Conflict { .. }) {
                    PolicyError::SessionConflict
                } else {
                    PolicyError::EvalError(format!("Failed to save session: {e}"))
                }
            })
        };

        operation
            .retry(
                ExponentialBuilder::default()
                    .with_min_delay(Duration::from_millis(10))
                    .with_max_delay(Duration::from_millis(100))
                    .with_max_times(3)
                    .with_jitter(),
            )
            .when(|e| matches!(e, PolicyError::SessionConflict))
            .await
            .map_err(|e| match e {
                PolicyError::SessionConflict => PolicyError::EvalError(
                    "Failed to save session after retries due to conflicts".into(),
                ),
                other => other,
            })
    }
}
