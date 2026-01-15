# cargo-operai

**Cargo subcommand for Operai SDK development**

[![Crates.io](https://img.shields.io/crates/v/cargo-operai)](https://crates.io/crates/cargo-operai)

## Overview

`cargo-operai` is a Cargo subcommand that provides development tools for building, testing, and serving Operai tools.

## Installation

```bash
cargo install cargo-operai
```

## Commands

### `cargo operai new`

Create a new tool project:

```bash
cargo operai new my-tool
cargo operai new my-tools --multi  # Create multi-tool project
```

### `cargo operai build`

Build the tool with embedding generation:

```bash
cargo operai build
cargo operai build --skip-embed              # Skip embedding generation
cargo operai build -- --features extra       # Pass args to cargo
```

### `cargo operai serve`

Start a local gRPC server:

```bash
cargo operai serve                           # Default port 50051
cargo operai serve --port 8080
cargo operai serve --manifest path/to/operai.toml
```

### `cargo operai mcp`

Start an MCP (Model Context Protocol) server:

```bash
cargo operai mcp                             # stdio transport
cargo operai mcp --transport http --port 3000
```

### `cargo operai call`

Invoke a tool for testing:

```bash
cargo operai call my-crate.my-tool '{"name": "world"}'
cargo operai call my-crate.my-tool --input-file input.json
cargo operai call my-crate.my-tool '{}' --server localhost:50051
```

### `cargo operai list`

List available tools:

```bash
cargo operai list
cargo operai list --format json
cargo operai list --server localhost:50051
```

### `cargo operai describe`

Get detailed tool information:

```bash
cargo operai describe my-crate.my-tool
cargo operai describe my-crate.my-tool --format json
```

## Configuration

### Project Config (`operai.toml`)

```toml
[[tools]]
name = "my-tool"
path = "target/release/libmy_tool.dylib"

[embedding]
provider = "fastembed"
model = "BAAI/bge-small-en-v1.5"
```

### User Config (`~/.brwse/config.toml`)

```toml
[openai]
api_key = "sk-..."
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `OPENAI_API_KEY` | OpenAI API key for embeddings |
| `RUST_LOG` | Log level (e.g., `debug`, `info`) |

## License

[PolyForm Noncommercial License 1.0.0](../../LICENSE)
