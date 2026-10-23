//! Maintenance of source-derived LoTW reference data.

mod lotw;

#[cfg(feature = "fixtures")]
pub mod fixtures;

/// Usage text for the dedicated executable and its explicit read/write actions.
const HELP: &str = "clacks-lotw-data - check or regenerate pinned LoTW reference JSON

Usage: clacks-lotw-data [--check | --write | --help]

    --check  Verify both committed JSON files without writing
    --write  Regenerate both JSON files from the pinned reference sources
    --help   Show this help (also shown when no arguments are supplied)

Run from this checkout with:
    cargo run --bin clacks-lotw-data -- --check
";

/// Run the reference-data executable with arguments following its name.
///
/// `--check` verifies both LoTW JSON files without writing; `--write` regenerates
/// them. No arguments or `--help` prints usage without changing files. Unsupported
/// arguments and read/parse/check failures return errors to the entry point.
pub fn run(arguments: impl IntoIterator<Item = String>) -> anyhow::Result<()> {
    let mut arguments = arguments.into_iter();
    let action = arguments.next();
    anyhow::ensure!(arguments.next().is_none(), "{HELP}");
    match action.as_deref() {
        None | Some("--help") => {
            print!("{HELP}");
            Ok(())
        }
        Some("--check") => lotw::update(true),
        Some("--write") => lotw::update(false),
        Some(_) => anyhow::bail!("{HELP}"),
    }
}
