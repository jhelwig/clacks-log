//! Compile the application package version into the desktop library.

use std::{env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("../../bin/clacks-log/Cargo.toml");
    println!("cargo::rerun-if-changed={}", manifest.display());
    let source = fs::read_to_string(&manifest)
        .map_err(|error| format!("cannot read {}: {error}", manifest.display()))?;
    let manifest_data = source
        .parse::<toml::Table>()
        .map_err(|error| format!("invalid Cargo manifest {}: {error}", manifest.display()))?;
    let version = manifest_data
        .get("package")
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} must declare package.version", manifest.display()))?;
    println!("cargo::rustc-env=CLACKS_DESKTOP_VERSION={version}");
    Ok(())
}
