//! Project scaffolding for creating new Operai tool projects.
//!
//! This module implements the `cargo operai new` command, which generates new tool projects
//! with appropriate boilerplate code. It supports three modes:
//!
//! - **Standalone projects**: Single-tool crates with their own `Cargo.toml` and `operai.toml`
//! - **Workspace members**: Tools added to an existing Cargo workspace
//! - **New workspaces**: Creates a workspace with the tool as the first member
//!
//! # Project Structure
//!
//! Generated projects include:
//! - `Cargo.toml` with appropriate dependencies and `[lib]` configuration for `cdylib`
//! - `build.rs` that calls `operai_build::setup()`
//! - `src/lib.rs` with example tool implementations (single or multi-tool templates)
//! - `operai.toml` for Operai-specific configuration (standalone projects only)
//! - `.gitignore` and `rustfmt.toml` for workspace projects

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::Args;
use console::style;
use tracing::info;

const OPERAI_VERSION: &str = env!("OPERAI_VERSION");
const OPERAI_BUILD_VERSION: &str = env!("OPERAI_BUILD_VERSION");

/// Command-line arguments for the `cargo operai new` command.
#[derive(Args)]
pub struct NewArgs {
    /// Name of the tool/project to create (e.g., "my-tool" or "my_tool")
    pub name: String,

    /// Generate a multi-tool template with example tools instead of a single tool
    #[arg(long)]
    pub multi: bool,

    /// Create a new Cargo workspace with this tool as the first member
    #[arg(long)]
    pub workspace: bool,

    /// Output directory for the new project (defaults to current directory)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

/// Searches for the Cargo workspace root by traversing parent directories.
///
/// Starting from `start`, this function walks up the directory tree looking for
/// a `Cargo.toml` file containing a `[workspace]` table. Returns `None` if no
/// workspace root is found before reaching the filesystem root.
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

/// Adds a new member to an existing workspace's `Cargo.toml`.
///
/// This function reads the workspace's `Cargo.toml`, adds the new member path
/// to the `workspace.members` array (if not already present), and writes the
/// updated TOML back to disk.
///
/// # Errors
///
/// Returns an error if:
/// - The workspace `Cargo.toml` cannot be read or parsed
/// - The `[workspace]` table is missing
/// - `workspace.members` exists but is not an array
/// - The file cannot be written after modification
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

/// Updates or creates the workspace's `operai.toml` with a new tool entry.
///
/// If `operai.toml` exists, appends a new `[[tools]]` entry for the tool.
/// Otherwise, creates a new `operai.toml` with build configuration and the tool entry.
///
/// # Tool Path Format
///
/// Generated tool paths use the format: `{tool_name}/target/release/lib{lib_name}.dylib`
/// where hyphens in `tool_name` are replaced with underscores for the library name.
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

/// Creates a new Cargo workspace with initial configuration files.
///
/// Generates the workspace skeleton with:
/// - `Cargo.toml` with workspace configuration and shared dependencies
/// - `operai.toml` with Operai-specific build configuration
/// - `.gitignore` configured for Operai projects
/// - `rustfmt.toml` with Rust 2024 edition settings
///
/// The workspace is configured with the first member (typically "tools") included
/// in the `workspace.members` array.
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

/// Creates a new tool package with appropriate boilerplate files.
///
/// Generates the tool project structure based on the template type (single vs multi-tool)
/// and whether it's part of a workspace. Always creates:
/// - `Cargo.toml` with dependencies (workspace member or standalone variants)
/// - `src/lib.rs` with tool implementation template
/// - `build.rs` with Operai build setup
///
/// For standalone projects (`!in_workspace`), also creates:
/// - `operai.toml` with tool configuration
/// - `.gitignore` with Operai-specific patterns
///
/// # Parameters
///
/// - `project_dir`: Directory where the tool package will be created
/// - `name`: Name of the tool (hyphens are preserved in Cargo.toml, converted to underscores in lib.rs)
/// - `multi`: If true, generates multi-tool template; otherwise single-tool template
/// - `in_workspace`: If true, generates workspace member configuration; otherwise standalone
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

/// Main entry point for the `cargo operai new` command.
///
/// Creates a new Operai tool project based on the provided arguments. The behavior
/// depends on the combination of flags:
///
/// # Modes
///
/// - **`--workspace`**: Creates a new workspace at `{output}/{name}` with the tool
///   as the first member (in the `tools/` subdirectory)
///
/// - **Inside existing workspace**: Detects workspace root, adds the tool as a member,
///   and updates both `Cargo.toml` and `operai.toml`
///
/// - **Standalone**: Creates a standalone project with its own configuration files
///
/// # Output Directory Behavior
///
/// - If `output` is `None`, uses the current directory
/// - Creates parent directories if they don't exist
/// - Returns an error if the target project directory already exists
///
/// # Examples
///
/// ```no_run
/// # use cargo_operai::commands::new::{NewArgs, run};
/// # use std::path::PathBuf;
/// # fn main() -> anyhow::Result<()> {
/// // Create a standalone tool in current directory
/// let args = NewArgs {
///     name: "my-tool".to_string(),
///     multi: false,
///     workspace: false,
///     output: None,
/// };
/// run(&args)?;
/// # Ok(())
/// # }
/// ```
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

/// Generates the contents of a workspace `Cargo.toml` file.
///
/// Creates a workspace configuration with:
/// - Workspace resolver set to "2"
/// - Shared dependencies in `[workspace.dependencies]`
/// - Lint configuration for both Rust and Clippy
/// - Release profile with LTO, single codegen unit, and stripped symbols
/// - Debug profile with full debug info
///
/// The `first_member` is added to the `members` array and should be the relative
/// path to the first workspace member (typically "tools").
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

/// Generates the contents of a workspace member's `Cargo.toml` file.
///
/// Creates a minimal package configuration that inherits from workspace settings:
/// - Uses `edition.workspace = true` to share edition from workspace
/// - Configured as `cdylib` for dynamic library output
/// - Uses `[lints.workspace = true]` to share lint configuration
/// - All dependencies reference workspace versions with `{ workspace = true }`
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

/// Generates the contents of a standalone project's `Cargo.toml` file.
///
/// Creates a complete package configuration with:
/// - Rust 2024 edition
/// - `cdylib` crate type for dynamic library output
/// - Explicit version-pinned dependencies (not using workspace inheritance)
/// - All required dependencies for Operai tool development
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

/// Generates the contents of `src/lib.rs` for a single-tool project.
///
/// Creates a template with:
/// - Module-level documentation
/// - Input and Output structs with `JsonSchema` derive
/// - A single tool function (name with hyphens converted to underscores)
/// - The `operai::generate_tool_entrypoint!()` macro invocation
///
/// # Example
///
/// For name `"hello-world"`, generates a function `hello_world` that processes
/// a message and returns it with a prefix.
fn generate_single_tool_lib(name: &str) -> String {
    let fn_name = name.replace('-', "_");
    format!(
        r#"//! {name} - A Brwse tool.

use operai::{{Context, JsonSchema, Result, tool}};
use serde::{{Deserialize, Serialize}};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Input {{
    pub message: String,
}}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Output {{
    pub result: String,
}}

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

/// Generates the contents of `src/lib.rs` for a multi-tool project.
///
/// Creates a template demonstrating multiple tools in a single crate:
/// - `echo` tool: Returns input message with character count
/// - `greet` tool: Demonstrates optional fields with `#[serde(default)]`
/// - Both tools properly annotated with `#[tool]` attribute
/// - The `operai::generate_tool_entrypoint!()` macro invocation
///
/// This template serves as documentation for best practices when implementing
/// tools with optional parameters and multiple functions.
fn generate_multi_tool_lib(name: &str) -> String {
    format!(
        r#"//! {name} - A multi-tool Brwse crate.

use operai::{{Context, JsonSchema, Result, tool}};
use serde::{{Deserialize, Serialize}};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EchoInput {{
    pub message: String,
}}

#[derive(Debug, Serialize, JsonSchema)]
pub struct EchoOutput {{
    pub echo: String,
    pub length: usize,
}}

#[tool]
pub async fn echo(_ctx: Context, input: EchoInput) -> Result<EchoOutput> {{
    let length = input.message.len();
    Ok(EchoOutput {{
        echo: input.message,
        length,
    }})
}}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GreetInput {{
    pub name: String,
    #[serde(default)]
    pub greeting: Option<String>,
}}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GreetOutput {{
    pub message: String,
}}

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

/// Generates the contents of `build.rs`.
///
/// Returns a minimal build script that calls `operai_build::setup()` to
/// configure the build process for Operai tool development.
fn generate_build_rs() -> &'static str {
    r"fn main() {
    operai_build::setup();
}
"
}

/// Generates the contents of `.gitignore` for standalone projects.
///
/// Returns gitignore patterns for:
/// - `/target`: Build artifacts directory
/// - `.brwse-embedding`: Operai embedding cache
/// - `Cargo.lock`: Lock file (for projects, not workspaces)
fn generate_gitignore() -> &'static str {
    r"/target
.brwse-embedding
Cargo.lock
"
}

/// Generates the contents of `operai.toml` for standalone projects.
///
/// Creates configuration with:
/// - Commented-out `[config]` section showing embedding provider/model options
/// - A `[[tools]]` entry pointing to the built `.dylib` file
/// - Example policy definitions (commented out) for reference
///
/// The library name has hyphens replaced with underscores to match Rust's
/// identifier conventions (e.g., "my-tool" becomes "libmy_tool.dylib").
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

/// Generates the contents of `operai.toml` for workspace projects.
///
/// Similar to `generate_operai_toml` but the tool path includes the member
/// directory prefix (e.g., "tools/target/release/libtools.dylib" for the
/// first member in a workspace).
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

/// Generates the contents of `.gitignore` for workspace projects.
///
/// Extends the standalone gitignore with `*.dylib` to ignore compiled
/// tool libraries from all workspace members.
fn generate_workspace_gitignore() -> &'static str {
    r"/target
.brwse-embedding
Cargo.lock
*.dylib
"
}

/// Generates the contents of `rustfmt.toml`.
///
/// Returns formatter configuration with:
/// - Edition set to "2024"
/// - Max line width of 100 characters
/// - `use_small_heuristics = "Max"` for consistent formatting
fn generate_rustfmt_toml() -> &'static str {
    r#"edition = "2024"
max_width = 100
use_small_heuristics = "Max"
"#
}

#[cfg(test)]
mod tests {
    //! Integration tests for the `new` command.
    //!
    //! These tests verify that project scaffolding works correctly across
    //! different modes (standalone, workspace member, new workspace) and
    //! template types (single-tool vs multi-tool).

    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::{Context, Result};

    use super::*;
    use crate::testing;

    /// Temporary directory helper that cleans up on drop.
    ///
    /// Creates uniquely-named temporary directories using a combination of
    /// timestamp, process ID, and an atomic counter to avoid collisions in
    /// concurrent tests.
    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        /// Creates a new temporary directory with the given prefix.
        ///
        /// The directory name includes: `{prefix}-{nanos}-{pid}-{counter}`
        /// where `nanos` is the current Unix timestamp in nanoseconds, `pid`
        /// is the current process ID, and `counter` is an atomically incremented
        /// value.
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

        /// Returns the path to the temporary directory.
        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        /// Removes the temporary directory and all its contents.
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    /// Helper to read a file with better error context.
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

    /// RAII guard for temporarily changing the current directory.
    ///
    /// Saves the current directory on creation, changes to the specified path,
    /// and automatically restores the original directory when dropped. This is
    /// essential for tests that change the working directory to avoid affecting
    /// subsequent tests.
    struct CurrentDirGuard {
        previous: PathBuf,
    }

    impl CurrentDirGuard {
        /// Changes the current directory and returns a guard that will restore it.
        ///
        /// The guard should be held for the duration of the test; when it's dropped,
        /// the original working directory is restored.
        fn set(path: &Path) -> Result<Self> {
            let previous = std::env::current_dir()?;
            std::env::set_current_dir(path)?;
            Ok(Self { previous })
        }
    }

    impl Drop for CurrentDirGuard {
        /// Restores the original current directory.
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
