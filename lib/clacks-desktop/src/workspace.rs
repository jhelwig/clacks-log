//! Local workspace files and operating-system application directory conventions.

#[cfg(test)]
mod tests;

use directories::ProjectDirs;
use std::path::{Path, PathBuf};

/// Initialized account resources, including the reads needed before presenting readiness.
pub(crate) struct OpenedWorkspace {
    /// Pool retained for all local workflows until graceful shutdown.
    pub store: clacks_client_store::store::ClientStore,
    /// Non-secret account and enrollment state.
    pub identity: clacks_client_store::identity::WorkspaceStatus,
    /// Persisted current entry defaults, independent of saved profiles.
    pub operating: clacks_client_store::operating::OperatingState,
}

/// Open an existing workspace or initialize a missing file with account defaults.
///
/// Existing invalid or partially initialized files are preserved. Read failures
/// close the pool before returning; callers may retry without retaining resources.
#[tracing::instrument(skip_all, name = "desktop.workspace.open")]
pub(crate) async fn open(paths: &WorkspacePaths) -> anyhow::Result<OpenedWorkspace> {
    use clacks_client_store::{
        identity, operating,
        store::{ClientStore, StoreSettings},
    };

    let store = match tokio::fs::metadata(&paths.database).await {
        Ok(_) => ClientStore::open_discovered(&paths.database, StoreSettings::default()).await?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let definitions = clacks_model::defaults::account_definitions()?;
            ClientStore::create_with_definitions(
                &paths.database,
                clacks_model::ids::AccountId::new(),
                &definitions,
                StoreSettings::default(),
            )
            .await?
        }
        Err(error) => return Err(error.into()),
    };

    let identity = match identity::status(&store).await {
        Ok(identity) => identity,
        Err(error) => {
            store.close().await?;
            return Err(error);
        }
    };

    let operating = match operating::load(&store).await {
        Ok(operating) => operating,
        Err(error) => {
            store.close().await?;
            return Err(error);
        }
    };

    Ok(OpenedWorkspace {
        store,
        identity,
        operating,
    })
}

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
