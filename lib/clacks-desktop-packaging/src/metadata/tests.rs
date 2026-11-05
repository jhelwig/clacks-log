use super::*;
use serde_json::json;

#[test]
fn release_tag_must_match_the_application_version() {
    let input = json!({"packages":[{"name":"clacks-log","version":"0.1.0"}]});
    let revision = "a".repeat(40);

    assert!(prepare(&input, &revision, Some("v0.2.0")).is_err());
    assert!(prepare(&input, &revision, Some("v0.1.0")).is_ok());
    assert!(prepare(&input, &revision, None).is_ok());
}

#[test]
fn rc_tags_select_prerelease_delivery_without_changing_the_msi_version() {
    let input = json!({"packages":[{"name":"clacks-log","version":"0.1.0"}]});
    let revision = "a".repeat(40);

    let release = prepare(&input, &revision, Some("v0.1.0")).unwrap();
    let candidate = prepare(&input, &revision, Some("v0.1.0-rc.1")).unwrap();

    assert!(!release.prerelease);
    assert!(candidate.prerelease);
    assert_eq!(candidate.version, release.version);
    for tag in ["v0.2.0-rc.1", "v0.1.0-rc.", "v0.1.0-rc.latest"] {
        assert!(prepare(&input, &revision, Some(tag)).is_err(), "{tag}");
    }
}
