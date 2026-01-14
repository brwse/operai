//! Runtime abstractions for local and remote tool execution.

use std::{collections::HashMap, sync::Arc};

use abi_stable::std_types::{RSlice, RStr};
use base64::prelude::*;
use futures::FutureExt;
use operai_abi::{CallContext, RuntimeContext, ToolResult};
use operai_core::{PolicyError, ToolInfo, ToolRegistry, policy::session::PolicyStore};
use rkyv::rancor::BoxedError;
use tonic::{Request, Status, transport::Channel};
use tracing::{error, info};

use crate::proto::{
    CallToolRequest, CallToolResponse, ListToolsRequest, ListToolsResponse, SearchResult,
    SearchToolsRequest, SearchToolsResponse, Tool, call_tool_response,
    toolbox_client::ToolboxClient,
};

/// Metadata associated with a tool call.
#[derive(Debug, Clone, Default)]
pub struct CallMetadata {
    /// Request identifier for tracing.
    pub request_id: String,
    /// Session identifier for stateful tools.
    pub session_id: String,
    /// User identifier for auditing.
    pub user_id: String,
    /// Credential payloads keyed by provider.
    pub credentials: HashMap<String, HashMap<String, String>>,
}

/// Unified runtime abstraction for local or remote execution.
#[derive(Clone)]
pub enum Runtime {
    /// Local runtime backed by an in-process registry.
    Local(LocalRuntime),
    /// Remote runtime backed by a gRPC client.
    Remote(RemoteRuntime),
}

impl Runtime {
    /// Lists tools from the configured runtime.
    pub async fn list_tools(&self, request: ListToolsRequest) -> Result<ListToolsResponse, Status> {
        match self {
            Self::Local(runtime) => runtime.list_tools(request).await,
            Self::Remote(runtime) => runtime.list_tools(request).await,
        }
    }

    /// Searches tools from the configured runtime.
    pub async fn search_tools(
        &self,
        request: SearchToolsRequest,
    ) -> Result<SearchToolsResponse, Status> {
        match self {
            Self::Local(runtime) => runtime.search_tools(request).await,
            Self::Remote(runtime) => runtime.search_tools(request).await,
        }
    }

    /// Calls a tool on the configured runtime.
    pub async fn call_tool(
        &self,
        request: CallToolRequest,
        metadata: CallMetadata,
    ) -> Result<CallToolResponse, Status> {
        match self {
            Self::Local(runtime) => runtime.call_tool(request, metadata).await,
            Self::Remote(runtime) => runtime.call_tool(request, metadata).await,
        }
    }
}

/// Runtime implementation that executes tools locally.
#[derive(Clone)]
pub struct LocalRuntime {
    registry: Arc<ToolRegistry>,
    policy_store: Arc<PolicyStore>,
    runtime_ctx: RuntimeContext,
}

impl LocalRuntime {
    /// Creates a local runtime from an existing registry and policy store.
    #[must_use]
    pub fn new(registry: Arc<ToolRegistry>, policy_store: Arc<PolicyStore>) -> Self {
        Self::with_context(registry, policy_store, RuntimeContext::new())
    }

    /// Creates a local runtime with an explicit runtime context.
    #[must_use]
    pub fn with_context(
        registry: Arc<ToolRegistry>,
        policy_store: Arc<PolicyStore>,
        runtime_ctx: RuntimeContext,
    ) -> Self {
        Self {
            registry,
            policy_store,
            runtime_ctx,
        }
    }

    /// Returns the tool registry backing this runtime.
    #[must_use]
    pub fn registry(&self) -> &Arc<ToolRegistry> {
        &self.registry
    }

    /// Returns the policy store backing this runtime.
    #[must_use]
    pub fn policy_store(&self) -> &Arc<PolicyStore> {
        &self.policy_store
    }

    /// Returns the runtime context used for tool initialization.
    #[must_use]
    pub fn runtime_context(&self) -> &RuntimeContext {
        &self.runtime_ctx
    }

    /// Waits for any in-flight calls to complete.
    pub async fn drain(&self) {
        self.registry.drain().await;
    }

    /// Lists tools from the local registry.
    pub async fn list_tools(&self, request: ListToolsRequest) -> Result<ListToolsResponse, Status> {
        let page_size: usize = if request.page_size <= 0 {
            100
        } else {
            usize::try_from(request.page_size.min(1000)).unwrap_or(1000)
        };

        let offset: usize = request.page_token.parse().unwrap_or(0);

        let all_tools: Vec<_> = self.registry.list().collect();
        let total = all_tools.len();

        let tools: Vec<Tool> = all_tools
            .into_iter()
            .skip(offset)
            .take(page_size)
            .map(tool_info_to_proto)
            .collect();

        let next_offset = offset + tools.len();
        let next_page_token = if next_offset < total {
            next_offset.to_string()
        } else {
            String::new()
        };

        Ok(ListToolsResponse {
            tools,
            next_page_token,
        })
    }

    /// Searches tools from the local registry.
    pub async fn search_tools(
        &self,
        request: SearchToolsRequest,
    ) -> Result<SearchToolsResponse, Status> {
        if request.query_embedding.is_empty() {
            return Err(Status::invalid_argument("query_embedding is required"));
        }

        let page_size = if request.page_size <= 0 {
            10
        } else {
            usize::try_from(request.page_size.min(100)).unwrap_or(100)
        };

        info!(
            embedding_dims = request.query_embedding.len(),
            "Searching with provided embedding"
        );

        let search_results = self.registry.search(&request.query_embedding, page_size);

        let results: Vec<SearchResult> = search_results
            .into_iter()
            .map(|(tool_info, score)| SearchResult {
                tool: Some(tool_info_to_proto(tool_info)),
                relevance_score: score,
            })
            .collect();

        info!(
            embedding_dims = request.query_embedding.len(),
            result_count = results.len(),
            "Search completed"
        );

        Ok(SearchToolsResponse {
            results,
            next_page_token: String::new(),
        })
    }

    /// Calls a tool via the local registry and policy engine.
    pub async fn call_tool(
        &self,
        request: CallToolRequest,
        metadata: CallMetadata,
    ) -> Result<CallToolResponse, Status> {
        let tool_id = extract_tool_id(&request.name)
            .ok_or_else(|| Status::invalid_argument("invalid tool name format"))?;

        let handle = self
            .registry
            .get(tool_id)
            .ok_or_else(|| Status::not_found(format!("tool not found: {tool_id}")))?;

        info!(tool_id = %tool_id, request_id = %metadata.request_id, "Invoking tool");

        let inflight_guard = self.registry.start_request_guard();

        let input_value = if let Some(s) = request.input.as_ref() {
            struct_to_json_value(s)
        } else {
            serde_json::Value::Object(serde_json::Map::new())
        };
        let input_json = serde_json::to_vec(&input_value).unwrap_or_else(|_| b"{}".to_vec());

        self.policy_store
            .evaluate_pre_effects(&metadata.session_id, tool_id, &input_value)
            .await
            .map_err(|e| match e {
                PolicyError::GuardFailed(msg) => Status::permission_denied(msg),
                _ => Status::internal(format!("policy evaluation error: {e}")),
            })?;

        let user_creds_bin = rkyv::to_bytes::<BoxedError>(&metadata.credentials)
            .map_err(|e| Status::internal(format!("failed to serialize credentials: {e}")))?;
        let system_creds_bin = &handle.system_credentials;

        let context = CallContext {
            request_id: RStr::from_str(&metadata.request_id),
            session_id: RStr::from_str(&metadata.session_id),
            user_id: RStr::from_str(&metadata.user_id),
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
                        Ok(CallToolResponse {
                            result: Some(call_tool_response::Result::Output(output_struct)),
                        }),
                        Some(output_value),
                        None,
                    )
                }
                ToolResult::Error => {
                    let error_msg =
                        String::from_utf8_lossy(call_result.output.as_slice()).to_string();
                    error!(tool_id = %tool_id, error = %error_msg, "Tool invocation failed");

                    (
                        Ok(CallToolResponse {
                            result: Some(call_tool_response::Result::Error(error_msg.clone())),
                        }),
                        None,
                        Some(error_msg),
                    )
                }
                other => {
                    error!(tool_id = %tool_id, result = ?other, "Tool invocation failed");
                    let msg = format!("tool error: {other:?}");
                    (
                        Ok(CallToolResponse {
                            result: Some(call_tool_response::Result::Error(msg.clone())),
                        }),
                        None,
                        Some(msg),
                    )
                }
            }
        } else {
            let msg = "Tool execution panicked".to_string();
            (Err(Status::internal(&msg)), None, Some(msg))
        };

        let policy_res_arg = match &policy_outcome_err {
            Some(e) => Err(e.as_str()),
            None => Ok(policy_outcome_val
                .as_ref()
                .unwrap_or(&serde_json::Value::Null)),
        };

        self.policy_store
            .evaluate_post_effects(&metadata.session_id, tool_id, &input_value, policy_res_arg)
            .await
            .map_err(|e| Status::internal(format!("policy effect error: {e}")))?;

        rpc_result
    }
}

/// Runtime implementation that forwards requests to a remote gRPC server.
#[derive(Clone)]
pub struct RemoteRuntime {
    client: ToolboxClient<Channel>,
}

impl RemoteRuntime {
    /// Connects to a remote runtime at the given endpoint.
    pub async fn connect(endpoint: impl AsRef<str>) -> Result<Self, tonic::transport::Error> {
        let endpoint = normalize_endpoint(endpoint.as_ref());
        let client = ToolboxClient::connect(endpoint).await?;
        Ok(Self { client })
    }

    /// Creates a remote runtime from an existing client.
    #[must_use]
    pub fn new(client: ToolboxClient<Channel>) -> Self {
        Self { client }
    }

    /// Lists tools from the remote runtime.
    pub async fn list_tools(&self, request: ListToolsRequest) -> Result<ListToolsResponse, Status> {
        let response = self.client.clone().list_tools(request).await?.into_inner();
        Ok(response)
    }

    /// Searches tools from the remote runtime.
    pub async fn search_tools(
        &self,
        request: SearchToolsRequest,
    ) -> Result<SearchToolsResponse, Status> {
        let response = self
            .client
            .clone()
            .search_tools(request)
            .await?
            .into_inner();
        Ok(response)
    }

    /// Calls a tool on the remote runtime.
    pub async fn call_tool(
        &self,
        request: CallToolRequest,
        metadata: CallMetadata,
    ) -> Result<CallToolResponse, Status> {
        let mut request = Request::new(request);
        apply_call_metadata(&mut request, &metadata)?;

        let response = self.client.clone().call_tool(request).await?.into_inner();
        Ok(response)
    }
}

pub(crate) fn tool_info_to_proto(info: &ToolInfo) -> Tool {
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

pub(crate) fn extract_tool_id(name: &str) -> Option<&str> {
    name.strip_prefix("tools/")
}

pub(crate) fn json_str_to_struct(json: &str) -> Option<prost_types::Struct> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    json_value_to_struct(&value)
}

pub(crate) fn json_value_to_struct(value: &serde_json::Value) -> Option<prost_types::Struct> {
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

pub(crate) fn struct_to_json_value(s: &prost_types::Struct) -> serde_json::Value {
    let map: serde_json::Map<String, serde_json::Value> = s
        .fields
        .iter()
        .map(|(k, v)| (k.clone(), prost_value_to_json_value(v)))
        .collect();
    serde_json::Value::Object(map)
}

pub(crate) fn prost_value_to_json_value(value: &prost_types::Value) -> serde_json::Value {
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

fn normalize_endpoint(endpoint: &str) -> String {
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        endpoint.to_string()
    } else {
        format!("http://{endpoint}")
    }
}

fn apply_call_metadata(
    request: &mut Request<CallToolRequest>,
    metadata: &CallMetadata,
) -> Result<(), Status> {
    let headers = request.metadata_mut();

    insert_header(headers, "x-request-id", &metadata.request_id)?;
    insert_header(headers, "x-session-id", &metadata.session_id)?;
    insert_header(headers, "x-user-id", &metadata.user_id)?;

    for (provider, values) in &metadata.credentials {
        let json = serde_json::to_string(&CredentialData { values })
            .map_err(|e| Status::internal(format!("credential serialization error: {e}")))?;
        let encoded = BASE64_STANDARD.encode(json);
        let header_name = format!("x-credential-{provider}");
        let key = tonic::metadata::MetadataKey::from_bytes(header_name.as_bytes())
            .map_err(|_| Status::invalid_argument("invalid credential header name"))?;
        let val = tonic::metadata::MetadataValue::try_from(encoded)
            .map_err(|_| Status::invalid_argument("invalid credential value"))?;
        headers.insert(key, val);
    }

    Ok(())
}

fn insert_header(
    headers: &mut tonic::metadata::MetadataMap,
    key: &'static str,
    value: &str,
) -> Result<(), Status> {
    if value.is_empty() {
        return Ok(());
    }
    let value = value
        .parse()
        .map_err(|_| Status::invalid_argument("invalid metadata value"))?;
    headers.insert(key, value);
    Ok(())
}

#[derive(serde::Serialize)]
struct CredentialData<'a> {
    values: &'a HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use abi_stable::{
        prefix_type::{PrefixRefTrait, WithMetadata},
        std_types::{ROption, RSlice, RStr, RVec},
    };
    use operai_abi::{
        CallArgs, CallResult, InitArgs, RuntimeContext, TOOL_ABI_VERSION, ToolDescriptor, ToolMeta,
        ToolModule, ToolModuleRef, ToolResult, async_ffi::FfiFuture,
    };
    use operai_core::policy::session::{InMemoryPolicySessionStore, PolicyStore};

    use super::*;
    use crate::proto::{CallToolRequest, call_tool_response};

    extern "C" fn static_tool_init(_args: InitArgs) -> FfiFuture<ToolResult> {
        FfiFuture::new(async { ToolResult::Ok })
    }

    extern "C" fn static_tool_call(_args: CallArgs<'_>) -> FfiFuture<CallResult> {
        let output = RVec::from_slice(br#"{"ok":true}"#);
        FfiFuture::new(async { CallResult::ok(output) })
    }

    extern "C" fn static_tool_shutdown() {}

    fn static_tool_module_ref() -> ToolModuleRef {
        let descriptor = ToolDescriptor {
            id: RStr::from_str("echo"),
            name: RStr::from_str("Echo"),
            description: RStr::from_str("Static echo tool"),
            input_schema: RStr::from_str(r#"{"type":"object"}"#),
            output_schema: RStr::from_str(r#"{"type":"object"}"#),
            credential_schema: ROption::RNone,
            capabilities: RSlice::from_slice(&[]),
            tags: RSlice::from_slice(&[]),
            embedding: RSlice::from_slice(&[]),
        };
        let descriptors = Box::leak(Box::new([descriptor]));

        let module = ToolModule {
            meta: ToolMeta::new(
                TOOL_ABI_VERSION,
                RStr::from_str("static-tool"),
                RStr::from_str("0.1.0"),
            ),
            descriptors: RSlice::from_slice(descriptors),
            init: static_tool_init,
            call: static_tool_call,
            shutdown: static_tool_shutdown,
        };

        let with_metadata: &'static WithMetadata<ToolModule> =
            Box::leak(Box::new(WithMetadata::new(module)));
        ToolModuleRef::from_prefix_ref(with_metadata.static_as_prefix())
    }

    #[tokio::test]
    async fn test_local_runtime_registers_static_tool() {
        let module = static_tool_module_ref();
        let mut registry = ToolRegistry::new();
        let runtime_ctx = RuntimeContext::new();

        registry
            .register_module(module, None, &runtime_ctx)
            .await
            .expect("static module should register");

        let registry = Arc::new(registry);
        let policy_store = Arc::new(PolicyStore::new(
            Arc::new(InMemoryPolicySessionStore::new()),
        ));
        let runtime = LocalRuntime::with_context(Arc::clone(&registry), policy_store, runtime_ctx);

        let response = runtime
            .call_tool(
                CallToolRequest {
                    name: "tools/static-tool.echo".to_string(),
                    input: None,
                },
                CallMetadata::default(),
            )
            .await
            .expect("call_tool should succeed");

        let Some(call_tool_response::Result::Output(output)) = response.result else {
            panic!("expected output result");
        };
        let ok_value = output.fields.get("ok").expect("missing `ok` field");
        match &ok_value.kind {
            Some(prost_types::value::Kind::BoolValue(true)) => {}
            other => panic!("expected `ok` to be true, got {other:?}"),
        }
    }
}
