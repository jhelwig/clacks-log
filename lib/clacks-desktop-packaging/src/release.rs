//! Version preparation and publication of exact-source desktop candidates.

use anyhow::Context;
use clacks_desktop_release::{PackagePlatform, ReleaseVersion};
use serde::{Deserialize, Serialize};
use std::path::Path;
use strum::IntoEnumIterator;

/// Version-preparation outcome consumed by the release workflow.
#[derive(Serialize)]
pub(crate) struct VersionPreparation {
    /// Whether the application manifest was updated.
    changed: bool,
    /// Request branch derived from the validated desktop version.
    branch: String,
}

/// A checked source commit and the desktop release version committed there.
struct Candidate {
    /// Exact source commit selected by the request or completed main push.
    revision: String,
    /// Parsed application version, from which the tag is derived.
    version: ReleaseVersion,
}

/// Request branch derived from the source-owned release identity.
fn request_branch(version: &ReleaseVersion) -> String {
    format!("release/{}", version.tag())
}

/// Change only the explicitly selected desktop application's version.
#[tracing::instrument(skip_all, fields(?manifest, version))]
pub(crate) fn set_desktop_version(
    manifest: &Path,
    version: &str,
) -> anyhow::Result<VersionPreparation> {
    let version = ReleaseVersion::parse(version)?;
    let text = std::fs::read_to_string(manifest)?;
    let mut document: toml_edit::DocumentMut = text.parse()?;
    let package = document
        .get_mut("package")
        .and_then(toml_edit::Item::as_table_mut)
        .context("desktop manifest has no package table")?;
    anyhow::ensure!(
        package.get("name").and_then(toml_edit::Item::as_str) == Some("clacks-log"),
        "expected the clacks-log application manifest"
    );
    let field = package
        .get_mut("version")
        .and_then(toml_edit::Item::as_value_mut)
        .context("desktop application needs an explicit package version")?;
    let current = field
        .as_str()
        .context("desktop package version is not a string")?;

    let changed = current != version.as_str();
    if changed {
        let mut replacement = toml_edit::Value::from(version.as_str());
        replacement.decor_mut().clone_from(field.decor());
        *field = replacement;
        std::fs::write(manifest, document.to_string())?;
    }

    Ok(VersionPreparation {
        changed,
        branch: request_branch(&version),
    })
}

/// Read the application version using Cargo's metadata for the current checkout.
fn candidate(revision: &str) -> anyhow::Result<Candidate> {
    let head = crate::command_output("git", &["rev-parse".into(), "HEAD".into()])?;
    let head = std::str::from_utf8(&head)?.trim();
    anyhow::ensure!(
        head == revision,
        "checked-out source differs from the requested release commit"
    );

    let metadata = crate::command_json(
        "cargo",
        &[
            "metadata".into(),
            "--no-deps".into(),
            "--format-version".into(),
            "1".into(),
            "--locked".into(),
            "--offline".into(),
        ],
    )?;
    Ok(Candidate {
        revision: revision.to_owned(),
        version: ReleaseVersion::parse(crate::metadata::application_version(&metadata)?)?,
    })
}

/// Start native release packaging after exact-source CI, creating only a matching tag.
fn start(repository: &str, candidate: Candidate) -> anyhow::Result<()> {
    crate::ci::gate(repository, &candidate.revision)?;

    let tag = candidate.version.tag();
    let remote = crate::command_output(
        "git",
        &[
            "ls-remote".into(),
            "--tags".into(),
            "--refs".into(),
            "origin".into(),
            format!("refs/tags/{tag}").into(),
        ],
    )?;
    if remote.is_empty() {
        crate::command_output(
            "git",
            &[
                "push".into(),
                "origin".into(),
                format!("{}:refs/tags/{tag}", candidate.revision).into(),
            ],
        )
        .with_context(|| {
            format!(
                "could not create release tag {tag} at {}",
                candidate.revision
            )
        })?;
    } else {
        anyhow::ensure!(
            tag_revision(repository, &tag)? == candidate.revision,
            "existing {tag} points at a different source commit"
        );
    }

    crate::command_output(
        "gh",
        &[
            "workflow".into(),
            "run".into(),
            "desktop-release.yml".into(),
            "--repo".into(),
            repository.into(),
            "--ref".into(),
            tag.as_str().into(),
            "-f".into(),
            "release=true".into(),
            "-f".into(),
            "platform=both".into(),
        ],
    )
    .with_context(|| {
        format!(
            "could not dispatch native packaging for existing tag {tag} at {}",
            candidate.revision
        )
    })?;

    Ok(())
}

/// Continue the workflow-selected request after matching it to the checked-out version.
#[tracing::instrument(skip_all, fields(repository, revision, branch))]
pub(crate) fn continue_release(
    repository: &str,
    revision: &str,
    branch: &str,
) -> anyhow::Result<()> {
    let candidate = candidate(revision)?;
    anyhow::ensure!(
        branch == request_branch(&candidate.version),
        "release request branch does not match committed desktop version"
    );

    start(repository, candidate)
}

/// Wait for existing CI and start a release whose version is committed in this checkout.
#[tracing::instrument(skip_all, fields(repository, revision))]
pub(crate) fn start_release(repository: &str, revision: &str) -> anyhow::Result<()> {
    let candidate = candidate(revision)?;
    crate::ci::wait(repository, revision)?;
    start(repository, candidate)
}

/// Resolve a tag through GitHub's commit endpoint, including annotated tags.
fn tag_revision(repository: &str, tag: &str) -> anyhow::Result<String> {
    let commit = crate::command_json(
        "gh",
        &[
            "api".into(),
            format!("repos/{repository}/commits/{tag}").into(),
        ],
    )?;
    Ok(crate::text(&commit, "sha")?.to_owned())
}

/// GitHub's current release and its uploaded files; build provenance belongs to publication.
#[derive(Deserialize)]
struct Release {
    /// Release selected for promotion.
    id: u64,
    /// Source tag associated with this release.
    tag_name: String,
    /// Whether it is still unpublished.
    draft: bool,
    /// Whether it remains a testing candidate.
    prerelease: bool,
    /// Existing downloadable files.
    assets: Vec<Asset>,
}

/// Identity of an existing uploaded installer.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct Asset {
    /// GitHub file identity retained across promotion.
    id: u64,
    /// Published installer filename.
    name: String,
    /// Digest supplied by GitHub when available.
    digest: Option<String>,
    /// Upload completion state.
    state: String,
}

impl Release {
    /// Select an uploaded release installer for every distributed platform.
    fn installers(&self, version: &ReleaseVersion) -> anyhow::Result<Vec<Asset>> {
        PackagePlatform::iter()
            .map(|platform| {
                let name = platform.installer_name(version.as_str(), false);
                self.assets
                    .iter()
                    .find(|asset| asset.name == name && asset.state == "uploaded")
                    .cloned()
                    .with_context(|| format!("candidate is missing uploaded installer {name}"))
            })
            .collect()
    }
}

/// Promote the selected existing candidate while preserving its source and installer identities.
#[tracing::instrument(skip_all, fields(repository, tag))]
pub(crate) fn promote(repository: &str, tag: &str) -> anyhow::Result<()> {
    let version = ReleaseVersion::from_tag(tag)?;
    let value = crate::command_json(
        "gh",
        &[
            "api".into(),
            format!("repos/{repository}/releases/tags/{tag}").into(),
        ],
    )?;
    let release: Release = serde_json::from_value(value)?;
    anyhow::ensure!(
        release.tag_name == tag && !release.draft && release.prerelease,
        "selected release must be a published desktop candidate"
    );
    let installers = release.installers(&version)?;
    let source = tag_revision(repository, tag)?;
    let endpoint = format!("repos/{repository}/releases/{}", release.id);

    // The action invocation is acceptance; only release visibility metadata changes here.
    crate::command_json(
        "gh",
        &[
            "api".into(),
            endpoint.clone().into(),
            "--method".into(),
            "PATCH".into(),
            "-F".into(),
            "prerelease=false".into(),
            "-f".into(),
            "make_latest=true".into(),
        ],
    )?;

    let after: Release =
        serde_json::from_value(crate::command_json("gh", &["api".into(), endpoint.into()])?)?;
    anyhow::ensure!(
        after.id == release.id && after.tag_name == tag && !after.draft && !after.prerelease,
        "promotion did not retain the selected final release"
    );
    anyhow::ensure!(
        tag_revision(repository, tag)? == source && after.installers(&version)? == installers,
        "candidate source or installer identities changed during promotion"
    );
    tracing::info!(
        release_id = release.id,
        "promoted existing desktop candidate"
    );
    Ok(())
}

#[cfg(test)]
mod tests;
