use super::*;
use serde_json::json;

/// Platform selection must not leak Windows package identity into Mac evidence.
#[test]
fn platform_metadata_does_not_claim_distribution_before_verification() {
    let input = json!({"packages":[{"name":"clacks-log","version":"0.1.0"}]});
    let revision = "a".repeat(40);

    for (platform, target, architecture, tool, mode) in [
        (
            PackagePlatform::WindowsX64,
            "x86_64-pc-windows-msvc",
            "x64",
            Some("1.2.0"),
            "unsigned",
        ),
        (
            PackagePlatform::MacosArm64,
            "aarch64-apple-darwin",
            "arm64",
            None,
            "not-established",
        ),
    ] {
        let metadata = prepare(&input, &revision, None, platform).unwrap();
        assert_eq!(metadata.target, target);
        assert_eq!(metadata.architecture, architecture);
        assert_eq!(metadata.tool_version.as_deref(), tool);
        assert_eq!(metadata.signing_mode, mode);
    }
}

#[test]
fn release_tag_must_match_the_application_version() {
    let input = json!({"packages":[{"name":"clacks-log","version":"0.1.0"}]});
    let revision = "a".repeat(40);

    assert!(
        prepare(
            &input,
            &revision,
            Some("v0.2.0"),
            PackagePlatform::WindowsX64
        )
        .is_err()
    );
    assert!(
        prepare(
            &input,
            &revision,
            Some("v0.1.0"),
            PackagePlatform::WindowsX64
        )
        .is_ok()
    );
    assert!(prepare(&input, &revision, None, PackagePlatform::WindowsX64).is_ok());
}

#[test]
fn rc_tags_select_prerelease_delivery_without_changing_the_msi_version() {
    let input = json!({"packages":[{"name":"clacks-log","version":"0.1.0"}]});
    let revision = "a".repeat(40);

    let release = prepare(
        &input,
        &revision,
        Some("v0.1.0"),
        PackagePlatform::WindowsX64,
    )
    .unwrap();
    let candidate = prepare(
        &input,
        &revision,
        Some("v0.1.0-rc.1"),
        PackagePlatform::WindowsX64,
    )
    .unwrap();

    assert!(!release.prerelease);
    assert!(candidate.prerelease);
    assert_eq!(candidate.version, release.version);
    for tag in ["v0.2.0-rc.1", "v0.1.0-rc.", "v0.1.0-rc.latest"] {
        assert!(
            prepare(&input, &revision, Some(tag), PackagePlatform::WindowsX64).is_err(),
            "{tag}"
        );
    }
}
