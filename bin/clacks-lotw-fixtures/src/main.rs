//! Entry point for reproducible Rust-authored LoTW test fixtures.

/// Generate or verify fixture bytes using the project's locked Rust tooling.
fn main() -> anyhow::Result<()> {
    clacks_reference_data::fixtures::run(std::env::args().skip(1))
}
