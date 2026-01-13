//! Helper functions for RPC benchmarking.
//!
//! This module provides utility functions used by the RPC benchmark in
//! `benches/rpc_bench.rs`. These functions are in a separate module so they
//! can be unit tested independently.

/// The default endpoint used for benchmarking when `BENCH_ENDPOINT` is not set.
pub const DEFAULT_BENCH_ENDPOINT: &str = "http://localhost:50052";

/// Parses an environment variable value for the benchmark endpoint.
///
/// Returns the default endpoint if the value is None or blank.
///
/// # Arguments
/// * `env_value` - The environment variable value to parse
///
/// # Returns
/// The endpoint URL to use for benchmarking
#[must_use]
pub fn bench_endpoint_from_env(env_value: Option<String>) -> String {
    env_value
        .filter(|endpoint| !endpoint.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_BENCH_ENDPOINT.to_string())
}

/// Builds a prost Struct with a "name" field for tool invocation.
///
/// # Arguments
/// * `name` - The name value to set in the struct
///
/// # Returns
/// A prost Struct with a single "name" field
#[must_use]
pub fn build_name_input(name: &str) -> prost_types::Struct {
    prost_types::Struct {
        fields: [(
            "name".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::StringValue(name.to_string())),
            },
        )]
        .into_iter()
        .collect(),
    }
}

/// Selects a tool name from a list of tools, preferring tools with "greet" in
/// the name.
///
/// This is used by benchmarks to select an appropriate tool for testing.
/// It prioritizes tools with "greet" in their name, but falls back to the
/// first available tool if no such tool exists.
///
/// # Arguments
/// * `tools` - A slice of available tools
///
/// # Returns
/// The name of the selected tool, or None if the list is empty
#[must_use]
pub fn select_tool_name(tools: &[crate::proto::Tool]) -> Option<String> {
    tools
        .iter()
        .find(|tool| tool.name.contains("greet"))
        .or_else(|| tools.first())
        .map(|tool| tool.name.clone())
}

#[cfg(test)]
mod tests {
    use tonic::transport::Channel;

    use super::*;
    use crate::proto::Tool;

    #[test]
    fn test_bench_endpoint_from_env_none_returns_default() {
        // Arrange
        let env_value = None;

        // Act
        let endpoint = bench_endpoint_from_env(env_value);

        // Assert
        assert_eq!(endpoint, DEFAULT_BENCH_ENDPOINT);
    }

    #[test]
    fn test_bench_endpoint_from_env_empty_string_returns_default() {
        // Arrange
        let env_value = Some(String::new());

        // Act
        let endpoint = bench_endpoint_from_env(env_value);

        // Assert
        assert_eq!(endpoint, DEFAULT_BENCH_ENDPOINT);
    }

    #[test]
    fn test_bench_endpoint_from_env_whitespace_only_returns_default() {
        // Arrange
        let env_value = Some("   ".to_string());

        // Act
        let endpoint = bench_endpoint_from_env(env_value);

        // Assert
        assert_eq!(endpoint, DEFAULT_BENCH_ENDPOINT);
    }

    #[test]
    fn test_bench_endpoint_from_env_custom_value_returns_that_value() {
        // Arrange
        let env_value = Some("http://example.com:1234".to_string());

        // Act
        let endpoint = bench_endpoint_from_env(env_value);

        // Assert
        assert_eq!(endpoint, "http://example.com:1234");
    }

    #[test]
    fn test_default_bench_endpoint_constant_is_valid_tonic_uri() {
        // Assert: the default endpoint constant can be parsed as a valid tonic URI
        assert!(
            Channel::from_shared(DEFAULT_BENCH_ENDPOINT).is_ok(),
            "DEFAULT_BENCH_ENDPOINT should be a valid URI"
        );
    }

    #[test]
    fn test_select_tool_name_with_greet_prefers_first_match() {
        // Arrange
        let tools = vec![
            Tool {
                name: "tools/hello-world.other".to_string(),
                ..Tool::default()
            },
            Tool {
                name: "tools/hello-world.greet".to_string(),
                ..Tool::default()
            },
            Tool {
                name: "tools/hello-world.greet-again".to_string(),
                ..Tool::default()
            },
        ];

        // Act
        let selected = select_tool_name(&tools);

        // Assert
        assert_eq!(selected.as_deref(), Some("tools/hello-world.greet"));
    }

    #[test]
    fn test_select_tool_name_without_greet_returns_first_tool() {
        // Arrange
        let tools = vec![
            Tool {
                name: "tools/hello-world.other".to_string(),
                ..Tool::default()
            },
            Tool {
                name: "tools/hello-world.second".to_string(),
                ..Tool::default()
            },
        ];

        // Act
        let selected = select_tool_name(&tools);

        // Assert
        assert_eq!(selected.as_deref(), Some("tools/hello-world.other"));
    }

    #[test]
    fn test_select_tool_name_with_empty_tools_returns_none() {
        // Arrange
        let tools: Vec<Tool> = Vec::new();

        // Act
        let selected = select_tool_name(&tools);

        // Assert
        assert!(selected.is_none());
    }

    #[test]
    fn test_build_name_input_sets_name_field() {
        // Arrange
        let expected_name = "Benchmark";

        // Act
        let input = build_name_input(expected_name);
        let name_value = input.fields.get("name").expect("missing `name` field");

        // Assert
        assert_eq!(input.fields.len(), 1);
        match &name_value.kind {
            Some(prost_types::value::Kind::StringValue(value)) => {
                assert_eq!(value, expected_name);
            }
            Some(other) => panic!("expected string value for `name`, got: {other:?}"),
            None => panic!("expected `name` field kind to be set"),
        }
    }

    #[test]
    fn test_build_name_input_with_empty_string() {
        // Arrange & Act
        let input = build_name_input("");
        let name_value = input.fields.get("name").expect("missing `name` field");

        // Assert
        match &name_value.kind {
            Some(prost_types::value::Kind::StringValue(value)) => {
                assert!(value.is_empty());
            }
            _ => panic!("expected empty string value for `name`"),
        }
    }
}
