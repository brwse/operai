# cargo-operai

Cargo subcommand for Operai Tool SDK development.

## Usage

```bash
cargo operai new my-tool        # Create new tool project
cargo operai embed              # Generate embeddings
cargo operai build              # Build with embeddings
cargo operai serve              # Run local gRPC server
cargo operai mcp                # Run MCP server (HTTP)
cargo operai mcp --stdio        # MCP over stdio
cargo operai call <tool> <json> # Test a tool
cargo operai list               # List available tools
cargo operai describe <tool>    # Show tool details
```

`call` accepts inline JSON or `@path/to/input.json`.

Common flags:
- `cargo operai mcp --searchable` to enable semantic search endpoints
- `cargo operai list --format json` for machine-readable output

## Embedding Configuration

Global defaults: `~/.config/operai/config.toml`

```toml
[embedding]
provider = "fastembed" # or "openai"
model = "nomic-embed-text-v1.5"

[embedding.openai]
api_key_env = "OPENAI_API_KEY"
```

Project overrides (read from `operai.toml`):

```toml
embedding_provider = "fastembed"
embedding_model = "nomic-embed-text-v1.5"
```

CLI flags override both:

```bash
cargo operai embed -P openai --model text-embedding-3-small
```

## Credentials for `cargo operai call`

- Default credentials file: `~/.config/operai/credentials.toml`
- Format: `provider:key=value;key2=value2`
- Use `env:` to read values from environment variables

Example credentials file:

```toml
[github]
token = "env:GITHUB_TOKEN"

[api]
api_key = "secret"
```

Example CLI usage:

```bash
cargo operai call my-tool.greet '{"name":"World"}' \
  --creds api:api_key=env:API_KEY
```
