//! CLI command for displaying detailed information about a specific tool.
//!
//! The `describe` command connects to a Toolbox gRPC server, retrieves the
//! complete list of available tools, and displays detailed information about
//! a specific tool including its metadata, input schema, and output schema.

use anyhow::{Context, Result};
use clap::Args;
use console::style;
use operai_runtime::proto::toolbox_client::ToolboxClient;

/// Command-line arguments for the `describe` subcommand.
#[derive(Args)]
pub struct DescribeArgs {
    /// The identifier of the tool to describe.
    ///
    /// Can be specified with or without the "tools/" prefix (e.g., both
    /// "hello-world.greet" and "tools/hello-world.greet" are accepted).
    pub tool_id: String,

    /// The gRPC server address to connect to.
    ///
    /// Defaults to `<http://localhost:50051>` if not provided.
    #[arg(short, long, default_value = "http://localhost:50051")]
    pub server: String,
}

/// Executes the describe command to display tool information.
///
/// Connects to the Toolbox server, retrieves all tools, finds the requested
/// tool by ID, and prints its details including name, version, description,
/// capabilities, tags, and input/output schemas.
///
/// # Errors
///
/// Returns an error if:
/// - Connection to the server fails
/// - The server returns an error response
/// - The requested tool is not found
pub async fn run(args: &DescribeArgs) -> Result<()> {
    let mut client = ToolboxClient::connect(args.server.clone())
        .await
        .context("failed to connect to server")?;

    let request = operai_runtime::proto::ListToolsRequest {
        page_size: 1000,
        page_token: String::new(),
    };

    let response = client
        .list_tools(request)
        .await
        .context("failed to list tools")?
        .into_inner();

    let expected_name = if args.tool_id.starts_with("tools/") {
        args.tool_id.clone()
    } else {
        format!("tools/{}", args.tool_id)
    };

    let tool = response
        .tools
        .iter()
        .find(|t| t.name == expected_name || t.name == args.tool_id);

    match tool {
        Some(tool) => {
            println!("{}", style("Tool Details").bold().underlined());
            println!();

            println!(
                "{}: {}",
                style("ID").cyan(),
                tool.name.strip_prefix("tools/").unwrap_or(&tool.name)
            );

            if !tool.display_name.is_empty() {
                println!("{}: {}", style("Name").cyan(), tool.display_name);
            }

            if !tool.version.is_empty() {
                println!("{}: {}", style("Version").cyan(), tool.version);
            }

            if !tool.description.is_empty() {
                println!("{}: {}", style("Description").cyan(), tool.description);
            }

            if !tool.capabilities.is_empty() {
                println!(
                    "{}: {}",
                    style("Capabilities").cyan(),
                    tool.capabilities.join(", ")
                );
            }

            if !tool.tags.is_empty() {
                println!("{}: {}", style("Tags").cyan(), tool.tags.join(", "));
            }

            println!();
            println!("{}", style("Input Schema").bold().underlined());
            if let Some(input_schema) = &tool.input_schema {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&struct_to_json(input_schema.clone()))?
                );
            }

            println!();
            println!("{}", style("Output Schema").bold().underlined());
            if let Some(output_schema) = &tool.output_schema {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&struct_to_json(output_schema.clone()))?
                );
            }
        }
        None => {
            anyhow::bail!("tool not found: {}", args.tool_id);
        }
    }

    Ok(())
}

/// Converts a protobuf `Struct` to a JSON `Object`.
///
/// Recursively converts all nested values from protobuf format to JSON format.
fn struct_to_json(s: prost_types::Struct) -> serde_json::Value {
    let map = s
        .fields
        .into_iter()
        .map(|(k, v)| (k, prost_value_to_json(v)))
        .collect();
    serde_json::Value::Object(map)
}

/// Converts a protobuf `Value` to a JSON `Value`.
///
/// Handles all protobuf value variants including null, numbers, strings,
/// booleans, structs (nested objects), and lists.
fn prost_value_to_json(v: prost_types::Value) -> serde_json::Value {
    match v.kind {
        Some(prost_types::value::Kind::NullValue(_)) | None => serde_json::Value::Null,
        Some(prost_types::value::Kind::NumberValue(n)) => serde_json::Number::from_f64(n)
            .map_or(serde_json::Value::Null, serde_json::Value::Number),
        Some(prost_types::value::Kind::StringValue(s)) => serde_json::Value::String(s),
        Some(prost_types::value::Kind::BoolValue(b)) => serde_json::Value::Bool(b),
        Some(prost_types::value::Kind::StructValue(s)) => struct_to_json(s),
        Some(prost_types::value::Kind::ListValue(l)) => {
            let values = l.values.into_iter().map(prost_value_to_json).collect();
            serde_json::Value::Array(values)
        }
    }
}

#[cfg(test)]
mod tests {
    /// Unit tests for the describe command.
    ///
    /// Tests cover both successful tool lookup and "not found" scenarios
    /// using a mock Toolbox server.
    use operai_runtime::proto::{
        CallToolRequest, CallToolResponse, ListToolsRequest, ListToolsResponse, SearchToolsRequest,
        SearchToolsResponse, Tool,
        toolbox_server::{Toolbox, ToolboxServer},
    };
    use tonic::{Request, Response, Status};

    use super::*;

    /// Mock Toolbox server implementation for testing.
    struct MockToolbox;

    #[tonic::async_trait]
    impl Toolbox for MockToolbox {
        /// Returns a fixed list of tools for testing.
        async fn list_tools(
            &self,
            _request: Request<ListToolsRequest>,
        ) -> Result<Response<ListToolsResponse>, Status> {
            Ok(Response::new(ListToolsResponse {
                tools: vec![
                    Tool {
                        name: "tools/hello-world.greet".to_string(),
                        display_name: "Greet".to_string(),
                        description: "Says hello".to_string(),
                        version: "1.0.0".to_string(),
                        capabilities: vec!["test".to_string()],
                        tags: vec!["demo".to_string()],
                        ..Default::default()
                    },
                    Tool {
                        name: "tools/calc.add".to_string(),
                        display_name: "Add".to_string(),
                        description: "Adds numbers".to_string(),
                        ..Default::default()
                    },
                ],
                next_page_token: String::new(),
            }))
        }

        /// Returns an unimplemented error for search operations.
        async fn search_tools(
            &self,
            _request: Request<SearchToolsRequest>,
        ) -> Result<Response<SearchToolsResponse>, Status> {
            Err(Status::unimplemented("not implemented"))
        }

        /// Returns an unimplemented error for call operations.
        async fn call_tool(
            &self,
            _request: Request<CallToolRequest>,
        ) -> Result<Response<CallToolResponse>, Status> {
            Err(Status::unimplemented("not implemented"))
        }
    }

    /// Tests successful tool lookup and display.
    #[tokio::test]
    async fn test_run_describe_found() -> Result<()> {
        let _lock = crate::testing::test_lock_async().await;

        let port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
            listener.local_addr()?.port()
        };
        let addr_str = format!("127.0.0.1:{port}");
        let addr: std::net::SocketAddr = addr_str.parse()?;

        let server = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(ToolboxServer::new(MockToolbox))
                .serve(addr)
                .await
                .unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let args = DescribeArgs {
            tool_id: "hello-world.greet".to_owned(),
            server: format!("http://{addr_str}"),
        };

        run(&args).await.context("run failed")?;

        server.abort();
        Ok(())
    }

    /// Tests error handling when a tool is not found.
    #[tokio::test]
    async fn test_run_describe_not_found() -> Result<()> {
        let _lock = crate::testing::test_lock_async().await;

        let port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
            listener.local_addr()?.port()
        };
        let addr_str = format!("127.0.0.1:{port}");
        let addr: std::net::SocketAddr = addr_str.parse()?;

        let server = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(ToolboxServer::new(MockToolbox))
                .serve(addr)
                .await
                .unwrap();
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let args = DescribeArgs {
            tool_id: "unknown.tool".to_owned(),
            server: format!("http://{addr_str}"),
        };

        let result = run(&args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("tool not found"));

        server.abort();
        Ok(())
    }
}
