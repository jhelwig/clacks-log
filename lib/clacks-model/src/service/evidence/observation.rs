//! Provider reports, explicit field coverage and semantic observation comparison.

use crate::service::{
    adif::AdifFields,
    destination::{ExternalRecordKey, Service},
    error::ServiceError,
};
use crate::{
    ids::{AcquisitionId, DestinationId, ObservationId},
    location::{CqZone, ItuZone},
    position::Position,
    reference::ReferenceAssociation,
    scalar::{RecordedCallsign, UtcSecond, UtcTimestamp},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Reported information; omission is not an assertion of absence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Reported<T> {
    /// This read supplied no replacement information.
    #[default]
    NotReported,
    /// An explicit provider value, including a negative status.
    Value(T),
    /// Explicit removal, usable only with established provider semantics.
    Clear,
}

/// Provider-reported confirmation, independent of upload outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfirmationStatus {
    /// Positive provider confirmation.
    Confirmed,
    /// Explicit provider negative, not a missing field.
    Unconfirmed,
}

/// Incoming time coverage, separate from saved QSO timestamp resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservedStart {
    /// An exact supplied second.
    Second(UtcSecond),
    /// A minute-aligned instant covering its sixty seconds.
    Minute(UtcSecond),
}

impl ObservedStart {
    /// Return inclusive whole-second bounds with checked minute alignment and overflow.
    pub fn bounds(self) -> anyhow::Result<(i64, i64)> {
        match self {
            Self::Second(v) => Ok((v.unix_seconds(), v.unix_seconds())),
            Self::Minute(v) => {
                let start = v.unix_seconds();
                if start.rem_euclid(60) != 0 {
                    return Err(ServiceError::Invalid {
                        detail: "incoming minute is not aligned".into(),
                    }
                    .into());
                }
                Ok((
                    start,
                    start.checked_add(59).ok_or_else(|| ServiceError::Invalid {
                        detail: "incoming minute overflow".into(),
                    })?,
                ))
            }
        }
    }
}

/// Actual provider mode and separately supplied parent/award classification.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeReport {
    /// Concrete reported mode, not an inferred award group.
    pub concrete: String,
    /// ADIF parent when independently established.
    pub parent: Option<String>,
    /// Provider award group when supplied.
    pub group: Option<String>,
}

/// Field coverage for one station report; no lower-source field filling is implied.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StationReport {
    /// Provider-supplied operator name.
    pub name: Reported<String>,
    /// Reported grid-area or explicit point.
    pub position: Reported<Position>,
    /// Reported entity identifier.
    pub dxcc: Reported<String>,
    /// Reported first-level subdivision.
    pub primary_subdivision: Reported<String>,
    /// Reported second-level subdivision.
    pub secondary_subdivision: Reported<String>,
    /// Reported CQ zone.
    pub cq_zone: Reported<CqZone>,
    /// Reported ITU zone.
    pub itu_zone: Reported<ItuZone>,
    /// Actual station-side references, not catalog metadata.
    pub references: Reported<BTreeSet<ReferenceAssociation>>,
}

/// Returned contact identity/status; missing fields never establish matching identity.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactReport {
    /// Our full station call.
    pub own_call: Reported<RecordedCallsign>,
    /// Their full station call.
    pub worked_call: Reported<RecordedCallsign>,
    /// Supplied start-time coverage.
    pub start: Reported<ObservedStart>,
    /// Primary service band.
    pub band: Reported<String>,
    /// Effective receive band if established.
    pub receive_band: Reported<String>,
    /// Exact primary hertz when supplied at that precision.
    pub frequency: Reported<crate::scalar::Hertz>,
    /// Separately reported receive hertz.
    pub receive_frequency: Reported<crate::scalar::Hertz>,
    /// Concrete mode and supplied classification.
    pub mode: Reported<ModeReport>,
    /// Explicit confirmation status.
    pub confirmation: Reported<ConfirmationStatus>,
}

/// Returned values separated by whose report they actually describe.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteFacts {
    /// Our submitted contact report and its service status.
    pub contact: ContactReport,
    /// Our returned station data, never relabeled as counterpart evidence.
    pub own_station: StationReport,
    /// Actual other-station report, when the provider exposes one.
    pub counterpart: Option<StationReport>,
}

/// Separately scoped provider timestamps; not a universal revision token.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderOrdering {
    /// Provider QSO receipt/update time.
    pub qso: Option<UtcSecond>,
    /// Provider confirmation update time.
    pub confirmation: Option<UtcSecond>,
}

/// Logical query stream whose progress is independently maintained.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum QueryStream {
    /// LoTW submitted-QSO receipt/update stream.
    LotwReceipt,
    /// LoTW confirmation-update stream.
    LotwConfirmation,
    /// QRZ selected logbook change query.
    Qrz,
}

/// Public query semantics sufficient to interpret returned coverage, without secrets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryIdentity {
    /// Remote collection selected.
    pub destination: DestinationId,
    /// Meaning of the progress watermark.
    pub stream: QueryStream,
    /// Requested provider field coverage.
    pub selected_fields: BTreeSet<String>,
    /// Optional historical own-call restriction.
    pub own_call: Option<RecordedCallsign>,
    /// Provider since filter, not a receipt acknowledgment.
    pub since: Option<UtcSecond>,
}

/// Original acquisition provenance, retained without a permanent chain of polls.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionProvenance {
    /// Download operation which obtained this information.
    pub id: AcquisitionId,
    /// Query semantics and coverage.
    pub query: QueryIdentity,
    /// Diagnostic local observation time; not provider revision order.
    pub observed_at: UtcTimestamp,
}

/// Provider identity with an explicit incomplete alternative, avoiding guessed keys.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteIdentity {
    /// Complete service-specific identity.
    Complete(ExternalRecordKey),
    /// Retain available source fields for later diagnostics/review.
    Incomplete {
        /// Provider whose identity could not be established.
        service: Service,
        /// Source identity fields without an invented complete key.
        available: AdifFields,
    },
}

/// One immutable meaningful provider observation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteObservation {
    /// Stable retained occurrence identity, preserved during recovery.
    pub id: ObservationId,
    /// Collection where this report was acquired.
    pub destination: DestinationId,
    /// Exact or explicitly incomplete provider identity.
    pub identity: RemoteIdentity,
    /// Reported facts and per-field coverage.
    pub facts: RemoteFacts,
    /// Scoped provider ordering evidence.
    pub ordering: ProviderOrdering,
    /// Original read provenance.
    pub acquisition: AcquisitionProvenance,
}

/// Established information for one external record, derived from ordered retained evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EstablishedObservation {
    /// Identity of the established external record.
    pub identity: RemoteIdentity,
    /// Currently established facts without filling omitted lower-source fields.
    pub facts: RemoteFacts,
    /// Available scoped update evidence.
    pub ordering: ProviderOrdering,
}

impl From<&RemoteObservation> for EstablishedObservation {
    fn from(v: &RemoteObservation) -> Self {
        Self {
            identity: v.identity.clone(),
            facts: v.facts.clone(),
            ordering: v.ordering.clone(),
        }
    }
}

/// Semantic comparison, separate from acceptance/acquisition chronology.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObservationComparison {
    /// No additional fact or ordering evidence.
    Unchanged,
    /// Previously unreported paths now supplied.
    Additional(Vec<String>),
    /// Previously supplied paths have changed.
    Changed(Vec<String>),
    /// Only useful provider ordering information advanced.
    OrderingOnly,
    /// All differing supplied components are demonstrably older.
    Older(Vec<String>),
    /// Contradictory information has equal provider timestamps.
    Unordered(Vec<String>),
}

/// Compare reported fields against established information, keeping timestamp scopes distinct.
pub fn compare(
    current: &EstablishedObservation,
    incoming: &RemoteObservation,
) -> anyhow::Result<ObservationComparison> {
    if current.identity != incoming.identity {
        return Err(ServiceError::Invalid {
            detail: "cannot compare observations of different external identities".into(),
        }
        .into());
    }
    let mut added = Vec::new();
    let mut changed = Vec::new();
    let mut older = Vec::new();
    let mut unordered = Vec::new();
    macro_rules! check {
        ($old:expr,$new:expr,$path:expr,$scope:ident) => {
            if !matches!($new, Reported::NotReported) && $old != $new {
                let path = $path.to_owned();
                let order = current.ordering.$scope.zip(incoming.ordering.$scope);
                if order.is_some_and(|(a, b)| b < a) {
                    older.push(path);
                } else if matches!($old, Reported::NotReported) {
                    added.push(path);
                } else if order.is_some_and(|(a, b)| a == b) {
                    unordered.push(path);
                } else {
                    changed.push(path);
                }
            }
        };
    }
    let a = &current.facts.contact;
    let b = &incoming.facts.contact;
    check!(&a.own_call, &b.own_call, "contact.own_call", qso);
    check!(&a.worked_call, &b.worked_call, "contact.worked_call", qso);
    check!(&a.start, &b.start, "contact.start", qso);
    check!(&a.band, &b.band, "contact.band", qso);
    check!(
        &a.receive_band,
        &b.receive_band,
        "contact.receive_band",
        qso
    );
    check!(&a.frequency, &b.frequency, "contact.frequency", qso);
    check!(
        &a.receive_frequency,
        &b.receive_frequency,
        "contact.receive_frequency",
        qso
    );
    check!(&a.mode, &b.mode, "contact.mode", qso);
    check!(
        &a.confirmation,
        &b.confirmation,
        "contact.confirmation",
        confirmation
    );
    macro_rules! station {
        ($a:expr,$b:expr,$prefix:expr,$scope:ident) => {{
            let a = $a;
            let b = $b;
            check!(&a.name, &b.name, format!("{}.name", $prefix), $scope);
            check!(
                &a.position,
                &b.position,
                format!("{}.position", $prefix),
                $scope
            );
            check!(&a.dxcc, &b.dxcc, format!("{}.dxcc", $prefix), $scope);
            check!(
                &a.primary_subdivision,
                &b.primary_subdivision,
                format!("{}.primary_subdivision", $prefix),
                $scope
            );
            check!(
                &a.secondary_subdivision,
                &b.secondary_subdivision,
                format!("{}.secondary_subdivision", $prefix),
                $scope
            );
            check!(
                &a.cq_zone,
                &b.cq_zone,
                format!("{}.cq_zone", $prefix),
                $scope
            );
            check!(
                &a.itu_zone,
                &b.itu_zone,
                format!("{}.itu_zone", $prefix),
                $scope
            );
            check!(
                &a.references,
                &b.references,
                format!("{}.references", $prefix),
                $scope
            );
        }};
    }
    station!(
        &current.facts.own_station,
        &incoming.facts.own_station,
        "own_station",
        qso
    );
    if let Some(b) = &incoming.facts.counterpart {
        station!(
            &current.facts.counterpart.clone().unwrap_or_default(),
            b,
            "counterpart",
            confirmation
        );
    }
    if !unordered.is_empty() {
        return Ok(ObservationComparison::Unordered(unordered));
    }
    if !changed.is_empty() {
        changed.extend(added);
        return Ok(ObservationComparison::Changed(changed));
    }
    if !added.is_empty() {
        return Ok(ObservationComparison::Additional(added));
    }
    if !older.is_empty() {
        return Ok(ObservationComparison::Older(older));
    }
    if [
        current.ordering.qso.zip(incoming.ordering.qso),
        current
            .ordering
            .confirmation
            .zip(incoming.ordering.confirmation),
    ]
    .into_iter()
    .flatten()
    .any(|(a, b)| b > a)
        || (current.ordering.qso.is_none() && incoming.ordering.qso.is_some())
        || (current.ordering.confirmation.is_none() && incoming.ordering.confirmation.is_some())
    {
        return Ok(ObservationComparison::OrderingOnly);
    }
    Ok(ObservationComparison::Unchanged)
}
