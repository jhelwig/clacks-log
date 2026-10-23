//! Entry point for checking or regenerating pinned LoTW reference data.

/// Run the reference-data tool, reporting failures to the invoking shell.
fn main() -> anyhow::Result<()> {
    clacks_reference_data::run(std::env::args().skip(1))
}
