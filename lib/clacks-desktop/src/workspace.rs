//! Local workspace files and operating-system application directory conventions.

use directories::ProjectDirs;
use std::path::{Path, PathBuf};

/// OS-conventional roots, separate from any selected workspace override.
pub(crate) struct AppDirectories {
    /// Per-user application configuration location.
    pub config: PathBuf,
    /// Machine-local per-user application data location.
    pub data: PathBuf,
}

impl AppDirectories {
    /// Locate application roots using OS conventions without creating files.
    pub fn discover() -> anyhow::Result<Self> {
        let project = ProjectDirs::from("", "", "Clacks Log").ok_or_else(|| {
            anyhow::anyhow!("operating-system application directories are unavailable")
        })?;
        Ok(Self {
            config: project.config_dir().into(),
            data: project.data_local_dir().into(),
        })
    }
}

/// Actual files belonging to one selected and canonicalized local workspace.
#[derive(Clone)]
pub(crate) struct WorkspacePaths {
    /// Canonical directory used for ownership and local data.
    pub directory: PathBuf,
    /// Relational native account data.
    pub database: PathBuf,
    /// Separate process-lifetime coordination file.
    pub lock_file: PathBuf,
    /// Structured diagnostic output location.
    pub log_directory: PathBuf,
}

impl WorkspacePaths {
    /// Establish the selected workspace directory without touching its database.
    pub fn resolve(config: &crate::config::DesktopConfig) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&config.workspace)?;
        let directory = config.workspace.canonicalize()?;
        Ok(Self {
            database: directory.join("workspace.sqlite"),
            lock_file: directory.join("instance.lock"),
            log_directory: config.logs.directory.clone(),
            directory,
        })
    }
}

/// Resolve a configured path against the directory in which it was supplied.
pub(crate) fn relative_to(value: &Path, base: &Path) -> PathBuf {
    if value.is_absolute() {
        value.into()
    } else {
        base.join(value)
    }
}

#[cfg(test)]
mod tests;
