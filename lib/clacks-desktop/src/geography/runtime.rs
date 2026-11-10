//! Finite geographic opening under the existing session task lifecycle.

use super::{Client, Selection};
use std::path::PathBuf;
use tokio::task::JoinSet;

/// Open the exact startup selection off session receiver and native UI threads.
/// An explicit invalid override never falls back to packaged resources.
pub(crate) fn begin(tasks: &mut JoinSet<anyhow::Result<Client>>, selected: Option<Selection>) {
    let source = if selected.is_some() {
        "override"
    } else {
        "packaged"
    };
    let span = tracing::Span::current();
    let dispatcher = tracing::dispatcher::get_default(Clone::clone);
    tasks.spawn_blocking(move || {
        tracing::dispatcher::with_default(&dispatcher, || {
            span.in_scope(|| {
                let selection = match selected {
                    Some(selection) => selection,
                    None => Selection { path: packaged()?, manifest_sha256: packaged_manifest_sha256()? },
                };
                tracing::info!(event = "geography.bundle_selected", selection = source, manifest_sha256 = %selection.manifest_sha256);
                Client::open(&selection)
            })
        })
    });
}

/// Use the common build output's identity embedded during native compilation.
///
/// Data-free developer builds keep manual work available and can explicitly
/// select external data; packaging supplies this identity before compilation.
pub(crate) fn packaged_manifest_sha256() -> anyhow::Result<String> {
    option_env!("CLACKS_GEOGRAPHY_MANIFEST_SHA256")
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("no geographic data identity supplied at build time"))
}

/// Find installed resources beside the executable, without copying into a workspace.
fn packaged() -> anyhow::Result<PathBuf> {
    let executable = std::env::current_exe()?;
    let parent = executable
        .parent()
        .ok_or_else(|| anyhow::anyhow!("executable has no parent"))?;
    #[cfg(target_os = "macos")]
    let path = parent.join("../Resources/geography");
    #[cfg(not(target_os = "macos"))]
    let path = parent.join("geography");
    Ok(path)
}

/// Preserve useful reader/worker distinctions without emitting raw error or path text.
pub(crate) fn error_class(error: &anyhow::Error) -> &'static str {
    use clacks_geography::error::GeographyError;
    match error.downcast_ref::<GeographyError>() {
        Some(GeographyError::UnsupportedSchema(_)) => "unsupported_schema",
        Some(GeographyError::InvalidArtifact { .. }) => "invalid_artifact",
        Some(GeographyError::InvalidQuery { .. }) => "invalid_query",
        Some(GeographyError::NumericalFailure { .. }) => "numerical_failure",
        Some(GeographyError::Read { source, .. }) => match source.kind() {
            std::io::ErrorKind::NotFound => "file_missing",
            std::io::ErrorKind::PermissionDenied => "file_permission",
            _ => "file_read",
        },
        Some(GeographyError::Decode { .. }) => "artifact_decode",
        None => crate::telemetry::failure::error_class(error),
    }
}
