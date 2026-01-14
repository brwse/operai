# operai-macro

Procedural macros for the Operai Tool SDK.

This crate provides:
- `#[tool]` attribute macro for defining tools
- `#[init]` attribute macro for initialization functions
- `#[shutdown]` attribute macro for shutdown functions
- `define_system_credential!` macro for system-level credentials
- `define_user_credential!` macro for user-provided credentials

## Tool Metadata From Doc Comments

`#[tool]` parses structured doc comments to populate metadata:

```rust
/// # Say Hello (ID: greet)
///
/// Greets a user by name.
///
/// ## Capabilities
/// - read
///
/// ## Tags
/// - example
#[tool]
async fn greet(ctx: Context, input: Input) -> Result<Output> { /* ... */ }
```

Sections:
- `# Title (ID: tool_id)` sets name and id
- `## Capabilities` lists required capabilities
- `## Tags` adds discovery tags

## Credential Macros

Credentials are defined as named structs and registered for the runtime.
System credentials are loaded from the manifest, while user credentials are
provided per request.
