//! Frozen service submissions used by actual attempts and later coverage comparisons.

use crate::ids::DestinationId;
use crate::service::{destination::Service, lotw::LotwSubmission, qrz::QrzSubmission};
use serde::{Deserialize, Serialize};

/// Complete prepared semantic values for one service destination.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubmissionProjection {
    /// Shared normalized signing input for LoTW.
    Lotw(LotwSubmission),
    /// Ordinary QRZ ADIF field set.
    Qrz(QrzSubmission),
}

impl SubmissionProjection {
    /// Return the collection whose values this submission covers.
    pub fn destination(&self) -> DestinationId {
        match self {
            Self::Lotw(v) => v.destination,
            Self::Qrz(v) => v.destination,
        }
    }

    /// Return the provider without interpreting the mode or destination configuration.
    pub fn service(&self) -> Service {
        match self {
            Self::Lotw(_) => Service::Lotw,
            Self::Qrz(_) => Service::Qrz,
        }
    }
}
