//! Fetches and displays detailed information about a specific tool from a Brwse
//! Toolbox server.

use anyhow::{Context, Result};
use clap::Args;
use console::style;
use operai_runtime::proto::toolbox_client::ToolboxClient;

#[derive(Args)]
pub struct DescribeArgs {
    /// Tool ID to describe (e.g., "hello-world.greet").
    pub tool_id: String,

    /// Server address.
    #[arg(short, long, default_value = "http://localhost:50051")]
    pub server: String,
}

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

fn struct_to_json(s: prost_types::Struct) -> serde_json::Value {
    let map = s
        .fields
        .into_iter()
        .map(|(k, v)| (k, prost_value_to_json(v)))
        .collect();
    serde_json::Value::Object(map)
}

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
    use operai_runtime::proto::{
        CallToolRequest, CallToolResponse, ListToolsRequest, ListToolsResponse, SearchToolsRequest,
        SearchToolsResponse, Tool,
        toolbox_server::{Toolbox, ToolboxServer},
    };
    use tonic::{Request, Response, Status};

    use super::*;

    struct MockToolbox;

    #[tonic::async_trait]
    impl Toolbox for MockToolbox {
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

        async fn search_tools(
            &self,
            _request: Request<SearchToolsRequest>,
        ) -> Result<Response<SearchToolsResponse>, Status> {
            Err(Status::unimplemented("not implemented"))
        }

        async fn call_tool(
            &self,
            _request: Request<CallToolRequest>,
        ) -> Result<Response<CallToolResponse>, Status> {
            Err(Status::unimplemented("not implemented"))
        }
    }

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
