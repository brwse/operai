//! MCP transport for the Operai runtime.

use std::{borrow::Cow, sync::Arc};

use operai_core::{ToolInfo, ToolRegistry, policy::session::PolicyStore};
use rmcp::{
    ErrorData, RoleServer,
    handler::server::ServerHandler,
    model::{
        CallToolRequestParam, CallToolResult, Content, Extensions, JsonObject, ListToolsResult,
        PaginatedRequestParam, ServerCapabilities, ServerInfo, Tool,
    },
    service::RequestContext,
    transport::{
        common::http_header::HEADER_SESSION_ID,
        streamable_http_server::{
            StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
        },
    },
};
use serde::de::DeserializeOwned;
use tonic::Code;

use crate::{
    proto::{CallToolRequest, ListToolsRequest, SearchToolsRequest, call_tool_response},
    runtime::{CallMetadata, LocalRuntime, json_value_to_struct, struct_to_json_value},
};

const SEARCH_TOOL_LIST: &str = "list_tool";
const SEARCH_TOOL_FIND: &str = "find_tool";
const SEARCH_TOOL_CALL: &str = "call_tool";

/// MCP service backed by a local Operai runtime.
#[derive(Clone)]
pub struct McpService {
    runtime: LocalRuntime,
    info: ServerInfo,
    search_mode: bool,
}

impl McpService {
    /// Creates a new MCP service from a registry and policy store.
    #[must_use]
    pub fn new(registry: Arc<ToolRegistry>, policy_store: Arc<PolicyStore>) -> Self {
        Self::from_runtime(LocalRuntime::new(registry, policy_store))
    }

    /// Creates a new MCP service from an existing local runtime.
    #[must_use]
    pub fn from_runtime(runtime: LocalRuntime) -> Self {
        Self::with_info(runtime, default_server_info())
    }

    /// Creates a new MCP service with a custom server info payload.
    #[must_use]
    pub fn with_info(runtime: LocalRuntime, info: ServerInfo) -> Self {
        Self {
            runtime,
            info,
            search_mode: false,
        }
    }

    /// Enables or disables searchable mode (list/find/call MCP tools only).
    #[must_use]
    pub fn searchable(mut self, enabled: bool) -> Self {
        self.search_mode = enabled;
        self
    }

    /// Returns true if searchable mode is enabled.
    #[must_use]
    pub fn is_searchable(&self) -> bool {
        self.search_mode
    }

    /// Returns the underlying local runtime.
    #[must_use]
    pub fn runtime(&self) -> &LocalRuntime {
        &self.runtime
    }

    /// Returns the MCP server info advertised during initialization.
    #[must_use]
    pub fn info(&self) -> &ServerInfo {
        &self.info
    }

    /// Builds a streamable HTTP service using the default MCP server config.
    #[must_use]
    pub fn streamable_http_service(&self) -> StreamableHttpService<Self, LocalSessionManager> {
        self.streamable_http_service_with_config(StreamableHttpServerConfig::default())
    }

    /// Builds a streamable HTTP service using a custom MCP server config.
    #[must_use]
    pub fn streamable_http_service_with_config(
        &self,
        config: StreamableHttpServerConfig,
    ) -> StreamableHttpService<Self, LocalSessionManager> {
        let service = self.clone();
        StreamableHttpService::new(move || Ok(service.clone()), Default::default(), config)
    }
}

impl ServerHandler for McpService {
    fn get_info(&self) -> ServerInfo {
        self.info.clone()
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, ErrorData>> + Send + '_ {
        let registry = Arc::clone(self.runtime.registry());
        let search_mode = self.search_mode;
        async move {
            let tools = if search_mode {
                search_mode_tools()
            } else {
                registry.list().map(tool_info_to_mcp).collect()
            };
            Ok(ListToolsResult::with_all_items(tools))
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, ErrorData>> + Send + '_ {
        let runtime = self.runtime.clone();
        let search_mode = self.search_mode;
        async move {
            if search_mode {
                return call_search_mode_tool(request, runtime, context).await;
            }

            let tool_name = ensure_runtime_tool_name(&request.name);
            let input = request
                .arguments
                .map(serde_json::Value::Object)
                .and_then(|value| json_value_to_struct(&value));

            let mut metadata = CallMetadata::default();
            metadata.request_id = context.id.to_string();
            metadata.session_id =
                extract_session_id_from_extensions(&context.extensions).unwrap_or_default();

            let response = runtime
                .call_tool(
                    CallToolRequest {
                        name: tool_name,
                        input,
                    },
                    metadata,
                )
                .await
                .map_err(status_to_error)?;

            match response.result {
                Some(call_tool_response::Result::Output(output)) => {
                    let value = struct_to_json_value(&output);
                    Ok(CallToolResult::structured(value))
                }
                Some(call_tool_response::Result::Error(message)) => {
                    Ok(CallToolResult::error(vec![Content::text(message)]))
                }
                None => Err(ErrorData::internal_error("missing tool response", None)),
            }
        }
    }
}

async fn call_search_mode_tool(
    request: CallToolRequestParam,
    runtime: LocalRuntime,
    context: RequestContext<RoleServer>,
) -> Result<CallToolResult, ErrorData> {
    let mut metadata = CallMetadata::default();
    metadata.request_id = context.id.to_string();
    metadata.session_id =
        extract_session_id_from_extensions(&context.extensions).unwrap_or_default();

    match request.name.as_ref() {
        SEARCH_TOOL_LIST => {
            let args = parse_args_or_default::<ListArgs>(request.arguments)?;
            let response = runtime
                .list_tools(ListToolsRequest {
                    page_size: args.page_size.unwrap_or(0),
                    page_token: args.page_token.unwrap_or_default(),
                })
                .await
                .map_err(status_to_error)?;
            Ok(CallToolResult::structured(list_tools_response_to_json(
                &response,
            )))
        }
        SEARCH_TOOL_FIND => {
            let args = parse_args::<FindArgs>(request.arguments)?;
            let response = runtime
                .search_tools(SearchToolsRequest {
                    query_embedding: args.query_embedding,
                    page_size: args.page_size.unwrap_or(0),
                    page_token: args.page_token.unwrap_or_default(),
                })
                .await
                .map_err(status_to_error)?;
            Ok(CallToolResult::structured(search_tools_response_to_json(
                &response,
            )))
        }
        SEARCH_TOOL_CALL => {
            let args = parse_args::<CallArgs>(request.arguments)?;
            let input = match args.input {
                None => None,
                Some(value @ serde_json::Value::Object(_)) => json_value_to_struct(&value),
                Some(_) => {
                    return Err(ErrorData::invalid_params("input must be an object", None));
                }
            };

            let response = runtime
                .call_tool(
                    CallToolRequest {
                        name: ensure_runtime_tool_name(&args.name),
                        input,
                    },
                    metadata,
                )
                .await
                .map_err(status_to_error)?;
            let value = call_tool_response_to_json(&response);
            match response.result {
                Some(call_tool_response::Result::Output(_)) => {
                    Ok(CallToolResult::structured(value))
                }
                Some(call_tool_response::Result::Error(_)) => {
                    Ok(CallToolResult::structured_error(value))
                }
                None => Err(ErrorData::internal_error("missing tool response", None)),
            }
        }
        _ => Err(ErrorData::resource_not_found(
            format!("tool not found: {}", request.name),
            None,
        )),
    }
}

fn default_server_info() -> ServerInfo {
    let mut info = ServerInfo::default();
    info.capabilities = ServerCapabilities::builder().enable_tools().build();
    info
}

fn ensure_runtime_tool_name(name: &str) -> String {
    if name.starts_with("tools/") {
        name.to_string()
    } else {
        format!("tools/{name}")
    }
}

fn tool_info_to_mcp(info: &ToolInfo) -> Tool {
    Tool {
        name: Cow::Owned(info.qualified_id.clone()),
        title: non_empty_string(&info.display_name),
        description: non_empty_cow(&info.description),
        input_schema: Arc::new(schema_to_object(&info.input_schema)),
        output_schema: schema_to_object_option(&info.output_schema),
        annotations: None,
        icons: None,
        meta: None,
    }
}

fn search_mode_tools() -> Vec<Tool> {
    vec![
        search_mode_list_tool(),
        search_mode_find_tool(),
        search_mode_call_tool(),
    ]
}

fn search_mode_list_tool() -> Tool {
    Tool {
        name: Cow::Borrowed(SEARCH_TOOL_LIST),
        title: Some("List tools".to_string()),
        description: Some(Cow::Borrowed("List available tools with pagination.")),
        input_schema: Arc::new(schema_to_object_value(serde_json::json!({
            "type": "object",
            "description": "List tools the server can run. Use this when you need full tool metadata (schemas, tags) before deciding what to call.",
            "properties": {
                "page_size": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Max tools to return in this page. Use smaller values if you plan to paginate."
                },
                "page_token": {
                    "type": "string",
                    "description": "Token from a previous list response to fetch the next page."
                }
            },
            "additionalProperties": false
        }))),
        output_schema: Some(Arc::new(schema_to_object_value(serde_json::json!({
            "type": "object",
            "description": "Tools plus pagination token. Each tool includes schemas to guide valid calls.",
            "properties": {
                "tools": {
                    "type": "array",
                    "description": "Tools returned for this page.",
                    "items": search_tool_schema()
                },
                "next_page_token": {
                    "type": "string",
                    "description": "Token to fetch the next page. Omit if there are no more results."
                }
            },
            "additionalProperties": false
        })))),
        annotations: None,
        icons: None,
        meta: None,
    }
}

fn search_mode_find_tool() -> Tool {
    Tool {
        name: Cow::Borrowed(SEARCH_TOOL_FIND),
        title: Some("Find tools".to_string()),
        description: Some(Cow::Borrowed("Search tools using a query embedding.")),
        input_schema: Arc::new(schema_to_object_value(serde_json::json!({
            "type": "object",
            "description": "Search tools by semantic similarity. Use this when you have an embedding for the user request and want the most relevant tools.",
            "properties": {
                "query_embedding": {
                    "type": "array",
                    "description": "Embedding vector for the query. Must be produced by the same model used to embed tools.",
                    "items": { "type": "number" }
                },
                "page_size": {
                    "type": "integer",
                    "minimum": 0,
                    "description": "Max results to return. Use smaller values if you plan to paginate."
                },
                "page_token": {
                    "type": "string",
                    "description": "Token from a previous search response to fetch the next page."
                }
            },
            "required": ["query_embedding"],
            "additionalProperties": false
        }))),
        output_schema: Some(Arc::new(schema_to_object_value(serde_json::json!({
            "type": "object",
            "description": "Search results ordered by relevance (higher is better). Use tool.name to call a tool.",
            "properties": {
                "results": {
                    "type": "array",
                    "description": "Search results sorted by relevance.",
                    "items": {
                        "type": "object",
                        "description": "Single search match with score and tool metadata.",
                        "properties": {
                            "tool": search_tool_schema(),
                            "relevance_score": {
                                "type": "number",
                                "description": "Similarity score between 0 and 1. Higher means more relevant."
                            }
                        },
                        "additionalProperties": false
                    }
                },
                "next_page_token": {
                    "type": "string",
                    "description": "Token to fetch the next page. Omit if there are no more results."
                }
            },
            "additionalProperties": false
        })))),
        annotations: None,
        icons: None,
        meta: None,
    }
}

fn search_mode_call_tool() -> Tool {
    Tool {
        name: Cow::Borrowed(SEARCH_TOOL_CALL),
        title: Some("Call tool".to_string()),
        description: Some(Cow::Borrowed(
            "Invoke a tool by name with structured input.",
        )),
        input_schema: Arc::new(schema_to_object_value(serde_json::json!({
            "type": "object",
            "description": "Invoke a tool by name. Use tool schemas from list/find to construct valid input.",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Tool name to invoke (crate.tool or tools/{crate.tool})."
                },
                "input": {
                    "type": "object",
                    "description": "Tool input object matching the tool's input schema."
                }
            },
            "required": ["name"],
            "additionalProperties": false
        }))),
        output_schema: Some(Arc::new(schema_to_object_value(serde_json::json!({
            "type": "object",
            "description": "Tool call result. If the call fails, error will be set.",
            "properties": {
                "output": {
                    "type": "object",
                    "description": "Structured output returned by the tool (per its output schema)."
                },
                "error": {
                    "type": "string",
                    "description": "Error message if the tool call failed."
                }
            },
            "additionalProperties": false
        })))),
        annotations: None,
        icons: None,
        meta: None,
    }
}

fn search_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "description": "Tool metadata. Use name + input_schema to prepare a call.",
        "properties": {
            "name": {
                "type": "string",
                "description": "Qualified tool name (tools/{crate.tool}). Use this to call the tool."
            },
            "display_name": {
                "type": "string",
                "description": "Human-readable tool name."
            },
            "version": {
                "type": "string",
                "description": "Tool version."
            },
            "description": {
                "type": "string",
                "description": "What the tool does."
            },
            "input_schema": {
                "type": "object",
                "description": "JSON schema describing required/optional input fields."
            },
            "output_schema": {
                "type": "object",
                "description": "JSON schema describing tool output."
            },
            "capabilities": {
                "type": "array",
                "description": "Capabilities associated with the tool.",
                "items": { "type": "string" }
            },
            "tags": {
                "type": "array",
                "description": "Tags associated with the tool.",
                "items": { "type": "string" }
            }
        },
        "additionalProperties": false
    })
}

fn non_empty_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn non_empty_cow(value: &str) -> Option<Cow<'static, str>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(Cow::Owned(trimmed.to_string()))
    }
}

fn schema_to_object(schema: &str) -> JsonObject {
    match serde_json::from_str::<serde_json::Value>(schema) {
        Ok(serde_json::Value::Object(map)) => map,
        _ => JsonObject::default(),
    }
}

fn schema_to_object_value(value: serde_json::Value) -> JsonObject {
    match value {
        serde_json::Value::Object(map) => map,
        _ => JsonObject::default(),
    }
}

fn schema_to_object_option(schema: &str) -> Option<Arc<JsonObject>> {
    match serde_json::from_str::<serde_json::Value>(schema) {
        Ok(serde_json::Value::Object(map)) => Some(Arc::new(map)),
        _ => None,
    }
}

fn list_tools_response_to_json(response: &crate::proto::ListToolsResponse) -> serde_json::Value {
    let tools = response
        .tools
        .iter()
        .map(proto_tool_to_json)
        .collect::<Vec<_>>();
    serde_json::json!({
        "tools": tools,
        "next_page_token": response.next_page_token
    })
}

fn search_tools_response_to_json(
    response: &crate::proto::SearchToolsResponse,
) -> serde_json::Value {
    let results = response
        .results
        .iter()
        .map(|result| {
            let tool = result
                .tool
                .as_ref()
                .map(proto_tool_to_json)
                .unwrap_or(serde_json::Value::Null);
            serde_json::json!({
                "tool": tool,
                "relevance_score": result.relevance_score
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "results": results,
        "next_page_token": response.next_page_token
    })
}

fn call_tool_response_to_json(response: &crate::proto::CallToolResponse) -> serde_json::Value {
    match response.result.as_ref() {
        Some(call_tool_response::Result::Output(output)) => {
            serde_json::json!({ "output": struct_to_json_value(output) })
        }
        Some(call_tool_response::Result::Error(message)) => {
            serde_json::json!({ "error": message })
        }
        None => serde_json::Value::Null,
    }
}

fn proto_tool_to_json(tool: &crate::proto::Tool) -> serde_json::Value {
    let input_schema = tool
        .input_schema
        .as_ref()
        .map(struct_to_json_value)
        .unwrap_or(serde_json::Value::Null);
    let output_schema = tool
        .output_schema
        .as_ref()
        .map(struct_to_json_value)
        .unwrap_or(serde_json::Value::Null);

    serde_json::json!({
        "name": tool.name,
        "display_name": tool.display_name,
        "version": tool.version,
        "description": tool.description,
        "input_schema": input_schema,
        "output_schema": output_schema,
        "capabilities": tool.capabilities,
        "tags": tool.tags
    })
}

fn status_to_error(status: tonic::Status) -> ErrorData {
    let message = status.message().to_string();
    match status.code() {
        Code::NotFound => ErrorData::resource_not_found(message, None),
        Code::InvalidArgument => ErrorData::invalid_params(message, None),
        Code::PermissionDenied | Code::Unauthenticated => ErrorData::invalid_request(message, None),
        _ => ErrorData::internal_error(message, None),
    }
}

fn extract_session_id_from_extensions(extensions: &Extensions) -> Option<String> {
    let parts = extensions.get::<http::request::Parts>()?;
    parts
        .headers
        .get(HEADER_SESSION_ID)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string())
}

#[derive(Debug, serde::Deserialize, Default)]
struct ListArgs {
    page_size: Option<i32>,
    page_token: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct FindArgs {
    query_embedding: Vec<f32>,
    page_size: Option<i32>,
    page_token: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct CallArgs {
    name: String,
    input: Option<serde_json::Value>,
}

fn parse_args<T: DeserializeOwned>(args: Option<JsonObject>) -> Result<T, ErrorData> {
    let value = serde_json::Value::Object(args.unwrap_or_default());
    serde_json::from_value(value)
        .map_err(|e| ErrorData::invalid_params(format!("invalid arguments: {e}"), None))
}

fn parse_args_or_default<T: DeserializeOwned + Default>(
    args: Option<JsonObject>,
) -> Result<T, ErrorData> {
    match args {
        Some(map) => serde_json::from_value(serde_json::Value::Object(map))
            .map_err(|e| ErrorData::invalid_params(format!("invalid arguments: {e}"), None)),
        None => Ok(T::default()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use abi_stable::{
        prefix_type::{PrefixRefTrait, WithMetadata},
        std_types::{ROption, RSlice, RStr, RVec},
    };
    use anyhow::Result;
    use operai_abi::{
        CallArgs, CallResult, InitArgs, RuntimeContext, TOOL_ABI_VERSION, ToolDescriptor, ToolMeta,
        ToolModule, ToolModuleRef, ToolResult, async_ffi::FfiFuture,
    };
    use operai_core::policy::session::{InMemoryPolicySessionStore, PolicyStore};
    use rmcp::{
        model::CallToolRequestParam, service::ServiceExt, transport::StreamableHttpClientTransport,
    };
    use serde_json::Value;
    use tokio::sync::oneshot;

    use super::*;

    #[test]
    fn test_extract_session_id_from_extensions() {
        let request = http::Request::builder()
            .header(HEADER_SESSION_ID, "session-123")
            .body(())
            .expect("request should build");
        let (parts, _body) = request.into_parts();

        let mut extensions = Extensions::new();
        extensions.insert(parts);

        let session_id =
            extract_session_id_from_extensions(&extensions).expect("session id should exist");
        assert_eq!(session_id, "session-123");
    }

    #[test]
    fn test_extract_session_id_from_extensions_missing_header() {
        let request = http::Request::builder()
            .body(())
            .expect("request should build");
        let (parts, _body) = request.into_parts();

        let mut extensions = Extensions::new();
        extensions.insert(parts);

        assert!(extract_session_id_from_extensions(&extensions).is_none());
    }

    #[test]
    fn test_search_mode_tools_exposes_expected_names() {
        let tools = search_mode_tools();
        let names: Vec<_> = tools.iter().map(|tool| tool.name.as_ref()).collect();
        assert!(names.contains(&SEARCH_TOOL_LIST));
        assert!(names.contains(&SEARCH_TOOL_FIND));
        assert!(names.contains(&SEARCH_TOOL_CALL));
    }

    extern "C" fn static_tool_init(_args: InitArgs) -> FfiFuture<ToolResult> {
        FfiFuture::new(async { ToolResult::Ok })
    }

    extern "C" fn static_tool_call(_args: CallArgs<'_>) -> FfiFuture<CallResult> {
        let output = RVec::from_slice(br#"{"ok":true}"#);
        FfiFuture::new(async { CallResult::ok(output) })
    }

    extern "C" fn static_tool_shutdown() {}

    fn static_tool_module_ref() -> ToolModuleRef {
        let embedding = Box::leak(Box::new([1.0_f32, 0.0_f32]));
        let descriptor = ToolDescriptor {
            id: RStr::from_str("echo"),
            name: RStr::from_str("Echo"),
            description: RStr::from_str("Static echo tool"),
            input_schema: RStr::from_str(r#"{"type":"object"}"#),
            output_schema: RStr::from_str(r#"{"type":"object"}"#),
            credential_schema: ROption::RNone,
            capabilities: RSlice::from_slice(&[]),
            tags: RSlice::from_slice(&[]),
            embedding: RSlice::from_slice(embedding),
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

    fn json_object(value: Value) -> JsonObject {
        match value {
            Value::Object(map) => map,
            _ => JsonObject::default(),
        }
    }

    #[tokio::test]
    async fn test_searchable_mcp_end_to_end() -> Result<()> {
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

        let service = McpService::from_runtime(runtime)
            .searchable(true)
            .streamable_http_service();
        let router = axum::Router::new().nest_service("/mcp", service);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await;
        });

        let uri = format!("http://{addr}/mcp");
        let client = ().serve(StreamableHttpClientTransport::from_uri(uri)).await?;

        let tools = client.list_all_tools().await?;
        let mut names: Vec<_> = tools.iter().map(|tool| tool.name.as_ref()).collect();
        names.sort();
        assert_eq!(
            names,
            vec![SEARCH_TOOL_CALL, SEARCH_TOOL_FIND, SEARCH_TOOL_LIST]
        );

        let list_result = client
            .call_tool(CallToolRequestParam {
                name: SEARCH_TOOL_LIST.into(),
                arguments: None,
            })
            .await?;
        let list_struct = list_result
            .structured_content
            .expect("list should return structured content");
        let tools_value = list_struct
            .get("tools")
            .and_then(Value::as_array)
            .expect("tools array missing");
        assert!(
            tools_value.iter().any(|tool| {
                tool.get("name").and_then(Value::as_str) == Some("tools/static-tool.echo")
            }),
            "expected list to include static tool"
        );

        let find_args = json_object(serde_json::json!({
            "query_embedding": [1.0, 0.0],
            "page_size": 5
        }));
        let find_result = client
            .call_tool(CallToolRequestParam {
                name: SEARCH_TOOL_FIND.into(),
                arguments: Some(find_args),
            })
            .await?;
        let find_struct = find_result
            .structured_content
            .expect("find should return structured content");
        let results_value = find_struct
            .get("results")
            .and_then(Value::as_array)
            .expect("results array missing");
        assert!(
            results_value.iter().any(|result| {
                result
                    .get("tool")
                    .and_then(Value::as_object)
                    .and_then(|tool| tool.get("name"))
                    .and_then(Value::as_str)
                    == Some("tools/static-tool.echo")
            }),
            "expected find to include static tool"
        );

        let call_args = json_object(serde_json::json!({
            "name": "static-tool.echo",
            "input": {}
        }));
        let call_result = client
            .call_tool(CallToolRequestParam {
                name: SEARCH_TOOL_CALL.into(),
                arguments: Some(call_args),
            })
            .await?;
        let call_struct = call_result
            .structured_content
            .expect("call should return structured content");
        let ok_value = call_struct
            .get("output")
            .and_then(Value::as_object)
            .and_then(|output| output.get("ok"))
            .and_then(Value::as_bool)
            .expect("ok value missing");
        assert!(ok_value, "expected ok to be true");

        client.cancel().await?;
        let _ = shutdown_tx.send(());
        let _ = server.await;

        Ok(())
    }
}
