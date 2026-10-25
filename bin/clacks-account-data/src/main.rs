//! Maintain the source-derived initial account definitions.

fn main() -> anyhow::Result<()> {
    clacks_reference_data::accounts::run(std::env::args().skip(1))
}
