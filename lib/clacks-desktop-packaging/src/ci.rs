//! Eligibility from existing Actions results, without dispatching verification.
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Successful existing verification results used for a release.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CiEvidence {
    /// Exact source commit covered by verification.
    pub revision: String,
    /// GitHub run identifier.
    pub run_id: u64,
    /// Latest attempt reported by GitHub.
    pub attempt: u64,
    /// Existing run URL.
    pub url: String,
    /// Required job names, URLs and conclusions.
    pub jobs: Vec<Value>,
}

/// Decide release eligibility from the latest run, including pending results.
#[tracing::instrument(skip_all, fields(revision))]
fn qualify(run: &Value, revision: &str) -> anyhow::Result<CiEvidence> {
    let url = crate::text(run, "url")?;
    anyhow::ensure!(
        crate::text(run, "headSha")? == revision,
        "existing verification covers a different source commit: {url}"
    );
    anyhow::ensure!(
        run.get("status").and_then(Value::as_str) == Some("completed")
            && run.get("conclusion").and_then(Value::as_str) == Some("success"),
        "latest existing Rust run has not completed successfully: {url}"
    );
    let jobs = run
        .get("jobs")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("missing existing job results: {url}"))?;
    let mut qualified = Vec::new();
    for name in ["verify (ubuntu-latest)", "verify-web"] {
        let job = jobs
            .iter()
            .find(|job| job.get("name").and_then(Value::as_str) == Some(name))
            .ok_or_else(|| anyhow::anyhow!("missing required job {name}: {url}"))?;
        anyhow::ensure!(
            job.get("status").and_then(Value::as_str) == Some("completed")
                && job.get("conclusion").and_then(Value::as_str) == Some("success"),
            "required job {name} has not completed successfully: {url}"
        );
        qualified.push(
            serde_json::json!({"name":name,"conclusion":"success","url":crate::text(job,"url")?}),
        );
    }
    let run_id = crate::number(run, "databaseId")?;
    let attempt = crate::number(run, "attempt")?;
    tracing::info!(run_id, attempt, %revision, "qualified existing Linux/web results");
    Ok(CiEvidence {
        revision: revision.into(),
        run_id,
        attempt,
        url: url.into(),
        jobs: qualified,
    })
}

/// Select the newest push verification for the exact source commit.
fn latest_run(repository: &str, revision: &str) -> anyhow::Result<u64> {
    let runs = crate::command_json(
        "gh",
        &[
            "run".into(),
            "list".into(),
            "--repo".into(),
            repository.into(),
            "--workflow".into(),
            "rust.yml".into(),
            "--commit".into(),
            revision.into(),
            "--event".into(),
            "push".into(),
            "--limit".into(),
            "1".into(),
            "--json".into(),
            "databaseId".into(),
        ],
    )?;
    let selected = runs.as_array().and_then(|runs| runs.first())
        .ok_or_else(|| anyhow::anyhow!("no existing Rust push workflow result for exact commit {revision}; no checks were dispatched"))?;
    crate::number(selected, "databaseId")
}

/// Query the newest exact-commit push run and its current latest results.
///
/// GitHub CLI owns workflow/commit filtering, result ordering, pagination and
/// job retrieval. No success filter or historical fallback is applied.
#[tracing::instrument(skip_all, fields(repository, revision))]
pub(crate) fn gate(repository: &str, revision: &str) -> anyhow::Result<CiEvidence> {
    let id = latest_run(repository, revision)?;
    // Omitting --attempt requests the latest attempt, including one started
    // since listing runs; do not request an older successful attempt.
    let run = crate::command_json(
        "gh",
        &[
            "run".into(),
            "view".into(),
            id.to_string().into(),
            "--repo".into(),
            repository.into(),
            "--json".into(),
            "databaseId,attempt,headSha,status,conclusion,url,jobs".into(),
        ],
    )?;
    qualify(&run, revision)
}

#[cfg(test)]
mod tests;
