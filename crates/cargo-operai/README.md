# cargo-operai

Cargo custom subcommand for the Operai tool framework.

## Overview

`cargo-operai` provides a CLI for scaffolding, building, and serving Operai tools. It integrates with Cargo to provide a seamless development experience.

## Installation

```bash
cargo install cargo-operai
```

## Commands

### `cargo operai new`

Create a new Operai tool project:

```bash
# Single tool project
cargo operai new my-tool

# Multi-tool workspace
cargo operai new my-workspace --multi
```

**Options:**
| Flag | Description |
|------|-------------|
| `--multi` | Generate a multi-tool template with example tools |
| `--workspace` | Create a new Cargo workspace with this tool as first member |
| `-o, --output` | Output directory for the new project (default: current directory) |

**Generated structure (single tool):**

```
my-tool/
├── Cargo.toml
├── build.rs
├── operai.toml
└── src/
    └── lib.rs
```

### `cargo operai build`

Build an Operai project and generate embeddings:

```bash
cargo operai build
```

**Options:**
| Flag | Description |
|------|-------------|
| `-p, --path <PATH>` | Path to the project (default: current directory) |
| `--skip-embed` | Skip embedding generation |
| `-- <CARGO_ARGS>` | Additional arguments passed to `cargo build` |

**What it does:**

1. Generates embeddings from tool documentation (unless `--skip-embed`)
2. Writes `.brwse-embedding` file
3. Runs `cargo build --release`

### `cargo operai serve`

Start a gRPC server hosting Operai tools:

```bash
cargo operai serve
```

**Options:**
| Flag | Description |
|------|-------------|
| `--port <PORT>` | Port to listen on (default: 50051) |
| `--config <PATH>` | Path to operai.toml |

**Output:**

```
[INFO] Loading tools from operai.toml
[INFO] Loaded: my-tool.greet
[INFO] Server listening on [::]:50051
```

### `cargo operai mcp`

Run a Model Context Protocol (MCP) server:

```bash
cargo operai mcp
```

**Options:**
| Flag | Description |
|------|-------------|
| `-c, --config <PATH>` | Path to operai.toml |
| `-a, --addr <ADDR>` | Address to bind HTTP server (default: `127.0.0.1:3333`) |
| `--path <PATH>` | HTTP path for MCP endpoint (default: `/mcp`) |
| `--searchable` | Enable semantic search mode for tool discovery |
| `--stdio` | Run in stdio mode instead of HTTP mode |

**Modes:**

- **HTTP mode** (default): Exposes tools via streaming HTTP endpoint
- **stdio mode**: Communicates via stdin/stdout for direct MCP client integration

### `cargo operai call`

Invoke a tool on a running server:

```bash
cargo operai call my-tool.greet '{"name": "World"}'
```

**Options:**
| Flag | Description |
|------|-------------|
| `-s, --server <URL>` | Server address (default: `localhost:50051`) |
| `-C, --creds <CREDS>` | Credential overrides (format: `provider:key=value;key2=value2`) |
| `--creds-file <PATH>` | Path to credentials TOML file (default: `~/.config/operai/credentials.toml`) |

**Output:**

```json
{ "message": "Hello, World!" }
```

### `cargo operai list`

List all tools on a running server:

```bash
cargo operai list
```

**Options:**
| Flag | Description |
|------|-------------|
| `--server <URL>` | Server address (default: `http://localhost:50051`) |
| `--format <FORMAT>` | Output format: `table`, `json` (default: table) |

**Table output:**

```
┌──────────────────────┬─────────────┬─────────────────────────────┐
│ ID                   │ Version     │ Description                 │
├──────────────────────┼─────────────┼─────────────────────────────┤
│ my-tool.greet        │ 0.1.0       │ Greets a user by name       │
│ my-tool.farewell     │ 0.1.0       │ Says goodbye to a user      │
└──────────────────────┴─────────────┴─────────────────────────────┘
```

### `cargo operai describe`

Show detailed information about a tool:

```bash
cargo operai describe my-tool.greet
```

**Options:**
| Flag | Description |
|------|-------------|
| `--server <URL>` | Server address (default: `http://localhost:50051`) |

**Output:**

```
Tool: my-tool.greet
Name: Say Hello!
Version: 0.1.0
Description: Greets a user by name

Input Schema:
{
  "type": "object",
  "properties": {
    "name": { "type": "string" }
  },
  "required": ["name"]
}

Output Schema:
{
  "type": "object",
  "properties": {
    "message": { "type": "string" }
  },
  "required": ["message"]
}

Capabilities: greeting
Tags: demo
```

## Configuration

Tools are configured via `operai.toml`:

```toml
[[tools]]
path = "target/release/libmy_tool.dylib"
enabled = true

[[policies]]
name = "rate-limit"
effects = [
    { stage = "Before", tools = ["*"], guard = "ctx.calls < 100" }
]
```

## Environment Variables

| Variable        | Description                               |
| --------------- | ----------------------------------------- |
| `OPERAI_CONFIG` | Override config file path                 |
| `RUST_LOG`      | Configure logging (e.g., `info`, `debug`) |

## Logging

Enable structured logging with the `RUST_LOG` environment variable:

```bash
RUST_LOG=info cargo operai serve
RUST_LOG=debug cargo operai serve
```

## Build from Source

```bash
cd crates/cargo-operai
cargo build --release
```

## Testing

```bash
cargo test
```

## License

See [LICENSE](../../LICENSE) for details.
