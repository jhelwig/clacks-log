//! Process ownership and activation regression coverage.

mod process;

use super::*;
use crate::workspace::WorkspacePaths;

/// Construct test workspace filenames without account or database initialization.
pub(super) fn paths(directory: &std::path::Path) -> WorkspacePaths {
    WorkspacePaths {
        directory: directory.into(),
        database: directory.join("workspace.sqlite"),
        lock_file: directory.join("instance.lock"),
        log_directory: directory.join("logs"),
    }
}

#[test]
fn leftover_file_is_not_ownership_and_live_lock_is() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let paths = paths(directory.path());
    std::fs::write(&paths.lock_file, "leftover")?;
    let Election::Owner(owner) = elect(&paths)? else {
        panic!("unheld file must not block");
    };

    assert!(matches!(elect(&paths)?, Election::Contended(_)));
    drop(owner);
    wait_for_owner(&paths)?;

    assert_eq!(std::fs::read_to_string(&paths.lock_file)?, "leftover");
    assert!(!paths.database.exists());
    Ok(())
}

#[test]
fn independent_workspaces_have_independent_owners() -> anyhow::Result<()> {
    let first = tempfile::tempdir()?;
    let second = tempfile::tempdir()?;
    let Election::Owner(_first) = elect(&paths(first.path()))? else {
        panic!("first owner");
    };
    let Election::Owner(_second) = elect(&paths(second.path()))? else {
        panic!("second owner");
    };
    Ok(())
}

#[test]
fn lock_open_errors_are_not_contention() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let paths = paths(directory.path());
    std::fs::create_dir(&paths.lock_file)?;

    assert!(elect(&paths).is_err());
    Ok(())
}

#[test]
fn activation_is_acknowledged_and_listener_cleanup_is_joined() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let Election::Owner(owner) = elect(&paths(directory.path()))? else {
        panic!("owner");
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let (sender, mut receiver) = tokio::sync::watch::channel(());

    let listener = owner.listen(runtime.handle(), sender)?;
    activation::request(&owner.address)?;

    assert!(receiver.has_changed()?);
    receiver.borrow_and_update();
    runtime.block_on(listener.shutdown())?;
    drop(owner);
    wait_for_owner(&paths(directory.path()))?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn stale_socket_is_recovered_but_other_files_are_preserved() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let Election::Owner(owner) = elect(&paths(directory.path()))? else {
        panic!("owner");
    };
    let socket = std::os::unix::net::UnixListener::bind(&owner.address.path)?;
    drop(socket);

    assert!(owner.address.path.exists());
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let (sender, _receiver) = tokio::sync::watch::channel(());

    let listener = owner.listen(runtime.handle(), sender.clone())?;
    use std::os::unix::fs::PermissionsExt;

    assert_eq!(
        std::fs::metadata(&owner.address.path)?.permissions().mode() & 0o777,
        0o600
    );
    activation::request(&owner.address)?;
    runtime.block_on(listener.shutdown())?;

    assert!(!owner.address.path.exists());
    std::fs::write(&owner.address.path, "preserve")?;

    assert!(owner.listen(runtime.handle(), sender).is_err());
    assert_eq!(std::fs::read_to_string(&owner.address.path)?, "preserve");
    std::fs::remove_file(&owner.address.path)?;
    Ok(())
}

#[test]
fn unavailable_owner_has_a_bounded_activation_failure() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let Election::Owner(owner) = elect(&paths(directory.path()))? else {
        panic!("owner");
    };
    let started = std::time::Instant::now();

    assert!(activation::request(&owner.address).is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(6));
    assert!(matches!(
        elect(&paths(directory.path()))?,
        Election::Contended(_)
    ));

    assert!(!directory.path().join("workspace.sqlite").exists());
    Ok(())
}

#[test]
fn malformed_messages_do_not_activate_or_stop_the_listener() -> anyhow::Result<()> {
    use interprocess::local_socket::{tokio::Stream, traits::tokio::Stream as _};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let directory = tempfile::tempdir()?;
    let Election::Owner(owner) = elect(&paths(directory.path()))? else {
        panic!("owner");
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let (sender, receiver) = tokio::sync::watch::channel(());

    let listener = owner.listen(runtime.handle(), sender)?;
    runtime.block_on(async {
        let mut stream = Stream::connect(owner.address.name.clone()).await?;
        stream.write_all(b"BADVERSN").await?;
        let mut acknowledgment = [0; 8];

        assert!(stream.read_exact(&mut acknowledgment).await.is_err());
        Ok::<_, anyhow::Error>(())
    })?;

    assert!(!receiver.has_changed()?);
    activation::request(&owner.address)?;

    assert!(receiver.has_changed()?);
    runtime.block_on(listener.shutdown())?;
    Ok(())
}

/// Wait for inherited test-process descriptors to close across concurrent fork/exec.
pub(crate) fn wait_for_owner(paths: &WorkspacePaths) -> anyhow::Result<()> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if matches!(elect(paths)?, Election::Owner(_)) {
            return Ok(());
        }
        anyhow::ensure!(
            std::time::Instant::now() < deadline,
            "ownership was not released"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[cfg(unix)]
#[test]
fn alias_launch_activates_the_same_owner() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let target = directory.path().join("target");
    std::fs::create_dir(&target)?;
    let alias = directory.path().join("alias");
    std::os::unix::fs::symlink(&target, &alias)?;
    let Election::Owner(owner) = elect(&paths(&target))? else {
        panic!("owner");
    };
    let Election::Contended(address) = elect(&paths(&alias))? else {
        panic!("alias must contend");
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let (sender, receiver) = tokio::sync::watch::channel(());

    let listener = owner.listen(runtime.handle(), sender)?;
    activation::request(&address)?;

    assert!(receiver.has_changed()?);
    runtime.block_on(listener.shutdown())?;
    Ok(())
}

#[test]
fn stalled_peers_cannot_hold_shutdown_open() -> anyhow::Result<()> {
    use interprocess::local_socket::{tokio::Stream, traits::tokio::Stream as _};
    let directory = tempfile::tempdir()?;
    let Election::Owner(owner) = elect(&paths(directory.path()))? else {
        panic!("owner");
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let (sender, _receiver) = tokio::sync::watch::channel(());

    let listener = owner.listen(runtime.handle(), sender)?;
    runtime.block_on(async {
        let mut peers = Vec::new();
        for _ in 0..4 {
            peers.push(Stream::connect(owner.address.name.clone()).await?);
        }
        tokio::time::timeout(std::time::Duration::from_secs(2), listener.shutdown()).await??;
        drop(peers);
        Ok::<_, anyhow::Error>(())
    })?;
    Ok(())
}

#[cfg(windows)]
#[test]
fn windows_case_alias_contends_with_the_same_workspace() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let alias_name = directory
        .path()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_ascii_uppercase();
    let alias = directory.path().parent().unwrap().join(alias_name);
    if !alias.try_exists()? {
        return Ok(());
    }
    let Election::Owner(_owner) = elect(&paths(directory.path()))? else {
        panic!("owner");
    };

    assert!(matches!(elect(&paths(&alias))?, Election::Contended(_)));
    Ok(())
}
