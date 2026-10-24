//! Rebuild checked queries and embedded migrations when their source files change.

/// Track SQL directories using Cargo's ordinary directory change detection.
fn main() {
    println!("cargo:rerun-if-changed=sql");
    println!("cargo:rerun-if-changed=migrations");
}
