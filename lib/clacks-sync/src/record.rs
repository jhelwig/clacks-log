//! Account-qualified native records and their accepted version context.

use crate::{
    clock::{ClockActor, RecordVector},
    error::MetadataError,
};
use clacks_model::ids::{
    AccountId, BaselineId, EquipmentConfigurationId, GroupId, OutputProfileId, ProfileId, QsoId,
    TagId, TimelineId,
};
use clacks_model::{
    contact::Qso,
    export::OutputProfile,
    group::GroupDefinition,
    profile::{EquipmentConfiguration, SavedLocationProfile},
    tag::LogbookTag,
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable kind-qualified identity; deletion does not release it for reuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RecordId {
    /// Synchronized service evidence or maintained service state.
    Service(clacks_model::service::evidence::ServiceRecordId),
    /// Contact identity.
    Qso(QsoId),
    /// Logbook tag identity.
    Tag(TagId),
    /// Saved location profile identity.
    Profile(ProfileId),
    /// Reusable equipment identity.
    Equipment(EquipmentConfigurationId),
    /// Reference group identity.
    Group(GroupId),
    /// Export context identity.
    OutputProfile(OutputProfileId),
}

/// Format the contained native identifier in its canonical text form.
impl fmt::Display for RecordId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Service(id) => fmt::Display::fmt(id, f),
            Self::Qso(id) => fmt::Display::fmt(id, f),
            Self::Tag(id) => fmt::Display::fmt(id, f),
            Self::Profile(id) => fmt::Display::fmt(id, f),
            Self::Equipment(id) => fmt::Display::fmt(id, f),
            Self::Group(id) => fmt::Display::fmt(id, f),
            Self::OutputProfile(id) => fmt::Display::fmt(id, f),
        }
    }
}

/// A native identity qualified by its owning account.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RecordKey {
    /// Account that owns the record.
    pub account: AccountId,
    /// Native identity and kind.
    pub id: RecordId,
}

/// Native synchronized payload without a duplicate synchronization schema.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordValue {
    /// Service domain payload and its accepted support.
    Service(Box<crate::service::ServiceRecord>),
    /// Recorded contact.
    Qso(Box<Qso>),
    /// Logbook tag.
    Tag(Box<LogbookTag>),
    /// Saved operating-location profile.
    Profile(Box<SavedLocationProfile>),
    /// Reusable equipment values.
    Equipment(Box<EquipmentConfiguration>),
    /// Reference-group definition.
    Group(Box<GroupDefinition>),
    /// Export-context definition.
    OutputProfile(Box<OutputProfile>),
}

impl RecordValue {
    /// Identify the native entity without using display names or serialized text.
    pub fn id(&self) -> RecordId {
        match self {
            Self::Service(v) => RecordId::Service(v.payload.id()),
            Self::Qso(v) => RecordId::Qso(v.id()),
            Self::Tag(v) => RecordId::Tag(v.id),
            Self::Profile(v) => RecordId::Profile(v.id()),
            Self::Equipment(v) => RecordId::Equipment(v.id()),
            Self::Group(v) => RecordId::Group(v.id()),
            Self::OutputProfile(v) => RecordId::OutputProfile(v.id),
        }
    }
}

/// Complete native value or explicit retained deletion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordState {
    /// A saved native record.
    Live(RecordValue),
    /// Explicit deletion of this identity.
    Deleted,
}

impl RecordState {
    /// Compare record values for whole-record edit policy, ignoring equivalent support-clock representation.
    ///
    /// This compares semantic support identities, not their causal validity. Use
    /// baseline-aware record comparison to establish ordering or projection equivalence.
    pub fn same_values(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Live(RecordValue::Service(a)), Self::Live(RecordValue::Service(b))) => {
                a.payload == b.payload
                    && crate::service::same_support_values(&a.support, &b.support)
            }
            _ => self == other,
        }
    }
    /// Enforce native save invariants and embedded identity before accepting a state.
    pub fn validate(&self, key: RecordKey) -> anyhow::Result<()> {
        if let Self::Live(v) = self {
            if v.id() != key.id {
                return Err(MetadataError::Scope {
                    detail: format!("key {key:?} contains {:?}", v.id()),
                }
                .into());
            }
            match v {
                RecordValue::Service(v) => v.validate(key.account)?,

                RecordValue::Group(_)
                | RecordValue::Qso(_)
                | RecordValue::Profile(_)
                | RecordValue::Equipment(_)
                | RecordValue::Tag(_)
                | RecordValue::OutputProfile(_) => {}
            }
        }
        Ok(())
    }
}

/// Position within a server history; ordering requires timeline interpretation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// Server history identity.
    pub timeline: TimelineId,
    /// Checked account revision, with zero reserved for origins/checkpoints.
    pub revision: u64,
}

impl Position {
    /// Check this timeline position's revision before comparison or persistence.
    pub fn validate(&self) -> Result<(), MetadataError> {
        crate::error::validate_counter(self.revision, "timeline revision")
    }
}

/// Accepted record context interpreted under a retained baseline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "StampFields")]
pub struct VersionStamp {
    /// Original acceptance position, preserved during recovery installation.
    accepted_at: Position,
    /// Interpretation for projected components.
    baseline: BaselineId,
    /// Explicit residual record-local counters.
    vector: RecordVector,
}

impl VersionStamp {
    /// Construct accepted causal context with its matching server component.
    ///
    /// Acceptance requires a positive bounded revision. The baseline's meaning
    /// and account ownership are established by the caller's retained evidence.
    pub fn new(
        accepted_at: Position,
        baseline: BaselineId,
        vector: RecordVector,
    ) -> Result<Self, MetadataError> {
        let value = Self {
            accepted_at,
            baseline,
            vector,
        };
        value.validate()?;
        Ok(value)
    }

    /// Return the original acceptance position, including after projection.
    pub fn accepted_at(&self) -> Position {
        self.accepted_at
    }

    /// Return the baseline used to interpret projected components.
    pub fn baseline(&self) -> BaselineId {
        self.baseline
    }

    /// Borrow the residual record-local counters without changing their meaning.
    pub fn vector(&self) -> &RecordVector {
        &self.vector
    }

    /// Require a positive accepted position and its matching server component within the counter domain.
    fn validate(&self) -> Result<(), MetadataError> {
        self.accepted_at.validate()?;
        if self.accepted_at.revision == 0 {
            return Err(MetadataError::Invalid {
                detail: "accepted revision must be positive".into(),
            });
        }
        if self
            .vector
            .component(ClockActor::Server(self.accepted_at.timeline))
            != self.accepted_at.revision
        {
            return Err(MetadataError::Invalid {
                detail: format!(
                    "accepted context lacks matching server component {:?}",
                    self.accepted_at
                ),
            });
        }
        Ok(())
    }
}

/// Saved accepted value and its causal identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "AcceptedFields")]
pub struct AcceptedRecord {
    /// Account-qualified native identity.
    pub(crate) key: RecordKey,
    /// Native state or retained deletion.
    pub(crate) state: RecordState,
    /// Actual acceptance context.
    pub stamp: VersionStamp,
}

impl AcceptedRecord {
    /// Return the owning account and native record identity.
    pub fn key(&self) -> RecordKey {
        self.key
    }

    /// Borrow the saved native value or retained deletion.
    pub fn state(&self) -> &RecordState {
        &self.state
    }

    /// Validate native state and the acceptance component before constructing an envelope.
    pub fn new(key: RecordKey, state: RecordState, stamp: VersionStamp) -> anyhow::Result<Self> {
        let record = Self { key, state, stamp };
        record.validate()?;
        Ok(record)
    }

    /// Validate a deserialized envelope before a synchronization transition uses it.
    pub fn validate(&self) -> anyhow::Result<()> {
        self.state.validate(self.key)?;
        Ok(())
    }
}

/// Observed shared state, distinguishing certified absence from missing evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Observation {
    /// A saved accepted envelope.
    Accepted(AcceptedRecord),
    /// No identity existed within this completed account view.
    Absent {
        /// Account-qualified identity whose absence was checked.
        key: RecordKey,
        /// Complete installed view used for absence certification.
        checkpoint: crate::account::InstalledCheckpoint,
    },
    /// The shared state cannot currently be established.
    Unavailable {
        /// Identity for which evidence is missing.
        key: RecordKey,
        /// Why evidence is unavailable.
        reason: crate::error::UnknownReason,
    },
}

impl Observation {
    /// Identify the observed record regardless of evidence availability.
    pub fn key(&self) -> RecordKey {
        match self {
            Self::Accepted(v) => v.key,
            Self::Absent { key, .. } | Self::Unavailable { key, .. } => *key,
        }
    }

    /// Validate the account domain and any accepted native state.
    pub fn validate(&self) -> anyhow::Result<()> {
        match self {
            Self::Accepted(v) => v.validate(),
            Self::Absent { key, checkpoint } if key.account != checkpoint.account => {
                Err(MetadataError::Scope {
                    detail: format!(
                        "absence checkpoint for {key:?} belongs to {}",
                        checkpoint.account
                    ),
                }
                .into())
            }
            Self::Absent { checkpoint, .. } => Ok(checkpoint.validate()?),
            Self::Unavailable { .. } => Ok(()),
        }
    }
}

/// Serialized accepted context validated at the native restoration boundary.
#[derive(Deserialize)]
struct StampFields {
    /// Original acceptance position.
    accepted_at: Position,
    /// Interpretation of residual counters.
    baseline: BaselineId,
    /// Recorded per-actor counters.
    vector: RecordVector,
}

impl TryFrom<StampFields> for VersionStamp {
    type Error = MetadataError;

    fn try_from(fields: StampFields) -> Result<Self, Self::Error> {
        Self::new(fields.accepted_at, fields.baseline, fields.vector)
    }
}

/// Untrusted serialized envelope input checked before native use.
#[derive(Deserialize)]
struct AcceptedFields {
    /// Account-qualified native identity.
    key: RecordKey,
    /// Saved native state or retained deletion.
    state: RecordState,
    /// Accepted causal context.
    stamp: VersionStamp,
}

impl TryFrom<AcceptedFields> for AcceptedRecord {
    type Error = anyhow::Error;

    fn try_from(fields: AcceptedFields) -> anyhow::Result<Self> {
        Self::new(fields.key, fields.state, fields.stamp)
    }
}
