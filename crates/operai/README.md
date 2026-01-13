# operai

Operai Tool SDK for building native tools.

This crate provides the SDK for building tools that can be loaded by the Operai Toolbox runtime. Tools are compiled as cdylib shared libraries and loaded dynamically at startup.

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
#[tool]
async fn greet(ctx: Context, input: Input) -> Result<Output> {
    Ok(Output {
        message: format!("Hello, {}!", input.name),
    })
}

// Required: generates the FFI entrypoint
operai::generate_tool_entrypoint!();
```

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

## Credentials

Tools can define credentials that are either system-level (shared) or user-level (per-request):

```rust
use operai::{define_system_credential, define_user_credential};

// System credentials come from environment variables
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
