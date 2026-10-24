//! Native path-policy checks without launching a renderer or opening a store.

use super::*;
use crate::config::{Cli, Environment};
use clap::Parser;

#[test]
fn workspace_override_creates_only_the_directory() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let environment = Environment {
        cwd: directory.path().into(),
        values: Default::default(),
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

#[test]
fn application_roots_use_configuration_and_local_data_conventions() -> anyhow::Result<()> {
    let directories = AppDirectories::discover()?;
    let expected = ProjectDirs::from("", "", "Clacks Log").unwrap();
    assert_eq!(directories.config, expected.config_dir());
    assert_eq!(directories.data, expected.data_local_dir());
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_workspace_aliases_have_one_canonical_directory() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let target = directory.path().join("target");
    std::fs::create_dir(&target)?;
    std::os::unix::fs::symlink(&target, directory.path().join("alias"))?;
    let environment = Environment {
        cwd: directory.path().into(),
        values: Default::default(),
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
