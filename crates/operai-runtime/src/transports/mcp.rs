//! Model Context Protocol (MCP) transport implementation.
//!
//! This module provides an MCP server implementation that exposes the Operai
//! tool registry through the Model Context Protocol. It supports two modes:
//!
//! - **Standard mode**: Direct tool invocation using the MCP protocol
//! - **Search mode**: Enhanced mode with semantic search capabilities and
//!   paginated tool discovery
//!
//! # Search Mode
//!
//! When search mode is enabled, the server exposes three meta-tools:
//! - `list_tool`: Paginated listing of all available tools
//! - `find_tool`: Semantic search for tools using embedding-based similarity
//! - `call_tool`: Invoke a tool by name with structured input
//!
//! # Architecture
//!
//! `McpService` implements the `rmcp::ServerHandler` trait, bridging
//! the MCP protocol with the local tool runtime. It handles:
//!
//! - Tool discovery and metadata conversion
//! - Request routing between standard and search modes
//! - Session extraction from HTTP headers for policy enforcement
//! - Error translation between gRPC and MCP error formats

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
    search::SearchEmbedder,
};

const SEARCH_TOOL_LIST: &str = "list_tool";
const SEARCH_TOOL_FIND: &str = "find_tool";
const SEARCH_TOOL_CALL: &str = "call_tool";

/// MCP server service implementation.
///
/// Provides the Model Context Protocol server that exposes Operai tools to
/// MCP clients. Supports both standard tool invocation and search mode for
/// enhanced tool discovery.
///
/// # Configuration
///
/// Use the builder-style methods to configure the service:
/// - [`Self::searchable`]: Enable search mode with meta-tools
/// - [`Self::with_search_embedder`]: Set the embedder for semantic search
/// - [`Self::with_info`]: Set custom server info (name, version, capabilities)
///
/// # Example
///
/// ```ignore
/// use operai_runtime::transports::mcp::McpService;
/// use std::sync::Arc;
///
/// # let registry = Arc::new(operai_core::ToolRegistry::new());
/// # let policy_store = Arc::new(operai_core::policy::session::PolicyStore::new(
/// #     Arc::new(operai_core::policy::session::InMemoryPolicySessionStore::new())
/// # ));
/// // Create service with search mode enabled
/// let service = McpService::new(registry, policy_store)
///     .searchable(true)
///     .with_search_embedder(Arc::new(my_embedder));
///
/// // Create HTTP transport
/// let http_service = service.streamable_http_service();
/// ```
#[derive(Clone)]
pub struct McpService {
    runtime: LocalRuntime,
    info: ServerInfo,
    search_mode: bool,
    search_embedder: Option<Arc<dyn SearchEmbedder>>,
}

impl McpService {
    /// Create a new MCP service with default server info.
    ///
    /// Uses a default [`ServerInfo`] with tools capability enabled.
    #[must_use]
    pub fn new(registry: Arc<ToolRegistry>, policy_store: Arc<PolicyStore>) -> Self {
        Self::from_runtime(LocalRuntime::new(registry, policy_store))
    }

    /// Create a new MCP service from an existing runtime.
    ///
    /// Uses a default [`ServerInfo`] with tools capability enabled.
    #[must_use]
    pub fn from_runtime(runtime: LocalRuntime) -> Self {
        Self::with_info(runtime, default_server_info())
    }

    /// Create a new MCP service with custom server info.
    #[must_use]
    pub fn with_info(runtime: LocalRuntime, info: ServerInfo) -> Self {
        Self {
            runtime,
            info,
            search_mode: false,
            search_embedder: None,
        }
    }

    /// Enable or disable search mode.
    ///
    /// When enabled, the service exposes meta-tools (`list_tool`, `find_tool`,
    /// `call_tool`) instead of directly exposing the tool registry.
    #[must_use]
    pub fn searchable(mut self, enabled: bool) -> Self {
        self.search_mode = enabled;
        self
    }

    /// Set the embedder for semantic search.
    ///
    /// Required when search mode is enabled and the `find_tool` meta-tool
    /// will be used. The embedder generates vector embeddings from text queries
    /// to find semantically similar tools.
    #[must_use]
    pub fn with_search_embedder(mut self, embedder: Arc<dyn SearchEmbedder>) -> Self {
        self.search_embedder = Some(embedder);
        self
    }

    /// Returns whether search mode is enabled.
    #[must_use]
    pub fn is_searchable(&self) -> bool {
        self.search_mode
    }

    /// Returns a reference to the underlying runtime.
    #[must_use]
    pub fn runtime(&self) -> &LocalRuntime {
        &self.runtime
    }

    /// Returns a reference to the server info.
    #[must_use]
    pub fn info(&self) -> &ServerInfo {
        &self.info
    }

    /// Creates a streamable HTTP service with default configuration.
    ///
    /// Returns an Axum-compatible service that can be mounted in a router
    /// to serve the MCP protocol over HTTP with streaming support.
    #[must_use]
    pub fn streamable_http_service(&self) -> StreamableHttpService<Self, LocalSessionManager> {
        self.streamable_http_service_with_config(StreamableHttpServerConfig::default())
    }

    /// Creates a streamable HTTP service with custom configuration.
    ///
    /// Returns an Axum-compatible service with custom server settings for
    /// timeouts, limits, and other HTTP transport parameters.
    #[must_use]
    pub fn streamable_http_service_with_config(
        &self,
        config: StreamableHttpServerConfig,
    ) -> StreamableHttpService<Self, LocalSessionManager> {
        let service = self.clone();
        StreamableHttpService::new(move || Ok(service.clone()), Arc::default(), config)
    }
}

impl ServerHandler for McpService {
    /// Returns the server info provided during construction.
    fn get_info(&self) -> ServerInfo {
        self.info.clone()
    }

    /// Lists available tools.
    ///
    /// In standard mode, returns all tools from the registry.
    /// In search mode, returns the three meta-tools for search/list/call.
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

    /// Calls a tool by name.
    ///
    /// In standard mode, directly invokes the specified tool from the registry.
    /// In search mode, routes to the appropriate meta-tool handler.
    ///
    /// Tool names are normalized to include the "tools/" prefix if not already
    /// present.
    fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, ErrorData>> + Send + '_ {
        let runtime = self.runtime.clone();
        let search_mode = self.search_mode;
        let search_embedder = self.search_embedder.clone();
        async move {
            if search_mode {
                return call_search_mode_tool(request, runtime, context, search_embedder).await;
            }

            let tool_name = ensure_runtime_tool_name(&request.name);
            let input = request
                .arguments
                .map(serde_json::Value::Object)
                .and_then(|value| json_value_to_struct(&value));

            let metadata = CallMetadata {
                request_id: context.id.to_string(),
                session_id: extract_session_id_from_extensions(&context.extensions)
                    .unwrap_or_default(),
                ..Default::default()
            };

            let response = runtime
                .call_tool(
                    CallToolRequest {
                        name: tool_name,
                        input,
                    },
                    metadata,
                )
                .await
                .map_err(|status| status_to_error(&status))?;

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

/// Handles tool invocations in search mode.
///
/// Routes calls to the appropriate meta-tool:
/// - `list_tool`: Paginated tool listing
/// - `find_tool`: Semantic search with embeddings
/// - `call_tool`: Direct tool invocation
async fn call_search_mode_tool(
    request: CallToolRequestParam,
    runtime: LocalRuntime,
    context: RequestContext<RoleServer>,
    search_embedder: Option<Arc<dyn SearchEmbedder>>,
) -> Result<CallToolResult, ErrorData> {
    let metadata = CallMetadata {
        request_id: context.id.to_string(),
        session_id: extract_session_id_from_extensions(&context.extensions).unwrap_or_default(),
        ..Default::default()
    };

    match request.name.as_ref() {
        SEARCH_TOOL_LIST => {
            let args = parse_args_or_default::<ListArgs>(request.arguments)?;
            let response = runtime
                .list_tools(ListToolsRequest {
                    page_size: args.page_size.unwrap_or(0),
                    page_token: args.page_token.unwrap_or_default(),
                })
                .await
                .map_err(|status| status_to_error(&status))?;
            Ok(CallToolResult::structured(list_tools_response_to_json(
                &response,
            )))
        }
        SEARCH_TOOL_FIND => {
            let args = parse_args::<FindArgs>(request.arguments)?;
            if args.query.trim().is_empty() {
                return Err(ErrorData::invalid_params("query must be non-empty", None));
            }
            let embedder = search_embedder.ok_or_else(|| {
                ErrorData::invalid_request("search embedder not configured", None)
            })?;
            let embedding = embedder.embed_query(&args.query).await.map_err(|err| {
                ErrorData::internal_error(format!("failed to embed query: {err}"), None)
            })?;
            let response = runtime
                .search_tools(SearchToolsRequest {
                    query_embedding: embedding,
                    query_text: String::new(),
                    page_size: args.page_size.unwrap_or(0),
                    page_token: args.page_token.unwrap_or_default(),
                })
                .await
                .map_err(|status| status_to_error(&status))?;
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
                .map_err(|status| status_to_error(&status))?;
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

/// Creates the default server info with tools capability enabled.
fn default_server_info() -> ServerInfo {
    ServerInfo {
        capabilities: ServerCapabilities::builder().enable_tools().build(),
        ..Default::default()
    }
}

/// Ensures a tool name has the "tools/" prefix.
///
/// Runtime tools are identified with a "tools/" prefix (e.g.,
/// "tools/crate.tool"). This function normalizes tool names by adding the
/// prefix if not present.
fn ensure_runtime_tool_name(name: &str) -> String {
    if name.starts_with("tools/") {
        name.to_string()
    } else {
        format!("tools/{name}")
    }
}

/// Converts internal tool info to MCP tool format.
///
/// Transforms [`ToolInfo`] from the tool registry into the MCP [`Tool`] format,
/// converting schemas and handling optional fields.
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

/// Returns the three meta-tools for search mode.
fn search_mode_tools() -> Vec<Tool> {
    vec![
        search_mode_list_tool(),
        search_mode_find_tool(),
        search_mode_call_tool(),
    ]
}

/// Creates the `list_tool` meta-tool for paginated tool listing.
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

/// Creates the `find_tool` meta-tool for semantic search.
fn search_mode_find_tool() -> Tool {
    Tool {
        name: Cow::Borrowed(SEARCH_TOOL_FIND),
        title: Some("Find tools".to_string()),
        description: Some(Cow::Borrowed("Search tools using a query string.")),
        input_schema: Arc::new(schema_to_object_value(serde_json::json!({
            "type": "object",
            "description": "Search tools by semantic similarity. Provide a plain-language query and the server will embed it.",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query text describing what you want to do."
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
            "required": ["query"],
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

/// Creates the `call_tool` meta-tool for direct tool invocation.
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

/// Returns the JSON schema for a tool metadata object.
///
/// Defines the structure of tool information returned by search/list
/// operations, including name, description, schemas, capabilities, and tags.
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

/// Returns `Some` trimmed string if non-empty, `None` otherwise.
fn non_empty_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Returns `Some` Cow-ified string if non-empty, `None` otherwise.
fn non_empty_cow(value: &str) -> Option<Cow<'static, str>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(Cow::Owned(trimmed.to_string()))
    }
}

/// Parses a JSON schema string into a `JsonObject`.
///
/// Returns an empty object if the schema is invalid or not an object.
fn schema_to_object(schema: &str) -> JsonObject {
    match serde_json::from_str::<serde_json::Value>(schema) {
        Ok(serde_json::Value::Object(map)) => map,
        _ => JsonObject::default(),
    }
}

/// Extracts a `JsonObject` from a JSON Value, returning empty object if not an
/// object.
fn schema_to_object_value(value: serde_json::Value) -> JsonObject {
    match value {
        serde_json::Value::Object(map) => map,
        _ => JsonObject::default(),
    }
}

/// Parses a JSON schema string into an optional Arc'd `JsonObject`.
///
/// Returns `None` if the schema is invalid or not an object.
fn schema_to_object_option(schema: &str) -> Option<Arc<JsonObject>> {
    match serde_json::from_str::<serde_json::Value>(schema) {
        Ok(serde_json::Value::Object(map)) => Some(Arc::new(map)),
        _ => None,
    }
}

/// Converts a protobuf `ListToolsResponse` to JSON.
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

/// Converts a protobuf `SearchToolsResponse` to JSON.
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
                .map_or(serde_json::Value::Null, proto_tool_to_json);
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

/// Converts a protobuf `CallToolResponse` to JSON.
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

/// Converts a protobuf Tool to JSON.
fn proto_tool_to_json(tool: &crate::proto::Tool) -> serde_json::Value {
    let input_schema = tool
        .input_schema
        .as_ref()
        .map_or(serde_json::Value::Null, struct_to_json_value);
    let output_schema = tool
        .output_schema
        .as_ref()
        .map_or(serde_json::Value::Null, struct_to_json_value);

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

/// Converts a gRPC Status to an MCP `ErrorData`.
///
/// Maps gRPC status codes to appropriate MCP error types:
/// - `NotFound` → `resource_not_found`
/// - `InvalidArgument` → `invalid_params`
/// - `PermissionDenied`/`Unauthenticated` → `invalid_request`
/// - Others → `internal_error`
fn status_to_error(status: &tonic::Status) -> ErrorData {
    let message = status.message().to_string();
    match status.code() {
        Code::NotFound => ErrorData::resource_not_found(message, None),
        Code::InvalidArgument => ErrorData::invalid_params(message, None),
        Code::PermissionDenied | Code::Unauthenticated => ErrorData::invalid_request(message, None),
        _ => ErrorData::internal_error(message, None),
    }
}

/// Extracts the session ID from HTTP request extensions.
///
/// Looks for the `session-id` header in the request parts stored in
/// the extensions. Used for policy enforcement and session tracking.
fn extract_session_id_from_extensions(extensions: &Extensions) -> Option<String> {
    let parts = extensions.get::<http::request::Parts>()?;
    parts
        .headers
        .get(HEADER_SESSION_ID)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
}

/// Arguments for the `list_tool` meta-tool.
#[derive(Debug, serde::Deserialize, Default)]
struct ListArgs {
    page_size: Option<i32>,
    page_token: Option<String>,
}

/// Arguments for the `find_tool` meta-tool.
#[derive(Debug, serde::Deserialize)]
struct FindArgs {
    query: String,
    page_size: Option<i32>,
    page_token: Option<String>,
}

/// Arguments for the `call_tool` meta-tool.
#[derive(Debug, serde::Deserialize)]
struct CallArgs {
    name: String,
    input: Option<serde_json::Value>,
}

/// Parses tool arguments from an optional `JsonObject`.
///
/// Returns an error if the arguments cannot be deserialized into the target
/// type.
fn parse_args<T: DeserializeOwned>(args: Option<JsonObject>) -> Result<T, ErrorData> {
    let value = serde_json::Value::Object(args.unwrap_or_default());
    serde_json::from_value(value)
        .map_err(|e| ErrorData::invalid_params(format!("invalid arguments: {e}"), None))
}

/// Parses tool arguments from an optional `JsonObject`, using default if None.
///
/// Returns the default value if arguments are None, or deserializes them if
/// present.
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
    use crate::SearchEmbedFuture;

    struct TestEmbedder;

    impl SearchEmbedder for TestEmbedder {
        fn embed_query(&self, _query: &str) -> SearchEmbedFuture<'_> {
            Box::pin(async { Ok(vec![1.0, 0.0]) })
        }
    }

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
            .with_search_embedder(Arc::new(TestEmbedder))
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
        names.sort_unstable();
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
            "query": "echo",
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
