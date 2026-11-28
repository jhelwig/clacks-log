//! Regression coverage for the project's maintenance classification decisions.

use super::evaluate;
use serde_json::{Value, json};

/// Creates resolved Cargo metadata with a renamed edge and a transitive package.
fn metadata(direct: &[&str], source: &str, root: &str, version: &str) -> String {
    let package_id = |name: &str| format!("{source}#{name}@{version}");
    let packages: Vec<Value> = ["old-clock", "adapter"]
        .into_iter()
        .map(|name| {
            json!({
                "id": package_id(name), "name": name, "version": version, "source": source
            })
        })
        .chain(std::iter::once(json!({
            "id": root, "name": "project", "version": "1.0.0", "source": null
        })))
        .collect();
    json!({
        "packages": packages,
        "workspace_members": [root],
        "resolve": {
            "nodes": [
                {
                    "id": root,
                    "dependencies": direct.iter().map(|name| package_id(name)).collect::<Vec<_>>(),
                    "deps": direct.iter().map(|name| json!({
                        "name": "renamed", "pkg": package_id(name),
                        "dep_kinds": [{"kind": "build", "target": "cfg(windows)"}]
                    })).collect::<Vec<_>>()
                },
                {"id": package_id("adapter"), "dependencies": [package_id("old-clock")]},
                {"id": package_id("old-clock"), "dependencies": []}
            ]
        }
    })
    .to_string()
}

/// Models cargo-deny's structured audit report for an unmaintained package.
fn report(source: &str, version: &str) -> String {
    json!({
        "lockfile": {"dependency-count": 3},
        "vulnerabilities": [],
        "warnings": {
            "unmaintained": [{
                "advisory": {"id": "RUSTSEC-2099-0001", "title": "Clock is unmaintained"},
                "package": {"name": "old-clock", "version": version, "source": source}
            }]
        }
    })
    .to_string()
}

const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";

#[test]
fn audit_stream_keeps_later_findings_without_duplicate_warnings() {
    let head = metadata(&["old-clock"], REGISTRY, "head/project", "1.0.0");
    let base = metadata(&["old-clock"], REGISTRY, "base/project", "1.0.0");
    let populated = report(REGISTRY, "1.0.0");
    let mut empty: Value =
        serde_json::from_str(&populated).expect("the authored audit report is valid JSON");
    empty["warnings"] = json!({});
    let stream = format!("{empty}\n{populated}\n{populated}\n");

    let decisions = evaluate(&head, &stream, Some(&base))
        .expect("all audit snapshots resolve the same existing direct package");

    assert_eq!(decisions.len(), 1);
    assert!(!decisions.iter().any(|item| item.blocking));
}

#[test]
fn empty_or_malformed_later_audit_records_fail_evaluation() {
    let head = metadata(&["old-clock"], REGISTRY, "head/project", "1.0.0");
    let valid = report(REGISTRY, "1.0.0");

    for input in [
        String::new(),
        format!("{valid}\n{{"),
        format!("{valid}\n{{}}"),
    ] {
        assert!(evaluate(&head, &input, None).is_err());
    }
}

#[test]
fn new_direct_use_blocks_but_transitive_use_only_warns() {
    let base = metadata(&[], REGISTRY, "base/project", "1.0.0");
    let direct = metadata(&["old-clock"], REGISTRY, "head/project", "1.0.0");
    let transitive = metadata(&["adapter"], REGISTRY, "head/project", "1.0.0");
    let findings = report(REGISTRY, "1.0.0");

    let introduced = evaluate(&direct, &findings, Some(&base))
        .expect("the supplied metadata and audit report have the supported JSON structure");
    assert!(introduced.iter().any(|item| item.blocking));
    let indirect = evaluate(&transitive, &findings, Some(&base))
        .expect("the supplied graph contains old-clock only through adapter");
    assert!(!indirect.iter().any(|item| item.blocking));
}

#[test]
fn promotion_from_transitive_to_direct_blocks() {
    let base = metadata(&["adapter"], REGISTRY, "base/project", "1.0.0");
    let head = metadata(&["adapter", "old-clock"], REGISTRY, "head/project", "1.0.0");

    let decisions = evaluate(&head, &report(REGISTRY, "1.0.0"), Some(&base))
        .expect("both graphs resolve old-clock but only the head uses it directly");
    assert!(decisions.iter().any(|item| item.blocking));
}

#[test]
fn existing_direct_use_warns_across_versions_and_checkout_paths() {
    let base = metadata(&["old-clock"], REGISTRY, "base/project", "1.0.0");
    let head = metadata(&["old-clock"], REGISTRY, "head/project", "2.0.0");

    let decisions = evaluate(&head, &report(REGISTRY, "2.0.0"), Some(&base))
        .expect("the same registry package is directly used in both checkouts");
    assert_eq!(decisions.len(), 1);
    assert!(!decisions.iter().any(|item| item.blocking));
}

#[test]
fn same_name_from_another_source_is_a_new_direct_dependency() {
    let base = metadata(
        &["old-clock"],
        "git+https://example.invalid/fork",
        "base/project",
        "1.0.0",
    );
    let head = metadata(&["old-clock"], REGISTRY, "head/project", "1.0.0");

    let decisions = evaluate(&head, &report(REGISTRY, "1.0.0"), Some(&base))
        .expect("both graphs are valid and intentionally select different sources");
    assert!(decisions.iter().any(|item| item.blocking));
}

#[test]
fn periodic_scans_warn_without_a_pull_request_base() {
    let head = metadata(&["old-clock"], REGISTRY, "head/project", "1.0.0");

    let decisions = evaluate(&head, &report(REGISTRY, "1.0.0"), None)
        .expect("periodic scans deliberately have no base graph");
    assert_eq!(decisions.len(), 1);
    assert!(!decisions.iter().any(|item| item.blocking));
}

#[test]
fn invalid_or_inconsistent_input_cannot_be_reported_as_a_pass() {
    let head = metadata(&["old-clock"], REGISTRY, "head/project", "1.0.0");
    assert!(evaluate(&head, "{}", None).is_err());
    assert!(evaluate("{}", &report(REGISTRY, "1.0.0"), None).is_err());
    assert!(evaluate(&head, &report(REGISTRY, "1.0.0"), Some("invalid")).is_err());
    assert!(evaluate(&head, &report(REGISTRY, "9.9.9"), None).is_err());
}

#[test]
fn affected_transitive_version_warns_when_another_version_is_direct() {
    let base = metadata(&["adapter"], REGISTRY, "base/project", "1.0.0");
    let mut head: Value =
        serde_json::from_str(&metadata(&["adapter"], REGISTRY, "head/project", "1.0.0"))
            .expect("the metadata helper emits valid JSON");
    let new_id = format!("{REGISTRY}#old-clock@2.0.0");
    head["packages"]
        .as_array_mut()
        .expect("the metadata helper supplies a packages array")
        .push(json!({
            "id": new_id, "name": "old-clock", "version": "2.0.0",
            "source": REGISTRY
        }));
    head["resolve"]["nodes"][0]["dependencies"]
        .as_array_mut()
        .expect("the metadata helper's first node is the workspace root")
        .push(json!(new_id));
    head["resolve"]["nodes"]
        .as_array_mut()
        .expect("the metadata helper supplies resolved nodes")
        .push(json!({"id": new_id, "dependencies": []}));

    let decisions = evaluate(&head.to_string(), &report(REGISTRY, "1.0.0"), Some(&base))
        .expect("the affected version is transitive while the direct version is unaffected");
    assert_eq!(decisions.len(), 1);
    assert!(!decisions.iter().any(|item| item.blocking));
}

#[test]
fn direct_use_by_another_workspace_member_counts_as_existing_direct_use() {
    let mut base: Value =
        serde_json::from_str(&metadata(&["adapter"], REGISTRY, "base/project", "1.0.0"))
            .expect("the metadata helper emits valid JSON");
    base["packages"]
        .as_array_mut()
        .expect("the metadata helper supplies a packages array")
        .push(json!({
            "id": "base/second", "name": "second", "version": "1.0.0", "source": null
        }));
    base["workspace_members"]
        .as_array_mut()
        .expect("the metadata helper supplies workspace members")
        .push(json!("base/second"));
    base["resolve"]["nodes"]
        .as_array_mut()
        .expect("the metadata helper supplies resolved nodes")
        .push(json!({
            "id": "base/second",
            "dependencies": [format!("{REGISTRY}#old-clock@1.0.0")]
        }));
    let head = metadata(&["old-clock"], REGISTRY, "head/project", "1.0.0");

    let decisions = evaluate(&head, &report(REGISTRY, "1.0.0"), Some(&base.to_string()))
        .expect("another workspace member directly used the same dependency on the base");
    assert!(!decisions.iter().any(|item| item.blocking));
}
