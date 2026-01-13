//! `cargo operai list` command implementation.

use anyhow::{Context, Result};
use clap::Args;
use console::style;
use operai_runtime::proto::toolbox_client::ToolboxClient;

fn normalize_tool_name(name: &str) -> &str {
    name.strip_prefix("tools/").unwrap_or(name)
}

fn truncate_description(description: &str) -> String {
    const MAX_DESCRIPTION_CHARS: usize = 40;
    const ELLIPSIS: &str = "...";
    const TRUNCATED_CHARS: usize = MAX_DESCRIPTION_CHARS - ELLIPSIS.len();

    let mut chars = description.chars();
    let first_40: String = chars.by_ref().take(MAX_DESCRIPTION_CHARS).collect();

    if chars.next().is_none() {
        return first_40;
    }

    let prefix: String = first_40.chars().take(TRUNCATED_CHARS).collect();
    format!("{prefix}{ELLIPSIS}")
}

/// Arguments for the `list` command.
#[derive(Args)]
pub struct ListArgs {
    /// Server address.
    #[arg(short, long, default_value = "http://127.0.0.1:50051")]
    pub server: String,

    /// Output format (table or json).
    #[arg(short, long, default_value = "table")]
    pub format: String,
}

pub async fn run(args: &ListArgs) -> Result<()> {
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

    if args.format == "json" {
        let tools_json: Vec<serde_json::Value> = response
            .tools
            .iter()
            .map(|t| tool_to_json(t.clone()))
            .collect();
        println!("{}", serde_json::to_string_pretty(&tools_json)?);
    } else if !response.tools.is_empty() {
        // ... (table output remains valid as it accesses fields directly)
        println!(
            "{:<40} {:<20} {}",
            style("TOOL ID").bold(),
            style("NAME").bold(),
            style("DESCRIPTION").bold()
        );
        println!("{}", "-".repeat(80));

        for tool in &response.tools {
            let name = normalize_tool_name(&tool.name);
            let display_name = &tool.display_name;
            let description = &tool.description;

            let desc_truncated = truncate_description(description);

            println!("{name:<40} {display_name:<20} {desc_truncated}");
        }

        println!(
            "\n{} {} tool(s) available",
            style("✓").green(),
            response.tools.len()
        );
    } else {
        println!("No tools found");
    }

    Ok(())
}

fn tool_to_json(tool: operai_runtime::proto::Tool) -> serde_json::Value {
    serde_json::json!({
        "name": tool.name,
        "displayName": tool.display_name,
        "version": tool.version,
        "description": tool.description,
        "inputSchema": tool.input_schema.map(struct_to_json),
        "outputSchema": tool.output_schema.map(struct_to_json),
        "capabilities": tool.capabilities,
        "tags": tool.tags,
    })
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
                        name: "tools/hello.greet".to_string(),
                        display_name: "Greet".to_string(),
                        description: "Says hello".to_string(),
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

    #[test]
    fn test_normalize_tool_name() {
        assert_eq!(normalize_tool_name("tools/foo"), "foo");
        assert_eq!(normalize_tool_name("foo"), "foo");
    }

    #[test]
    fn test_truncate_description() {
        assert_eq!(truncate_description("short"), "short");
        assert_eq!(
            truncate_description("this is a very long description that should be truncated"),
            "this is a very long description that ..."
        );
    }

    #[tokio::test]
    async fn test_run_list() -> Result<()> {
        let _lock = crate::testing::test_lock_async().await;

        // Find a free port
        let port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
            listener.local_addr()?.port()
        };
        let addr_str = format!("127.0.0.1:{port}");
        let addr: std::net::SocketAddr = addr_str.parse()?;

        // Start server
        let server = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(ToolboxServer::new(MockToolbox))
                .serve(addr)
                .await
                .unwrap();
        });

        // Give server a moment to start
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        let args = ListArgs {
            server: format!("http://{addr_str}"),
            format: "table".to_owned(),
        };

        run(&args).await.context("run failed")?;

        server.abort();
        Ok(())
    }
}
