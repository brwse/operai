//! Credential types and registration.

use serde::Serialize;

use crate::entrypoint::Sealed;

/// Errors that can occur when accessing credentials.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CredentialError {
    /// The requested credential was not found.
    #[error("credential '{0}' not found")]
    NotFound(String),

    /// Failed to deserialize the credential.
    #[error("failed to deserialize credential: {0}")]
    DeserializationError(String),
}

/// Schema for a single field within a credential.
#[derive(Debug, Clone, Serialize)]
pub struct CredentialFieldSchema {
    pub description: &'static str,
    pub required: bool,
}

/// Registry entry for a credential definition.
///
/// This struct can only be constructed by the `define_system_credential!` and
/// `define_user_credential!` macros because it requires a [`Sealed`] token
/// that cannot be created outside this crate.
///
/// # Sealed Construction
///
/// The `__sealed` field uses the [sealed trait pattern](https://predr.ag/blog/definitive-guide-to-sealed-traits-in-rust/)
/// to prevent external construction. This ensures that all credential entries
/// are properly registered through the macro system.
#[derive(Debug)]
pub struct CredentialEntry {
    /// Unique identifier for the credential (e.g., "api").
    pub name: &'static str,
    pub description: &'static str,
    pub fields: &'static [(&'static str, CredentialFieldSchema)],
    /// Sealed token preventing external construction.
    #[doc(hidden)]
    pub __sealed: Sealed,
}

impl Serialize for CredentialEntry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::{SerializeMap, SerializeStruct};

        /// Wrapper to serialize a slice of tuples as a map without allocation.
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

// Collect credential entries for runtime introspection
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
