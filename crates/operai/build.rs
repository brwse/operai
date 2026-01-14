fn main() {
    // Allow tool crates to use cfg(operai_embedding) without unexpected_cfgs warnings.
    println!("cargo:rustc-check-cfg=cfg(operai_embedding)");
}
