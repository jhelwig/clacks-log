//! Minimal key-maintenance configuration from the normal server file.

use crate::config::WrappingKeyConfig;
use serde::Deserialize;
use std::path::Path;

/// Settings required for encryption maintenance, without web-runtime credentials.
#[derive(Deserialize)]
pub(crate) struct KeySettings {
    /// Active and retained deployment key files.
    pub wrapping_keys: WrappingKeyConfig,
}

impl KeySettings {
    /// Read only key settings; unrelated server settings are not validated or loaded.
    pub(crate) fn load(path: &Path) -> anyhow::Result<Self> {
        Ok(config::Config::builder()
            .add_source(config::File::from(path))
            .build()?
            .try_deserialize()?)
    }
}
