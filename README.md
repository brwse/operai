# Operai

High-performance tool runtime for Brwse. Loads Rust tool crates compiled as `cdylib` shared libraries and exposes them over gRPC or MCP.

## Features

- Native plugin tools with a stable ABI (`abi_stable`)
- Async tool execution via `async-ffi`
- JSON Schema IO via `schemars`
- Semantic search with pre-computed embeddings compiled into tools
- CEL-based policy engine (before/after effects)
- gRPC and MCP transports

## Quick Start

The easiest way to get started is the `cargo-operai` CLI.

### 1. Install

```bash
cargo install cargo-operai
```

### 2. Create a Tool

```bash
cargo operai new my-tool
cd my-tool
```

### 3. Generate Embeddings (optional but recommended)

```bash
cargo operai embed
```

This creates `.brwse-embedding`, which is compiled into the tool by the build script.

### 4. Build

```bash
cargo operai build
```

### 5. Serve Locally (gRPC)

```bash
cargo operai serve
```

The server listens on `http://0.0.0.0:50051` by default.

### 6. Interact

```bash
# List tools
cargo operai list

# Describe a tool
cargo operai describe my-tool.greet

# Call a tool
cargo operai call my-tool.greet '{"name": "World"}'
```

### 7. MCP (optional)

```bash
# HTTP (streamable) transport
cargo operai mcp

# Stdio transport (for MCP clients that use stdio)
cargo operai mcp --stdio
```

Defaults: `127.0.0.1:3333` with path `/mcp`. Use `--searchable` to expose only
the list/find/call tools and enable semantic search.

## Tool Authoring

A minimal tool definition:

```rust
use operai::{tool, Context, JsonSchema, Result};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, JsonSchema)]
struct Input {
    name: String,
}

#[derive(Serialize, JsonSchema)]
struct Output {
    message: String,
}

/// # Greet (ID: greet)
///
/// Greets a user by name.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - example
#[tool]
async fn greet(_ctx: Context, input: Input) -> Result<Output> {
    Ok(Output {
        message: format!("Hello, {}!", input.name),
    })
}

operai::generate_tool_entrypoint!();
```

The doc comment drives tool metadata:

- `# Title (ID: tool_id)` sets the display name and tool id
- `## Capabilities` and `## Tags` sections become structured metadata

To embed `.brwse-embedding` at build time, add a `build.rs`:

```rust
fn main() {
    operai_build::setup();
}
```

## Manifest (`operai.toml`)

`cargo operai serve` and `cargo operai mcp` read `operai.toml` by default.

```toml
# Project-level embedding overrides for cargo-operai
embedding_provider = "fastembed"  # fastembed | openai
embedding_model = "nomic-embed-text-v1.5"

[[tools]]
# Use a direct path...
path = "target/release/libmy_tool.dylib"

# ...or let the runtime resolve from package name.
# package = "my-tool"

# System credentials for this tool (keyed by credential name)
[tools.credentials.api]
api_key = "secret-key"
endpoint = "https://api.example.com"

[[policies]]
name = "audit-logging"
version = "1.0"
[[policies.effects]]
tool = "*"
stage = "after"
when = "true"
```

Notes:
- `package` resolves to `target/release/lib<package>.(so|dylib|dll)` relative to the manifest.
- `credentials` keys must match `define_system_credential!` names (e.g. `ApiCredential("api")`).

## Embedding Configuration

Global defaults live in `~/.config/operai/config.toml`:

```toml
[embedding]
provider = "fastembed" # or "openai"
model = "nomic-embed-text-v1.5"

[embedding.fastembed]
model = "nomic-embed-text-v1.5"
show_download_progress = true

[embedding.openai]
api_key_env = "OPENAI_API_KEY"
```

Per-project overrides are read from top-level `embedding_provider` and
`embedding_model` in `operai.toml` (as shown above). CLI flags take precedence.

## gRPC API

The Toolbox exposes a gRPC service following AIP-style patterns:

### ListTools

```protobuf
rpc ListTools(ListToolsRequest) returns (ListToolsResponse);
```

### SearchTools

```protobuf
rpc SearchTools(SearchToolsRequest) returns (SearchToolsResponse);
```

`SearchToolsRequest` expects a pre-computed embedding vector using the same
model used for tool embeddings.

### CallTool

```protobuf
rpc CallTool(CallToolRequest) returns (CallToolResponse);
```

Request headers:
- `x-request-id`: optional request identifier
- `x-session-id`: optional session identifier
- `x-user-id`: authenticated user identifier
- `x-credential-{name}`: base64-encoded JSON of `CredentialData`
  (`{"values": {"key": "value"}}`)

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Tracing log filter for servers/CLI | `info` |
| `OPENAI_API_KEY` | API key for the OpenAI embedding provider | unset |

## License

PolyForm-Noncommercial-1.0.0
