//! One successful winner per contact and normalized lookup key.

use super::values::{DirectorySnapshot, LookupKey};
use crate::ids::QsoId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Separate contact-associated snapshot values, never a reusable station cache.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RecordFields", into = "RecordFields")]
pub struct DirectoryRecord {
    /// Contact owning every retained result.
    qso: QsoId,
    /// One whole successful winner per normalized request key.
    snapshots: BTreeMap<LookupKey, DirectorySnapshot>,
}

impl DirectoryRecord {
    /// Start a contact's directory collection without claiming a successful lookup.
    pub fn new(qso: QsoId) -> Self {
        Self {
            qso,
            snapshots: BTreeMap::new(),
        }
    }
    /// Return the owning contact identity.
    pub fn qso(&self) -> QsoId {
        self.qso
    }
    /// Borrow the winning result for exactly one normalized key.
    pub fn snapshot(&self, key: &LookupKey) -> Option<&DirectorySnapshot> {
        self.snapshots.get(key)
    }
    /// Enumerate retained key winners for persistence without retaining refresh history.
    pub fn snapshots(&self) -> impl Iterator<Item = &DirectorySnapshot> {
        self.snapshots.values()
    }

    /// Keep the latest successful complete result; identity mismatch is an integrity error.
    ///
    /// Returns whether native values changed. Failure/not-found are not input values.
    pub fn record(&mut self, snapshot: DirectorySnapshot) -> anyhow::Result<bool> {
        if let Some(current) = self.snapshots.get(snapshot.key()) {
            if current.lookup_id() == snapshot.lookup_id() {
                anyhow::ensure!(
                    current == &snapshot,
                    "directory lookup identity has different contents"
                );
                return Ok(false);
            }
            if current.order(&snapshot).is_ge() {
                return Ok(false);
            }
        }
        self.snapshots.insert(snapshot.key().clone(), snapshot);
        Ok(true)
    }

    /// Combine independently received key winners without merging any provider fields.
    ///
    /// Validates the whole result before changing this value on failure.
    pub fn merge_from(&mut self, other: &Self) -> anyhow::Result<bool> {
        anyhow::ensure!(
            self.qso == other.qso,
            "directory records belong to different contacts"
        );
        let mut combined = self.clone();
        let mut changed = false;
        for snapshot in other.snapshots.values() {
            changed |= combined.record(snapshot.clone())?;
        }
        *self = combined;
        Ok(changed)
    }
}

/// Native serialization retaining ownership and independently checked map keys.
#[derive(Serialize, Deserialize)]
struct RecordFields {
    /// Contact owning these facts.
    qso: QsoId,
    /// Complete winners indexed by their normalized requested identity.
    snapshots: BTreeMap<LookupKey, DirectorySnapshot>,
}

impl TryFrom<RecordFields> for DirectoryRecord {
    type Error = anyhow::Error;
    /// Reject a payload whose lookup association differs from its storage key.
    fn try_from(value: RecordFields) -> Result<Self, Self::Error> {
        anyhow::ensure!(
            value
                .snapshots
                .iter()
                .all(|(key, snapshot)| key == snapshot.key()),
            "directory map key differs from snapshot key"
        );
        Ok(Self {
            qso: value.qso,
            snapshots: value.snapshots,
        })
    }
}

impl From<DirectoryRecord> for RecordFields {
    /// Retain exact winners and association through serialization/retry.
    fn from(value: DirectoryRecord) -> Self {
        Self {
            qso: value.qso,
            snapshots: value.snapshots,
        }
    }
}
