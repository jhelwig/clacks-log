//! Source-owned desktop package metadata.
use serde::{Deserialize, Serialize};

/// Supported native package identity selected explicitly by the build job.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub(crate) enum PackagePlatform {
    /// Existing native Windows installer target.
    WindowsX64,
    /// Apple Silicon application distributed inside a disk image.
    MacosArm64,
}

/// Validated identity carried through package construction and final evidence.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct BuildMetadata {
    /// Cargo workspace application version, also used by MSI.
    pub version: String,
    /// Exact commit packaged, never a moving branch name.
    pub revision: String,
    /// Native Rust target triple selected for the package.
    pub target: String,
    /// Installer architecture.
    pub architecture: String,
    /// Pinned package compiler version, when one package compiler is used.
    pub tool_version: Option<String>,
    /// Whether the validated release tag selects a GitHub prerelease.
    pub prerelease: bool,
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
    let packages = input
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("Cargo metadata has no packages"))?;
    let package = packages
        .iter()
        .find(|p| p.get("name").and_then(serde_json::Value::as_str) == Some("clacks-log"))
        .ok_or_else(|| anyhow::anyhow!("Cargo metadata has no clacks-log package"))?;
    let version = package
        .get("version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing application version"))?;
    let prerelease = if let Some(tag) = tag {
        if tag == format!("v{version}") {
            false
        } else {
            let candidate = tag.strip_prefix(&format!("v{version}-rc."));
            anyhow::ensure!(
                candidate.is_some_and(
                    |number| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())
                ),
                "release tag {tag} must be v{version} or v{version}-rc.N (N is numeric)"
            );
            true
        }
    } else {
        false
    };
    tracing::info!(%version, prerelease, ?platform, "validated application package identity");

    let mut metadata = BuildMetadata {
        version: version.into(),
        revision: revision.to_ascii_lowercase(),
        target: "x86_64-pc-windows-msvc".into(),
        architecture: "x64".into(),
        tool_version: Some("1.2.0".into()),
        prerelease,
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
#[cfg(test)]
mod tests;
