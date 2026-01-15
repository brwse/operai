# operai-runtime

**Operai gRPC server**

[![Crates.io](https://img.shields.io/crates/v/operai-runtime)](https://crates.io/crates/operai-runtime)

## Overview

`operai-runtime` provides the server-side runtime for the Operai Toolbox. It exposes tools over gRPC and MCP (Model Context Protocol), handling tool discovery, invocation, and lifecycle management.

## Features

- **gRPC transport** — Full Protobuf-based API for tool operations
- **MCP support** — Model Context Protocol for AI agent integration (feature-gated)
- **Local and remote runtimes** — Run tools in-process or connect to remote servers
- **Tool discovery** — List, search, and describe available tools
- **Semantic search** — Find tools by embedding similarity

## Feature Flags

| Flag | Description |
|------|-------------|
| `mcp` | Enable Model Context Protocol support |
| `static-link` | Link tools statically instead of dynamic loading |

## Usage

### Building a Runtime

```rust
use operai_runtime::RuntimeBuilder;

let runtime = RuntimeBuilder::new()
    .manifest_path("operai.toml")
    .build()
    .await?;
```

### Local Runtime

```rust
use operai_runtime::LocalRuntime;

let runtime = LocalRuntime::from_manifest("operai.toml").await?;

// Call a tool
let result = runtime.call(
    "my-crate.my-tool",
    &input,
    CallMetadata::default(),
).await?;
```

### gRPC Service

```rust
use operai_runtime::ToolboxService;
use tonic::transport::Server;

let service = ToolboxService::new(runtime);

Server::builder()
    .add_service(service.into_server())
    .serve("[::]:50051".parse()?)
    .await?;
```

### MCP Service

```rust
use operai_runtime::McpService;

let mcp = McpService::new(runtime);
// Use with rmcp transport
```

## Key Types

| Type | Description |
|------|-------------|
| `RuntimeBuilder` | Builder for configuring and creating runtimes |
| `LocalRuntime` | In-process runtime with loaded tools |
| `RemoteRuntime` | Client for connecting to remote toolbox servers |
| `Runtime` | Trait for runtime implementations |
| `ToolboxService` | gRPC service implementation |
| `McpService` | MCP service implementation (requires `mcp` feature) |
| `CallMetadata` | Per-request metadata (credentials, trace ID, etc.) |

## Proto API

The gRPC API is defined in `proto/brwse/toolbox/v1alpha1/`:

- `ListTools` — Enumerate available tools
- `DescribeTool` — Get tool metadata and schema
- `CallTool` — Invoke a tool with input
- `SearchTools` — Semantic search over tool embeddings

## License

[PolyForm Noncommercial License 1.0.0](../../LICENSE)
