//! The GitHub release fields used to select public desktop downloads.

use super::{DesktopRelease, RELEASES_URL};
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

        let version = self.tag_name.strip_prefix("desktop-v").ok_or(())?;
        let mut parts = version.split('.');
        let (Some(year), Some(month), Some(number), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(());
        };
        if year.len() != 2
            || ![year, month, number].iter().all(|part| {
                part.bytes().all(|byte| byte.is_ascii_digit())
                    && (part.len() == 1 || !part.starts_with('0'))
            })
            || !matches!(month.parse::<u8>(), Ok(1..=12))
            || number.parse::<u16>().is_err()
        {
            return Err(());
        }

        Ok(DesktopRelease {
            windows: self.installer(version, "windows-x64.msi"),
            macos: self.installer(version, "macos-arm64.dmg"),
            version: version.to_owned(),
            notes: self.body.filter(|body| !body.trim().is_empty()),
        })
    }

    /// Return the link for the expected platform installer at this release tag.
    fn installer(&self, version: &str, platform: &str) -> Option<String> {
        let name = format!("clacks-log-{version}-{platform}");
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
    use serde_json::json;

    #[test]
    fn selects_installers_for_the_same_release_and_keeps_available_downloads() {
        let windows_name = "clacks-log-26.11.1-windows-x64.msi";
        let macos_name = "clacks-log-26.11.1-macos-arm64.dmg";
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

        let parsed: GitHubRelease = serde_json::from_value(response.clone()).unwrap();
        let release = parsed.into_desktop_release().unwrap();
        assert_eq!(release.version, "26.11.1");
        assert_eq!(release.windows.as_deref(), Some(windows_url.as_str()));
        assert_eq!(release.macos.as_deref(), Some(macos_url.as_str()));

        response["assets"].as_array_mut().unwrap().pop();
        let parsed: GitHubRelease = serde_json::from_value(response).unwrap();
        let release = parsed.into_desktop_release().unwrap();
        assert_eq!(release.windows.as_deref(), Some(windows_url.as_str()));
        assert!(release.macos.is_none());
    }
}
