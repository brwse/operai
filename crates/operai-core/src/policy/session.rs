use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use thiserror::Error;

use super::{CompiledPolicy, Policy};

/// Runtime policy session state.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PolicySession {
    /// Optimistic concurrency version.
    #[serde(default)]
    pub version: u64,
    pub context: HashMap<String, JsonValue>,
    pub history: Vec<HistoryEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEvent {
    pub tool: String,
    pub input: JsonValue,
    // We store output only if success, potentially errors too?
    // For now simplistic.
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub timestamp: u64,
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("Session conflict: expected version {expected}, found {found}")]
    Conflict { expected: u64, found: u64 },
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Lock poisoned")]
    LockPoisoned,
}

use crate::PolicyError;

#[async_trait]
pub trait PolicySessionStore: std::fmt::Debug + Send + Sync {
    /// Load a session by ID. Returns Default if not found.
    async fn load(&self, session_id: &str) -> Result<PolicySession, SessionError>;

    /// Save a session (with OCC).
    async fn save(&self, session_id: &str, session: &PolicySession) -> Result<(), SessionError>;
}

/// In-memory implementation of `PolicySessionStore`.
#[derive(Debug, Default)]
pub struct InMemoryPolicySessionStore {
    sessions: RwLock<HashMap<String, PolicySession>>,
}

impl InMemoryPolicySessionStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl PolicySessionStore for InMemoryPolicySessionStore {
    async fn load(&self, session_id: &str) -> Result<PolicySession, SessionError> {
        let map = self
            .sessions
            .read()
            .map_err(|_| SessionError::LockPoisoned)?;
        Ok(map.get(session_id).cloned().unwrap_or_default())
    }

    async fn save(&self, session_id: &str, session: &PolicySession) -> Result<(), SessionError> {
        let mut map = self
            .sessions
            .write()
            .map_err(|_| SessionError::LockPoisoned)?;
        let current = map.entry(session_id.to_string()).or_default();

        if current.version != session.version {
            return Err(SessionError::Conflict {
                expected: current.version,
                found: session.version,
            });
        }

        let mut new_session = session.clone();
        new_session.version += 1;
        *current = new_session;
        Ok(())
    }
}

/// Registry that manages Policies and their Session state.
#[derive(Debug)]
pub struct PolicyStore {
    policies: RwLock<HashMap<String, CompiledPolicy>>,
    store: Arc<dyn PolicySessionStore + Send + Sync>,
}

impl PolicyStore {
    pub fn new(store: Arc<dyn PolicySessionStore + Send + Sync>) -> Self {
        Self {
            policies: RwLock::new(HashMap::new()),
            store,
        }
    }

    /// Registers a policy.
    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    ///
    /// # Errors
    /// Returns `PolicyError` if compilation fails.
    pub fn register(&self, policy: Policy) -> Result<(), PolicyError> {
        let compiled = policy.compile()?;
        let mut map = self.policies.write().expect("lock poisoned");
        map.insert(compiled.original.name.clone(), compiled);
        Ok(())
    }

    /// Retrieves a policy by name.
    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    pub fn get(&self, name: &str) -> Option<Policy> {
        self.policies
            .read()
            .expect("lock poisoned")
            .get(name)
            .map(|cp| cp.original.clone())
    }

    /// Evaluates "Before" effects (Guards & Reservations) for all policies.
    /// Performs atomic state reservation using OCC.
    ///
    /// # Errors
    /// Returns `PolicyError` if evaluation fails or conflict resolution fails
    /// after retries.
    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    pub async fn evaluate_pre_effects(
        &self,
        session_id: &str,
        tool: &str,
        input: &JsonValue,
    ) -> Result<(), PolicyError> {
        // Simple retry loop for OCC (Reservation)
        const MAX_RETRIES: u32 = 3;
        for _ in 0..MAX_RETRIES {
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

            match self.store.save(session_id, &session).await {
                Ok(()) => return Ok(()),
                Err(SessionError::Conflict { .. }) => {} // Retry
                Err(e) => {
                    return Err(PolicyError::EvalError(format!(
                        "Failed to save session: {e}"
                    )));
                }
            }
        }

        Err(PolicyError::EvalError(
            "Failed to reserve session after retries due to conflicts".into(),
        ))
    }

    /// Evaluates "After" effects for all policies.
    ///
    /// # Errors
    /// Returns `PolicyError` if evaluation fails or conflict resolution fails
    /// after retries.
    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    pub async fn evaluate_post_effects(
        &self,
        session_id: &str,
        tool: &str,
        input: &JsonValue,
        output: Result<&JsonValue, &str>,
    ) -> Result<(), PolicyError> {
        // Simple retry loop for OCC
        const MAX_RETRIES: u32 = 3;
        for _ in 0..MAX_RETRIES {
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

            match self.store.save(session_id, &session).await {
                Ok(()) => return Ok(()),
                Err(SessionError::Conflict { .. }) => {} // Retry
                Err(e) => {
                    return Err(PolicyError::EvalError(format!(
                        "Failed to save session: {e}"
                    )));
                }
            }
        }

        Err(PolicyError::EvalError(
            "Failed to save session after retries due to conflicts".into(),
        ))
    }
}
