//! Native path-policy checks without launching a renderer or opening a store.

use super::*;
use crate::config::{Cli, StartupContext};
use clap::Parser;

#[test]
fn workspace_override_creates_only_the_directory() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let environment = StartupContext {
        cwd: directory.path().into(),
        environment: ::config::Environment::default().source(Some(Default::default())),
        directories: AppDirectories {
            config: directory.path().join("config"),
            data: directory.path().join("data"),
        },
    };

    let config = crate::config::resolve(
        Cli::try_parse_from(["clacks-log", "--workspace", "λ-local"])?,
        &environment,
    )?;
    let paths = WorkspacePaths::resolve(&config)?;

    assert_eq!(
        paths.directory,
        directory.path().join("λ-local").canonicalize()?
    );

    assert_eq!(paths.database, paths.directory.join("workspace.sqlite"));
    assert_eq!(paths.lock_file, paths.directory.join("instance.lock"));
    assert!(!paths.database.exists());
    assert!(!paths.lock_file.exists());
    std::fs::write(directory.path().join("file"), "not a directory")?;

    let config = crate::config::resolve(
        Cli::try_parse_from(["clacks-log", "--workspace", "file"])?,
        &environment,
    )?;

    assert!(WorkspacePaths::resolve(&config).is_err());
    assert_eq!(
        std::fs::read_to_string(directory.path().join("file"))?,
        "not a directory"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_workspace_aliases_have_one_canonical_directory() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let target = directory.path().join("target");
    std::fs::create_dir(&target)?;
    std::os::unix::fs::symlink(&target, directory.path().join("alias"))?;
    let environment = StartupContext {
        cwd: directory.path().into(),
        environment: ::config::Environment::default().source(Some(Default::default())),
        directories: AppDirectories {
            config: directory.path().join("config"),
            data: directory.path().join("data"),
        },
    };
    let first = crate::config::resolve(
        Cli::try_parse_from(["clacks-log", "--workspace", "target"])?,
        &environment,
    )?;
    let second = crate::config::resolve(
        Cli::try_parse_from(["clacks-log", "--workspace", "alias"])?,
        &environment,
    )?;

    assert_eq!(
        WorkspacePaths::resolve(&first)?.lock_file,
        WorkspacePaths::resolve(&second)?.lock_file
    );
    Ok(())
}

#[tokio::test]
async fn first_open_then_reopen_uses_same_account() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let paths = WorkspacePaths {
        directory: directory.path().into(),
        database: directory.path().join("workspace.sqlite"),
        lock_file: directory.path().join("instance.lock"),
        log_directory: directory.path().join("logs"),
    };
    let first = super::open(&paths).await?;

    assert!(!first.identity.enrolled);
    assert_eq!(first.settings.operating, Default::default());
    let account = first.identity.account;
    first.store.close().await?;
    let next = super::open(&paths).await?;

    assert_eq!(next.identity.account, account);
    next.store.close().await?;
    Ok(())
}

#[tokio::test]
async fn existing_invalid_database_is_preserved() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let paths = WorkspacePaths {
        directory: directory.path().into(),
        database: directory.path().join("workspace.sqlite"),
        lock_file: directory.path().join("instance.lock"),
        log_directory: directory.path().join("logs"),
    };
    std::fs::write(&paths.database, b"not a workspace")?;

    assert!(super::open(&paths).await.is_err());
    assert_eq!(std::fs::read(&paths.database)?, b"not a workspace");
    Ok(())
}

#[tokio::test]
async fn ready_requires_initial_reads() -> anyhow::Result<()> {
    use clacks_client_store::operating::{self, OperatingSaveOutcome, OperatingState};
    use clacks_model::{
        contact::StationSnapshot, profile::CurrentOperatingValues, scalar::RecordedCallsign,
    };
    let directory = tempfile::tempdir()?;
    let paths = WorkspacePaths {
        directory: directory.path().into(),
        database: directory.path().join("workspace.sqlite"),
        lock_file: directory.path().join("instance.lock"),
        log_directory: directory.path().join("logs"),
    };
    let initial = super::open(&paths).await?;
    let next = OperatingState {
        current: Some(CurrentOperatingValues {
            station: StationSnapshot {
                callsign: RecordedCallsign::new("W7HEX/P")?,
                location: None,
                references: Default::default(),
            },
            operator: None,
            transmit_power: None,
            equipment: None,
        }),
        selected_profile: None,
        selected_equipment: None,
    };

    assert!(matches!(
        operating::save(&initial.store, &initial.settings.operating, &next).await?,
        OperatingSaveOutcome::Saved(_)
    ));
    initial.store.close().await?;
    let ready = super::open(&paths).await?;

    assert_eq!(ready.settings.operating, next);
    ready.store.close().await?;
    Ok(())
}
