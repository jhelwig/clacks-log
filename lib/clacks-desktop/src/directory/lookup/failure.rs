//! Secret-safe directory diagnoses retained across shared lookup consumers.

use super::LookupClient;
use crate::directory::providers::{ProviderError, SafeProviderFailure};
use std::{
    sync::{Arc, Weak},
    time::Duration,
};

/// Weak identity of the actual installed configuration; retains no credential material.
pub(crate) type LookupOrigin = Weak<()>;

/// Current shared state that prevents new network admission, after ordinary cache sharing.
#[derive(Clone, Debug, thiserror::Error)]
pub(crate) enum AdmissionBlock {
    /// Workspace shutdown closed new admission.
    #[error("The workspace is closing")]
    Closed,
    /// No provider/material is selected.
    #[error("No usable directory provider is configured")]
    NotConfigured,
    /// Installed configuration is unreadable or invalid.
    #[error("{message}")]
    Configuration {
        /// Safe installation diagnostic.
        message: String,
    },
    /// The actual current material requires explicit recovery.
    #[error("Directory credentials were rejected")]
    Authentication,
    /// A provider-requested quiet period still applies.
    #[error("Directory rate limit reached; retry after at least {remaining:?}")]
    RateLimited {
        /// Remaining monotonic provider delay at this observation.
        remaining: Duration,
    },
    /// The lookup state cannot be accessed.
    #[error("Directory lookup state is unavailable")]
    Internal,
}

impl LookupClient {
    /// Explain current network admission without bypassing ordinary cached/shared results.
    pub fn admission_block(&self) -> Option<AdmissionBlock> {
        let Ok(state) = self.inner.state.lock() else {
            return Some(AdmissionBlock::Internal);
        };
        if state.closed {
            return Some(AdmissionBlock::Closed);
        }
        match &state.selected {
            Err(message) => Some(AdmissionBlock::Configuration {
                message: message.clone(),
            }),
            Ok(None) => Some(AdmissionBlock::NotConfigured),
            Ok(Some(selected)) if selected.paused => Some(AdmissionBlock::Authentication),
            Ok(Some(selected)) => selected.retry_after.and_then(|(started, delay)| {
                let remaining = delay.saturating_sub(started.elapsed());
                (!remaining.is_zero()).then_some(AdmissionBlock::RateLimited { remaining })
            }),
        }
    }

    /// Distinguish an obsolete response even when replacement reused durable version metadata.
    pub fn is_current_origin(&self, origin: &LookupOrigin) -> bool {
        self.inner.state.lock().is_ok_and(|state| matches!(&state.selected, Ok(Some(selected)) if Weak::ptr_eq(origin, &Arc::downgrade(&selected.origin))))
    }
}

/// Application-relevant failure category, independent of display text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LookupFailureKind {
    /// Connection could not complete.
    Transport,
    /// Response could not be decoded.
    Parse,
    /// Provider subscription excludes directory access.
    Subscription,
    /// Provider refused the operation.
    Refusal,
    /// Response was oversized or inconsistent.
    InvalidResponse,
    /// Returned facts could not form a valid native snapshot.
    InvalidFacts,
    /// Provider requested fewer requests or its quiet period is still active.
    RateLimited,
    /// Installed directory configuration cannot be used.
    Configuration,
    /// Workspace lookup resources are unavailable.
    Internal,
}

/// Safe user explanation; no provider bodies, URLs or credential values.
#[derive(Clone, Debug)]
pub(crate) struct LookupFailure {
    /// Weak configuration identity for an actual request or current quiet-period refusal.
    pub origin: Option<LookupOrigin>,
    /// Typed reason for diagnostic and consumer decisions.
    pub kind: LookupFailureKind,
    /// Existing secret-safe explanation, suitable for presentation.
    pub message: String,
}

impl LookupFailure {
    /// Preserve an application-owned safe explanation with its actual category.
    pub fn new(kind: LookupFailureKind, message: impl Into<String>) -> Self {
        Self {
            origin: None,
            kind,
            message: message.into(),
        }
    }

    /// Drop internal transport sources while retaining the provider's safe diagnosis.
    pub fn from_provider(error: &ProviderError) -> Self {
        let kind = match error {
            ProviderError::Authentication => LookupFailureKind::Configuration,
            ProviderError::RateLimited { .. } => LookupFailureKind::RateLimited,
            ProviderError::Unavailable { reason, .. } => match reason {
                SafeProviderFailure::Transport => LookupFailureKind::Transport,
                SafeProviderFailure::Parse => LookupFailureKind::Parse,
                SafeProviderFailure::Subscription => LookupFailureKind::Subscription,
                SafeProviderFailure::Refusal => LookupFailureKind::Refusal,
                SafeProviderFailure::InvalidResponse => LookupFailureKind::InvalidResponse,
            },
        };
        Self::new(kind, error.to_string())
    }
}

impl std::fmt::Display for LookupFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}
