//! Clacks-specific source identity and release eligibility.
mod ci;
mod cli;
pub mod geography;
mod metadata;

use anyhow::Context;
use serde_json::Value;
use std::{ffi::OsString, process::Command};

/// Run the desktop packaging command, returning validation and I/O failures.
pub fn run() -> anyhow::Result<()> {
    cli::run()
}

/// Read a required textual field from external structured command output.
fn text<'a>(value: &'a Value, key: &str) -> anyhow::Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing text field {key}"))
}
/// Read a required unsigned identifier from external command output.
fn number(value: &Value, key: &str) -> anyhow::Result<u64> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow::anyhow!("missing numeric field {key}"))
}
/// Invoke a supported external tool using literal argument boundaries.
fn command_output(tool: impl AsRef<std::ffi::OsStr>, args: &[OsString]) -> anyhow::Result<Vec<u8>> {
    let output = Command::new(tool)
        .args(args)
        .output()
        .context("could not start required packaging tool")?;
    // Provider output can contain private response bodies. Report status only.
    anyhow::ensure!(
        output.status.success(),
        "packaging tool failed with {}",
        output.status
    );
    Ok(output.stdout)
}
/// Decode JSON emitted by a supported command without logging response bodies.
fn command_json(tool: &str, args: &[OsString]) -> anyhow::Result<Value> {
    Ok(serde_json::from_slice(&command_output(tool, args)?)?)
}
/// Write build evidence only into the caller-selected output path.
fn write_json(path: &std::path::Path, value: &impl serde::Serialize) -> anyhow::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}
