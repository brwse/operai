//! Context for tool invocations.

use std::collections::HashMap;

use operai_abi::CallContext;
use rkyv::rancor::BoxedError;
use serde::de::DeserializeOwned;

use crate::credential::CredentialError;

/// Provides access to request metadata and credentials during tool invocation.
///
/// System credentials come from environment variables and persist across
/// invocations. User credentials are per-request and passed via gRPC.
#[derive(Debug, Clone)]
pub struct Context {
    request_id: String,
    session_id: String,
    user_id: String,
    system_credentials: HashMap<String, HashMap<String, String>>,
    user_credentials: HashMap<String, HashMap<String, String>>,
}

impl Context {
    /// Creates a new context from an FFI `CallContext`.
    ///
    /// This is only intended for use by the `generate_tool_entrypoint!` macro.
    #[doc(hidden)]
    #[must_use]
    pub fn __from_call_context(call_ctx: &CallContext<'_>) -> Self {
        let request_id = call_ctx.request_id.to_string();
        let session_id = call_ctx.session_id.to_string();
        let user_id = call_ctx.user_id.to_string();

        let user_credentials: HashMap<String, HashMap<String, String>> =
            if call_ctx.user_credentials.is_empty() {
                HashMap::new()
            } else {
                rkyv::from_bytes::<_, BoxedError>(call_ctx.user_credentials.as_slice())
                    .unwrap_or_default()
            };

        let system_credentials: HashMap<String, HashMap<String, String>> =
            if call_ctx.system_credentials.is_empty() {
                HashMap::new()
            } else {
                rkyv::from_bytes::<_, BoxedError>(call_ctx.system_credentials.as_slice())
                    .unwrap_or_default()
            };

        Self {
            request_id,
            session_id,
            user_id,
            system_credentials,
            user_credentials,
        }
    }

    /// Creates an empty context useful for testing.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            request_id: String::new(),
            session_id: String::new(),
            user_id: String::new(),
            system_credentials: HashMap::new(),
            user_credentials: HashMap::new(),
        }
    }

    /// Creates a context with the specified metadata, useful for testing.
    #[must_use]
    pub fn with_metadata(request_id: &str, session_id: &str, user_id: &str) -> Self {
        Self {
            request_id: request_id.to_string(),
            session_id: session_id.to_string(),
            user_id: user_id.to_string(),
            system_credentials: HashMap::new(),
            user_credentials: HashMap::new(),
        }
    }

    /// Server-generated UUID for correlating logs across components.
    #[must_use]
    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    /// Optional client-provided identifier for stateful tool interactions.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// The authenticated user's ID from the OIDC token.
    #[must_use]
    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    /// Retrieves a system credential by name, deserializing into the requested
    /// type.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialError::NotFound`] if the credential doesn't exist,
    /// or [`CredentialError::DeserializationError`] if deserialization fails.
    pub fn system_credential<T: DeserializeOwned>(&self, name: &str) -> Result<T, CredentialError> {
        Self::get_credential(&self.system_credentials, name)
    }

    /// Retrieves a user credential by name, deserializing into the requested
    /// type.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialError::NotFound`] if the credential doesn't exist,
    /// or [`CredentialError::DeserializationError`] if deserialization fails.
    pub fn user_credential<T: DeserializeOwned>(&self, name: &str) -> Result<T, CredentialError> {
        Self::get_credential(&self.user_credentials, name)
    }

    fn get_credential<T: DeserializeOwned>(
        store: &HashMap<String, HashMap<String, String>>,
        name: &str,
    ) -> Result<T, CredentialError> {
        let cred_map = store
            .get(name)
            .ok_or_else(|| CredentialError::NotFound(name.to_string()))?;

        serde_json::to_value(cred_map)
            .and_then(serde_json::from_value)
            .map_err(|e| CredentialError::DeserializationError(e.to_string()))
    }

    /// Adds a system credential for testing.
    #[must_use]
    pub fn with_system_credential(mut self, name: &str, values: HashMap<String, String>) -> Self {
        self.system_credentials.insert(name.to_string(), values);
        self
    }

    /// Adds a user credential for testing.
    #[must_use]
    pub fn with_user_credential(mut self, name: &str, values: HashMap<String, String>) -> Self {
        self.user_credentials.insert(name.to_string(), values);
        self
    }
}

#[cfg(test)]
mod tests {
    use operai_abi::abi_stable::std_types::{RSlice, RStr};
    use rkyv::rancor::BoxedError;
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestCred {
        api_key: String,
        endpoint: Option<String>,
    }

    #[test]
    fn test_empty_context() {
        let ctx = Context::empty();
        assert!(ctx.request_id().is_empty());
        assert!(ctx.session_id().is_empty());
        assert!(ctx.user_id().is_empty());
    }

    #[test]
    fn test_context_with_metadata() {
        let ctx = Context::with_metadata("req-123", "sess-456", "user-789");
        assert_eq!(ctx.request_id(), "req-123");
        assert_eq!(ctx.session_id(), "sess-456");
        assert_eq!(ctx.user_id(), "user-789");
    }

    #[test]
    fn test_system_credential() {
        let mut values = HashMap::new();
        values.insert("api_key".to_string(), "secret123".to_string());

        let ctx = Context::empty().with_system_credential("api", values);

        let cred: TestCred = ctx.system_credential("api").unwrap();
        assert_eq!(cred.api_key, "secret123");
        assert_eq!(cred.endpoint, None);
    }

    #[test]
    fn test_user_credential() {
        let mut values = HashMap::new();
        values.insert("api_key".to_string(), "user-secret".to_string());
        values.insert(
            "endpoint".to_string(),
            "https://api.example.com".to_string(),
        );

        let ctx = Context::empty().with_user_credential("api", values);

        let cred: TestCred = ctx.user_credential("api").unwrap();
        assert_eq!(cred.api_key, "user-secret");
        assert_eq!(cred.endpoint, Some("https://api.example.com".to_string()));
    }

    #[test]
    fn test_system_credential_not_found_includes_name_and_display_message() {
        // Arrange
        let ctx = Context::empty();

        // Act
        let result: Result<TestCred, _> = ctx.system_credential("nonexistent");

        // Assert
        let err = result.unwrap_err();
        assert!(matches!(err, CredentialError::NotFound(ref name) if name == "nonexistent"));
        assert_eq!(err.to_string(), "credential 'nonexistent' not found");
    }

    #[test]
    fn test_user_credential_not_found_includes_name_and_display_message() {
        // Arrange
        let ctx = Context::empty();

        // Act
        let result: Result<TestCred, _> = ctx.user_credential("missing");

        // Assert
        let err = result.unwrap_err();
        assert!(matches!(err, CredentialError::NotFound(ref name) if name == "missing"));
        assert_eq!(err.to_string(), "credential 'missing' not found");
    }

    #[test]
    fn test_system_credential_with_missing_required_field_returns_deserialization_error() {
        // Arrange
        let mut values = HashMap::new();
        values.insert(
            "endpoint".to_string(),
            "https://api.example.com".to_string(),
        );
        let ctx = Context::empty().with_system_credential("api", values);

        // Act
        let result: Result<TestCred, _> = ctx.system_credential("api");

        // Assert
        let err = result.unwrap_err();
        assert!(
            matches!(err, CredentialError::DeserializationError(ref msg) if msg == "missing field `api_key`")
        );
        assert_eq!(
            err.to_string(),
            "failed to deserialize credential: missing field `api_key`"
        );
    }

    #[test]
    fn test_from_call_context_copies_metadata_and_decodes_credentials() {
        // Arrange
        let mut system_values = HashMap::new();
        system_values.insert("api_key".to_string(), "sys-secret".to_string());

        let mut user_values = HashMap::new();
        user_values.insert("api_key".to_string(), "user-secret".to_string());
        user_values.insert(
            "endpoint".to_string(),
            "https://api.example.com".to_string(),
        );

        let mut system_credentials = HashMap::new();
        system_credentials.insert("api".to_string(), system_values);

        let mut user_credentials = HashMap::new();
        user_credentials.insert("api".to_string(), user_values);

        let system_creds_bin = rkyv::to_bytes::<BoxedError>(&system_credentials).unwrap();
        let user_creds_bin = rkyv::to_bytes::<BoxedError>(&user_credentials).unwrap();

        let request_id = "req-123".to_string();
        let session_id = "sess-456".to_string();
        let user_id = "user-789".to_string();

        let call_ctx = CallContext {
            request_id: RStr::from_str(&request_id),
            session_id: RStr::from_str(&session_id),
            user_id: RStr::from_str(&user_id),
            user_credentials: RSlice::from_slice(&user_creds_bin),
            system_credentials: RSlice::from_slice(&system_creds_bin),
        };

        // Act
        let ctx = Context::__from_call_context(&call_ctx);

        // Assert
        assert_eq!(ctx.request_id(), "req-123");
        assert_eq!(ctx.session_id(), "sess-456");
        assert_eq!(ctx.user_id(), "user-789");

        let system_cred: TestCred = ctx.system_credential("api").unwrap();
        assert_eq!(system_cred.api_key, "sys-secret");
        assert_eq!(system_cred.endpoint, None);

        let user_cred: TestCred = ctx.user_credential("api").unwrap();
        assert_eq!(user_cred.api_key, "user-secret");
        assert_eq!(
            user_cred.endpoint,
            Some("https://api.example.com".to_string())
        );
    }

    #[test]
    fn test_multiple_credentials_are_independent() {
        // Arrange
        let mut api_values = HashMap::new();
        api_values.insert("api_key".to_string(), "api-secret".to_string());

        let mut db_values = HashMap::new();
        db_values.insert("api_key".to_string(), "db-secret".to_string());
        db_values.insert("endpoint".to_string(), "postgres://localhost".to_string());

        let ctx = Context::empty()
            .with_system_credential("api", api_values)
            .with_system_credential("database", db_values);

        // Act
        let api_cred: TestCred = ctx.system_credential("api").unwrap();
        let db_cred: TestCred = ctx.system_credential("database").unwrap();

        // Assert
        assert_eq!(api_cred.api_key, "api-secret");
        assert_eq!(api_cred.endpoint, None);
        assert_eq!(db_cred.api_key, "db-secret");
        assert_eq!(db_cred.endpoint, Some("postgres://localhost".to_string()));
    }

    #[test]
    fn test_cloned_context_retains_credentials() {
        // Arrange
        let mut values = HashMap::new();
        values.insert("api_key".to_string(), "secret".to_string());

        let original = Context::with_metadata("req-1", "sess-1", "user-1")
            .with_system_credential("api", values);

        // Act
        let cloned = original.clone();

        // Assert
        assert_eq!(cloned.request_id(), "req-1");
        assert_eq!(cloned.session_id(), "sess-1");
        assert_eq!(cloned.user_id(), "user-1");

        let cred: TestCred = cloned.system_credential("api").unwrap();
        assert_eq!(cred.api_key, "secret");
    }

    #[test]
    fn test_system_and_user_credentials_are_separate_namespaces() {
        // Arrange
        let mut system_values = HashMap::new();
        system_values.insert("api_key".to_string(), "system-secret".to_string());

        let mut user_values = HashMap::new();
        user_values.insert("api_key".to_string(), "user-secret".to_string());

        let ctx = Context::empty()
            .with_system_credential("api", system_values)
            .with_user_credential("api", user_values);

        // Act
        let system_cred: TestCred = ctx.system_credential("api").unwrap();
        let user_cred: TestCred = ctx.user_credential("api").unwrap();

        // Assert - same credential name, different values
        assert_eq!(system_cred.api_key, "system-secret");
        assert_eq!(user_cred.api_key, "user-secret");
    }

    #[test]
    fn test_credential_with_empty_string_value_deserializes() {
        // Arrange
        let mut values = HashMap::new();
        values.insert("api_key".to_string(), String::new());

        let ctx = Context::empty().with_system_credential("api", values);

        // Act
        let cred: TestCred = ctx.system_credential("api").unwrap();

        // Assert
        assert_eq!(cred.api_key, "");
    }

    #[test]
    fn test_from_call_context_with_empty_credentials_returns_empty_stores() {
        // Arrange
        let request_id = "req-123".to_string();
        let session_id = "sess-456".to_string();
        let user_id = "user-789".to_string();

        let call_ctx = CallContext {
            request_id: RStr::from_str(&request_id),
            session_id: RStr::from_str(&session_id),
            user_id: RStr::from_str(&user_id),
            user_credentials: RSlice::from_slice(&[]),
            system_credentials: RSlice::from_slice(&[]),
        };

        // Act
        let ctx = Context::__from_call_context(&call_ctx);

        // Assert
        assert_eq!(ctx.request_id(), "req-123");
        let result: Result<TestCred, _> = ctx.system_credential("any");
        assert!(matches!(result, Err(CredentialError::NotFound(_))));
    }

    #[test]
    fn test_context_debug_output_includes_metadata() {
        // Arrange
        let ctx = Context::with_metadata("req-abc", "sess-xyz", "user-123");

        // Act
        let debug = format!("{ctx:?}");

        // Assert
        assert!(debug.contains("req-abc"));
        assert!(debug.contains("sess-xyz"));
        assert!(debug.contains("user-123"));
    }
}
