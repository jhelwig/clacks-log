//! Secret-safe directory diagnoses retained across shared lookup consumers.

use crate::directory::providers::{ProviderError, SafeProviderFailure};

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
    /// Typed reason for diagnostic and consumer decisions.
    pub kind: LookupFailureKind,
    /// Existing secret-safe explanation, suitable for presentation.
    pub message: String,
}

impl LookupFailure {
    /// Preserve an application-owned safe explanation with its actual category.
    pub fn new(kind: LookupFailureKind, message: impl Into<String>) -> Self {
        Self {
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
