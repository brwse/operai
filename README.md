# Operai

High-performance tool runtime for Brwse. Loads native tools compiled as Rust cdylib shared libraries and exposes them via gRPC.

## Features

- **Native Performance**: Tools are compiled to native code as shared libraries
- **Stable ABI**: `#[repr(C)]` types ensure binary compatibility across Rust versions
- **Async Support**: Tools can use async/await with the runtime-provided executor
- **Semantic Search**: Pre-computed embeddings enable natural language tool discovery
- **Observability**: Built-in tracing and metrics integration

## Getting Started

The easiest way to get started with `operai` is using the `cargo-operai` CLI.

### 1. Installation

Install the `cargo-operai` subcommand:

```bash
cargo install cargo-operai
```

### 2. Creating a Project

Create a new tool project:

```bash
cargo operai new my-tool
cd my-tool
```

This creates a new library crate configured with the necessary dependencies and boilerplate.

### 3. Building

Build your tool. This command automatically handles generating semantic embeddings for your tool description:

```bash
cargo operai build
```

### 4. Running Locally

Start a local development server:

```bash
cargo operai serve
```

This starts a gRPC server on port 50051 (default) hosting your tool.

### 5. Interacting

In another terminal, you can list and call your tools:

```bash
# List available tools
cargo operai list

# Call a tool
cargo operai call my-tool.greet '{"name": "World"}'
```

## Credentials

Tools can define credentials that are either system-level (from environment variables) or user-level (per-request):

```rust
use operai::{define_system_credential, define_user_credential};

// System credentials come from manifest configuration
define_system_credential! {
    ApiCredential("api") {
        api_key: String,
        #[optional]
        endpoint: Option<String>,
    }
}

// User credentials are passed per-request via gRPC
define_user_credential! {
    UserToken("user_token") {
        token: String,
    }
}

/// # My tool
///
/// A tool that uses credentials
/// 
/// ## Capabilities
/// - read
/// 
/// ## Tags
/// - credential
/// - integration
async fn my_tool(ctx: Context, input: Input) -> Result<Output, Error> {
    let api_cred = ApiCredential::get(&ctx)?;
    let user_token = UserToken::get(&ctx)?;
    // ...
}
```

**Configuration (`operai.toml`):**

```toml
[[tools]]
package = "my-tool"

[tools.credentials.my-tool]
api_key = "secret-key"
endpoint = "https://api.example.com"
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      gRPC Clients                           │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                       Operai Runtime                        │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              gRPC Service (tonic)                    │   │
│  │         ListTools | SearchTools | CallTool           │   │
│  └──────────────────────────────────────────────────────┘   │
│                              │                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                    Registry                          │   │
│  │        Tool lookup, semantic search, metrics         │   │
│  └──────────────────────────────────────────────────────┘   │
│                              │                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                    Loader                            │   │
│  │          dlopen/dlsym via libloading                 │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │
            ┌─────────────────┼─────────────────┐
            ▼                 ▼                 ▼
    ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
    │ Tool A.dylib │  │ Tool B.dylib │  │ Tool C.dylib │
    │   (cdylib)   │  │   (cdylib)   │  │   (cdylib)   │
    └──────────────┘  └──────────────┘  └──────────────┘
```

## Crate Structure

- **operai-abi**: Stable C ABI types (`#[repr(C)]`)
- **operai-macro**: Procedural macros (`#[tool]`, credentials)
- **operai**: SDK for tool authors
- **operai-core**: Loader, registry, manifest parsing
- **operai-build**: Build script helpers for embedding generation
- **cargo-operai**: CLI tool for creating, building, and serving tools

## gRPC API

The Toolbox exposes a gRPC service following [AIP.dev](https://aip.dev) patterns:

### ListTools (AIP-132)

```protobuf
rpc ListTools(ListToolsRequest) returns (ListToolsResponse);
```

### SearchTools (AIP-136)

```protobuf
rpc SearchTools(SearchToolsRequest) returns (SearchToolsResponse);
```

### CallTool (AIP-136)

```protobuf
rpc CallTool(CallToolRequest) returns (CallToolResponse);
```

Request headers:
- `x-request-id`: Unique request identifier for tracing
- `x-session-id`: Optional session identifier for stateful tools
- `x-user-id`: Authenticated user identifier
- `x-credential-{name}`: Base64-encoded JSON of CredentialData for each credential required by the tool

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `LOG_LEVEL` | Logging level (trace/debug/info/warn/error) | `info` |

## License

PolyForm-Noncommercial-1.0.0
