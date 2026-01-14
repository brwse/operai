# operai-runtime

Operai runtime library.

This crate provides:
- Local runtime execution backed by a tool registry
- Remote runtime execution via gRPC
- gRPC service implementation (`ToolboxService`)
- MCP transport (feature `mcp`)

## Runtime Builder

```rust
use operai_runtime::RuntimeBuilder;

let runtime = RuntimeBuilder::new()
    .with_manifest_path("operai.toml")
    .build_local()
    .await?;
```

## gRPC Service

```rust
use operai_runtime::{RuntimeBuilder, transports::grpc::ToolboxService};

let runtime = RuntimeBuilder::new().build_local().await?;
let service = ToolboxService::from_runtime(runtime);
```

## MCP Service (feature = "mcp")

```rust
use operai_runtime::McpService;

let runtime = RuntimeBuilder::new().build_local().await?;
let service = McpService::from_runtime(runtime).searchable(true);
```

## Call Metadata

When calling tools programmatically, you can supply request/session/user IDs
and credentials via `CallMetadata`.
