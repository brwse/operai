use std::path::Path;

fn main() {
    println!("cargo::rerun-if-changed=Cargo.toml");
    println!("cargo::rerun-if-changed=../../Cargo.toml");

    let workspace_toml_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");

    if let Ok(contents) = std::fs::read_to_string(&workspace_toml_path) {
        if let Ok(parsed) = contents.parse::<toml::Table>() {
            if let Some(workspace) = parsed.get("workspace").and_then(|w| w.as_table()) {
                if let Some(deps) = workspace.get("dependencies").and_then(|d| d.as_table()) {
                    if let Some(operai) = deps.get("operai") {
                        if let Some(version) = operai.as_table().and_then(|t| t.get("version")) {
                            if let Some(v) = version.as_str() {
                                println!("cargo::rustc-env=OPERAI_VERSION={v}");
                            }
                        }
                    }

                    if let Some(operai_build) = deps.get("operai-build") {
                        if let Some(version) =
                            operai_build.as_table().and_then(|t| t.get("version"))
                        {
                            if let Some(v) = version.as_str() {
                                println!("cargo::rustc-env=OPERAI_BUILD_VERSION={v}");
                            }
                        }
                    }
                }
            }
        }
    }
}
