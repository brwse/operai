//! `cargo operai call` command implementation.

use std::{collections::HashMap, path::PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use console::style;
use operai_runtime::{CallMetadata, RuntimeBuilder};

/// Arguments for the `call` command.
#[derive(Args)]
pub struct CallArgs {
    /// Tool ID to call (e.g., "hello-world.greet").
    pub tool_id: String,

    /// Input JSON (inline or @file.json).
    pub input: String,

    #[arg(short, long, default_value = "localhost:50051")]
    pub server: String,

    /// Credentials in the format `provider:key=value;key2=value2`.
    /// Can be specified multiple times.
    /// Values starting with `env:` will be read from environment variables.
    /// Use `\;` and `\=` to escape data.
    #[arg(short = 'C', long = "creds")]
    pub credentials: Vec<String>,

    /// Path to a TOML file containing credentials.
    /// Defaults to `~/.config/operai/credentials.toml`.
    #[arg(long = "creds-file")]
    pub credentials_file: Option<PathBuf>,
}

pub async fn run(args: &CallArgs) -> Result<()> {
    let input_json = if args.input.starts_with('@') {
        let path = PathBuf::from(&args.input[1..]);
        std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read input file: {}", path.display()))?
    } else {
        args.input.clone()
    };

    let input_value: serde_json::Value =
        serde_json::from_str(&input_json).context("invalid input JSON")?;

    println!(
        "{} Calling tool: {}",
        style("→").cyan(),
        style(&args.tool_id).bold()
    );

    let credentials = load_credentials(args)?;

    let runtime = RuntimeBuilder::new()
        .remote(args.server.clone())
        .build_remote()
        .await
        .context("failed to connect to toolbox server")?;

    // Convert input JSON to google.protobuf.Struct
    let input_struct = json_to_struct(input_value).context("failed to convert input to Struct")?;

    let request = operai_runtime::proto::CallToolRequest {
        name: format!("tools/{}", args.tool_id),
        input: Some(input_struct),
    };

    let metadata = CallMetadata {
        credentials,
        ..Default::default()
    };

    let response_inner = runtime
        .call_tool(request, metadata)
        .await
        .context("failed to call tool")?;

    match response_inner.result {
        Some(operai_runtime::proto::call_tool_response::Result::Output(output)) => {
            println!("{} Result:", style("✓").green().bold());
            // Convert struct back to JSON for printing
            let output_json = serde_json::to_string_pretty(&struct_to_json(output))?;
            println!("{output_json}");
        }
        Some(operai_runtime::proto::call_tool_response::Result::Error(error)) => {
            println!("{} Error: {}", style("✗").red().bold(), error);
        }
        None => {
            println!("{} No result returned", style("?").yellow().bold());
        }
    }

    Ok(())
}

fn json_to_struct(value: serde_json::Value) -> Result<prost_types::Struct> {
    match value {
        serde_json::Value::Object(map) => {
            let mut fields = std::collections::BTreeMap::new();
            for (k, v) in map {
                fields.insert(k, json_to_value(v)?);
            }
            Ok(prost_types::Struct { fields })
        }
        _ => anyhow::bail!("input must be a JSON object"),
    }
}

fn json_to_value(value: serde_json::Value) -> Result<prost_types::Value> {
    let kind = match value {
        serde_json::Value::Null => prost_types::value::Kind::NullValue(0),
        serde_json::Value::Bool(b) => prost_types::value::Kind::BoolValue(b),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                prost_types::value::Kind::NumberValue(f)
            } else {
                anyhow::bail!("invalid number value: {n}")
            }
        }
        serde_json::Value::String(s) => prost_types::value::Kind::StringValue(s),
        serde_json::Value::Array(a) => {
            let mut values = Vec::new();
            for v in a {
                values.push(json_to_value(v)?);
            }
            prost_types::value::Kind::ListValue(prost_types::ListValue { values })
        }
        serde_json::Value::Object(o) => {
            let mut fields = std::collections::BTreeMap::new();
            for (k, v) in o {
                fields.insert(k, json_to_value(v)?);
            }
            prost_types::value::Kind::StructValue(prost_types::Struct { fields })
        }
    };
    Ok(prost_types::Value { kind: Some(kind) })
}

fn struct_to_json(s: prost_types::Struct) -> serde_json::Value {
    let map = s
        .fields
        .into_iter()
        .map(|(k, v)| (k, prost_value_to_json(v)))
        .collect();
    serde_json::Value::Object(map)
}

fn prost_value_to_json(v: prost_types::Value) -> serde_json::Value {
    match v.kind {
        Some(prost_types::value::Kind::NullValue(_)) | None => serde_json::Value::Null,
        Some(prost_types::value::Kind::NumberValue(n)) => serde_json::Number::from_f64(n)
            .map_or(serde_json::Value::Null, serde_json::Value::Number),
        Some(prost_types::value::Kind::StringValue(s)) => serde_json::Value::String(s),
        Some(prost_types::value::Kind::BoolValue(b)) => serde_json::Value::Bool(b),
        Some(prost_types::value::Kind::StructValue(s)) => struct_to_json(s),
        Some(prost_types::value::Kind::ListValue(l)) => {
            let values = l.values.into_iter().map(prost_value_to_json).collect();
            serde_json::Value::Array(values)
        }
    }
}

fn load_credentials(args: &CallArgs) -> Result<HashMap<String, HashMap<String, String>>> {
    let mut credentials: HashMap<String, HashMap<String, String>> = HashMap::new();

    // 1. Load from file (default or specified)
    let file_path = if let Some(path) = &args.credentials_file {
        Some(path.clone())
    } else {
        dirs::home_dir().map(|h| h.join(".config/operai/credentials.toml"))
    };

    if let Some(path) = file_path {
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read credentials file: {}", path.display()))?;
            let file_creds: HashMap<String, HashMap<String, String>> =
                toml::from_str(&content).context("failed to parse credentials TOML")?;

            for (provider, values) in file_creds {
                let mut processed_values = HashMap::new();
                for (k, v) in values {
                    processed_values.insert(k, process_value(&v)?);
                }
                credentials.insert(provider, processed_values);
            }
        } else if args.credentials_file.is_some() {
            anyhow::bail!("credentials file not found: {}", path.display());
        }
    }

    // 2. Load from CLI args (overrides file)
    for cred_str in &args.credentials {
        let (provider, values) = parse_credential_string(cred_str)?;
        let mut processed_values = HashMap::new();
        for (k, v) in values {
            processed_values.insert(k, process_value(&v)?);
        }

        credentials
            .entry(provider)
            .and_modify(|d| d.extend(processed_values.clone()))
            .or_insert(processed_values);
    }

    Ok(credentials)
}

fn process_value(value: &str) -> Result<String> {
    if let Some(var_name) = value.strip_prefix("env:") {
        std::env::var(var_name)
            .with_context(|| format!("environment variable not found: {var_name}"))
    } else {
        Ok(value.to_string())
    }
}

fn parse_credential_string(s: &str) -> Result<(String, HashMap<String, String>)> {
    let (provider, rest) = s
        .split_once(':')
        .context("invalid credential format: missing provider (expected provider:key=value)")?;

    let mut values = HashMap::new();
    let mut current_key = String::new();
    let mut current_value = String::new();
    let mut parsing_key = true;
    let mut escape = false;

    for c in rest.chars() {
        if escape {
            if parsing_key {
                current_key.push(c);
            } else {
                current_value.push(c);
            }
            escape = false;
            continue;
        }

        match c {
            '\\' => escape = true,
            '=' if parsing_key => parsing_key = false,
            ';' if !parsing_key => {
                if !current_key.is_empty() {
                    values.insert(current_key.trim().to_string(), current_value);
                }
                current_key = String::new();
                current_value = String::new();
                parsing_key = true;
            }
            _ => {
                if parsing_key {
                    current_key.push(c);
                } else {
                    current_value.push(c);
                }
            }
        }
    }

    if !parsing_key && !current_key.is_empty() {
        values.insert(current_key.trim().to_string(), current_value);
    }

    Ok((provider.to_string(), values))
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_parse_credential_string() {
        let (provider, values) = parse_credential_string("github:token=123").unwrap();
        assert_eq!(provider, "github");
        assert_eq!(values.get("token").unwrap(), "123");

        let (provider, values) =
            parse_credential_string("aws:access_key=123;secret_key=456").unwrap();
        assert_eq!(provider, "aws");
        assert_eq!(values.get("access_key").unwrap(), "123");
        assert_eq!(values.get("secret_key").unwrap(), "456");

        let (provider, values) =
            parse_credential_string(r"complex:key=val\;ue;key2=val\=ue").unwrap();
        assert_eq!(provider, "complex");
        assert_eq!(values.get("key").unwrap(), "val;ue");
        assert_eq!(values.get("key2").unwrap(), "val=ue");
    }
}
