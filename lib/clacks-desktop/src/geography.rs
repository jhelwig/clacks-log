//! One session-owned geographic reader shared by editor and capture operations.

pub(crate) mod runtime;
#[cfg(test)]
mod tests;

use clacks_geography::{artifact::read::Bundle, lookup::LookupResult};
use clacks_model::position::Position;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{Mutex, oneshot};

/// Artifact identity supplied independently of the selected bundle directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Selection {
    /// Directory containing the immutable selected release.
    pub path: PathBuf,
    /// Expected manifest digest from package inputs or explicit configuration.
    pub manifest_sha256: String,
}

/// Cloneable access to the session's single identity-checked immutable release.
#[derive(Clone)]
pub(crate) struct Client {
    /// Exclusive reader access, acquired before occupying a blocking thread.
    bundle: Arc<Mutex<Bundle>>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GeographyClient")
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Open the exact selected artifact on a finite blocking startup operation.
    pub(crate) fn open(selection: &Selection) -> anyhow::Result<Self> {
        let bundle = clacks_geography::artifact::read::open_bundle(
            &selection.path,
            &selection.manifest_sha256,
        )?;
        tracing::info!(event="geography.bundle_opened",release_id=%bundle.manifest().release_id,schema_version=bundle.manifest().schema_version);
        Ok(Self {
            bundle: Arc::new(Mutex::new(bundle)),
        })
    }

    /// Query native evidence, preserving the full area of a grid-derived position.
    /// Await any started blocking read before returning, even for abandoned editors.
    pub(crate) async fn lookup(
        &self,
        position: Position,
    ) -> Result<LookupResult, GeographyFailure> {
        self.lookup_interested(position, "capture", || true).await
    }

    /// Recheck editor interest after async admission and before starting a blocking read.
    #[tracing::instrument(skip_all, name = "geography.request", level = "debug", fields(consumer = context))]
    async fn lookup_interested(
        &self,
        position: Position,
        context: &'static str,
        interested: impl FnOnce() -> bool,
    ) -> Result<LookupResult, GeographyFailure> {
        let mut bundle = self.bundle.clone().lock_owned().await;
        let release_id = bundle.manifest().release_id.clone();
        if !interested() {
            tracing::debug!(event="geography.request_obsolete", consumer=context, %release_id, phase="before_read");
            return Err(GeographyFailure::Closing);
        }
        let span = tracing::Span::current();
        let dispatcher = tracing::dispatcher::get_default(Clone::clone);
        let result = tokio::task::spawn_blocking(move || {
            tracing::dispatcher::with_default(&dispatcher, || {
                span.in_scope(|| bundle.lookup(position.lookup_geometry()))
            })
        })
        .await
        .map_err(anyhow::Error::from)
        .and_then(|result| result.map_err(anyhow::Error::from));
        result.map_err(|cause| {
            tracing::warn!(event="geography.query_failed", consumer=context, %release_id, error.type=runtime::error_class(&cause));
            GeographyFailure::QueryFailed(Arc::new(cause))
        })
    }
}

/// Safe presentation category retaining query causes for diagnostic handling.
#[derive(Clone, Debug)]
pub(crate) enum GeographyFailure {
    /// Initial bundle validation has not yet settled.
    Opening,
    /// The selected startup bundle could not be opened or validated.
    Unavailable,
    /// Session shutdown rejects new work.
    Closing,
    /// A reader operation failed; this is distinct from successful no coverage.
    QueryFailed(Arc<anyhow::Error>),
}

impl std::fmt::Display for GeographyFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

impl std::error::Error for GeographyFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::QueryFailed(cause) => Some(cause.as_ref().as_ref()),
            _ => None,
        }
    }
}

impl GeographyFailure {
    /// Return bounded user-facing text without paths or position evidence.
    pub(crate) fn message(&self) -> &'static str {
        match self {
            Self::Opening => "Geographic assistance is opening.",
            Self::Unavailable => {
                "Geographic assistance is unavailable. Check the selected bundle and restart."
            }
            Self::Closing => "The application is closing.",
            Self::QueryFailed(_) => {
                "Geographic lookup failed. Fields can still be entered manually."
            }
        }
    }

    /// Return a bounded diagnostic category without the original cause.
    pub(crate) fn category(&self) -> &'static str {
        match self {
            Self::Opening => "opening",
            Self::Unavailable => "unavailable",
            Self::Closing => "closing",
            Self::QueryFailed(_) => "query_failed",
        }
    }
}

/// Independent startup status; absent assistance never prevents workspace use.
#[derive(Clone, Debug, Default)]
pub(crate) enum GeographyStatus {
    /// Selected bundle validation remains in progress.
    #[default]
    Opening,
    /// Validated shared reader for editors and automatic capture.
    Ready(Client),
    /// Bounded startup diagnosis; opening owner handles original causes once.
    Unavailable(GeographyFailure),
}

/// A finite editor request admitted through the existing session command channel.
#[derive(Debug)]
pub(crate) struct GeographyOperation {
    /// Bounded originating editor and station role, without entered values.
    pub context: &'static str,
    /// Checked native position whose whole evidence is queried.
    pub position: Position,
    /// Owned response interest; dropping it abandons the editor request.
    pub reply: oneshot::Sender<Result<LookupResult, GeographyFailure>>,
}

impl GeographyOperation {
    /// Skip abandoned work before admission, then settle any started reader query.
    pub(crate) async fn run(self, client: Client) {
        if self.reply.is_closed() {
            tracing::debug!(
                event = "geography.request_obsolete",
                consumer = self.context,
                phase = "before_admission"
            );
            return;
        }
        let result = client
            .lookup_interested(self.position, self.context, || !self.reply.is_closed())
            .await;
        if let Err(result) = self.reply.send(result) {
            let release_id = result
                .as_ref()
                .ok()
                .map(|result| result.release_id.as_str());
            tracing::debug!(
                event = "geography.request_obsolete",
                consumer = self.context,
                release_id,
                phase = "after_read"
            );
        }
    }

    /// Explicitly settle rejected admission without leaving an unresolved receiver.
    pub(crate) fn reject(self, failure: GeographyFailure) {
        tracing::debug!(
            event = "geography.request_rejected",
            consumer = self.context,
            reason = failure.category()
        );
        let _ = self.reply.send(Err(failure));
    }
}
