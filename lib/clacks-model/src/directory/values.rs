//! Checked lookup identities, returned facts and complete successful snapshots.

use crate::{
    ids::LookupId,
    location::RecordedLocation,
    position::{Coordinates, Locator},
    reference::ReferenceAssociation,
    scalar::{RecordedCallsign, UtcTimestamp},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Directory selected for new client requests; this is not a publication service.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DirectoryProvider {
    /// QRZ's authenticated XML callsign directory.
    Qrz,
    /// HamQTH's authenticated callbook.
    HamQth,
}

impl DirectoryProvider {
    /// Return the provider's stable configuration/storage label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Qrz => "qrz",
            Self::HamQth => "hamqth",
        }
    }
}

impl std::str::FromStr for DirectoryProvider {
    type Err = anyhow::Error;
    /// Decode only the supported concrete provider labels.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "qrz" => Ok(Self::Qrz),
            "hamqth" => Ok(Self::HamQth),
            _ => anyhow::bail!("unknown directory provider"),
        }
    }
}

/// Canonical request text, separate from a contact's recorded worked callsign.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct LookupKey(String);

impl LookupKey {
    /// Derive a request key without editing the recorded call or guessing slash prefixes.
    pub fn from_recorded(call: &RecordedCallsign) -> Self {
        let original = call.as_str();
        let key = original
            .strip_suffix("/P")
            .filter(|v| !v.is_empty())
            .unwrap_or(original);
        Self(key.to_owned())
    }

    /// Borrow the exact key retained with a request or successful snapshot.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for LookupKey {
    type Error = anyhow::Error;

    /// Check stored canonical text without applying request transformations again.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        anyhow::ensure!(
            !value.trim().is_empty() && value.trim() == value && value.to_uppercase() == value,
            "invalid directory lookup key"
        );
        Ok(Self(value))
    }
}

impl From<LookupKey> for String {
    /// Retain the exact canonical request text in native serialization.
    fn from(value: LookupKey) -> Self {
        value.0
    }
}

/// Provider facts and their interpretation, never a recorded contact update.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryFacts {
    /// Provider name, separately selected from recorded on-air names.
    pub name: Option<String>,
    /// Whole directory location, independent of the actual QSO location.
    pub location: Option<RecordedLocation>,
    /// Provider references retained separately from recorded participation.
    pub references: BTreeSet<ReferenceAssociation>,
    /// Directory locality, not an inferred geographic identifier.
    pub locality: Option<String>,
    /// Identity returned by the directory, which never replaces the worked call.
    pub returned_callsign: Option<RecordedCallsign>,
    /// Original supplied locator and precision, even if coordinates were selected.
    pub supplied_locator: Option<Locator>,
    /// Original coordinate pair, even if a supplied grid area was selected.
    pub supplied_coordinates: Option<Coordinates>,
    /// Provider's coordinate-origin classification, when supplied.
    pub coordinate_source: Option<String>,
}

/// Complete successful response and its immutable ordering identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectorySnapshotFields {
    /// Directory that actually supplied this result.
    pub provider: DirectoryProvider,
    /// Normalized requested callsign, not the returned identity.
    pub key: LookupKey,
    /// Successful completion time recorded by the fetching client.
    pub lookup_time: UtcTimestamp,
    /// Request identity used for deterministic equal-time arbitration.
    pub lookup_id: LookupId,
    /// Complete replacement facts, including their absent fields.
    pub facts: DirectoryFacts,
}

/// Retained successful response; failure/not-found never constructs this value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "DirectorySnapshotFields", into = "DirectorySnapshotFields")]
pub struct DirectorySnapshot(DirectorySnapshotFields);

impl DirectorySnapshot {
    /// Construct a complete result without copying provider facts into recorded fields.
    pub fn new(fields: DirectorySnapshotFields) -> anyhow::Result<Self> {
        for (name, value) in [
            ("directory name", &fields.facts.name),
            ("directory locality", &fields.facts.locality),
            ("coordinate source", &fields.facts.coordinate_source),
        ] {
            crate::contact::validate_optional_text(value.as_deref(), name)?;
        }
        Ok(Self(fields))
    }
    /// Borrow the normalized request association used for eligibility.
    pub fn key(&self) -> &LookupKey {
        &self.0.key
    }
    /// Return the provider that supplied this historical result.
    pub fn provider(&self) -> DirectoryProvider {
        self.0.provider
    }
    /// Return the original successful client completion timestamp.
    pub fn lookup_time(&self) -> UtcTimestamp {
        self.0.lookup_time
    }
    /// Return the immutable request identity retained on retries and recovery.
    pub fn lookup_id(&self) -> LookupId {
        self.0.lookup_id
    }
    /// Borrow the complete separate directory facts.
    pub fn facts(&self) -> &DirectoryFacts {
        &self.0.facts
    }
    /// Compare the successful-time and identity tuple, not arrival order.
    pub fn compare_lookup(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .lookup_time
            .unix_seconds()
            .cmp(&other.0.lookup_time.unix_seconds())
            .then_with(|| {
                self.0
                    .lookup_time
                    .subsecond_nanoseconds()
                    .cmp(&other.0.lookup_time.subsecond_nanoseconds())
            })
            .then_with(|| self.0.lookup_id.cmp(&other.0.lookup_id))
    }
}

impl TryFrom<DirectorySnapshotFields> for DirectorySnapshot {
    type Error = anyhow::Error;
    /// Restore already checked native components without changing lookup ordering.
    fn try_from(value: DirectorySnapshotFields) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<DirectorySnapshot> for DirectorySnapshotFields {
    /// Preserve complete native result fields in storage and synchronization.
    fn from(value: DirectorySnapshot) -> Self {
        value.0
    }
}

/// Public account configuration; credential bytes are not part of this value.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ConfigurationFields")]
pub struct DirectoryConfiguration {
    /// Stale-write revision; zero denotes a never-configured account.
    pub revision: i64,
    /// Provider used for new requests, absent when disabled.
    pub selected: Option<DirectoryProvider>,
    /// Providers with saved account material; this does not authorize fallback.
    pub configured: BTreeSet<DirectoryProvider>,
}

/// Serialized public configuration checked at restoration boundaries.
#[derive(Deserialize)]
struct ConfigurationFields {
    /// Stored stale-write revision.
    revision: i64,
    /// Stored account-level selection.
    selected: Option<DirectoryProvider>,
    /// Stored public material availability.
    configured: BTreeSet<DirectoryProvider>,
}

impl TryFrom<ConfigurationFields> for DirectoryConfiguration {
    type Error = anyhow::Error;
    /// Reject revisions outside the signed durable counter domain.
    fn try_from(value: ConfigurationFields) -> Result<Self, Self::Error> {
        anyhow::ensure!(
            value.revision >= 0,
            "negative directory configuration revision"
        );
        Ok(Self {
            revision: value.revision,
            selected: value.selected,
            configured: value.configured,
        })
    }
}
