//! Classifies cargo-deny maintenance findings using resolved Cargo metadata.
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;

/// Package identity across versions and checkout locations.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
struct Identity {
    /// Actual package name, independent of dependency aliases.
    name: String,
    /// Cargo registry or Git source; local packages have no source.
    source: Option<String>,
}

/// Resolved package record.
#[derive(Deserialize)]
struct Package {
    /// Graph-local package identifier.
    id: String,
    /// Package name and source.
    #[serde(flatten)]
    identity: Identity,
    /// Resolved version.
    version: String,
}

/// Graph node with immediate dependencies across dependency kinds and targets.
#[derive(Deserialize)]
struct Node {
    /// Package represented by this node.
    id: String,
    /// Immediate resolved dependency IDs.
    dependencies: Vec<String>,
}

/// Cargo's resolved dependency graph.
#[derive(Deserialize)]
struct Resolve {
    /// Resolved nodes, including workspace roots.
    nodes: Vec<Node>,
}

/// Cargo metadata fields needed for direct-use classification.
#[derive(Deserialize)]
struct Metadata {
    /// Resolved package records.
    packages: Vec<Package>,
    /// Roots whose immediate dependencies count as direct use.
    workspace_members: Vec<String>,
    /// Resolved graph; unresolved metadata is rejected.
    resolve: Resolve,
}
impl Metadata {
    /// Returns resolved direct packages, retaining their affected versions.
    fn direct_dependencies(&self) -> Result<Vec<&Package>, String> {
        if self.workspace_members.is_empty() {
            return Err("metadata has no workspace members".to_owned());
        }
        let mut direct = Vec::new();
        for member in &self.workspace_members {
            let node = self
                .resolve
                .nodes
                .iter()
                .find(|node| &node.id == member)
                .ok_or_else(|| format!("workspace member {member} has no resolved node"))?;
            for id in &node.dependencies {
                let package = self
                    .packages
                    .iter()
                    .find(|package| &package.id == id)
                    .ok_or_else(|| format!("resolved dependency {id} has no package record"))?;
                direct.push(package);
            }
        }
        Ok(direct)
    }
}

/// Advisory details included in an annotation.
#[derive(Deserialize)]
struct Advisory {
    /// RustSec advisory identifier.
    id: String,
    /// Advisory title.
    title: String,
}

/// Scanner-matched affected package.
#[derive(Deserialize)]
struct FindingPackage {
    /// Package name and source.
    #[serde(flatten)]
    identity: Identity,
    /// Affected resolved version.
    version: String,
}

/// Matched advisory and package.
#[derive(Deserialize)]
struct Finding {
    /// Matching advisory.
    advisory: Advisory,
    /// Affected package.
    package: FindingPackage,
}

/// Required audit report envelope, separating empty results from invalid input.
#[derive(Deserialize)]
struct Report {
    /// Required scan summary.
    #[serde(rename = "lockfile")]
    _lockfile: Lockfile,
    /// Vulnerabilities remain governed by cargo-deny's exit status.
    #[serde(rename = "vulnerabilities")]
    _vulnerabilities: Vec<serde_json::Value>,
    /// Informational findings grouped by advisory kind.
    warnings: BTreeMap<String, Vec<Finding>>,
}

/// Required lockfile summary in cargo-deny's output.
#[derive(Deserialize)]
struct Lockfile {
    /// Number of dependencies inspected.
    #[serde(rename = "dependency-count")]
    _dependency_count: usize,
}

/// One unmaintained finding with its pull-request outcome.
#[derive(Debug)]
pub struct Decision {
    /// Whether this finding introduces an unmaintained direct dependency.
    pub blocking: bool,
    /// Package, advisory and classification for reporting.
    pub message: String,
}

/// Classifies findings against direct dependency identities on the PR base.
///
/// Transitive findings always warn. Direct findings block only when a base is
/// supplied and the package identity was not directly used on that base.
/// Version changes do not change identity. All records in the audit JSON stream
/// are read, and repeated package/advisory findings appear once. Empty, malformed
/// or inconsistent inputs return an error rather than a passing classification.
pub fn evaluate(head: &str, report: &str, base: Option<&str>) -> Result<Vec<Decision>, String> {
    let head: Metadata =
        serde_json::from_str(head).map_err(|error| format!("invalid head metadata: {error}"))?;
    let reports = serde_json::Deserializer::from_str(report)
        .into_iter::<Report>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("invalid cargo-deny report: {error}"))?;
    if reports.is_empty() {
        return Err("cargo-deny report contains no audit records".to_owned());
    }
    let head_direct = head.direct_dependencies()?;
    let base_direct = base
        .map(|json| {
            let metadata: Metadata = serde_json::from_str(json)
                .map_err(|error| format!("invalid base metadata: {error}"))?;
            Ok::<_, String>(
                metadata
                    .direct_dependencies()?
                    .into_iter()
                    .map(|package| package.identity.clone())
                    .collect::<BTreeSet<_>>(),
            )
        })
        .transpose()?;

    let mut decisions = Vec::new();
    let mut seen = BTreeSet::new();
    for finding in reports
        .iter()
        .filter_map(|report| report.warnings.get("unmaintained"))
        .flatten()
    {
        let package = &finding.package;
        if !head.packages.iter().any(|resolved| {
            resolved.identity == package.identity && resolved.version == package.version
        }) {
            return Err(format!(
                "scanner package {} {} is absent from head metadata",
                package.identity.name, package.version
            ));
        }
        if !seen.insert((&package.identity, &package.version, &finding.advisory.id)) {
            continue;
        }
        let direct = head_direct.iter().any(|resolved| {
            resolved.identity == package.identity && resolved.version == package.version
        });
        let blocking = direct
            && base_direct
                .as_ref()
                .is_some_and(|base| !base.contains(&package.identity));
        let reason = if blocking {
            "new direct dependency (including promotion from transitive use)"
        } else if !direct {
            "transitive dependency; maintenance status does not block"
        } else if base_direct.is_some() {
            "direct dependency already present on PR base"
        } else {
            "direct dependency; maintenance monitoring without a PR base"
        };
        decisions.push(Decision {
            blocking,
            message: format!(
                "{} {}: {} — {} ({reason})",
                package.identity.name, package.version, finding.advisory.id, finding.advisory.title
            ),
        });
    }
    Ok(decisions)
}

/// Reads an input file with path context on failure.
fn read(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("cannot read {path}: {error}"))
}

/// Escapes a message for a GitHub workflow annotation.
fn annotation(message: &str) -> String {
    message
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// Reads HEAD_METADATA HEAD_REPORT [BASE_METADATA] and reports the policy result.
///
/// Omitting the base provides non-blocking maintenance monitoring. Input/output
/// failures return errors, distinct from a completed policy evaluation.
pub fn run() -> Result<bool, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (head_path, report_path, base_path) = match args.as_slice() {
        [head, report] => (head, report, None),
        [head, report, base] => (head, report, Some(base)),
        _ => {
            return Err(
                "usage: dependency-policy-cli HEAD_METADATA HEAD_REPORT [BASE_METADATA]".to_owned(),
            );
        }
    };
    let head = read(head_path)?;
    let report = read(report_path)?;
    let base = base_path.map(|path| read(path)).transpose()?;
    let decisions = evaluate(&head, &report, base.as_deref())?;
    let mut summary = String::from("### Dependency maintenance\n\n");
    for decision in &decisions {
        let level = if decision.blocking {
            "error"
        } else {
            "warning"
        };
        println!("::{level}::{}", annotation(&decision.message));
        summary.push_str(&format!(
            "- **{level}**: {}\n",
            decision.message.replace(['\r', '\n'], " ")
        ));
    }
    if decisions.is_empty() {
        println!("No unmaintained dependency findings.");
        summary.push_str("No unmaintained dependency findings.\n");
    }
    if let Some(path) = std::env::var_os("GITHUB_STEP_SUMMARY") {
        let mut file = OpenOptions::new()
            .append(true)
            .open(&path)
            .map_err(|error| format!("cannot open GitHub summary {path:?}: {error}"))?;
        file.write_all(summary.as_bytes())
            .map_err(|error| format!("cannot write GitHub summary: {error}"))?;
    }
    let blocking_count = decisions
        .iter()
        .filter(|decision| decision.blocking)
        .count();
    let warning_count = decisions.len() - blocking_count;
    if let Some(path) = std::env::var_os("GITHUB_OUTPUT") {
        let mut file = OpenOptions::new()
            .append(true)
            .open(&path)
            .map_err(|error| format!("cannot open GitHub outputs {path:?}: {error}"))?;
        writeln!(
            file,
            "blocking_count={blocking_count}\nwarning_count={warning_count}"
        )
        .map_err(|error| format!("cannot write GitHub outputs: {error}"))?;
    }
    Ok(blocking_count == 0)
}

#[cfg(test)]
mod tests;
