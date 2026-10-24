//! Filesystem-boundary interruption and commit-point regressions.

use super::*;
use crate::publication_recipe;

#[test]
fn interruptions_preserve_or_commit_the_pointer_at_the_defined_boundary() {
    let directory = tempfile::tempdir().unwrap();
    let mut recipe =
        crate::recipe::read_recipe(&publication_recipe::recipe(directory.path())).unwrap();
    let output = directory.path().join("output");
    let cache = directory.path().join("cache");
    build_release(&recipe, &cache, &output, &AtomicBool::new(false)).unwrap();
    let original = fs::read(output.join("current.json")).unwrap();
    recipe.release_id = "two".into();
    let cancelled = AtomicBool::new(false);
    INJECTION.with(|slot| slot.set(Some((Boundary::BeforePointer, Action::Cancel))));
    assert!(build_release(&recipe, &cache, &output, &cancelled).is_err());
    assert_eq!(fs::read(output.join("current.json")).unwrap(), original);
    cancelled.store(false, std::sync::atomic::Ordering::Relaxed);
    INJECTION.with(|slot| slot.set(Some((Boundary::BeforePointer, Action::Fail))));
    assert!(build_release(&recipe, &cache, &output, &cancelled).is_err());
    assert_eq!(fs::read(output.join("current.json")).unwrap(), original);
    INJECTION.with(|slot| slot.set(Some((Boundary::AfterPointer, Action::Cancel))));
    assert_eq!(
        build_release(&recipe, &cache, &output, &cancelled)
            .unwrap()
            .outcome,
        BuildOutcome::Published
    );
    assert_ne!(fs::read(output.join("current.json")).unwrap(), original);
}

#[test]
fn post_commit_directory_failure_is_a_published_warning() {
    let directory = tempfile::tempdir().unwrap();
    let recipe = crate::recipe::read_recipe(&publication_recipe::recipe(directory.path())).unwrap();
    INJECTION.with(|slot| slot.set(Some((Boundary::DirectorySync, Action::Fail))));
    let report = build_release(
        &recipe,
        &directory.path().join("cache"),
        &directory.path().join("output"),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(report.outcome, BuildOutcome::PublishedWithCleanupWarning);
    assert!(!report.warnings.is_empty());
    assert!(directory.path().join("output/current.json").is_file());
}

/// Filesystem operation at which a private test-only fault is injected.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Boundary {
    BeforePointer,
    AfterPointer,
    DirectorySync,
}
/// Interruption observed at the selected boundary.
#[derive(Clone, Copy)]
enum Action {
    Cancel,
    Fail,
}
thread_local! {
    /// One fault scoped to the calling test thread.
    static INJECTION:std::cell::Cell<Option<(Boundary,Action)>>=const{std::cell::Cell::new(None)};
}

/// Apply a fault at a filesystem boundary without adding production hooks or public APIs.
pub(super) fn at_boundary(boundary: Boundary, cancelled: &AtomicBool) -> Result<()> {
    INJECTION.with(|slot| {
        if let Some((wanted, action)) = slot.get()
            && wanted == boundary
        {
            slot.set(None);
            match action {
                Action::Cancel => cancelled.store(true, std::sync::atomic::Ordering::Relaxed),
                Action::Fail => {
                    return Err(std::io::Error::other("injected filesystem failure").into());
                }
            }
        }
        Ok(())
    })
}
