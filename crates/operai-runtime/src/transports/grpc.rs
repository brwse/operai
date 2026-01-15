//! gRPC server implementation for the Toolbox service.
//!
//! This module provides a tonic-based gRPC server that exposes the Tool Runtime
//! functionality over the network. It implements the `Toolbox` service trait,
//! allowing clients to list, search, and call tools remotely.
//!
//! # Metadata Extraction
//!
//! The service extracts metadata from gRPC request headers:
//! - `x-request-id`: Request identifier for tracing
//! - `x-session-id`: Session identifier for policy evaluation
//! - `x-user-id`: User identifier for authorization
//! - `x-credential-*`: Base64-encoded JSON credentials for external services
//!
//! # Credentials Format
//!
//! Credential headers use the format `x-credential-{provider}` where the value
//! is a base64-encoded JSON object with a `values` field containing key-value pairs.

use std::{collections::HashMap, sync::Arc};

use base64::prelude::*;
use operai_core::{ToolRegistry, policy::session::PolicyStore};
use tonic::{Request, Response, Status};
use tracing::{instrument, warn};

use crate::{
    proto::{
        CallToolRequest, CallToolResponse, ListToolsRequest, ListToolsResponse, SearchToolsRequest,
        SearchToolsResponse, toolbox_server::Toolbox,
    },
    runtime::{CallMetadata, LocalRuntime},
};

/// gRPC service implementation for the Toolbox API.
///
/// This service wraps a [`LocalRuntime`] and exposes it via the gRPC `Toolbox` protocol.
/// It handles metadata extraction from request headers and credential parsing.
///
/// # Fields
///
/// * `runtime` - The underlying local runtime that executes tool calls
pub struct ToolboxService {
    runtime: LocalRuntime,
}

impl ToolboxService {
    /// Creates a new `ToolboxService` with the given tool registry and policy store.
    ///
    /// # Arguments
    ///
    /// * `registry` - The tool registry containing available tools
    /// * `policy_store` - The policy store for authorization and evaluation
    #[must_use]
    pub fn new(registry: Arc<ToolRegistry>, policy_store: Arc<PolicyStore>) -> Self {
        Self::from_runtime(LocalRuntime::new(registry, policy_store))
    }

    /// Creates a new `ToolboxService` from an existing `LocalRuntime`.
    ///
    /// # Arguments
    ///
    /// * `runtime` - A configured local runtime instance
    #[must_use]
    pub fn from_runtime(runtime: LocalRuntime) -> Self {
        Self { runtime }
    }

    /// Returns a reference to the underlying runtime.
    #[must_use]
    pub fn runtime(&self) -> &LocalRuntime {
        &self.runtime
    }

    /// Extracts metadata headers from a gRPC request.
    ///
    /// Returns a tuple of `(request_id, session_id, user_id)`. Missing headers
    /// are returned as empty strings.
    ///
    /// # Headers Extracted
    ///
    /// - `x-request-id`: Unique request identifier
    /// - `x-session-id`: Session identifier for policy evaluation
    /// - `x-user-id`: User identifier for authorization
    fn extract_metadata<T>(request: &Request<T>) -> (String, String, String) {
        let get = |key| {
            request
                .metadata()
                .get(key)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string()
        };
        (get("x-request-id"), get("x-session-id"), get("x-user-id"))
    }

    /// Extracts user credentials from gRPC request metadata headers.
    ///
    /// Credentials are passed via headers with the format `x-credential-{provider}`.
    /// Each header value must be a base64-encoded JSON object containing a `values` field.
    ///
    /// # Example Header
    ///
    /// ```text
    /// x-credential-github: eyJ2YWx1ZXMiOnt0b2tlbiI6ImFiYyIsIm9yZyI6ImJyd3NlIn19
    /// ```
    ///
    /// Which decodes to:
    /// ```json
    /// {"values":{"token":"abc","org":"brwse"}}
    /// ```
    ///
    /// # Returns
    ///
    /// A map of provider name to credential values. Invalid or malformed credential
    /// headers are silently ignored (logged as warnings).
    fn extract_credentials<T>(request: &Request<T>) -> HashMap<String, HashMap<String, String>> {
        #[derive(serde::Deserialize)]
        struct CredentialData {
            values: HashMap<String, String>,
        }

        request
            .metadata()
            .iter()
            .filter_map(|kv| {
                let tonic::metadata::KeyAndValueRef::Ascii(key, value) = kv else {
                    return None;
                };
                let cred_name = key.as_str().strip_prefix("x-credential-")?;
                let value_str = value.to_str().ok()?;

                let decoded = BASE64_STANDARD.decode(value_str).map_err(|e| {
                    warn!(credential = %cred_name, error = %e, "Failed to decode base64 credential");
                }).ok()?;

                let cred_data: CredentialData = serde_json::from_slice(&decoded).map_err(|e| {
                    warn!(credential = %cred_name, error = %e, "Failed to parse credential JSON");
                }).ok()?;

                Some((cred_name.to_string(), cred_data.values))
            })
            .collect()
    }
}

/// gRPC service implementation for the Toolbox protocol.
///
/// This trait implementation handles the three main operations:
/// - Listing available tools with pagination
/// - Searching tools by semantic similarity
/// - Calling tools with input validation and policy enforcement
#[tonic::async_trait]
impl Toolbox for ToolboxService {
    /// Lists all available tools in the registry.
    ///
    /// Supports pagination via `page_size` and `page_token` parameters.
    /// Returns tools in a deterministic order with a `next_page_token` for pagination.
    #[instrument(skip(self, request), fields(page_size, page_token))]
    async fn list_tools(
        &self,
        request: Request<ListToolsRequest>,
    ) -> Result<Response<ListToolsResponse>, Status> {
        let response = self.runtime.list_tools(request.into_inner()).await?;
        Ok(Response::new(response))
    }

    /// Searches for tools by semantic similarity using an embedding vector.
    ///
    /// The `query_embedding` must match the dimensionality of tool embeddings
    /// in the registry. Returns tools ranked by cosine similarity.
    #[instrument(skip(self, request), fields(embedding_dims))]
    async fn search_tools(
        &self,
        request: Request<SearchToolsRequest>,
    ) -> Result<Response<SearchToolsResponse>, Status> {
        let response = self.runtime.search_tools(request.into_inner()).await?;
        Ok(Response::new(response))
    }

    /// Invokes a tool with the provided input and metadata.
    ///
    /// This method:
    /// 1. Extracts request metadata (request_id, session_id, user_id)
    /// 2. Parses user credentials from headers
    /// 3. Validates the tool name format
    /// 4. Enforces pre-call policy evaluation
    /// 5. Executes the tool with the provided input
    /// 6. Enforces post-call policy evaluation
    /// 7. Returns the tool output or error
    ///
    /// Tool execution errors are returned in the response rather than as gRPC errors,
    /// allowing the transport to succeed even when tool execution fails.
    #[instrument(skip(self, request), fields(tool_name))]
    async fn call_tool(
        &self,
        request: Request<CallToolRequest>,
    ) -> Result<Response<CallToolResponse>, Status> {
        let (request_id, session_id, user_id) = Self::extract_metadata(&request);
        let user_creds = Self::extract_credentials(&request);
        let metadata = CallMetadata {
            request_id,
            session_id,
            user_id,
            credentials: user_creds,
        };

        let response = self
            .runtime
            .call_tool(request.into_inner(), metadata)
            .await?;

        Ok(Response::new(response))
    }
}

#[cfg(test)]
mod tests {
    //! Integration tests for the gRPC transport layer.
    //!
    //! These tests build a sample tool library (hello-world) and verify the
    //! gRPC service implementation correctly handles all operations.

    use std::{
        collections::{HashMap, HashSet},
        path::{Path, PathBuf},
        process::Command,
        sync::OnceLock,
    };

    use operai_abi::RuntimeContext;
    use tonic::Code;

    use super::*;
    use crate::{
        proto::call_tool_response,
        runtime::{
            extract_tool_id, json_str_to_struct, json_value_to_struct, prost_value_to_json_value,
            struct_to_json_value,
        },
    };

    /// Cached path to the hello-world cdylib for testing.
    static HELLO_WORLD_CDYLIB_PATH: OnceLock<PathBuf> = OnceLock::new();

    /// Returns the workspace root directory.
    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    /// Returns the target directory and profile name from the test executable path.
    fn cargo_target_dir_and_profile() -> (PathBuf, String) {
        let exe_path = std::env::current_exe().expect("test executable path should be available");
        let deps_dir = exe_path
            .parent()
            .expect("test executable should live in a deps directory");
        let profile_dir = deps_dir
            .parent()
            .expect("deps directory should have a profile directory parent");
        let profile = profile_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("debug")
            .to_string();
        let target_dir = profile_dir
            .parent()
            .expect("profile directory should have a target directory parent");
        (target_dir.to_path_buf(), profile)
    }

    /// Returns the platform-specific filename for the hello-world cdylib.
    fn expected_hello_world_cdylib_file_name() -> String {
        format!(
            "{}hello_world{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        )
    }

    /// Searches for the hello-world cdylib in the target directory.
    ///
    /// Looks in both the profile directory directly and in the deps subdirectory.
    fn find_hello_world_cdylib(target_dir: &Path, profile: &str) -> Option<PathBuf> {
        let file_name = expected_hello_world_cdylib_file_name();
        let profile_dir = target_dir.join(profile);

        let direct_path = profile_dir.join(&file_name);
        if direct_path.is_file() {
            return Some(direct_path);
        }

        let deps_dir = profile_dir.join("deps");
        let entries = std::fs::read_dir(deps_dir).ok()?;
        for entry in entries {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.file_name().and_then(|s| s.to_str()) == Some(file_name.as_str()) {
                return Some(path);
            }
        }

        None
    }

    /// Builds the hello-world cdylib using cargo.
    fn build_hello_world_cdylib(target_dir: &Path, profile: &str) {
        let mut cmd = Command::new("cargo");
        cmd.current_dir(workspace_root());
        cmd.args(["build", "-p", "hello-world"]);
        if profile == "release" {
            cmd.arg("--release");
        }
        cmd.env("CARGO_TARGET_DIR", target_dir);

        let status = cmd.status().expect("cargo build should start");
        assert!(status.success(), "cargo build -p hello-world failed");
    }

    /// Returns the path to the hello-world cdylib, building it if necessary.
    fn hello_world_cdylib_path() -> PathBuf {
        HELLO_WORLD_CDYLIB_PATH
            .get_or_init(|| {
                let (target_dir, profile) = cargo_target_dir_and_profile();

                if let Some(path) = find_hello_world_cdylib(&target_dir, &profile) {
                    return path;
                }

                build_hello_world_cdylib(&target_dir, &profile);

                find_hello_world_cdylib(&target_dir, &profile)
                    .unwrap_or_else(|| panic!("hello-world cdylib not found after build"))
            })
            .clone()
    }

    /// Creates a test service with the hello-world tools loaded.
    async fn service_with_hello_world_registry() -> (ToolboxService, Arc<ToolRegistry>) {
        let lib_path = hello_world_cdylib_path();

        let mut registry = ToolRegistry::new();
        let runtime_ctx = RuntimeContext::new();
        registry
            .load_library(lib_path, None, None, &runtime_ctx)
            .await
            .expect("hello-world tool library should load successfully");

        let registry = Arc::new(registry);
        let session_store =
            Arc::new(operai_core::policy::session::InMemoryPolicySessionStore::new());
        let policy_store = Arc::new(operai_core::policy::session::PolicyStore::new(
            session_store,
        ));
        (
            ToolboxService::new(Arc::clone(&registry), policy_store),
            registry,
        )
    }

    /// Creates a protobuf string value for test assertions.
    fn make_string_value(s: &str) -> prost_types::Value {
        prost_types::Value {
            kind: Some(prost_types::value::Kind::StringValue(s.to_string())),
        }
    }

    /// Helper to get a field from a protobuf Struct for test assertions.
    fn output_struct_field<'a>(
        output: &'a prost_types::Struct,
        field: &str,
    ) -> &'a prost_types::Value {
        output
            .fields
            .get(field)
            .unwrap_or_else(|| panic!("missing output field `{field}`"))
    }

    /// Helper to extract a string field from a protobuf Struct for test assertions.
    fn output_string(output: &prost_types::Struct, field: &str) -> String {
        match &output_struct_field(output, field).kind {
            Some(prost_types::value::Kind::StringValue(s)) => s.clone(),
            other => panic!("expected `{field}` to be a string, got {other:?}"),
        }
    }

    /// Helper to extract a number field from a protobuf Struct for test assertions.
    fn output_number(output: &prost_types::Struct, field: &str) -> f64 {
        match &output_struct_field(output, field).kind {
            Some(prost_types::value::Kind::NumberValue(n)) => *n,
            other => panic!("expected `{field}` to be a number, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_list_tools_with_default_page_size_returns_all_tools() {
        // Arrange
        let (service, registry) = service_with_hello_world_registry().await;
        let request = Request::new(ListToolsRequest {
            page_size: 0,
            page_token: String::new(),
        });

        // Act
        let response = <ToolboxService as Toolbox>::list_tools(&service, request)
            .await
            .expect("list_tools should succeed");
        let body = response.into_inner();

        // Assert
        assert_eq!(body.tools.len(), registry.len());
        assert!(body.next_page_token.is_empty());
        for tool in &body.tools {
            assert!(tool.name.starts_with("tools/"));
            assert!(
                tool.input_schema.is_some(),
                "tool.input_schema should be present for valid JSON schema"
            );
            assert!(
                tool.output_schema.is_some(),
                "tool.output_schema should be present for valid JSON schema"
            );
        }
    }

    #[tokio::test]
    async fn test_list_tools_pagination_uses_page_token_offset() {
        // Arrange
        let (service, _registry) = service_with_hello_world_registry().await;
        let all_tools = <ToolboxService as Toolbox>::list_tools(
            &service,
            Request::new(ListToolsRequest {
                page_size: 1000,
                page_token: String::new(),
            }),
        )
        .await
        .expect("list_tools should succeed")
        .into_inner()
        .tools;

        let expected_names: HashSet<String> = all_tools.iter().map(|t| t.name.clone()).collect();
        assert!(
            expected_names.contains("tools/hello-world.echo")
                && expected_names.contains("tools/hello-world.greet"),
            "hello-world tools should be present"
        );

        // Act
        let page1 = <ToolboxService as Toolbox>::list_tools(
            &service,
            Request::new(ListToolsRequest {
                page_size: 1,
                page_token: String::new(),
            }),
        )
        .await
        .expect("list_tools should succeed")
        .into_inner();

        let page2 = <ToolboxService as Toolbox>::list_tools(
            &service,
            Request::new(ListToolsRequest {
                page_size: 1,
                page_token: page1.next_page_token.clone(),
            }),
        )
        .await
        .expect("list_tools should succeed")
        .into_inner();

        // Assert
        assert_eq!(page1.tools.len(), 1);
        assert_eq!(page2.tools.len(), 1);
        assert!(page2.next_page_token.is_empty());

        let mut paged_names: HashSet<String> = HashSet::new();
        paged_names.extend(page1.tools.iter().map(|t| t.name.clone()));
        paged_names.extend(page2.tools.iter().map(|t| t.name.clone()));
        assert_eq!(paged_names, expected_names);
    }

    #[tokio::test]
    async fn test_list_tools_invalid_page_token_is_treated_as_zero_offset() {
        // Arrange
        let (service, _registry) = service_with_hello_world_registry().await;
        let request_with_empty_token = Request::new(ListToolsRequest {
            page_size: 1,
            page_token: String::new(),
        });
        let request_with_invalid_token = Request::new(ListToolsRequest {
            page_size: 1,
            page_token: "not-a-number".to_string(),
        });

        // Act
        let page_with_empty_token =
            <ToolboxService as Toolbox>::list_tools(&service, request_with_empty_token)
                .await
                .expect("list_tools should succeed")
                .into_inner();
        let page_with_invalid_token =
            <ToolboxService as Toolbox>::list_tools(&service, request_with_invalid_token)
                .await
                .expect("list_tools should succeed")
                .into_inner();

        // Assert
        assert_eq!(page_with_empty_token.tools.len(), 1);
        assert_eq!(page_with_invalid_token.tools.len(), 1);
        assert_eq!(
            page_with_invalid_token.tools[0].name,
            page_with_empty_token.tools[0].name
        );
    }

    #[tokio::test]
    async fn test_search_tools_with_empty_embedding_returns_invalid_argument() {
        // Arrange
        let service = ToolboxService::new(
            Arc::new(ToolRegistry::new()),
            Arc::new(operai_core::policy::session::PolicyStore::new(Arc::new(
                operai_core::policy::session::InMemoryPolicySessionStore::new(),
            ))),
        );
        let request = Request::new(SearchToolsRequest {
            query_embedding: Vec::new(),
            page_size: 10,
            page_token: String::new(),
        });

        // Act
        let status = <ToolboxService as Toolbox>::search_tools(&service, request)
            .await
            .expect_err("search_tools should reject an empty embedding");

        // Assert
        assert_eq!(status.code(), Code::InvalidArgument);
        assert_eq!(status.message(), "query_embedding is required");
    }

    #[tokio::test]
    async fn test_search_tools_with_embedding_returns_ok_even_if_no_results() {
        // Arrange
        let service = ToolboxService::new(
            Arc::new(ToolRegistry::new()),
            Arc::new(operai_core::policy::session::PolicyStore::new(Arc::new(
                operai_core::policy::session::InMemoryPolicySessionStore::new(),
            ))),
        );
        let request = Request::new(SearchToolsRequest {
            query_embedding: vec![0.1, 0.2, 0.3],
            page_size: 10,
            page_token: String::new(),
        });

        // Act
        let response = <ToolboxService as Toolbox>::search_tools(&service, request)
            .await
            .expect("search_tools should succeed");
        let body = response.into_inner();

        // Assert
        assert!(body.results.is_empty());
        assert!(body.next_page_token.is_empty());
    }

    #[tokio::test]
    async fn test_call_tool_with_invalid_name_format_returns_invalid_argument() {
        // Arrange
        let service = ToolboxService::new(
            Arc::new(ToolRegistry::new()),
            Arc::new(operai_core::policy::session::PolicyStore::new(Arc::new(
                operai_core::policy::session::InMemoryPolicySessionStore::new(),
            ))),
        );
        let request = Request::new(CallToolRequest {
            name: "hello-world.greet".to_string(),
            input: None,
        });

        // Act
        let status = <ToolboxService as Toolbox>::call_tool(&service, request)
            .await
            .expect_err("call_tool should reject invalid resource names");

        // Assert
        assert_eq!(status.code(), Code::InvalidArgument);
        assert_eq!(status.message(), "invalid tool name format");
    }

    #[tokio::test]
    async fn test_call_tool_with_unknown_tool_returns_not_found() {
        // Arrange
        let service = ToolboxService::new(
            Arc::new(ToolRegistry::new()),
            Arc::new(operai_core::policy::session::PolicyStore::new(Arc::new(
                operai_core::policy::session::InMemoryPolicySessionStore::new(),
            ))),
        );
        let request = Request::new(CallToolRequest {
            name: "tools/hello-world.greet".to_string(),
            input: None,
        });

        // Act
        let status = <ToolboxService as Toolbox>::call_tool(&service, request)
            .await
            .expect_err("call_tool should return NOT_FOUND for missing tools");

        // Assert
        assert_eq!(status.code(), Code::NotFound);
        assert_eq!(status.message(), "tool not found: hello-world.greet");
    }

    #[tokio::test]
    async fn test_call_tool_echo_happy_path_returns_expected_output_and_drains_inflight() {
        // Arrange
        let (service, registry) = service_with_hello_world_registry().await;

        let input = prost_types::Struct {
            fields: [("message".to_string(), make_string_value("hi"))]
                .into_iter()
                .collect(),
        };
        let request = Request::new(CallToolRequest {
            name: "tools/hello-world.echo".to_string(),
            input: Some(input),
        });

        // Act
        let response = <ToolboxService as Toolbox>::call_tool(&service, request)
            .await
            .expect("call_tool should succeed");
        let body = response.into_inner();

        // Assert
        let Some(result) = body.result else {
            panic!("CallToolResponse.result should be set");
        };
        match result {
            call_tool_response::Result::Output(output) => {
                assert_eq!(output_string(&output, "echo"), "hi");
                assert!((output_number(&output, "length") - 2.0).abs() < f64::EPSILON);
            }
            call_tool_response::Result::Error(message) => {
                panic!("expected output, got error: {message}");
            }
        }

        assert_eq!(registry.inflight_count(), 0);
    }

    #[tokio::test]
    async fn test_call_tool_propagates_request_id_from_metadata() {
        // Arrange
        let (service, _registry) = service_with_hello_world_registry().await;

        let input = prost_types::Struct {
            fields: [("name".to_string(), make_string_value("Test"))]
                .into_iter()
                .collect(),
        };
        let mut request = Request::new(CallToolRequest {
            name: "tools/hello-world.greet".to_string(),
            input: Some(input),
        });
        request.metadata_mut().insert(
            "x-request-id",
            "req-123"
                .parse()
                .expect("x-request-id metadata value should parse"),
        );

        // Act
        let response = <ToolboxService as Toolbox>::call_tool(&service, request)
            .await
            .expect("call_tool should succeed");
        let body = response.into_inner();

        // Assert
        let Some(result) = body.result else {
            panic!("CallToolResponse.result should be set");
        };
        match result {
            call_tool_response::Result::Output(output) => {
                assert_eq!(output_string(&output, "request_id"), "req-123");
            }
            call_tool_response::Result::Error(message) => {
                panic!("expected output, got error: {message}");
            }
        }
    }

    #[tokio::test]
    async fn test_call_tool_with_missing_input_returns_error_result() {
        // Arrange
        let (service, registry) = service_with_hello_world_registry().await;
        let request = Request::new(CallToolRequest {
            name: "tools/hello-world.echo".to_string(),
            input: None,
        });

        // Act
        let response = <ToolboxService as Toolbox>::call_tool(&service, request)
            .await
            .expect("call_tool transport should succeed even when tool errors");
        let body = response.into_inner();

        // Assert
        let Some(result) = body.result else {
            panic!("CallToolResponse.result should be set");
        };
        match result {
            call_tool_response::Result::Output(output) => {
                panic!("expected error, got output: {output:?}");
            }
            call_tool_response::Result::Error(message) => {
                assert!(!message.is_empty());
            }
        }

        assert_eq!(registry.inflight_count(), 0);
    }

    #[test]
    fn test_extract_credentials_parses_valid_and_ignores_invalid() {
        // Arrange
        let mut request = Request::new(());

        let valid_json = r#"{"values":{"token":"abc","org":"brwse"}}"#;
        let valid_encoded = base64::prelude::BASE64_STANDARD.encode(valid_json);

        request.metadata_mut().insert(
            "x-credential-github",
            valid_encoded
                .parse()
                .expect("base64 metadata value should parse"),
        );
        request.metadata_mut().insert(
            "x-credential-badbase64",
            "not-base64"
                .parse()
                .expect("invalid base64 string is still a valid metadata value"),
        );
        let invalid_json_encoded = base64::prelude::BASE64_STANDARD.encode("not-json");
        request.metadata_mut().insert(
            "x-credential-badjson",
            invalid_json_encoded
                .parse()
                .expect("base64 metadata value should parse"),
        );

        // Act
        let creds = ToolboxService::extract_credentials(&request);

        // Assert
        let expected_github: HashMap<String, String> = [
            ("token".to_string(), "abc".to_string()),
            ("org".to_string(), "brwse".to_string()),
        ]
        .into_iter()
        .collect();
        assert_eq!(creds.get("github"), Some(&expected_github));
        assert!(!creds.contains_key("badbase64"));
        assert!(!creds.contains_key("badjson"));
    }

    #[test]
    fn test_json_value_to_struct_supports_nested_values() {
        // Arrange
        let value = serde_json::json!({
            "str": "a",
            "num": 1,
            "bool": true,
            "null": null,
            "list": ["x", 2],
            "obj": { "k": "v" }
        });

        // Act
        let output = json_value_to_struct(&value).expect("object JSON should convert to Struct");

        // Assert
        assert_eq!(output_string(&output, "str"), "a");
        assert!((output_number(&output, "num") - 1.0).abs() < f64::EPSILON);
        match &output_struct_field(&output, "bool").kind {
            Some(prost_types::value::Kind::BoolValue(true)) => {}
            other => panic!("expected `bool` to be true, got {other:?}"),
        }
        assert!(matches!(
            output_struct_field(&output, "null").kind,
            Some(prost_types::value::Kind::NullValue(_))
        ));
        match &output_struct_field(&output, "list").kind {
            Some(prost_types::value::Kind::ListValue(list)) => {
                assert_eq!(list.values.len(), 2);
            }
            other => panic!("expected `list` to be a list, got {other:?}"),
        }
        match &output_struct_field(&output, "obj").kind {
            Some(prost_types::value::Kind::StructValue(obj)) => {
                assert_eq!(output_string(obj, "k"), "v");
            }
            other => panic!("expected `obj` to be a struct, got {other:?}"),
        }
    }

    #[test]
    fn test_extract_tool_id_strips_tools_prefix() {
        assert_eq!(extract_tool_id("tools/my-tool"), Some("my-tool"));
        assert_eq!(
            extract_tool_id("tools/namespace.tool-name"),
            Some("namespace.tool-name")
        );
    }

    #[test]
    fn test_extract_tool_id_returns_none_without_prefix() {
        assert_eq!(extract_tool_id("my-tool"), None);
        assert_eq!(extract_tool_id("tool/my-tool"), None);
        assert_eq!(extract_tool_id(""), None);
    }

    #[test]
    fn test_extract_metadata_returns_empty_strings_for_missing_headers() {
        // Arrange
        let request = Request::new(());

        // Act
        let (request_id, session_id, user_id) = ToolboxService::extract_metadata(&request);

        // Assert
        assert_eq!(request_id, "");
        assert_eq!(session_id, "");
        assert_eq!(user_id, "");
    }

    #[test]
    fn test_extract_metadata_returns_header_values_when_present() {
        // Arrange
        let mut request = Request::new(());
        request
            .metadata_mut()
            .insert("x-request-id", "req-abc".parse().unwrap());
        request
            .metadata_mut()
            .insert("x-session-id", "sess-xyz".parse().unwrap());
        request
            .metadata_mut()
            .insert("x-user-id", "user-123".parse().unwrap());

        // Act
        let (request_id, session_id, user_id) = ToolboxService::extract_metadata(&request);

        // Assert
        assert_eq!(request_id, "req-abc");
        assert_eq!(session_id, "sess-xyz");
        assert_eq!(user_id, "user-123");
    }

    #[tokio::test]
    async fn test_list_tools_caps_page_size_at_1000() {
        // Arrange
        let (service, _registry) = service_with_hello_world_registry().await;
        let request = Request::new(ListToolsRequest {
            page_size: 10000, // Request way more than the cap
            page_token: String::new(),
        });

        // Act
        let response = <ToolboxService as Toolbox>::list_tools(&service, request)
            .await
            .expect("list_tools should succeed");
        let body = response.into_inner();

        // Assert - should succeed without error even with oversized page_size
        assert!(!body.tools.is_empty());
    }

    #[tokio::test]
    async fn test_search_tools_uses_default_page_size_for_zero_or_negative() {
        // Arrange
        let (service, _registry) = service_with_hello_world_registry().await;
        let request_zero = Request::new(SearchToolsRequest {
            query_embedding: vec![0.1; 768],
            page_size: 0,
            page_token: String::new(),
        });
        let request_negative = Request::new(SearchToolsRequest {
            query_embedding: vec![0.1; 768],
            page_size: -5,
            page_token: String::new(),
        });

        // Act - both should succeed without error
        let response_zero = <ToolboxService as Toolbox>::search_tools(&service, request_zero)
            .await
            .expect("search_tools should succeed with page_size=0");
        let response_negative =
            <ToolboxService as Toolbox>::search_tools(&service, request_negative)
                .await
                .expect("search_tools should succeed with negative page_size");

        // Assert - responses should be valid
        assert!(response_zero.into_inner().next_page_token.is_empty());
        assert!(response_negative.into_inner().next_page_token.is_empty());
    }

    #[test]
    fn test_json_str_to_struct_returns_none_for_invalid_json() {
        assert!(json_str_to_struct("not valid json").is_none());
        assert!(json_str_to_struct("").is_none());
    }

    #[test]
    fn test_json_str_to_struct_returns_none_for_non_object_json() {
        assert!(json_str_to_struct("123").is_none());
        assert!(json_str_to_struct("\"string\"").is_none());
        assert!(json_str_to_struct("[1, 2, 3]").is_none());
        assert!(json_str_to_struct("null").is_none());
    }

    #[test]
    fn test_json_prost_roundtrip_preserves_values() {
        // Arrange - use f64 for numbers since prost's NumberValue uses f64
        let original = serde_json::json!({
            "name": "test",
            "count": 42.0,
            "active": true,
            "tags": ["a", "b"],
            "nested": { "key": "value" }
        });

        // Act
        let prost_struct =
            json_value_to_struct(&original).expect("valid object should convert to Struct");
        let roundtripped = struct_to_json_value(&prost_struct);

        // Assert
        assert_eq!(original, roundtripped);
    }

    #[test]
    fn test_extract_credentials_ignores_non_credential_headers() {
        // Arrange
        let mut request = Request::new(());
        request
            .metadata_mut()
            .insert("x-request-id", "req-123".parse().unwrap());
        request
            .metadata_mut()
            .insert("content-type", "application/json".parse().unwrap());

        // Act
        let creds = ToolboxService::extract_credentials(&request);

        // Assert
        assert!(creds.is_empty());
    }

    #[tokio::test]
    async fn test_call_tool_greet_with_custom_greeting() {
        // Arrange
        let (service, _registry) = service_with_hello_world_registry().await;

        let input = prost_types::Struct {
            fields: [
                ("name".to_string(), make_string_value("World")),
                ("greeting".to_string(), make_string_value("Howdy")),
            ]
            .into_iter()
            .collect(),
        };
        let request = Request::new(CallToolRequest {
            name: "tools/hello-world.greet".to_string(),
            input: Some(input),
        });

        // Act
        let response = <ToolboxService as Toolbox>::call_tool(&service, request)
            .await
            .expect("call_tool should succeed");
        let body = response.into_inner();

        // Assert
        let Some(call_tool_response::Result::Output(output)) = body.result else {
            panic!("expected output, got {:?}", body.result);
        };
        assert_eq!(output_string(&output, "message"), "Howdy, World!");
    }

    #[test]
    fn test_prost_value_to_json_value_with_none_kind_returns_null() {
        // Arrange
        let value = prost_types::Value { kind: None };

        // Act
        let result = prost_value_to_json_value(&value);

        // Assert
        assert!(result.is_null());
    }

    #[test]
    fn test_prost_value_to_json_value_converts_nan_to_zero() {
        // Arrange - NaN cannot be represented in JSON, so it falls back to 0
        let value = prost_types::Value {
            kind: Some(prost_types::value::Kind::NumberValue(f64::NAN)),
        };

        // Act
        let result = prost_value_to_json_value(&value);

        // Assert
        assert_eq!(result, serde_json::json!(0));
    }

    #[test]
    fn test_prost_value_to_json_value_converts_infinity_to_zero() {
        // Arrange - Infinity cannot be represented in JSON, so it falls back to 0
        let value = prost_types::Value {
            kind: Some(prost_types::value::Kind::NumberValue(f64::INFINITY)),
        };

        // Act
        let result = prost_value_to_json_value(&value);

        // Assert
        assert_eq!(result, serde_json::json!(0));
    }

    #[test]
    fn test_prost_value_to_json_value_converts_neg_infinity_to_zero() {
        // Arrange - Negative infinity cannot be represented in JSON, so it falls back
        // to 0
        let value = prost_types::Value {
            kind: Some(prost_types::value::Kind::NumberValue(f64::NEG_INFINITY)),
        };

        // Act
        let result = prost_value_to_json_value(&value);

        // Assert
        assert_eq!(result, serde_json::json!(0));
    }

    #[test]
    fn test_json_str_to_struct_parses_valid_object() {
        // Arrange
        let json = r#"{"key": "value"}"#;

        // Act
        let result = json_str_to_struct(json);

        // Assert
        let output = result.expect("valid object JSON should convert to Struct");
        assert_eq!(output_string(&output, "key"), "value");
    }

    #[test]
    fn test_extract_tool_id_handles_empty_after_prefix() {
        // Arrange / Act / Assert
        assert_eq!(extract_tool_id("tools/"), Some(""));
    }

    #[test]
    fn test_extract_credentials_handles_empty_values_map() {
        // Arrange
        let mut request = Request::new(());
        let json = r#"{"values":{}}"#;
        let encoded = base64::prelude::BASE64_STANDARD.encode(json);
        request.metadata_mut().insert(
            "x-credential-empty",
            encoded.parse().expect("base64 should parse"),
        );

        // Act
        let creds = ToolboxService::extract_credentials(&request);

        // Assert
        let expected: HashMap<String, String> = HashMap::new();
        assert_eq!(creds.get("empty"), Some(&expected));
    }

    #[tokio::test]
    async fn test_list_tools_with_negative_page_size_uses_default() {
        // Arrange
        let (service, registry) = service_with_hello_world_registry().await;
        let request = Request::new(ListToolsRequest {
            page_size: -10,
            page_token: String::new(),
        });

        // Act
        let response = <ToolboxService as Toolbox>::list_tools(&service, request)
            .await
            .expect("list_tools should succeed");
        let body = response.into_inner();

        // Assert - should return all tools (uses default of 100)
        assert_eq!(body.tools.len(), registry.len());
    }

    #[tokio::test]
    async fn test_list_tools_with_offset_beyond_total_returns_empty() {
        // Arrange
        let (service, _registry) = service_with_hello_world_registry().await;
        let request = Request::new(ListToolsRequest {
            page_size: 10,
            page_token: "1000".to_string(), // Offset way beyond available tools
        });

        // Act
        let response = <ToolboxService as Toolbox>::list_tools(&service, request)
            .await
            .expect("list_tools should succeed");
        let body = response.into_inner();

        // Assert
        assert!(body.tools.is_empty());
        assert!(body.next_page_token.is_empty());
    }
}
