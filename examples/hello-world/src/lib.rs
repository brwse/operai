use operai::{Context, JsonSchema, Result, init, schemars, shutdown, tool};
use serde::{Deserialize, Serialize};

#[init]
async fn init() -> Result<()> {
    Ok(())
}

#[derive(Deserialize, JsonSchema)]
struct GreetInput {
    name: String,
    greeting: Option<String>,
}

#[derive(Serialize, JsonSchema)]
struct GreetOutput {
    message: String,
    request_id: String,
}

/// # Greet (ID: greet)
///
/// Greets the user by name.
///
/// ## Capabilities
/// - read
#[tool]
async fn greet(ctx: Context, input: GreetInput) -> Result<GreetOutput> {
    let greeting = input.greeting.as_deref().unwrap_or("Hello");
    Ok(GreetOutput {
        message: format!("{}, {}!", greeting, input.name),
        request_id: ctx.request_id().to_string(),
    })
}

#[derive(Deserialize, JsonSchema)]
struct EchoInput {
    message: String,
}

#[derive(Serialize, JsonSchema)]
struct EchoOutput {
    echo: String,
    length: usize,
}

/// # Echo (ID: echo)
///
/// Echoes the input back.
///
/// ## Capabilities
/// - read
#[tool]
async fn echo(_ctx: Context, input: EchoInput) -> Result<EchoOutput> {
    Ok(EchoOutput {
        length: input.message.len(),
        echo: input.message,
    })
}

#[shutdown]
fn shutdown() {}

operai::generate_tool_entrypoint!();
