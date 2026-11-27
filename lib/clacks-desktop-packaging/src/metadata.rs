//! Source-owned desktop package metadata.

use clacks_desktop_release::{PackagePlatform, ReleaseVersion};
use serde::{Deserialize, Serialize};

/// Validated identity carried through package construction and final evidence.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct BuildMetadata {
    /// User-facing installer filename derived from source version and platform.
    pub installer_name: String,
    /// Actions artifact name derived from the installer filename's stem.
    pub artifact_name: String,
    /// Cargo executable package version, also used by MSI.
    pub version: String,
    /// Exact commit packaged, never a moving branch name.
    pub revision: String,
    /// Native Rust target triple selected for the package.
    pub target: String,
    /// Installer architecture.
    pub architecture: String,
    /// Pinned package compiler version, when one package compiler is used.
    pub tool_version: Option<String>,
    /// Preparation status; macOS distribution signing is established afterward.
    pub signing_mode: String,
}

/// Validate Cargo metadata and an optional release tag before packaging.
#[tracing::instrument(skip_all, fields(%revision, tag, ?platform))]
pub(crate) fn prepare(
    input: &serde_json::Value,
    revision: &str,
    tag: Option<&str>,
    platform: PackagePlatform,
) -> anyhow::Result<BuildMetadata> {
    anyhow::ensure!(
        revision.len() == 40 && revision.bytes().all(|b| b.is_ascii_hexdigit()),
        "expected an exact 40-character commit SHA"
    );

    let version = application_version(input)?;
    if let Some(tag) = tag {
        let release = ReleaseVersion::from_tag(tag)?;
        anyhow::ensure!(
            release.to_string() == version,
            "release tag {tag} must match desktop-v{version}"
        );
    }
    tracing::info!(%version, ?platform, "validated application package identity");

    let installer_name = platform.installer_name(version, tag.is_none());
    let artifact_name = std::path::Path::new(&installer_name)
        .file_stem()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| anyhow::anyhow!("installer filename has no UTF-8 stem"))?
        .to_owned();
    let mut metadata = BuildMetadata {
        installer_name,
        artifact_name,
        version: version.into(),
        revision: revision.to_ascii_lowercase(),
        target: "x86_64-pc-windows-msvc".into(),
        architecture: "x64".into(),
        tool_version: Some("1.2.0".into()),
        signing_mode: "unsigned".into(),
    };
    if let PackagePlatform::MacosArm64 = platform {
        metadata.target = "aarch64-apple-darwin".into();
        metadata.architecture = "arm64".into();
        metadata.tool_version = None;
        metadata.signing_mode = "not-established".into();
    }

    Ok(metadata)
}

/// Read the executable package version from Cargo's workspace metadata.
pub(crate) fn application_version(input: &serde_json::Value) -> anyhow::Result<&str> {
    let packages = input
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("Cargo metadata has no packages"))?;
    let package = packages
        .iter()
        .find(|p| p.get("name").and_then(serde_json::Value::as_str) == Some("clacks-log"))
        .ok_or_else(|| anyhow::anyhow!("Cargo metadata has no clacks-log package"))?;
    package
        .get("version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing application version"))
}

#[cfg(test)]
mod tests;
