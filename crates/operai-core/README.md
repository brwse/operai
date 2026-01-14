# operai-core

Core library for the Operai runtime.

This crate provides:
- Dynamic loading of tool libraries (`cdylib`) via `abi_stable`
- Tool registry with lookup by qualified name
- Manifest parsing for tool configuration
- Semantic search via embeddings
- CEL-based policy evaluation

## Manifest

The manifest is a TOML file (typically `operai.toml`) describing tools and
policies.

```toml
[[tools]]
# Use a direct path...
path = "target/release/libhello_world.dylib"

# ...or resolve from package name.
# package = "hello-world"

enabled = true
checksum = "<sha256>"

[tools.credentials.api]
api_key = "secret"

[[policies]]
name = "audit-logging"
version = "1.0"
[[policies.effects]]
tool = "*"
stage = "after"
when = "true"
```

## Tool Registry

The registry stores loaded tools, their schemas, credentials, and embeddings.
Tools are identified by qualified id (`crate-name.tool-id`).

## Policies

Policies are evaluated before and after tool execution using CEL expressions.
They can guard requests (`fail_message`) or update session context.
