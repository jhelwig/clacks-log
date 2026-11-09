//! Build geographic releases and atomically update the current-release pointer.

use crate::{
    compile::{compile_bundle, verify_bundle},
    recipe::BuildRecipe,
    source::check_cancelled,
};
use anyhow::{Context, Result, ensure};
use clacks_geography::artifact::{CurrentRelease, validate::open_bundle, write::write_bundle};
use sha2_rsa::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    sync::atomic::AtomicBool,
};

/// Result relative to the atomic selection commit point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildOutcome {
    /// A validated immutable release became the current selection.
    Published,
    /// The same verified release and pointer were already current.
    Unchanged,
    /// Selection committed, but subsequent durability/temporary cleanup needs attention.
    PublishedWithCleanupWarning,
}

impl BuildOutcome {
    /// Completed publication category; post-commit cleanup cannot undo publication.
    pub fn diagnostic_name(&self) -> &'static str {
        match self {
            Self::Published | Self::PublishedWithCleanupWarning => "published",
            Self::Unchanged => "unchanged",
        }
    }
}

/// Outcome and diagnostics from one completed publication attempt.
#[derive(Debug)]
pub struct BuildReport {
    /// Whether selection changed and whether post-commit cleanup succeeded.
    pub outcome: BuildOutcome,
    /// Immutable selected release identity.
    pub release_id: String,
    /// Number of independent recipe cases verified before publication.
    pub checked_cases: usize,
    /// Reviewed unsupported-coverage records retained in the release.
    pub reviewed_omissions: usize,
    /// Post-commit warnings; these cannot turn committed selection into a failed attempt.
    pub warnings: Vec<String>,
}

/// Build from cached originals, validate, and atomically select an immutable release.
///
/// Serializes publishers through a persistent OS-locked file. Failure before pointer
/// replacement preserves the previous pointer. Previous release directories remain
/// available to open readers. Cancellation after the commit does not undo publication.
#[tracing::instrument(skip_all,name="geography.publish",fields(release_id=%build_id))]
pub fn build_release(
    recipe: &BuildRecipe,
    build_id: &str,
    cache: &Path,
    output: &Path,
    cancelled: &AtomicBool,
) -> Result<BuildReport> {
    check_cancelled(cancelled)?;
    crate::recipe::validate_recipe(recipe)?;
    crate::recipe::validate_release(build_id)?;
    ensure_directory(output)?;
    let releases = output.join("releases");
    ensure_directory(&releases)?;
    let lock_path = output.join(".publish.lock");
    reject_symlink(&lock_path)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .context("open stable publication lock")?;
    lock.lock().context("lock geographic publication")?;
    check_cancelled(cancelled)?;
    let current_path = output.join("current.json");
    reject_symlink(&current_path)?;
    let current = read_current(output)?;
    let compiled = compile_bundle(recipe, build_id, cache, cancelled)?;
    let staging = tempfile::Builder::new()
        .prefix(".staging-")
        .tempdir_in(&releases)?;
    let manifest = write_bundle(staging.path(), &compiled.contents)?;
    check_cancelled(cancelled)?;
    let mut reader = open_bundle(staging.path())?;
    let acceptance = verify_bundle(&mut reader, recipe)?;
    drop(reader);
    for file in &manifest.files {
        sync_file(&staging.path().join(&file.path))?;
    }
    sync_file(&staging.path().join("manifest.json"))?;
    sync_directory(&staging.path().join("layers"))?;
    sync_directory(&staging.path().join("notices"))?;
    sync_directory(staging.path())?;
    let manifest_bytes = fs::read(staging.path().join("manifest.json"))?;
    let pointer = CurrentRelease {
        release_id: build_id.to_owned(),
        manifest_sha256: hex::encode(Sha256::digest(&manifest_bytes)),
    };
    let destination = releases.join(build_id);
    reject_symlink(&destination)?;
    let installed = !destination.try_exists()?;
    if !installed {
        let existing = open_bundle(&destination).context("validate existing immutable release")?;
        ensure!(
            fs::read(destination.join("manifest.json"))? == manifest_bytes,
            "release ID already exists with different bytes"
        );
        drop(existing);
    } else {
        check_cancelled(cancelled)?;
        fs::rename(staging.path(), &destination).context("install immutable geographic release")?;
        sync_directory(&releases)?;
    }
    check_cancelled(cancelled)?;
    let unchanged = current.as_ref() == Some(&pointer);
    let mut warnings = Vec::new();
    if !unchanged {
        let mut temporary = tempfile::NamedTempFile::new_in(output)?;
        serde_json::to_writer_pretty(temporary.as_file_mut(), &pointer)?;
        temporary.as_file_mut().write_all(b"\n")?;
        temporary.as_file_mut().sync_all()?;
        check_cancelled(cancelled)?;
        temporary
            .persist(&current_path)
            .map_err(|error| error.error)
            .context("atomically replace current geographic selection")?;
        let compiled_features = manifest
            .layers
            .iter()
            .map(|layer| layer.feature_count)
            .sum::<u64>();
        let seam_reports = compiled.seams.len();
        let max_sampled_displacement_m = compiled
            .seams
            .iter()
            .map(|seam| seam.max_sampled_displacement_m)
            .fold(0., f64::max);
        tracing::info!(name:"geography.bundle_published",release_id=%build_id,manifest_sha256=%pointer.manifest_sha256,compiled_features,reviewed_omissions=acceptance.reviewed_omissions,seam_reports,max_sampled_displacement_m,"Validated geographic release selected");
        let omission_ids: Vec<_> = recipe
            .omissions
            .iter()
            .map(|omission| &omission.id)
            .collect();
        tracing::info!(name:"geography.reviewed_omissions_applied",release_id=%build_id,reviewed_omissions=acceptance.reviewed_omissions,?omission_ids,"Reviewed geographic coverage omissions retained");
        // The pointer has committed. Further failures are reported as warnings.
        let durability = sync_directory(output);
        if let Err(error) = durability {
            warnings.push(format!(
                "selection committed; directory durability: {error:#}"
            ));
        }
    }
    if installed {
        let _moved_path = staging.keep();
    } else if let Err(error) = staging.close() {
        warnings.push(format!("temporary directory cleanup: {error}"));
    }
    let outcome = if unchanged {
        BuildOutcome::Unchanged
    } else if !warnings.is_empty() {
        BuildOutcome::PublishedWithCleanupWarning
    } else {
        BuildOutcome::Published
    };
    for warning in &warnings {
        tracing::warn!(reason=%warning,"Geographic release maintenance warning");
    }
    Ok(BuildReport {
        outcome,
        release_id: build_id.to_owned(),
        checked_cases: acceptance.checked_cases,
        reviewed_omissions: acceptance.reviewed_omissions,
        warnings,
    })
}

/// Read and validate the selected immutable release and its manifest digest.
fn read_current(output: &Path) -> Result<Option<CurrentRelease>> {
    let path = output.join("current.json");
    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        metadata.is_file() && metadata.len() <= 1024 * 1024,
        "invalid current pointer file"
    );
    let pointer: CurrentRelease = serde_json::from_slice(&fs::read(&path)?)?;
    crate::recipe::validate_relative(&pointer.release_id)?;
    ensure!(
        !pointer.release_id.contains('/'),
        "invalid selected release identity"
    );
    crate::recipe::validate_digest(&pointer.manifest_sha256)?;
    let release = output.join("releases").join(&pointer.release_id);
    reject_symlink(&release)?;
    let _reader =
        open_bundle(&release).context("validate previously selected geographic release")?;
    ensure!(
        hex::encode(Sha256::digest(fs::read(release.join("manifest.json"))?))
            == pointer.manifest_sha256,
        "current manifest digest mismatch"
    );
    Ok(Some(pointer))
}

/// Create or validate a nonsymlink publication directory.
fn ensure_directory(path: &Path) -> Result<()> {
    reject_symlink(path)?;
    fs::create_dir_all(path)?;
    ensure!(
        fs::metadata(path)?.is_dir(),
        "publication path is not a directory"
    );
    Ok(())
}

/// Reject symlinks while allowing a not-yet-created publication entry.
fn reject_symlink(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => ensure!(
            !metadata.file_type().is_symlink(),
            "publication entry is a symlink"
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    };
    Ok(())
}

/// Flush encoded bytes before installing a release or selecting its manifest.
fn sync_file(path: &Path) -> Result<()> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?
        .sync_all()?;
    Ok(())
}

/// Flush directory entries where portable filesystem APIs support directory handles.
fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    if path.try_exists()? {
        fs::File::open(path)?.sync_all()?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
