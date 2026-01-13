use std::{collections::HashMap, sync::Arc};

use abi_stable::std_types::{RSlice, RStr};
use base64::prelude::*;
use futures::FutureExt;
use operai_abi::{CallContext, ToolResult};
use operai_core::{PolicyError, Registry, ToolInfo, policy::session::PolicyStore};
use rkyv::rancor::BoxedError;
use tonic::{Request, Response, Status};
use tracing::{error, info, instrument, warn};

use crate::proto::{
    CallToolRequest, CallToolResponse, ListToolsRequest, ListToolsResponse, SearchResult,
    SearchToolsRequest, SearchToolsResponse, Tool, call_tool_response, toolbox_server::Toolbox,
};

pub struct ToolboxService {
    registry: Arc<Registry>,
    policy_store: Arc<PolicyStore>,
}

impl ToolboxService {
    #[must_use]
    pub fn new(registry: Arc<Registry>, policy_store: Arc<PolicyStore>) -> Self {
        Self {
            registry,
            policy_store,
        }
    }

    fn tool_info_to_proto(info: &ToolInfo) -> Tool {
        Tool {
            name: format!("tools/{}", info.qualified_id),
            display_name: info.display_name.clone(),
            version: info.crate_version.clone(),
            description: info.description.clone(),
            input_schema: json_str_to_struct(&info.input_schema),
            output_schema: json_str_to_struct(&info.output_schema),
            capabilities: info.capabilities.clone(),
            tags: info.tags.clone(),
        }
    }

    /// Extracts tool ID from resource name format (e.g.,
    /// `tools/my-tool`).
    fn extract_tool_id(name: &str) -> Option<&str> {
        name.strip_prefix("tools/")
    }

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

    /// Parses `x-credential-{name}` headers containing base64-encoded JSON.
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

#[tonic::async_trait]
impl Toolbox for ToolboxService {
    #[instrument(skip(self, request), fields(page_size, page_token))]
    async fn list_tools(
        &self,
        request: Request<ListToolsRequest>,
    ) -> Result<Response<ListToolsResponse>, Status> {
        let req = request.into_inner();

        let page_size: usize = if req.page_size <= 0 {
            100
        } else {
            usize::try_from(req.page_size.min(1000)).unwrap_or(1000)
        };

        // page_token is a stringified offset for simple cursor pagination
        let offset: usize = req.page_token.parse().unwrap_or(0);

        let all_tools: Vec<_> = self.registry.list().collect();
        let total = all_tools.len();

        let tools: Vec<Tool> = all_tools
            .into_iter()
            .skip(offset)
            .take(page_size)
            .map(Self::tool_info_to_proto)
            .collect();

        let next_offset = offset + tools.len();
        let next_page_token = if next_offset < total {
            next_offset.to_string()
        } else {
            String::new()
        };

        Ok(Response::new(ListToolsResponse {
            tools,
            next_page_token,
        }))
    }

    #[instrument(skip(self, request), fields(embedding_dims))]
    async fn search_tools(
        &self,
        request: Request<SearchToolsRequest>,
    ) -> Result<Response<SearchToolsResponse>, Status> {
        let req = request.into_inner();

        if req.query_embedding.is_empty() {
            return Err(Status::invalid_argument("query_embedding is required"));
        }

        let page_size = if req.page_size <= 0 {
            10
        } else {
            usize::try_from(req.page_size.min(100)).unwrap_or(100)
        };

        info!(
            embedding_dims = req.query_embedding.len(),
            "Searching with provided embedding"
        );

        let search_results = self.registry.search(&req.query_embedding, page_size);

        let results: Vec<SearchResult> = search_results
            .into_iter()
            .map(|(tool_info, score)| SearchResult {
                tool: Some(Self::tool_info_to_proto(tool_info)),
                relevance_score: score,
            })
            .collect();

        info!(
            embedding_dims = req.query_embedding.len(),
            result_count = results.len(),
            "Search completed"
        );

        Ok(Response::new(SearchToolsResponse {
            results,
            next_page_token: String::new(),
        }))
    }

    #[instrument(skip(self, request), fields(tool_name))]
    async fn call_tool(
        &self,
        request: Request<CallToolRequest>,
    ) -> Result<Response<CallToolResponse>, Status> {
        let (request_id, session_id, user_id) = Self::extract_metadata(&request);
        let user_creds = Self::extract_credentials(&request);
        let req = request.into_inner();

        let tool_id = Self::extract_tool_id(&req.name)
            .ok_or_else(|| Status::invalid_argument("invalid tool name format"))?;

        let handle = self
            .registry
            .get(tool_id)
            .ok_or_else(|| Status::not_found(format!("tool not found: {tool_id}")))?;

        info!(
            tool_id = %tool_id,
            request_id = %request_id,
            "Invoking tool"
        );

        let inflight_guard = self.registry.start_request_guard();

        let input_value = if let Some(s) = req.input.as_ref() {
            struct_to_json_value(s)
        } else {
            serde_json::Value::Object(serde_json::Map::new())
        };
        let input_json = serde_json::to_vec(&input_value).unwrap_or_else(|_| b"{}".to_vec());

        // --- Policy Check (Guards) ---
        // We now enforce policy regardless of specific policy ID in headers.
        // We apply ALL policies. (Or maybe we should still respect policy ID filtering
        // if desired? The user said "all policies should be applied", which
        // implies global enforcement.)

        self.policy_store
            .evaluate_pre_effects(&session_id, tool_id, &input_value)
            .await
            .map_err(|e| match e {
                PolicyError::GuardFailed(msg) => Status::permission_denied(msg),
                _ => Status::internal(format!("policy evaluation error: {e}")),
            })?;

        let user_creds_bin =
            rkyv::to_bytes::<BoxedError>(&user_creds).expect("failed to serialize credentials");
        let system_creds_bin = &handle.system_credentials;

        let context = CallContext {
            request_id: RStr::from_str(&request_id),
            session_id: RStr::from_str(&session_id),
            user_id: RStr::from_str(&user_id),
            user_credentials: RSlice::from_slice(&user_creds_bin),
            system_credentials: RSlice::from_slice(system_creds_bin),
        };

        let result =
            std::panic::AssertUnwindSafe(handle.call(context, RSlice::from_slice(&input_json)))
                .catch_unwind()
                .await;

        drop(inflight_guard);

        let (rpc_result, policy_outcome_val, policy_outcome_err) = if let Ok(call_result) = result {
            match call_result.result {
                ToolResult::Ok => {
                    let output_value: serde_json::Value =
                        serde_json::from_slice(call_result.output.as_slice())
                            .unwrap_or(serde_json::Value::Null);
                    let output_struct = json_value_to_struct(&output_value).unwrap_or_default();

                    (
                        Ok(Response::new(CallToolResponse {
                            result: Some(call_tool_response::Result::Output(output_struct)),
                        })),
                        Some(output_value),
                        None,
                    )
                }
                ToolResult::Error => {
                    let error_msg =
                        String::from_utf8_lossy(call_result.output.as_slice()).to_string();
                    error!(tool_id = %tool_id, error = %error_msg, "Tool invocation failed");

                    (
                        Ok(Response::new(CallToolResponse {
                            result: Some(call_tool_response::Result::Error(error_msg.clone())),
                        })),
                        None,
                        Some(error_msg),
                    )
                }
                other => {
                    error!(tool_id = %tool_id, result = ?other, "Tool invocation failed");
                    let msg = format!("tool error: {other:?}");
                    (
                        Ok(Response::new(CallToolResponse {
                            result: Some(call_tool_response::Result::Error(msg.clone())),
                        })),
                        None,
                        Some(msg),
                    )
                }
            }
        } else {
            let msg = "Tool execution panicked".to_string();
            (
                Err(Status::internal(&msg)), /* gRPC error for panic? Or CallToolResponse
                                              * error? Earlier code returned
                                              * Status::internal. */
                None,
                Some(msg),
            )
        };

        // Unified Policy Update
        let policy_res_arg = match &policy_outcome_err {
            Some(e) => Err(e.as_str()),
            None => Ok(policy_outcome_val
                .as_ref()
                .unwrap_or(&serde_json::Value::Null)),
        };

        self.policy_store
            .evaluate_post_effects(&session_id, tool_id, &input_value, policy_res_arg)
            .await
            .map_err(|e| Status::internal(format!("policy effect error: {e}")))?;

        rpc_result
    }
}

fn json_str_to_struct(json: &str) -> Option<prost_types::Struct> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    json_value_to_struct(&value)
}

fn json_value_to_struct(value: &serde_json::Value) -> Option<prost_types::Struct> {
    match value {
        serde_json::Value::Object(map) => {
            let fields = map
                .iter()
                .map(|(k, v)| (k.clone(), json_value_to_prost_value(v)))
                .collect();
            Some(prost_types::Struct { fields })
        }
        _ => None,
    }
}

fn json_value_to_prost_value(value: &serde_json::Value) -> prost_types::Value {
    use prost_types::value::Kind;

    let kind = match value {
        serde_json::Value::Null => Kind::NullValue(0),
        serde_json::Value::Bool(b) => Kind::BoolValue(*b),
        serde_json::Value::Number(n) => Kind::NumberValue(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(s) => Kind::StringValue(s.clone()),
        serde_json::Value::Array(arr) => {
            let values = arr.iter().map(json_value_to_prost_value).collect();
            Kind::ListValue(prost_types::ListValue { values })
        }
        serde_json::Value::Object(map) => {
            let fields = map
                .iter()
                .map(|(k, v)| (k.clone(), json_value_to_prost_value(v)))
                .collect();
            Kind::StructValue(prost_types::Struct { fields })
        }
    };

    prost_types::Value { kind: Some(kind) }
}

fn struct_to_json_value(s: &prost_types::Struct) -> serde_json::Value {
    let map: serde_json::Map<String, serde_json::Value> = s
        .fields
        .iter()
        .map(|(k, v)| (k.clone(), prost_value_to_json_value(v)))
        .collect();
    serde_json::Value::Object(map)
}

fn prost_value_to_json_value(value: &prost_types::Value) -> serde_json::Value {
    use prost_types::value::Kind;

    match &value.kind {
        None | Some(Kind::NullValue(_)) => serde_json::Value::Null,
        Some(Kind::BoolValue(b)) => serde_json::Value::Bool(*b),
        Some(Kind::NumberValue(n)) => serde_json::Value::Number(
            serde_json::Number::from_f64(*n).unwrap_or_else(|| serde_json::Number::from(0)),
        ),
        Some(Kind::StringValue(s)) => serde_json::Value::String(s.clone()),
        Some(Kind::ListValue(list)) => {
            let arr: Vec<serde_json::Value> =
                list.values.iter().map(prost_value_to_json_value).collect();
            serde_json::Value::Array(arr)
        }
        Some(Kind::StructValue(s)) => struct_to_json_value(s),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        path::{Path, PathBuf},
        process::Command,
        sync::OnceLock,
    };

    use operai_abi::RuntimeContext;
    use tonic::Code;

    use super::*;

    static HELLO_WORLD_CDYLIB_PATH: OnceLock<PathBuf> = OnceLock::new();

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

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

    fn expected_hello_world_cdylib_file_name() -> String {
        format!(
            "{}hello_world{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX
        )
    }

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

    async fn service_with_hello_world_registry() -> (ToolboxService, Arc<Registry>) {
        let lib_path = hello_world_cdylib_path();

        let mut registry = Registry::new();
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

    fn make_string_value(s: &str) -> prost_types::Value {
        prost_types::Value {
            kind: Some(prost_types::value::Kind::StringValue(s.to_string())),
        }
    }

    fn output_struct_field<'a>(
        output: &'a prost_types::Struct,
        field: &str,
    ) -> &'a prost_types::Value {
        output
            .fields
            .get(field)
            .unwrap_or_else(|| panic!("missing output field `{field}`"))
    }

    fn output_string(output: &prost_types::Struct, field: &str) -> String {
        match &output_struct_field(output, field).kind {
            Some(prost_types::value::Kind::StringValue(s)) => s.clone(),
            other => panic!("expected `{field}` to be a string, got {other:?}"),
        }
    }

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
            Arc::new(Registry::new()),
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
            Arc::new(Registry::new()),
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
            Arc::new(Registry::new()),
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
            Arc::new(Registry::new()),
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
        assert_eq!(
            ToolboxService::extract_tool_id("tools/my-tool"),
            Some("my-tool")
        );
        assert_eq!(
            ToolboxService::extract_tool_id("tools/namespace.tool-name"),
            Some("namespace.tool-name")
        );
    }

    #[test]
    fn test_extract_tool_id_returns_none_without_prefix() {
        assert_eq!(ToolboxService::extract_tool_id("my-tool"), None);
        assert_eq!(ToolboxService::extract_tool_id("tool/my-tool"), None);
        assert_eq!(ToolboxService::extract_tool_id(""), None);
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
        assert_eq!(ToolboxService::extract_tool_id("tools/"), Some(""));
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
