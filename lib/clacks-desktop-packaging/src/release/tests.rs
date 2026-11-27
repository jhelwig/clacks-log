//! Project-owned version preparation and promotion file selection.

use super::*;

#[test]
fn preparation_changes_only_the_selected_application_version() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let manifest = directory.path().join("Cargo.toml");
    let original = "[package]\nname = \"clacks-log\"\nversion = \"0.1.0\" # desktop\n\n[dependencies]\nclacks-desktop = { path = \"../../lib/clacks-desktop\" }\n";
    std::fs::write(&manifest, original)?;

    let preparation = set_desktop_version(&manifest, "26.11.1")?;
    assert!(preparation.changed);
    assert_eq!(preparation.branch, "release/desktop-v26.11.1");
    assert_eq!(
        std::fs::read_to_string(&manifest)?,
        original.replacen("version = \"0.1.0\"", "version = \"26.11.1\"", 1)
    );
    let preparation = set_desktop_version(&manifest, "26.11.1")?;
    assert!(!preparation.changed);
    assert_eq!(preparation.branch, "release/desktop-v26.11.1");

    Ok(())
}

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
