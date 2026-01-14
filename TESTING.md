# Testing Guidelines

This document describes testing expectations for the Operai Rust workspace.

## Philosophy

Tests exist to prevent regressions and document behavior. Prefer deterministic
unit and integration tests over broad or flaky coverage.

## What to Test

### Do Test

1. **Public API contracts**
   - Tool metadata extraction from doc comments
   - Credential parsing and schema serialization
   - Manifest parsing and validation
   - Policy evaluation behavior (CEL expressions, guard failures)

2. **Runtime behavior**
   - Tool registry loading/unloading and duplicate detection
   - gRPC list/search/call behavior (local runtime)
   - Policy store registration and session handling

3. **CLI behavior**
   - `cargo operai` argument parsing and defaults
   - `embed`/`build` flag precedence (CLI > project > global)
   - Credential parsing (`--creds`, `--creds-file`, `env:` values)

4. **Edge cases and error paths**
   - Missing/invalid `operai.toml`
   - Missing tool libraries or checksum mismatch
   - Invalid JSON input for `call`
   - Empty embeddings or invalid embedding files

### Do Not Test

- Third-party crates (tokio, serde, tonic, fastembed, async-openai)
- Network calls to external services (use mocks instead)
- Trivial getters/setters or auto-derived traits

## Recommended Patterns

### Unit Tests

Place unit tests in a `#[cfg(test)]` module at the bottom of the source file.
Use focused tests with clear names.

### Async Tests

Use `tokio::test`. When tests mutate global state (env vars, cwd), use
`flavor = "current_thread"` and a shared lock to avoid races.

```rust
#[tokio::test(flavor = "current_thread")]
async fn test_embed_reads_project_config() {
    let _guard = crate::testing::test_lock_async().await;
    // ...
}
```

### Temporary Directories

Prefer unique temp dirs and clean up in `Drop`. Several crates already include
lightweight `TempDir` helpers; follow those patterns instead of adding new deps.

### Network and Ports

When spinning up local servers in tests, bind to `127.0.0.1:0` and capture the
assigned port.

### CLI Parsing

Use `clap::Parser::try_parse_from` and assert on parsed struct fields. Avoid
spawning subprocesses unless you are explicitly testing output integration.

### Embedding Tests

Never hit real embedding APIs. Use local mock servers (see the OpenAI mock in
`crates/cargo-operai/src/embedding.rs` tests) or deterministic fixtures.

## Integration Tests

Use integration tests for cross-module behavior (runtime + registry + policy).
Place these in `crates/<crate>/tests/` if you need black-box coverage.

## Running Tests

```bash
# All tests
cargo test

# Per crate
cargo test -p cargo-operai
cargo test -p operai-runtime

# With output
cargo test -- --nocapture
```

## Checklist for New Code

- [ ] Happy path behavior covered
- [ ] Error paths covered (invalid input, missing config, etc.)
- [ ] Async behavior tested with `tokio::test` when needed
- [ ] No external network dependency
- [ ] Clear test names that describe the scenario
