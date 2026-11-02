//! Native versioned WebSocket messages; connection correlation never establishes causality.

use super::ClientIdentity;
use crate::{
    acceptance::RejectionReason,
    account::{Baseline, InstalledCheckpoint},
    client::{PreparedChange, ReviewInput},
    lineage::Lineage,
    record::{AcceptedRecord, Observation},
    transfer::discovery::{DiscoveryEnd, DiscoveryPage, DiscoveryStart},
};
use serde::{Deserialize, Serialize};

/// Initial native synchronization wire contract, independent of application versions.
pub const VERSION: u16 = 1;

/// Common discovery messages used in either direction over the authenticated hub connection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferMessage {
    /// Request missing accepted state from the peer's complete coverage.
    Request {
        /// Receiver's actual completed view, absent before initial receipt.
        after: Option<InstalledCheckpoint>,
    },
    /// Begin discovering a fixed complete target.
    Start(DiscoveryStart),
    /// Transfer native obligations and accepted input.
    Page(DiscoveryPage),
    /// Certify source enumeration exhaustion.
    End(DiscoveryEnd),
    /// Acknowledge one durably received page, without claiming account coverage.
    PageAck {
        /// Exact page sequence committed.
        sequence: u64,
    },
    /// Report actual complete coverage after durable completion.
    CoverageAck(InstalledCheckpoint),
    /// Supply individually retained accepted facts without claiming a complete prefix.
    Facts(DiscoveryPage),
    /// End or acknowledge an accepted-facts response; no account coverage is certified.
    FactsEnd,
    /// Retain incomplete accepted input while continuing unrelated exchanges.
    Deferred,
}

/// Committed server response to a native pending proposal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalOutcome {
    /// Newly accepted value whose transaction has committed.
    Accepted(AcceptedRecord),
    /// Existing accepted context already represents the proposal.
    Covered(AcceptedRecord),
    /// Compared shared state changed and must be incorporated before preparation.
    Stale(Observation),
    /// Current authority, references or service conditions forbid this proposal.
    Rejected(RejectionReason),
}

/// Messages sent by the native application over its authorized account connection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClientMessage {
    /// Negotiate before any state-changing exchange.
    Hello {
        /// Required protocol contract.
        version: u16,
        /// Previously completed coverage retained by this workspace.
        checkpoint: Option<InstalledCheckpoint>,
        /// Retained interpretation offered for recovery negotiation.
        baselines: Vec<Baseline>,
        /// Genuine retained timeline ancestry, absent before initial receipt.
        lineage: Option<Lineage>,
    },
    /// Supply or request accepted state using the common transfer procedure.
    Transfer {
        /// Connection-local exchange number, not a durable history identity.
        exchange: u64,
        /// Transfer operation.
        message: TransferMessage,
    },
    /// Submit an already durably frozen pending proposal.
    Propose {
        /// Connection-local request number.
        request: u64,
        /// Existing native frozen change, including comparison conditions.
        change: PreparedChange,
    },
}

impl ClientMessage {
    /// Connection-local transfer identity when this message belongs to an exchange.
    pub fn exchange_id(&self) -> Option<u64> {
        match self {
            Self::Transfer { exchange, .. } => Some(*exchange),
            Self::Hello { .. } | Self::Propose { .. } => None,
        }
    }

    /// Bounded synchronization phase represented by this message, without payloads.
    pub fn diagnostic_phase(&self) -> &'static str {
        match self {
            Self::Hello { .. } => "negotiation",
            Self::Propose { .. } => "publication",
            Self::Transfer { .. } => "transfer",
        }
    }
}

/// Messages sent by the hub after current native authorization.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerMessage {
    /// Describe authoritative membership and real accepted coverage.
    Hello {
        /// Selected compatible protocol contract.
        version: u16,
        /// Account/actor derived from verified authorization.
        identity: ClientIdentity,
        /// Current complete accepted coverage.
        checkpoint: InstalledCheckpoint,
        /// Retained baseline interpretations.
        baselines: Vec<Baseline>,
        /// Retained position-dependent history ancestry.
        lineage: Lineage,
    },
    /// Transfer accepted state in either direction.
    Transfer {
        /// Connection-local exchange number.
        exchange: u64,
        /// Transfer operation.
        message: TransferMessage,
    },
    /// Return a committed proposal outcome.
    Proposal {
        /// Original connection-local request number.
        request: u64,
        /// Committed native result.
        outcome: ProposalOutcome,
    },
    /// Return already-accepted divergence for client-owned resolution.
    Review {
        /// Existing comparison inputs, preserved locally by the receiver.
        input: Box<ReviewInput>,
    },
    /// Wake the peer to reassess durable state; this is not coverage evidence.
    Changed,
    /// Report an actionable failure without including secret or payload dumps.
    Failure {
        /// Bounded operational category.
        kind: FailureKind,
        /// Safe diagnostic supplied by the handling boundary.
        message: String,
    },
}

/// Operational failure categories for native exchange handling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureKind {
    /// Client/server wire contracts are incompatible.
    Incompatible,
    /// Message scope, sequence or native contents are invalid.
    Invalid,
    /// Current authority no longer admits this connection's work.
    Unauthorized,
    /// Storage or transport handling failed without claiming completion.
    Unavailable,
}
