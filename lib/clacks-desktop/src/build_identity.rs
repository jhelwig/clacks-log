//! Compile-time application identity for diagnostics and package verification.

use std::fmt;

/// The version and available provenance embedded in this desktop build.
pub(crate) const BUILD_IDENTITY: BuildIdentity = BuildIdentity {
    version: env!("CLACKS_DESKTOP_VERSION"),
    revision: option_env!("CLACKS_BUILD_REVISION"),
    workflow_run: option_env!("CLACKS_BUILD_RUN_ID"),
    workflow_attempt: option_env!("CLACKS_BUILD_RUN_ATTEMPT"),
    dirty: option_env!("CLACKS_BUILD_DIRTY"),
};

/// Version and provenance of the application executable.
pub(crate) struct BuildIdentity {
    /// Numeric application version read from the executable package.
    pub version: &'static str,
    /// Source commit when provided by the build.
    revision: Option<&'static str>,
    /// GitHub Actions run when built by the packaging workflow.
    workflow_run: Option<&'static str>,
    /// Attempt within that workflow run.
    workflow_attempt: Option<&'static str>,
    /// Working-tree state when the build supplies it.
    dirty: Option<&'static str>,
}

impl BuildIdentity {
    /// Format known provenance for About and command-line diagnostics.
    pub(crate) fn provenance(&self) -> Option<String> {
        let mut lines = Vec::new();
        for (label, value) in [
            ("Source commit", self.revision),
            ("Workflow run", self.workflow_run),
            ("Workflow attempt", self.workflow_attempt),
            (
                "Working tree",
                match self.dirty {
                    Some("true") => Some("dirty"),
                    Some("false") => Some("clean"),
                    _ => None,
                },
            ),
        ] {
            if let Some(value) = value {
                lines.push(format!("{label}: {value}"));
            }
        }
        (!lines.is_empty()).then(|| lines.join("\n"))
    }
}

impl fmt::Display for BuildIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.version)?;
        if let Some(provenance) = self.provenance() {
            write!(formatter, "\n{provenance}")?;
        }
        Ok(())
    }
}
