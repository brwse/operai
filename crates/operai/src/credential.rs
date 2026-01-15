
//! Credential schema definitions and runtime introspection.
//!
//! This module provides types for defining and registering credential schemas
//! that can be discovered at runtime. Credentials are used to authenticate with
//! external services and APIs (e.g., API keys, OAuth tokens, database credentials).
//!
//! # Defining Credentials
//!
//! Credentials are defined using the `define_system_credential!` and
//! `define_user_credential!` macros from the `operai-macro` crate:
//!
//! ```ignore
//! use operai::define_system_credential;
//!
//! define_system_credential!(ApiKey("api_key") {
//!     /// API key for authentication
//!     key: String,
//!     #[optional]
//!     /// Optional endpoint override
//!     endpoint: Option<String>,
//! });
//! ```
//!
//! The macro generates:
//! - A struct with the provided fields
//! - A `get(ctx: &Context)` method for retrieving the credential
//! - Registration with the credential inventory
//!
//! # Using Credentials
//!
//! To retrieve a credential value, call the generated `get()` method:
//!
//! ```ignore
//! #[operai::tool]
//! async fn my_tool(ctx: operai::Context, input: Input) -> operai::Result<Output> {
//!     let api_key = ApiKey::get(&ctx)?;
//!     Ok(Output { /* use api_key.key */ })
//! }
//! ```
//!
//! # Credential Namespaces
//!
//! Credentials are organized into two separate namespaces:
//!
//! - **System credentials**: Provider-level credentials configured by the operator
//! - **User credentials**: User-specific credentials for authentication
//!
//! Both namespaces are independent, allowing the same credential name to exist in both
//! with different values. Use `define_system_credential!` for system credentials and
//! `define_user_credential!` for user credentials.
//!
//! # Runtime Discovery
//!
//! Registered credentials can be iterated at runtime using `inventory::iter`:
//!
//! ```ignore
//! use operai::credential::CredentialEntry;
//!
//! for entry in inventory::iter::<CredentialEntry>() {
//!     println!("Credential: {}", entry.name);
//!     for (field_name, field_schema) in entry.fields {
//!         println!("  - {}: {} (required: {})",
//!             field_name, field_schema.description, field_schema.required);
//!     }
//! }
//! ```

use serde::Serialize;

use crate::entrypoint::Sealed;

/// Errors that can occur when working with credentials.
///
/// This enum represents the various failure modes that can arise during
/// credential lookup, deserialization, or validation.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CredentialError {
    /// A requested credential could not be found.
    ///
    /// This typically occurs when the runtime attempts to retrieve a credential
    /// by name but no matching credential has been registered or configured.
    #[error("credential '{0}' not found")]
    NotFound(String),

    /// Failed to deserialize credential data.
    ///
    /// This occurs when credential data exists but cannot be parsed into its
    /// expected type, typically due to malformed data or schema mismatches.
    #[error("failed to deserialize credential: {0}")]
    DeserializationError(String),
}

/// Schema definition for a single credential field.
///
/// This struct describes the metadata for a field within a credential schema,
/// including its purpose and whether it is mandatory or optional.
#[derive(Debug, Clone, Serialize)]
pub struct CredentialFieldSchema {
    /// Human-readable description of what this field represents.
    pub description: &'static str,

    /// Whether this field must be provided for the credential to be valid.
    pub required: bool,
}

/// A registered credential type schema.
///
/// `CredentialEntry` represents a credential type that can be discovered and
/// used at runtime. Each entry defines the structure of a specific credential
/// type, including its name, description, and the fields it requires.
///
/// # Sealed
///
/// The `__sealed` field prevents instances of this type from being constructed
/// outside of the registration mechanism. This ensures all credential entries
/// go through the proper `inventory::submit!` process.
///
/// # Serialization
///
/// When serialized, `CredentialEntry` produces a JSON object with the following structure:
///
/// ```json
/// {
///   "name": "credential_name",
///   "description": "Human-readable description",
///   "fields": {
///     "field_name": {
///       "description": "Field description",
///       "required": true
///     }
///   }
/// }
/// ```
///
/// Note that the `__sealed` field is intentionally excluded from serialization.
#[derive(Debug)]
pub struct CredentialEntry {
    /// Unique identifier for this credential type.
    pub name: &'static str,

    /// Human-readable description of this credential type.
    pub description: &'static str,

    /// Ordered list of field definitions for this credential.
    ///
    /// Each tuple contains the field name and its schema definition.
    pub fields: &'static [(&'static str, CredentialFieldSchema)],

    /// Seal to prevent construction outside of registration.
    #[doc(hidden)]
    pub __sealed: Sealed,
}

impl Serialize for CredentialEntry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::{SerializeMap, SerializeStruct};

        struct FieldsMap<'a>(&'a [(&'static str, CredentialFieldSchema)]);

        impl Serialize for FieldsMap<'_> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let mut map = serializer.serialize_map(Some(self.0.len()))?;
                for (key, value) in self.0 {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }

        let mut state = serializer.serialize_struct("CredentialEntry", 3)?;
        state.serialize_field("name", self.name)?;
        state.serialize_field("description", self.description)?;
        state.serialize_field("fields", &FieldsMap(self.fields))?;
        state.end()
    }
}

/// Collect credential entries for runtime introspection.
///
/// This macro invocation registers `CredentialEntry` with the `inventory` crate,
/// enabling runtime discovery of all registered credential types via
/// `inventory::iter::<CredentialEntry>()`.
inventory::collect!(CredentialEntry);

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::entrypoint::Sealed;

    #[test]
    fn test_credential_error_not_found_display_message() {
        let error = CredentialError::NotFound("api".to_string());
        assert_eq!(error.to_string(), "credential 'api' not found");
    }

    #[test]
    fn test_credential_error_deserialization_display_message() {
        let error = CredentialError::DeserializationError("bad data".to_string());
        assert_eq!(
            error.to_string(),
            "failed to deserialize credential: bad data"
        );
    }

    #[test]
    fn test_credential_field_schema_serializes_to_expected_json() {
        let schema = CredentialFieldSchema {
            description: "API key used for authentication",
            required: true,
        };

        let value = serde_json::to_value(schema).unwrap();
        assert_eq!(
            value,
            json!({
                "description": "API key used for authentication",
                "required": true,
            })
        );
    }

    #[test]
    fn test_credential_entry_serializes_fields_as_map() {
        static FIELDS: [(&str, CredentialFieldSchema); 2] = [
            (
                "api_key",
                CredentialFieldSchema {
                    description: "API key",
                    required: true,
                },
            ),
            (
                "endpoint",
                CredentialFieldSchema {
                    description: "API endpoint",
                    required: false,
                },
            ),
        ];

        let entry = CredentialEntry {
            name: "api",
            description: "API credentials",
            fields: &FIELDS,
            __sealed: Sealed(()),
        };

        let value = serde_json::to_value(entry).unwrap();
        assert_eq!(
            value,
            json!({
                "name": "api",
                "description": "API credentials",
                "fields": {
                    "api_key": {
                        "description": "API key",
                        "required": true,
                    },
                    "endpoint": {
                        "description": "API endpoint",
                        "required": false,
                    },
                }
            })
        );
    }

    #[test]
    fn test_credential_entry_does_not_serialize_sealed_token() {
        static FIELDS: [(&str, CredentialFieldSchema); 0] = [];

        let entry = CredentialEntry {
            name: "api",
            description: "API credentials",
            fields: &FIELDS,
            __sealed: Sealed(()),
        };

        let value = serde_json::to_value(entry).unwrap();
        let object = value.as_object().unwrap();
        assert!(object.get("__sealed").is_none());
    }

    #[test]
    fn test_credential_entry_serializes_empty_fields_as_empty_object() {
        static FIELDS: [(&str, CredentialFieldSchema); 0] = [];

        let entry = CredentialEntry {
            name: "api",
            description: "API credentials",
            fields: &FIELDS,
            __sealed: Sealed(()),
        };

        let value = serde_json::to_value(entry).unwrap();
        let object = value.as_object().unwrap();
        let fields = object.get("fields").unwrap();
        assert_eq!(fields, &json!({}));
    }

    #[test]
    fn test_credential_error_debug_output_contains_variant_name() {
        let error = CredentialError::NotFound("api_key".to_string());
        let debug = format!("{error:?}");

        assert!(debug.contains("NotFound"));
        assert!(debug.contains("api_key"));
    }

    #[test]
    fn test_credential_error_deserialization_debug_output_contains_variant_and_message() {
        let error = CredentialError::DeserializationError("invalid json".to_string());
        let debug = format!("{error:?}");

        assert!(debug.contains("DeserializationError"));
        assert!(debug.contains("invalid json"));
    }

    #[test]
    fn test_credential_field_schema_debug_output() {
        let schema = CredentialFieldSchema {
            description: "test field",
            required: false,
        };
        let debug = format!("{schema:?}");

        assert!(debug.contains("CredentialFieldSchema"));
        assert!(debug.contains("test field"));
        assert!(debug.contains("false"));
    }

    #[test]
    fn test_credential_entry_debug_output_contains_name_and_description() {
        static FIELDS: [(&str, CredentialFieldSchema); 0] = [];

        let entry = CredentialEntry {
            name: "test_cred",
            description: "Test credential",
            fields: &FIELDS,
            __sealed: Sealed(()),
        };
        let debug = format!("{entry:?}");

        assert!(debug.contains("CredentialEntry"));
        assert!(debug.contains("test_cred"));
        assert!(debug.contains("Test credential"));
    }

    #[test]
    fn test_credential_entry_serialization_preserves_field_order() {
        // Important: fields should serialize in the order they're defined
        static FIELDS: [(&str, CredentialFieldSchema); 3] = [
            (
                "first",
                CredentialFieldSchema {
                    description: "First field",
                    required: true,
                },
            ),
            (
                "second",
                CredentialFieldSchema {
                    description: "Second field",
                    required: false,
                },
            ),
            (
                "third",
                CredentialFieldSchema {
                    description: "Third field",
                    required: true,
                },
            ),
        ];

        let entry = CredentialEntry {
            name: "ordered",
            description: "Test order",
            fields: &FIELDS,
            __sealed: Sealed(()),
        };

        // Serialize to string to check order (JSON objects don't guarantee order,
        // but serde_json preserves insertion order by default)
        let json_str = serde_json::to_string(&entry).unwrap();

        // Verify fields appear in expected order in the JSON string
        let first_pos = json_str.find("\"first\"").unwrap();
        let second_pos = json_str.find("\"second\"").unwrap();
        let third_pos = json_str.find("\"third\"").unwrap();

        assert!(
            first_pos < second_pos,
            "first should appear before second in JSON"
        );
        assert!(
            second_pos < third_pos,
            "second should appear before third in JSON"
        );
    }
}
