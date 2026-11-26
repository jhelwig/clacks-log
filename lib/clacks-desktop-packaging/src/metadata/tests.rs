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
fn desktop_tag_validates_the_application_version() {
    let input = json!({"packages":[
        {"name":"clacks-desktop","version":"0.1.0"},
        {"name":"clacks-log","version":"26.11.1"}
    ]});
    let revision = "a".repeat(40);
    let platform = PackagePlatform::WindowsX64;
    let metadata = prepare(&input, &revision, Some("desktop-v26.11.1"), platform).unwrap();
    assert_eq!(metadata.version, "26.11.1");

    for tag in ["desktop-v26.11.2", "sync-server-v26.11.1"] {
        assert!(
            prepare(&input, &revision, Some(tag), platform).is_err(),
            "{tag}"
        );
    }

    for version in ["126.11.1", "26.13.1", "26.11.65536"] {
        let input = json!({"packages":[{"name":"clacks-log","version":version}]});
        assert!(
            prepare(
                &input,
                &revision,
                Some(&format!("desktop-v{version}")),
                platform
            )
            .is_err(),
            "{version}"
        );
    }

    let development = json!({"packages":[{"name":"clacks-log","version":"0.1.0"}]});
    assert_eq!(
        prepare(&development, &revision, None, platform)
            .unwrap()
            .version,
        "0.1.0"
    );
}
