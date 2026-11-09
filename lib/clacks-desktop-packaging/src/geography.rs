//! Identity and integrity checks for a common production-built native payload.
use anyhow::Context;
use clacks_geography::artifact::read::open_bundle;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

/// Observed identity and size of a verified native payload.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DataEvidence {
    /// Data identity read from the verified manifest.
    pub release_id: String,
    /// Build-provided manifest digest verified against the payload.
    pub manifest_sha256: String,
    /// Total declared file length, including the manifest.
    pub unpacked_bytes: u64,
}

/// Check an assembled payload against the independently supplied build identity.
///
/// Use after ordinary platform copying or native installer read-back. Production
/// acceptance belongs to generation; this checks schema and payload integrity.
pub fn inspect(manifest_sha256: &str, bundle: &Path) -> anyhow::Result<DataEvidence> {
    let reader = open_bundle(bundle, manifest_sha256)
        .context("geographic payload integrity check failed")?;
    let manifest_bytes = fs::metadata(bundle.join("manifest.json"))?.len();
    let manifest = reader.manifest();
    let total = manifest.files.iter().try_fold(manifest_bytes, |total, f| {
        total
            .checked_add(f.bytes)
            .context("geographic payload size overflow")
    })?;
    Ok(DataEvidence {
        release_id: manifest.release_id.clone(),
        manifest_sha256: manifest_sha256.to_owned(),
        unpacked_bytes: total,
    })
}

#[cfg(test)]
mod tests;
