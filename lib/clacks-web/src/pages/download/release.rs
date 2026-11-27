//! The GitHub release fields used to select public desktop downloads.

use super::{DesktopRelease, RELEASES_URL};
use clacks_desktop_release::{PackagePlatform, ReleaseVersion};
use serde::Deserialize;

/// Release fields read from GitHub Latest.
#[derive(Deserialize)]
pub(super) struct GitHubRelease {
    /// Git tag identifying the packaged desktop version.
    tag_name: String,
    /// Whether this release is still unpublished.
    draft: bool,
    /// Whether this release is designated a prerelease.
    prerelease: bool,
    /// Markdown notes published for the release.
    body: Option<String>,
    /// Files attached to the release.
    assets: Vec<Asset>,
}

/// One file attached to a release.
#[derive(Deserialize)]
struct Asset {
    /// Published asset filename.
    name: String,
    /// Direct browser download URL returned by GitHub.
    browser_download_url: String,
}

impl GitHubRelease {
    /// Select assets belonging to a final desktop release with a valid tag.
    pub(super) fn into_desktop_release(self) -> Result<DesktopRelease, ()> {
        if self.draft || self.prerelease {
            return Err(());
        }

        let version = ReleaseVersion::from_tag(&self.tag_name).map_err(|_| ())?;

        Ok(DesktopRelease {
            windows: self.installer(version.as_str(), PackagePlatform::WindowsX64),
            macos: self.installer(version.as_str(), PackagePlatform::MacosArm64),
            version,
            notes: self.body.filter(|body| !body.trim().is_empty()),
        })
    }

    /// Return the link for the expected platform installer at this release tag.
    fn installer(&self, version: &str, platform: PackagePlatform) -> Option<String> {
        let name = platform.installer_name(version, false);
        let expected_url = format!("{RELEASES_URL}/download/{}/{name}", self.tag_name);

        self.assets
            .iter()
            .find(|asset| asset.name == name && asset.browser_download_url == expected_url)
            .map(|asset| asset.browser_download_url.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    use serde_json::json;

    #[test]
    fn selects_installers_for_the_same_release_and_keeps_available_downloads() -> anyhow::Result<()>
    {
        let windows_name = "Clacks-Log-26.11.1-Windows-x64.msi";
        let macos_name = "Clacks-Log-26.11.1-macOS-Apple-Silicon.dmg";
        let windows_url = format!("{RELEASES_URL}/download/desktop-v26.11.1/{windows_name}");
        let macos_url = format!("{RELEASES_URL}/download/desktop-v26.11.1/{macos_name}");
        let mut response = json!({
            "tag_name": "desktop-v26.11.1",
            "draft": false,
            "prerelease": false,
            "body": "Release notes",
            "assets": [
                { "name": "clacks-log-26.11.0-macos-arm64.dmg", "browser_download_url": format!("{RELEASES_URL}/download/desktop-v26.11.0/clacks-log-26.11.0-macos-arm64.dmg") },
                { "name": windows_name, "browser_download_url": windows_url },
                { "name": macos_name, "browser_download_url": macos_url }
            ]
        });

        let parsed: GitHubRelease = serde_json::from_value(response.clone())?;
        let release = parsed
            .into_desktop_release()
            .map_err(|()| anyhow::anyhow!("fixture identifies a final desktop release"))?;

        assert_eq!(release.version.as_str(), "26.11.1");
        assert_eq!(release.windows.as_deref(), Some(windows_url.as_str()));
        assert_eq!(release.macos.as_deref(), Some(macos_url.as_str()));

        response["assets"]
            .as_array_mut()
            .context("fixture assets are an array")?
            .pop();
        let parsed: GitHubRelease = serde_json::from_value(response)?;
        let release = parsed
            .into_desktop_release()
            .map_err(|()| anyhow::anyhow!("removing one installer preserves the final release"))?;

        assert_eq!(release.windows.as_deref(), Some(windows_url.as_str()));
        assert!(release.macos.is_none());

        Ok(())
    }
}
