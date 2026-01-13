# Rust Testing Guidelines

This document establishes testing standards for the Rust codebase, specifically for Kubernetes controllers and backend services.

## Philosophy

Tests exist to **prevent regressions and document behavior**, not to hit coverage metrics. Write tests that catch real bugs and serve as living documentation.

## What to Test

### Do Test

1. **Public API contracts**
   - CRD serialization/deserialization (JSON round-trips)
   - Error type display messages and metric labels
   - Condition management (setting, retrieving, transitions)
   - Resource reference validation

2. **Business logic**
   - Status phase transitions (Pending → Ready → Failed)
   - Dependency resolution (waiting for tools, providers, etc.)
   - Retention policies and cleanup logic
   - Configuration parsing and defaults

3. **Edge cases and error handling**
   - Empty resource lists
   - Missing dependent resources
   - Invalid configurations
   - Optional field serialization (skip_serializing_if behavior)

4. **Trait implementations**
   - `Display` for error types
   - `ConditionExt` methods
   - Custom `From` conversions

### Don't Test

- Private implementation details that may change
- Framework/library code (kube-rs, serde, tokio have their own tests)
- Trivial getters/setters with no logic
- Kubernetes API calls directly (mock the client instead)

## Test Structure

### Unit Tests (Inline Modules)

Place unit tests in a `#[cfg(test)]` module at the bottom of each source file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specific_behavior() {
        // Arrange
        let input = create_test_input();

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected_value);
    }
}
```

### Async Tests

For async code, use `tokio::test`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_operation() {
        let result = async_function().await;
        assert!(result.is_ok());
    }
}
```

### Test Naming Convention

Use descriptive names that explain the scenario:

```rust
// Good
#[test]
fn test_tool_with_empty_name_returns_validation_error() { ... }

#[test]
fn test_condition_ready_when_all_dependencies_satisfied() { ... }

#[test]
fn test_serialization_skips_none_optional_fields() { ... }

// Bad
#[test]
fn test_tool() { ... }

#[test]
fn test1() { ... }
```

## Testing Patterns by Code Type

### CRD Types (src/types/*.rs)

Test serialization, deserialization, and default values:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_spec_serialization_roundtrip() {
        let spec = ToolSpec {
            name: "my-tool".to_string(),
            version: Some("1.0".to_string()),
            ..Default::default()
        };

        let json = serde_json::to_string(&spec).unwrap();
        let parsed: ToolSpec = serde_json::from_str(&json).unwrap();

        assert_eq!(spec.name, parsed.name);
        assert_eq!(spec.version, parsed.version);
    }

    #[test]
    fn test_optional_fields_not_serialized_when_none() {
        let spec = ToolSpec {
            name: "test".to_string(),
            optional_field: None,
            ..Default::default()
        };

        let json = serde_json::to_value(&spec).unwrap();
        assert!(!json.as_object().unwrap().contains_key("optionalField"));
    }

    #[test]
    fn test_phase_transitions_are_valid() {
        assert!(matches!(ToolPhase::Pending.next(), Some(ToolPhase::Ready)));
        assert!(matches!(ToolPhase::Failed.next(), None));
    }
}
```

### Condition Trait (src/types/common.rs)

Test the ConditionExt trait thoroughly:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_condition_adds_new_condition() {
        let mut conditions = vec![];
        conditions.set_condition("Ready", "True", "AllReady", "Everything is ready");

        assert_eq!(conditions.len(), 1);
        assert_eq!(conditions[0].type_, "Ready");
        assert_eq!(conditions[0].status, "True");
    }

    #[test]
    fn test_set_condition_updates_existing() {
        let mut conditions = vec![Condition {
            type_: "Ready".to_string(),
            status: "False".to_string(),
            ..Default::default()
        }];

        conditions.set_condition("Ready", "True", "AllReady", "Now ready");

        assert_eq!(conditions.len(), 1);
        assert_eq!(conditions[0].status, "True");
    }

    #[test]
    fn test_is_condition_true_returns_false_for_missing() {
        let conditions: Vec<Condition> = vec![];
        assert!(!conditions.is_condition_true("Ready"));
    }
}
```

### Error Types (src/lib.rs)

Test Display implementation and metric labels:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_includes_context() {
        let error = Error::InvalidResource("missing field 'name'".to_string());
        let display = format!("{}", error);

        assert!(display.contains("missing field 'name'"));
    }

    #[test]
    fn test_error_metric_label_is_stable() {
        let error = Error::SerializationFailed;
        assert_eq!(error.metric_label(), "serialization_failed");
    }
}
```

### Controller Logic (src/controller/*.rs)

For controllers, focus on testable helper functions. Extract pure logic:

```rust
// In controller code, extract testable functions:
fn should_cleanup(resource: &Tool, config: &Config) -> bool {
    resource.metadata.deletion_timestamp.is_some()
        && config.cleanup_enabled
}

fn calculate_replicas(spec: &ToolboxSpec, status: &ToolboxStatus) -> i32 {
    match spec.scaling.mode {
        ScalingMode::Fixed => spec.scaling.replicas.unwrap_or(1),
        ScalingMode::Auto => status.desired_replicas.unwrap_or(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_cleanup_when_deletion_timestamp_set() {
        let mut resource = test_tool("my-tool");
        resource.metadata.deletion_timestamp = Some(Time(chrono::Utc::now()));

        let config = Config { cleanup_enabled: true, ..Default::default() };

        assert!(should_cleanup(&resource, &config));
    }

    #[test]
    fn test_calculate_replicas_uses_fixed_value() {
        let spec = ToolboxSpec {
            scaling: ScalingConfig {
                mode: ScalingMode::Fixed,
                replicas: Some(3),
            },
            ..Default::default()
        };

        assert_eq!(calculate_replicas(&spec, &Default::default()), 3);
    }
}
```

### Fixtures (src/fixtures.rs)

Use builder patterns for test data:

```rust
// Fixture helper
pub fn test_tool(name: &str) -> Tool {
    Tool {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some("default".to_string()),
            ..Default::default()
        },
        spec: ToolSpec::default(),
        status: None,
    }
}

pub fn test_tool_with_status(name: &str, phase: ToolPhase) -> Tool {
    let mut tool = test_tool(name);
    tool.status = Some(ToolStatus {
        phase: Some(phase),
        ..Default::default()
    });
    tool
}

// In tests
#[test]
fn test_ready_tool_can_be_deployed() {
    let tool = test_tool_with_status("my-tool", ToolPhase::Ready);
    assert!(can_deploy(&tool));
}
```

## Integration Tests

Place integration tests in `tests/` directory for cross-module behavior:

```
rust/operai-controller/
├── src/
│   └── ...
└── tests/
    ├── reconciliation.rs
    └── crd_validation.rs
```

## Using Dev Dependencies

The following test utilities are available:

```rust
// assert-json-diff - Compare JSON structures
use assert_json_diff::assert_json_eq;

#[test]
fn test_json_output_matches_expected() {
    let actual = serde_json::to_value(&resource).unwrap();
    let expected = json!({
        "apiVersion": "operai.dev/v1alpha1",
        "kind": "Tool"
    });
    assert_json_eq!(actual, expected);
}

// tokio-test - Async test utilities
use tokio_test::assert_ok;

#[tokio::test]
async fn test_async_succeeds() {
    assert_ok!(async_operation().await);
}
```

## Running Tests

```bash
# Run all tests
cargo test

# Run tests for a specific module
cargo test types::tool

# Run a specific test
cargo test test_tool_serialization

# Run with output
cargo test -- --nocapture

# Run only unit tests (not integration)
cargo test --lib
```

## Checklist for New Code

When adding new code, ensure tests cover:

- [ ] Happy path behavior
- [ ] Error conditions (invalid input, missing dependencies)
- [ ] Edge cases (empty collections, boundary values)
- [ ] Serialization if the type uses serde
- [ ] Display/Debug implementations
- [ ] Default values if Default is derived/implemented

## Anti-Patterns to Avoid

1. **Testing implementation details** - Don't test private helper functions directly
2. **Excessive mocking** - If you need to mock everything, the code may need refactoring
3. **Test interdependence** - Each test should be independent
4. **Ignoring test failures** - Fix or remove flaky tests, don't ignore them
5. **Testing trivial code** - `fn get_name(&self) -> &str { &self.name }` needs no test
