use super::*;
use serde_json::json;

const REVISION: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn latest_results() -> Value {
    json!({
        "databaseId":2,"attempt":2,"headSha":REVISION,
        "status":"completed","conclusion":"success",
        "url":"https://github.com/jhelwig/clacks-log/actions/runs/2",
        "jobs":[
            {"name":"verify (ubuntu-latest)","status":"completed","conclusion":"success","url":"https://github.com/jhelwig/clacks-log/actions/runs/2/jobs/3"},
            {"name":"verify-web","status":"completed","conclusion":"success","url":"https://github.com/jhelwig/clacks-log/actions/runs/2/jobs/4"}
        ]
    })
}

#[test]
fn pending_or_unsuccessful_latest_results_do_not_qualify() {
    for (status, conclusion) in [
        ("queued", ""),
        ("in_progress", ""),
        ("completed", "failure"),
        ("completed", "cancelled"),
    ] {
        let mut run = latest_results();
        run["status"] = json!(status);
        run["conclusion"] = json!(conclusion);

        assert!(qualify(&run, REVISION).is_err(), "{status}/{conclusion}");
    }
    assert!(qualify(&latest_results(), REVISION).is_ok());
}

#[test]
fn release_requires_both_linux_and_web_success() {
    for name in ["verify (ubuntu-latest)", "verify-web"] {
        for conclusion in ["failure", "skipped", "cancelled"] {
            let mut run = latest_results();
            let job = run["jobs"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|job| job["name"] == name)
                .unwrap();
            job["conclusion"] = json!(conclusion);

            assert!(qualify(&run, REVISION).is_err(), "{name}/{conclusion}");
        }
        let mut run = latest_results();
        run["jobs"]
            .as_array_mut()
            .unwrap()
            .retain(|job| job["name"] != name);

        assert!(qualify(&run, REVISION).is_err(), "missing {name}");
    }
}
