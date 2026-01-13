//! Build script utilities for Operai tools.

use std::{env, fs, path::Path};

/// Sets up the build environment for an Operai tool crate.
///
/// This currently handles embedding generation from `.brwse-embedding`.
pub fn setup() {
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
            r#"/// Pre-computed embedding vector for semantic search.
pub const EMBEDDING: &[f32] = &[{array_content}];

/// Embedding dimension.
pub const EMBEDDING_DIM: usize = {dim};
"#
        )
    } else {
        r#"/// Pre-computed embedding vector (not generated).
pub const EMBEDDING: &[f32] = &[];

/// Embedding dimension.
pub const EMBEDDING_DIM: usize = 0;
"#
        .to_string()
    };

    fs::write(&dest_path, content).expect("failed to write embedding.rs");
}
