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
    if let Some(tag) = tag {
        anyhow::ensure!(
            tag == format!("desktop-v{version}"),
            "release tag {tag} must match desktop-v{version}"
        );
        let mut components = version.split('.');
        let valid = match (
            components.next(),
            components.next(),
            components.next(),
            components.next(),
        ) {
            (Some(year), Some(month), Some(number), None) => {
                year.len() == 2
                    && year.bytes().all(|byte| byte.is_ascii_digit())
                    && matches!(month.parse::<u8>(), Ok(1..=12))
                    && number.parse::<u16>().is_ok()
            }
            _ => false,
        };
        anyhow::ensure!(
            valid,
            "desktop release version {version} must be YY.M.N with month 1–12 and build number at most 65535"
        );
    }
    tracing::info!(%version, ?platform, "validated application package identity");

    let mut metadata = BuildMetadata {
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
#[cfg(test)]
mod tests;
