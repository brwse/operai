# operai

Operai Tool SDK for building native tools.

This crate provides the SDK for building tools that can be loaded by the Operai
runtime. Tools are compiled as `cdylib` shared libraries and loaded dynamically
at startup.

## Quick Start

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
#[tool]
async fn greet(_ctx: Context, input: Input) -> Result<Output> {
    Ok(Output {
        message: format!("Hello, {}!", input.name),
    })
}

// Required: generates the FFI entrypoint
operai::generate_tool_entrypoint!();
```

## Tool Metadata

The `#[tool]` macro extracts metadata from doc comments:

- `# Title (ID: tool_id)` sets the display name and tool id
- `## Capabilities` and `## Tags` sections populate structured metadata

## Lifecycle Hooks

Tools can define initialization and shutdown functions for setup and cleanup:

```rust
use operai::{init, shutdown, Result};

#[init]
async fn setup() -> Result<()> {
    // Initialize connections, load config, etc.
    Ok(())
}

#[shutdown]
fn cleanup() {
    // Close connections, flush buffers, etc.
}
```

## Context

`Context` provides request metadata (`request_id`, `session_id`, `user_id`) and
access to credentials.

## Credentials

Tools can define credentials that are either system-level (shared) or user-level
(per-request):

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
```

Credential structs include a `get(&Context)` helper:

```rust
let api = ApiCredential::get(&ctx)?;
let user = UserToken::get(&ctx)?;
```

## Build Script

To compile `.brwse-embedding` into the tool, add a `build.rs`:

```rust
fn main() {
    operai_build::setup();
}
```
