//! Library-owned file/environment sources and value-free startup diagnostics.

use super::file::FileConfig;
use ::config::{Config, ConfigError, Environment, File, FileFormat};
use std::{io, path::Path};

/// Read the exact selected file and parse its schema without exposing source excerpts.
pub(super) fn file(path: &Path, explicit: bool) -> anyhow::Result<FileConfig> {
    // File::from(path) performs suffix discovery; --config names an exact file.
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if !explicit && error.kind() == io::ErrorKind::NotFound => {
            return Ok(FileConfig::default());
        }
        Err(error) => {
            return Err(anyhow::anyhow!(
                "startup configuration could not be read ({:?})",
                error.kind()
            ));
        }
    };
    Config::builder()
        .add_source(File::from_str(&text, FileFormat::Toml))
        .build()
        .and_then(Config::try_deserialize)
        .map_err(|error| invalid("file", error))
}

/// Read a standard environment namespace through the library's injectable source.
pub(super) fn environment(source: &Environment, prefix: &str) -> anyhow::Result<Config> {
    Config::builder()
        .add_source(
            source
                .clone()
                .prefix(prefix)
                .prefix_separator("_")
                .ignore_empty(true),
        )
        .build()
        .map_err(|error| invalid("environment", error))
}

/// Expose a setting name or error category while discarding secret-bearing causes.
pub(super) fn invalid(section: &str, error: ConfigError) -> anyhow::Error {
    match error {
        ConfigError::Type { key: Some(key), .. } | ConfigError::At { key: Some(key), .. } => {
            anyhow::anyhow!(
                "invalid startup configuration in {section}: {key}; check its value type"
            )
        }
        ConfigError::FileParse { .. } => {
            anyhow::anyhow!("invalid TOML startup configuration; check file syntax")
        }
        _ => anyhow::anyhow!(
            "invalid startup configuration in {section}; check supported keys and value types"
        ),
    }
}

/// Read an optional external setting, distinguishing absence from a source/type error.
pub(super) fn optional(source: &Config, key: &str) -> anyhow::Result<Option<String>> {
    match source.get_string(key) {
        Ok(value) => Ok(Some(value)),
        Err(ConfigError::NotFound(_)) => Ok(None),
        Err(error) => Err(invalid("environment", error)),
    }
}
