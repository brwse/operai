# operai-core

**Core library for Operai runtime**

[![Crates.io](https://img.shields.io/crates/v/operai-core)](https://crates.io/crates/operai-core)

## Overview

`operai-core` provides the foundational components for the Operai Toolbox runtime, including dynamic library loading, tool registry management, manifest parsing, and policy evaluation.

This is an internal crate used by `operai-runtime`. Most users should interact with the runtime directly.

## Features

- **Dynamic loading** — Load tool libraries (`cdylib`) at runtime with ABI version checking
- **Tool registry** — Lookup tools by qualified name with concurrent access support
- **Manifest parsing** — Read and validate `operai.toml` configuration files
- **Policy engine** — CEL-based access control for tool invocations
- **Session management** — Track user sessions and permissions

## Key Components

### Loader

```rust
use operai_core::{ToolLibrary, LoadError};

// Load a tool library from a dynamic library path
let lib = ToolLibrary::load("/path/to/tool.dylib")?;
```

### Registry

```rust
use operai_core::{ToolRegistry, ToolHandle, ToolInfo};

let registry = ToolRegistry::new();
registry.register(tool_handle)?;

// Lookup by qualified name
let tool = registry.get("my-crate.my-tool")?;
```

### Manifest

```rust
use operai_core::{Manifest, ToolConfig};

let manifest = Manifest::load("operai.toml")?;
for tool in manifest.tools() {
    println!("{}: {}", tool.name, tool.path);
}
```

### Policy

```rust
use operai_core::{Policy, Effect};

let policy = Policy::new(r#"
  capabilities.contains("read") && user.role == "admin"
"#)?;

let effect = policy.evaluate(&context)?;
match effect {
    Effect::Allow => { /* proceed */ }
    Effect::Deny => { /* reject */ }
}
```

## Exports

| Type | Description |
|------|-------------|
| `ToolLibrary` | Handle to a loaded dynamic library |
| `ToolRegistry` | Thread-safe registry of available tools |
| `ToolHandle` | Reference to a registered tool |
| `ToolInfo` | Metadata about a tool |
| `Manifest` | Parsed `operai.toml` configuration |
| `ToolConfig` | Configuration for a single tool |
| `Policy` | CEL expression for access control |
| `Effect` | Result of policy evaluation (Allow/Deny) |
| `LoadError` | Errors from library loading |
| `RegistryError` | Errors from registry operations |
| `ManifestError` | Errors from manifest parsing |
| `PolicyError` | Errors from policy evaluation |

## License

[PolyForm Noncommercial License 1.0.0](../../LICENSE)
