//! UTC version allocation and reuse of native GitHub preparation requests.

use anyhow::Context;
use chrono::{Datelike, Utc};
use clacks_desktop_release::ReleaseVersion;
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

/// Desktop application manifest, both locally and at an existing request's commit.
const MANIFEST: &str = "bin/clacks-log/Cargo.toml";
/// Namespace for preparation requests identified by their GitHub workflow run.
const REQUEST_PREFIX: &str = "release/desktop-";

/// Prepared version and the native GitHub request that holds its allocation.
#[derive(Serialize)]
pub(crate) struct VersionPreparation {
    /// Whether this invocation allocated a new version instead of reusing a request.
    new_request: bool,
    /// Request branch, stable across attempts of one workflow run.
    branch: String,
    /// Calendar version allocated to this request.
    version: ReleaseVersion,
    /// Existing request URL, including requests whose merged branch was deleted.
    pull_request: Option<String>,
}

/// Source commit and optional PR associated with a preparation branch.
struct Request {
    /// Commit whose desktop manifest records the allocation.
    head: String,
    /// Native PR URL when the request has reached PR creation.
    pull_request: Option<String>,
}

/// Allocate the next UTC version, or reuse this workflow run's committed allocation.
#[tracing::instrument(skip_all, fields(repository, request_id))]
pub(crate) fn prepare(repository: &str, request_id: u64) -> anyhow::Result<VersionPreparation> {
    let requests = requests(repository)?;
    let branch = format!("{REQUEST_PREFIX}{request_id}");
    if let Some(request) = requests.get(&branch) {
        let version = version_at(repository, &request.head)?;
        tracing::info!(%version, %branch, "reusing desktop release preparation");
        return Ok(VersionPreparation {
            new_request: false,
            branch,
            version,
            pull_request: request.pull_request.clone(),
        });
    }

    let mut allocated = Vec::new();
    for page in api_pages(&format!("repos/{repository}/tags?per_page=100"))? {
        for tag in page.as_array().context("expected desktop tag page")? {
            if let Ok(version) = ReleaseVersion::from_tag(crate::text(tag, "name")?) {
                allocated.push(version);
            }
        }
    }
    for request in requests.values() {
        allocated.push(version_at(repository, &request.head)?);
    }

    let mut document: toml_edit::DocumentMut = std::fs::read_to_string(MANIFEST)?.parse()?;
    let current = version_field(&mut document)?
        .as_str()
        .context("desktop package version is not a string")?;
    if let Ok(version) = ReleaseVersion::parse(current) {
        allocated.push(version);
    }
    let now = Utc::now();
    let year = u8::try_from(now.year().rem_euclid(100))?;
    let month = u8::try_from(now.month())?;
    let version = ReleaseVersion::next_for_month(year, month, &allocated)?;

    write_version(Path::new(MANIFEST), document, version)?;
    tracing::info!(%version, %branch, "allocated desktop release version");
    Ok(VersionPreparation {
        new_request: true,
        branch,
        version,
        pull_request: None,
    })
}

/// Read preparation PRs and branches; branch heads take precedence over PR snapshots.
fn requests(repository: &str) -> anyhow::Result<BTreeMap<String, Request>> {
    let mut requests = BTreeMap::new();
    for page in api_pages(&format!(
        "repos/{repository}/pulls?state=all&base=main&per_page=100"
    ))? {
        for pr in page.as_array().context("expected preparation PR page")? {
            let Some(branch) = pr.pointer("/head/ref").and_then(Value::as_str) else {
                continue;
            };
            if !branch.starts_with(REQUEST_PREFIX)
                || pr.pointer("/head/repo/full_name").and_then(Value::as_str) != Some(repository)
            {
                continue;
            }
            requests.entry(branch.to_owned()).or_insert(Request {
                head: pr
                    .pointer("/head/sha")
                    .and_then(Value::as_str)
                    .context("preparation PR has no source commit")?
                    .to_owned(),
                pull_request: Some(crate::text(pr, "html_url")?.to_owned()),
            });
        }
    }
    for page in api_pages(&format!(
        "repos/{repository}/git/matching-refs/heads/{REQUEST_PREFIX}"
    ))? {
        for reference in page
            .as_array()
            .context("expected preparation branch page")?
        {
            let branch = crate::text(reference, "ref")?
                .strip_prefix("refs/heads/")
                .context("expected a preparation branch ref")?;
            let head = reference
                .pointer("/object/sha")
                .and_then(Value::as_str)
                .context("preparation branch has no source commit")?
                .to_owned();
            requests
                .entry(branch.to_owned())
                .and_modify(|request| request.head.clone_from(&head))
                .or_insert(Request {
                    head,
                    pull_request: None,
                });
        }
    }
    Ok(requests)
}

/// Retrieve all GitHub API pages in one outer array.
fn api_pages(endpoint: &str) -> anyhow::Result<Vec<Value>> {
    let value = crate::command_json(
        "gh",
        &[
            "api".into(),
            endpoint.into(),
            "--paginate".into(),
            "--slurp".into(),
        ],
    )?;
    let Value::Array(pages) = value else {
        anyhow::bail!("expected GitHub API pages")
    };
    Ok(pages)
}

/// Read the actual application version committed at a native preparation head.
fn version_at(repository: &str, head: &str) -> anyhow::Result<ReleaseVersion> {
    let bytes = crate::command_output(
        "gh",
        &[
            "api".into(),
            format!("repos/{repository}/contents/{MANIFEST}?ref={head}").into(),
            "-H".into(),
            "Accept: application/vnd.github.raw+json".into(),
        ],
    )?;
    let mut document: toml_edit::DocumentMut = std::str::from_utf8(&bytes)?.parse()?;
    ReleaseVersion::parse(
        version_field(&mut document)?
            .as_str()
            .context("desktop package version is not a string")?,
    )
}

/// Select the owning application's explicit version field for reading or replacement.
fn version_field(document: &mut toml_edit::DocumentMut) -> anyhow::Result<&mut toml_edit::Value> {
    let package = document
        .get_mut("package")
        .and_then(toml_edit::Item::as_table_mut)
        .context("desktop manifest has no package table")?;
    anyhow::ensure!(
        package.get("name").and_then(toml_edit::Item::as_str) == Some("clacks-log"),
        "expected the clacks-log application manifest"
    );
    package
        .get_mut("version")
        .and_then(toml_edit::Item::as_value_mut)
        .context("desktop application needs an explicit package version")
}

/// Replace only the application's version while preserving surrounding TOML.
fn write_version(
    path: &Path,
    mut document: toml_edit::DocumentMut,
    version: ReleaseVersion,
) -> anyhow::Result<()> {
    let field = version_field(&mut document)?;
    let mut replacement = toml_edit::Value::from(version.to_string());
    replacement.decor_mut().clone_from(field.decor());
    *field = replacement;
    std::fs::write(path, document.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests;
