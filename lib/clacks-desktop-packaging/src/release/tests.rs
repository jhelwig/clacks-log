//! Promotion selects the existing installers for every distributed platform.

use super::*;

#[test]
fn promotion_selects_both_candidate_installers_and_requires_completeness() -> anyhow::Result<()> {
    let version = ReleaseVersion::parse("26.11.1")?;
    let mut release: Release = serde_json::from_value(serde_json::json!({
        "id": 1,
        "tag_name": "desktop-v26.11.1",
        "draft": false,
        "prerelease": true,
        "assets": [
            {
                "id": 2,
                "name": "Clacks-Log-26.11.1-Windows-x64.msi",
                "state": "uploaded",
                "digest": "sha256:windows"
            },
            {
                "id": 3,
                "name": "Clacks-Log-26.11.1-macOS-Apple-Silicon.dmg",
                "state": "uploaded",
                "digest": "sha256:macos"
            },
            { "id": 4, "name": "unrelated.txt", "state": "uploaded", "digest": null }
        ]
    }))?;

    let files = release.installers(&version)?;
    assert_eq!(files.iter().map(|file| file.id).collect::<Vec<_>>(), [2, 3]);

    release.assets.retain(|asset| asset.id != 3);
    assert!(release.installers(&version).is_err());

    Ok(())
}
