//! Deployment key files and account encryption maintenance.

pub(crate) mod account;
pub(crate) mod command;
pub(crate) mod config;
pub(crate) mod migration;
pub(crate) mod references;
pub(crate) mod report;
pub(crate) mod rewrap;

#[cfg(test)]
pub(crate) mod tests;

use anyhow::Context;
use rand::{RngCore, rngs::OsRng};
use std::{io::Write, path::Path};
use zeroize::Zeroizing;

/// Session-scoped exclusion shared by the two database key-maintenance commands.
const MAINTENANCE_LOCK: &str = "clacks.key-maintenance.v1";

/// Maximum encrypted values committed in one maintenance transaction.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MaintenanceBatchSize(std::num::NonZeroU16);

impl TryFrom<u16> for MaintenanceBatchSize {
    type Error = anyhow::Error;

    fn try_from(value: u16) -> anyhow::Result<Self> {
        let value = std::num::NonZeroU16::new(value).context("batch size must be 1–100")?;
        anyhow::ensure!(value.get() <= 100, "batch size must be 1–100");
        Ok(Self(value))
    }
}

impl MaintenanceBatchSize {
    /// Return the checked maximum rows for a single transaction.
    pub(crate) fn get(self) -> u16 {
        self.0.get()
    }
}

/// Write a fresh 32-byte raw key in a private directory without replacing any existing file.
///
/// The caller must protect the destination directory and retain the resulting
/// file with the database backup. No database or provider credentials are needed.
pub fn generate(output: &Path) -> anyhow::Result<()> {
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut bytes = Zeroizing::new([0u8; 32]);
    OsRng.try_fill_bytes(bytes.as_mut_slice())?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .context("Could not create key file in the secret directory")?;
    temporary.write_all(bytes.as_slice())?;
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(output)
        .map_err(|error| error.error)
        .context("Could not publish wrapping key; existing files are never replaced")?;
    Ok(())
}
