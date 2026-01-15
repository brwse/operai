//! Build script support for embedding Operai agent code into binaries.
//!
//! This module provides build-time code generation functionality for Operai projects.
//! It reads a binary embedding file (`.brwse-embedding`) containing pre-computed
//! vector embeddings of agent code, and generates Rust constants that can be
//! compiled into the final binary.
//!
//! # Generated Output
//!
//! When a `.brwse-embedding` file exists, this module generates:
//! - `EMBEDDING`: A slice of `f32` values representing the embedding vector
//! - `EMBEDDING_DIM`: The dimension/length of the embedding vector
//!
//! When the file doesn't exist, empty constants are generated.
//!
//! # Embedding File Format
//!
//! The `.brwse-embedding` file contains raw little-endian f32 values. Each
//! embedding is serialized as 4 bytes per float value. The file size must
//! be divisible by 4.

use std::{env, fs, path::Path};

/// Performs build-time setup for embedding generation.
///
/// This function is called from build scripts (`build.rs`) to:
/// 1. Set the `operai_embedding` cfg flag
/// 2. Read `.brwse-embedding` if it exists
/// 3. Generate `embedding.rs` in the OUT_DIR with embedding constants
///
/// # Panics
///
/// - If `OUT_DIR` environment variable is not set
/// - If the embedding file size is not divisible by 4
/// - If reading or writing files fails
///
/// # Cargo Build Behavior
///
/// Marks `.brwse-embedding` as a build dependency, causing the build to
/// rerun when that file changes.
pub fn setup() {
    println!("cargo:rustc-check-cfg=cfg(operai_embedding)");
    println!("cargo:rustc-cfg=operai_embedding");
    println!("cargo:rerun-if-changed=.brwse-embedding");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let dest_path = Path::new(&out_dir).join("embedding.rs");
    let embedding_path = Path::new(".brwse-embedding");

    let content = if embedding_path.exists() {
        let bytes = fs::read(embedding_path).expect("failed to read .brwse-embedding");

        if !bytes.len().is_multiple_of(4) {
            panic!("invalid embedding file: size not divisible by 4");
        }

        let floats: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();

        let dim = floats.len();
        let float_strs: Vec<String> = floats.iter().map(|f| format!("{f}_f32")).collect();
        let array_content = float_strs.join(", ");

        format!(
            r#"pub const EMBEDDING: &[f32] = &[{array_content}];

#[allow(dead_code)]
pub const EMBEDDING_DIM: usize = {dim};
"#
        )
    } else {
        r#"pub const EMBEDDING: &[f32] = &[];

#[allow(dead_code)]
pub const EMBEDDING_DIM: usize = 0;
"#
        .to_string()
    };

    fs::write(&dest_path, content).expect("failed to write embedding.rs");
}
