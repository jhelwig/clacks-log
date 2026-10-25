//! Real subprocess fixtures for OS lock election and termination recovery.

use super::*;
use std::{
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

/// Child-process lifetime that is always reaped, including after a test failure.
struct ChildOwner {
    /// Isolated fixture process, terminated and reaped during cleanup.
    child: Child,
}

impl Drop for ChildOwner {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Start an isolated invocation of the private process fixture.
fn start(directory: &std::path::Path, slot: &str) -> anyhow::Result<ChildOwner> {
    Ok(ChildOwner {
        child: Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "instance::tests::process::child_entry",
                "--ignored",
                "--test-threads=1",
            ])
            .env("CLACKS_TEST_WORKSPACE", directory)
            .env("CLACKS_TEST_SLOT", slot)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()?,
    })
}

/// Wait for the atomically published election result without assuming scheduling order.
fn result(directory: &std::path::Path, slot: &str) -> anyhow::Result<String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match std::fs::read_to_string(directory.join(slot)) {
            Ok(value) => return Ok(value),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        anyhow::ensure!(Instant::now() < deadline, "child did not report election");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "isolated child fixture, invoked by real process tests"]
fn child_entry() -> anyhow::Result<()> {
    let directory = std::path::PathBuf::from(
        std::env::var_os("CLACKS_TEST_WORKSPACE").expect("fixture directory"),
    );
    let slot = std::env::var("CLACKS_TEST_SLOT")?;
    let election = elect(&paths(&directory))?;
    let value = match &election {
        Election::Owner(_) => "owner",
        Election::Contended(_) => "contended",
    };
    let temporary = directory.join(format!("{slot}.tmp"));
    std::fs::write(&temporary, value)?;
    std::fs::rename(temporary, directory.join(&slot))?;
    if matches!(election, Election::Owner(_)) {
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    Ok(())
}

#[test]
fn simultaneous_processes_elect_one_owner_and_crash_releases_it() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let first = start(directory.path(), "first")?;
    let second = start(directory.path(), "second")?;
    let outcomes = [
        result(directory.path(), "first")?,
        result(directory.path(), "second")?,
    ];

    assert_eq!(
        outcomes
            .iter()
            .filter(|value| value.as_str() == "owner")
            .count(),
        1
    );

    assert_eq!(
        outcomes
            .iter()
            .filter(|value| value.as_str() == "contended")
            .count(),
        1
    );

    assert!(matches!(
        elect(&paths(directory.path()))?,
        Election::Contended(_)
    ));
    drop(first);
    drop(second);
    wait_for_owner(&paths(directory.path()))?;

    assert!(!directory.path().join("workspace.sqlite").exists());
    Ok(())
}
