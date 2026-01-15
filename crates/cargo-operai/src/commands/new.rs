//! Scaffolds a new Brwse tool project with template code and configuration.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Args;
use console::style;
use tracing::info;

/// The current version of the operai crate (set by build.rs from workspace
/// Cargo.toml).
const OPERAI_VERSION: &str = env!("OPERAI_VERSION");
/// The current version of the operai-build crate (set by build.rs from
/// workspace Cargo.toml).
const OPERAI_BUILD_VERSION: &str = env!("OPERAI_BUILD_VERSION");

/// Arguments for the `new` command.
#[derive(Args)]
pub struct NewArgs {
    /// Name of the new tool project.
    pub name: String,

    /// Create a multi-tool crate template.
    #[arg(long)]
    pub multi: bool,

    /// Create a new Cargo workspace with optimized settings.
    #[arg(long)]
    pub workspace: bool,

    /// Output directory (defaults to current directory).
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

/// Finds the Cargo workspace root by traversing parent directories.
///
/// Returns `Some(path)` if a `Cargo.toml` with `[workspace]` is found,
/// `None` otherwise.
fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.canonicalize().ok()?;
    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(contents) = std::fs::read_to_string(&cargo_toml) {
                if let Ok(parsed) = contents.parse::<toml::Table>() {
                    if parsed.contains_key("workspace") {
                        return Some(current);
                    }
                }
            }
        }
        if !current.pop() {
            break;
        }
    }
    None
}

/// Adds a new member to an existing workspace `Cargo.toml`.
fn add_workspace_member(workspace_root: &Path, member_path: &str) -> Result<()> {
    let cargo_toml_path = workspace_root.join("Cargo.toml");
    let contents =
        std::fs::read_to_string(&cargo_toml_path).context("failed to read workspace Cargo.toml")?;

    let mut doc = contents
        .parse::<toml::Table>()
        .context("failed to parse workspace Cargo.toml")?;

    let workspace = doc
        .get_mut("workspace")
        .and_then(toml::Value::as_table_mut)
        .context("expected [workspace] table")?;

    let members = workspace
        .entry("members")
        .or_insert_with(|| toml::Value::Array(Vec::new()))
        .as_array_mut()
        .context("expected workspace.members to be an array")?;

    // Check if member already exists
    let member_value = toml::Value::String(member_path.to_owned());
    if !members.contains(&member_value) {
        members.push(member_value);
    }

    let output = toml::to_string_pretty(&doc).context("failed to serialize Cargo.toml")?;
    std::fs::write(&cargo_toml_path, output).context("failed to write workspace Cargo.toml")?;

    Ok(())
}

/// Adds a tool entry to the workspace's `operai.toml`.
fn update_workspace_operai_toml(workspace_root: &Path, tool_name: &str) -> Result<()> {
    let operai_toml_path = workspace_root.join("operai.toml");
    let lib_name = tool_name.replace('-', "_");

    if operai_toml_path.exists() {
        // Append new tool entry
        let existing =
            std::fs::read_to_string(&operai_toml_path).context("failed to read operai.toml")?;

        let new_entry =
            format!("\n[[tools]]\npath = \"{tool_name}/target/release/lib{lib_name}.dylib\"\n");

        std::fs::write(&operai_toml_path, format!("{existing}{new_entry}"))
            .context("failed to update operai.toml")?;
    } else {
        // Create new operai.toml
        let operai_toml = generate_workspace_operai_toml(tool_name);
        std::fs::write(&operai_toml_path, operai_toml).context("failed to write operai.toml")?;
    }

    Ok(())
}

/// Creates a new Cargo workspace with optimized settings.
fn create_workspace(workspace_dir: &Path, first_member: &str) -> Result<()> {
    std::fs::create_dir_all(workspace_dir).context("failed to create workspace directory")?;

    let cargo_toml = generate_workspace_cargo_toml(first_member);
    std::fs::write(workspace_dir.join("Cargo.toml"), cargo_toml)
        .context("failed to write workspace Cargo.toml")?;

    let operai_toml = generate_workspace_operai_toml(first_member);
    std::fs::write(workspace_dir.join("operai.toml"), operai_toml)
        .context("failed to write workspace operai.toml")?;

    let gitignore = generate_workspace_gitignore();
    std::fs::write(workspace_dir.join(".gitignore"), gitignore)
        .context("failed to write .gitignore")?;

    let rustfmt_toml = generate_rustfmt_toml();
    std::fs::write(workspace_dir.join("rustfmt.toml"), rustfmt_toml)
        .context("failed to write rustfmt.toml")?;

    Ok(())
}

/// Creates a tool package (without operai.toml if in a workspace).
fn create_tool_package(
    project_dir: &Path,
    name: &str,
    multi: bool,
    in_workspace: bool,
) -> Result<()> {
    std::fs::create_dir_all(project_dir.join("src"))
        .context("failed to create project directory")?;

    let cargo_toml = if in_workspace {
        generate_workspace_member_cargo_toml(name)
    } else {
        generate_standalone_cargo_toml(name)
    };
    std::fs::write(project_dir.join("Cargo.toml"), cargo_toml)
        .context("failed to write Cargo.toml")?;

    let lib_rs = if multi {
        generate_multi_tool_lib(name)
    } else {
        generate_single_tool_lib(name)
    };
    std::fs::write(project_dir.join("src/lib.rs"), lib_rs).context("failed to write src/lib.rs")?;

    let build_rs = generate_build_rs();
    std::fs::write(project_dir.join("build.rs"), build_rs).context("failed to write build.rs")?;

    // Only create operai.toml in standalone mode
    if !in_workspace {
        let operai_toml = generate_operai_toml(name);
        std::fs::write(project_dir.join("operai.toml"), operai_toml)
            .context("failed to write operai.toml")?;

        let gitignore = generate_gitignore();
        std::fs::write(project_dir.join(".gitignore"), gitignore)
            .context("failed to write .gitignore")?;
    }

    Ok(())
}

/// Runs the `new` command.
pub fn run(args: &NewArgs) -> Result<()> {
    let output_dir = args.output.clone().unwrap_or_else(|| PathBuf::from("."));

    // Ensure output directory exists before canonicalization
    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir).context("failed to create output directory")?;
    }

    let output_dir = output_dir
        .canonicalize()
        .context("failed to resolve output directory")?;

    if args.workspace {
        // Create a new workspace with the tool as the first member
        let workspace_dir = output_dir.join(&args.name);

        if workspace_dir.exists() {
            bail!("directory already exists: {}", workspace_dir.display());
        }

        info!(name = %args.name, "Creating new workspace");

        create_workspace(&workspace_dir, "tools")?;

        let project_dir = workspace_dir.join("tools");
        create_tool_package(&project_dir, "tools", args.multi, true)?;

        println!(
            "{} Created workspace: {}",
            style("✓").green().bold(),
            workspace_dir.display()
        );
        println!();
        println!("Next steps:");
        println!("  cd {}", args.name);
        println!("  cargo operai build    # Build with embeddings");
        println!("  cargo operai serve    # Start local dev server");
    } else {
        // Check if we're inside an existing workspace
        let workspace_root = find_workspace_root(&output_dir);

        let project_dir = output_dir.join(&args.name);

        if project_dir.exists() {
            bail!("directory already exists: {}", project_dir.display());
        }

        info!(name = %args.name, "Creating new tool project");

        if let Some(ref workspace_root) = workspace_root {
            // Compute relative path from workspace root to project
            let relative_path = if output_dir == *workspace_root {
                args.name.clone()
            } else {
                pathdiff::diff_paths(&project_dir, workspace_root)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| args.name.clone())
            };

            create_tool_package(&project_dir, &args.name, args.multi, true)?;
            add_workspace_member(workspace_root, &relative_path)?;
            update_workspace_operai_toml(workspace_root, &args.name)?;

            println!(
                "{} Created tool project: {} (added to workspace)",
                style("✓").green().bold(),
                args.name
            );
        } else {
            // Standalone project
            create_tool_package(&project_dir, &args.name, args.multi, false)?;

            println!(
                "{} Created tool project: {}",
                style("✓").green().bold(),
                args.name
            );
        }

        println!();
        println!("Next steps:");
        println!("  cd {}", args.name);
        println!("  cargo operai build    # Build with embeddings");
        println!("  cargo operai serve    # Start local dev server");
    }

    Ok(())
}

fn generate_workspace_cargo_toml(first_member: &str) -> String {
    format!(
        r#"[workspace]
resolver = "2"
members = ["{first_member}"]

[workspace.package]
edition = "2024"

[workspace.lints.rust]
unsafe_code = "allow"

[workspace.lints.clippy]
all = {{ level = "deny", priority = -1 }}
pedantic = {{ level = "deny", priority = -1 }}
missing_safety_doc = "deny"
allow_attributes = "deny"
allow_attributes_without_reason = "deny"
# AI-friendly relaxations
must_use_candidate = "allow"
struct_excessive_bools = "allow"
unused_async = "allow"
struct_field_names = "allow"
too_many_lines = "allow"

[workspace.dependencies]
operai = "{OPERAI_VERSION}"
operai-build = "{OPERAI_BUILD_VERSION}"
serde = {{ version = "1.0", features = ["derive"] }}
schemars = "1.2"
tokio = {{ version = "1", features = ["rt"] }}
abi_stable = "0.11"

[profile.release]
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true
overflow-checks = true

[profile.dev]
debug = true
"#
    )
}

fn generate_workspace_member_cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition.workspace = true

[lib]
crate-type = ["cdylib"]

[lints]
workspace = true

[dependencies]
operai = {{ workspace = true }}
serde = {{ workspace = true }}
schemars = {{ workspace = true }}
tokio = {{ workspace = true }}
abi_stable = {{ workspace = true }}

[build-dependencies]
operai-build = {{ workspace = true }}
"#
    )
}

fn generate_standalone_cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
operai = "{OPERAI_VERSION}"
serde = {{ version = "1.0", features = ["derive"] }}
schemars = "1.2"
tokio = {{ version = "1", features = ["rt"] }}
abi_stable = "0.11"

[build-dependencies]
operai-build = "{OPERAI_BUILD_VERSION}"
"#
    )
}

fn generate_single_tool_lib(name: &str) -> String {
    let fn_name = name.replace('-', "_");
    format!(
        r#"//! {name} - A Brwse tool.

use operai::{{Context, JsonSchema, Result, tool}};
use serde::{{Deserialize, Serialize}};

/// Input for the {fn_name} tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct Input {{
    /// The input message.
    pub message: String,
}}

/// Output from the {fn_name} tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Output {{
    /// The result message.
    pub result: String,
}}

/// # {name} (ID: {fn_name})
///
/// Processes input and returns a result.
#[tool]
pub async fn {fn_name}(_ctx: Context, input: Input) -> Result<Output> {{
    Ok(Output {{
        result: format!("Processed: {{}}", input.message),
    }})
}}

operai::generate_tool_entrypoint!();
"#
    )
}

fn generate_multi_tool_lib(name: &str) -> String {
    format!(
        r#"//! {name} - A multi-tool Brwse crate.

use operai::{{Context, JsonSchema, Result, tool}};
use serde::{{Deserialize, Serialize}};

/// Input for the echo tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EchoInput {{
    /// The message to echo back.
    pub message: String,
}}

/// Output from the echo tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct EchoOutput {{
    /// The echoed message.
    pub echo: String,
    /// Length of the original message.
    pub length: usize,
}}

/// # Echo
///
/// Echoes back the input message.
#[tool]
pub async fn echo(_ctx: Context, input: EchoInput) -> Result<EchoOutput> {{
    let length = input.message.len();
    Ok(EchoOutput {{
        echo: input.message,
        length,
    }})
}}

/// Input for the greet tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GreetInput {{
    /// Name of the person to greet.
    pub name: String,
    /// Optional custom greeting.
    #[serde(default)]
    pub greeting: Option<String>,
}}

/// Output from the greet tool.
#[derive(Debug, Serialize, JsonSchema)]
pub struct GreetOutput {{
    /// The greeting message.
    pub message: String,
}}

/// # Say Hello! (ID: greet)
///
/// Greets a user by name with an optional custom greeting.
#[tool]
pub async fn greet(_ctx: Context, input: GreetInput) -> Result<GreetOutput> {{
    let greeting = input.greeting.as_deref().unwrap_or("Hello");
    Ok(GreetOutput {{
        message: format!("{{greeting}}, {{}}!", input.name),
    }})
}}

operai::generate_tool_entrypoint!();
"#
    )
}

fn generate_build_rs() -> &'static str {
    r"fn main() {
    operai_build::setup();
}
"
}

fn generate_gitignore() -> &'static str {
    r"/target
.brwse-embedding
Cargo.lock
"
}

fn generate_operai_toml(name: &str) -> String {
    let lib_name = name.replace('-', "_");
    format!(
        r#"# Operai Configuration

# Build configuration
[config]
# embedding_provider = "fastembed"  # fastembed | openai
# embedding_model = "nomic-embed-text-v1.5"

# Tool definitions
[[tools]]
path = "target/release/lib{lib_name}.dylib"

# Policy definitions (examples)

# [[policies]]
# name = "audit-logging"
# version = "1.0"
# [[policies.effects]]
# tool = "*"
# stage = "after"
# when = "true"

# [[policies]]
# path = "policies/compliance.toml"
"#
    )
}

fn generate_workspace_operai_toml(first_member: &str) -> String {
    let lib_name = first_member.replace('-', "_");
    format!(
        r#"# Operai Configuration

# Build configuration
[config]
# embedding_provider = "fastembed"  # fastembed | openai
# embedding_model = "nomic-embed-text-v1.5"

# Tool definitions
[[tools]]
path = "{first_member}/target/release/lib{lib_name}.dylib"
"#
    )
}

fn generate_workspace_gitignore() -> &'static str {
    r"/target
.brwse-embedding
Cargo.lock
*.dylib
"
}

fn generate_rustfmt_toml() -> &'static str {
    r#"edition = "2024"
max_width = 100
use_small_heuristics = "Max"
"#
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::{Context, Result};

    use super::*;
    use crate::testing;

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new(prefix: &str) -> Result<Self> {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);

            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let mut path = std::env::temp_dir();
            path.push(format!("{prefix}-{nanos}-{}-{unique}", std::process::id()));
            fs::create_dir_all(&path)?;
            Ok(Self { path })
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn read_to_string(path: &Path) -> Result<String> {
        fs::read_to_string(path).with_context(|| format!("read file: {path:?}"))
    }

    #[test]
    fn test_run_creates_single_tool_project_and_sanitizes_fn_name() -> Result<()> {
        // Arrange
        let temp = TestTempDir::new("operai-new")?;
        let name = "hello-world";
        let output_dir = temp.path().to_path_buf();
        let project_dir = output_dir.join(name);

        let args = NewArgs {
            name: name.to_owned(),
            multi: false,
            workspace: false,
            output: Some(output_dir),
        };

        // Act
        run(&args)?;

        // Assert
        assert!(project_dir.is_dir());
        assert!(project_dir.join("src").is_dir());

        for required_path in [
            project_dir.join("Cargo.toml"),
            project_dir.join("build.rs"),
            project_dir.join("operai.toml"),
            project_dir.join(".gitignore"),
            project_dir.join("src/lib.rs"),
        ] {
            assert!(
                required_path.is_file(),
                "missing file: {}",
                required_path.display()
            );
        }

        let cargo_toml = read_to_string(&project_dir.join("Cargo.toml"))?;
        assert!(cargo_toml.contains(r#"name = "hello-world""#));

        let parsed: toml::Value = toml::from_str(&cargo_toml).context("parse Cargo.toml")?;
        let dependencies = parsed
            .get("dependencies")
            .and_then(toml::Value::as_table)
            .context("expected [dependencies] table")?;
        let operai = dependencies
            .get("operai")
            .context("expected operai dependency")?;
        // Standalone projects use version string, workspace members use table
        assert!(
            operai.is_str()
                || operai
                    .as_table()
                    .is_some_and(|t| t.contains_key("workspace")),
            "expected operai to be a version string or workspace reference"
        );

        let lib_rs = read_to_string(&project_dir.join("src/lib.rs"))?;
        assert!(lib_rs.contains("pub async fn hello_world"));

        let build_rs = read_to_string(&project_dir.join("build.rs"))?;
        assert!(build_rs.contains("operai_build::setup()"));

        Ok(())
    }

    #[test]
    fn test_run_creates_multi_tool_project_template_when_multi_true() -> Result<()> {
        // Arrange
        let temp = TestTempDir::new("operai-new")?;
        let name = "multi-tool";
        let output_dir = temp.path().to_path_buf();
        let project_dir = output_dir.join(name);

        let args = NewArgs {
            name: name.to_owned(),
            multi: true,
            workspace: false,
            output: Some(output_dir),
        };

        // Act
        run(&args)?;

        // Assert
        let lib_rs = read_to_string(&project_dir.join("src/lib.rs"))?;
        assert!(lib_rs.contains("pub async fn echo"));

        assert!(lib_rs.contains("pub async fn greet"));
        assert!(lib_rs.contains(r#"unwrap_or("Hello")"#));
        assert!(lib_rs.contains("#[serde(default)]"));
        assert!(lib_rs.contains("operai::generate_tool_entrypoint!();"));

        Ok(())
    }

    #[test]
    fn test_run_returns_error_when_project_directory_already_exists() -> Result<()> {
        // Arrange
        let temp = TestTempDir::new("operai-new")?;
        let name = "existing";
        let output_dir = temp.path().to_path_buf();
        let project_dir = output_dir.join(name);
        fs::create_dir_all(&project_dir).context("create pre-existing project dir")?;

        let args = NewArgs {
            name: name.to_owned(),
            multi: false,
            workspace: false,
            output: Some(output_dir),
        };

        // Act
        let err = run(&args).expect_err("expected error when project directory exists");

        // Assert
        let message = err.to_string();
        assert!(message.contains("directory already exists:"));
        assert!(message.contains(&project_dir.display().to_string()));

        Ok(())
    }

    #[test]
    fn test_generate_single_tool_lib_replaces_multiple_hyphens_with_underscores() {
        // Arrange
        let name = "my-cool-tool";

        // Act
        let lib_rs = generate_single_tool_lib(name);

        // Assert - function name should have all hyphens replaced
        assert!(lib_rs.contains("pub async fn my_cool_tool"));
        assert!(lib_rs.contains("Input for the my_cool_tool tool"));
        // Package name in doc comment should remain as-is
        assert!(lib_rs.contains("//! my-cool-tool"));
    }

    #[test]
    fn test_generate_standalone_cargo_toml_preserves_hyphenated_package_name() {
        // Arrange
        let name = "my-tool-name";

        // Act
        let cargo_toml = generate_standalone_cargo_toml(name);

        // Assert
        assert!(cargo_toml.contains(r#"name = "my-tool-name""#));
        assert!(cargo_toml.contains(r#"edition = "2024""#));
        assert!(cargo_toml.contains(r#"crate-type = ["cdylib"]"#));
    }

    /// RAII guard that restores the current directory when dropped.
    /// Must be dropped BEFORE `TestTempDir` to ensure directory is restored
    /// before the temp directory is deleted.
    struct CurrentDirGuard {
        previous: PathBuf,
    }

    impl CurrentDirGuard {
        /// Atomically captures the current directory and changes to `path`.
        fn set(path: &Path) -> Result<Self> {
            let previous = std::env::current_dir()?;
            std::env::set_current_dir(path)?;
            Ok(Self { previous })
        }
    }

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.previous);
        }
    }

    #[test]
    fn test_run_uses_current_directory_when_output_is_none() -> Result<()> {
        // Arrange - acquire lock before changing global state (current directory)
        let _lock = testing::test_lock();

        let temp = TestTempDir::new("operai-new")?;
        let name = "default-output-test";

        // Create guard AFTER temp to ensure correct drop order:
        // 1. Guard restores directory
        // 2. TestTempDir cleans up temp directory
        let _guard = CurrentDirGuard::set(temp.path())?;

        let args = NewArgs {
            name: name.to_owned(),
            multi: false,
            workspace: false,
            output: None, // Should default to current directory
        };

        // Act
        run(&args)?;

        // Assert - guard ensures directory is restored even on panic/failure
        let project_dir = temp.path().join(name);
        assert!(project_dir.is_dir());
        assert!(project_dir.join("Cargo.toml").is_file());

        Ok(())
    }

    #[test]
    fn test_generate_build_rs_calls_operai_build_setup() {
        // Act
        let build_rs = generate_build_rs();

        // Assert - verify it calls the shared build logic
        assert!(build_rs.contains("operai_build::setup()"));
    }

    #[test]
    fn test_generate_gitignore_excludes_build_artifacts() {
        // Act
        let gitignore = generate_gitignore();

        // Assert
        assert!(gitignore.contains("/target"));
        assert!(gitignore.contains(".brwse-embedding"));
        assert!(gitignore.contains("Cargo.lock"));
    }

    #[test]
    fn test_generate_single_tool_lib_with_underscored_name_preserves_underscores() {
        // Arrange - name already uses underscores (valid Rust identifier)
        let name = "my_tool";

        // Act
        let lib_rs = generate_single_tool_lib(name);

        // Assert - underscores should be preserved
        assert!(lib_rs.contains("pub async fn my_tool"));
        assert!(lib_rs.contains("//! my_tool"));
    }

    #[test]
    fn test_generate_operai_toml_contains_provider_options() {
        // Act
        let config = generate_operai_toml("test-tool");

        // Assert - verify config documents available options
        assert!(config.contains("embedding_provider"));
        assert!(config.contains("fastembed"));
        assert!(config.contains("openai"));
        assert!(config.contains("embedding_model"));
        assert!(config.contains("[[tools]]"));
        assert!(config.contains("path = \"target/release/libtest_tool.dylib\""));
    }

    #[test]
    fn test_generate_multi_tool_lib_includes_optional_greeting_with_serde_default() {
        // Arrange
        let name = "greet-tools";

        // Act
        let lib_rs = generate_multi_tool_lib(name);

        // Assert - verify optional field handling is demonstrated
        assert!(lib_rs.contains("Option<String>"));
        assert!(lib_rs.contains("#[serde(default)]"));
        assert!(lib_rs.contains("unwrap_or"));
    }

    #[test]
    fn test_run_creates_nested_output_directories_when_they_do_not_exist() -> Result<()> {
        // Arrange
        let temp = TestTempDir::new("operai-new")?;
        let name = "nested-test";
        // Create a nested output path that doesn't exist yet
        let nested_output = temp.path().join("a").join("b").join("c");
        let project_dir = nested_output.join(name);

        let args = NewArgs {
            name: name.to_owned(),
            multi: false,
            workspace: false,
            output: Some(nested_output.clone()),
        };

        // Act
        run(&args)?;

        // Assert - both nested dirs and project should be created
        assert!(nested_output.is_dir());
        assert!(project_dir.is_dir());
        assert!(project_dir.join("Cargo.toml").is_file());

        Ok(())
    }
}
