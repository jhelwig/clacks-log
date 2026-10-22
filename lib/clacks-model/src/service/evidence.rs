//! Imported claims, submissions and provider observations retained separately from QSO facts.

pub mod submission;

pub mod download;
pub mod observation;
pub mod processing;

use super::{
    adif::{AdifFields, ProjectionContext},
    destination::{Destination, Service},
    error::ServiceError,
    lotw::LotwConfiguration,
};
use crate::{
    contact::Qso,
    ids::{
        AssertionId, AssociationId, AttemptId, DestinationId, DispositionId, ObservationId, QsoId,
        SubmissionResultId, SuppressionId,
    },
    location::RecordedLocation,
};
use observation::{ModeReport, RemoteIdentity, RemoteObservation};
use processing::{DeletionSuppression, ExternalAssociation, ProcessingDisposition};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use submission::{ConfirmationBasis, SubmissionAttempt, SubmissionResult};

/// Recognized positive status supplied by an import; absence does not create a claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportedClaim {
    /// Imported positive upload status, independent of counterpart confirmation.
    Uploaded,
    /// Imported positive confirmation for the imported contact identity.
    Confirmed,
}

/// Source record identity and mapping interpretation, without fabricated provider responses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportProvenance {
    /// Import source description or file identity.
    pub source: String,
    /// Source record identifier or location.
    pub record: String,
    /// Mapping/catalog identity used to interpret the source.
    pub mapping: String,
}

/// Frozen source-only comparison baseline; never retroactively enriched by live fallback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedBaseline {
    /// Exact source end time retained even when interchange conversion is unavailable.
    #[serde(default)]
    pub native_end: Option<crate::scalar::UtcSecond>,
    /// Provider whose semantic values this baseline covers.
    pub service: Service,
    /// Supplied normalized service inputs, not proof of historical sent bytes.
    pub fields: AdifFields,
    /// Source-only LoTW station comparison values, retaining uninterpretable subdivisions.
    pub own_location: Option<RecordedLocation>,
    /// Relevant contact identity for confirmation applicability.
    pub confirmation: ConfirmationBasis,
    /// Source fields known to be available.
    pub coverage: BTreeSet<String>,
}

/// Trusted imported service claim retaining independent scheduling/confirmation meaning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedAssertion {
    /// Stable imported fact identity.
    pub id: AssertionId,
    /// Contact described by the source claim.
    pub qso: QsoId,
    /// Provider to which it applies.
    pub service: Service,
    /// Known destination only; no destination is inferred from defaults.
    pub destination: Option<DestinationId>,
    /// Positive claim recognized by the source adapter.
    pub claim: ImportedClaim,
    /// Date supplied by the source, if any.
    pub date: Option<crate::scalar::UtcSecond>,
    /// Source and mapping explanation.
    pub provenance: ImportProvenance,
    /// Imported values explicitly covered by the scheduling/applicability policy.
    pub baseline: ImportedBaseline,
}

/// Inputs to capture source values without directory/provider/certificate defaults.
pub struct ImportedBaselineInput<'a> {
    /// Valid prepared imported contact.
    pub qso: &'a Qso,
    /// Claim provider.
    pub service: Service,
    /// Source coverage, preserving unknown versus supplied data.
    pub coverage: BTreeSet<String>,
    /// Current explicit LoTW compatibility data.
    pub lotw: &'a LotwConfiguration,
    /// Reference/mode context; no eligible fallback is available here.
    pub context: &'a ProjectionContext<'a>,
}

/// Capture the imported contact's service baseline without inventing a dispatch.
pub fn imported_baseline(input: &ImportedBaselineInput<'_>) -> anyhow::Result<ImportedBaseline> {
    let q = input.qso;
    q.validate()?;
    let mut fields = if input.service == Service::Qrz {
        super::qrz::source_fields(q, input.context)?
    } else {
        let converted_time = super::adif::contact_time(q.start).ok();
        let mut values = super::adif::mode(&q.mode, input.context.recognition);
        values.extend([
            (
                "STATION_CALLSIGN".into(),
                q.own_station.callsign.as_str().into(),
            ),
            ("CALL".into(), q.contacted_station.callsign.as_str().into()),
        ]);
        if let Some((date, time)) = converted_time {
            values.insert("QSO_DATE".into(), date);
            values.insert("TIME_ON".into(), time);
        }
        if let Some(f) = q.primary.frequency() {
            values.insert("FREQ".into(), super::adif::frequency(f));
        }
        if let Some(b) = q.primary.band() {
            values.insert("BAND".into(), b.label().to_ascii_uppercase());
        }
        if let Some(rx) = &q.receive_override
            && rx != &q.primary
        {
            if let Some(b) = rx.band() {
                values.insert("BAND_RX".into(), b.label().to_ascii_uppercase());
            }
            if let Some(f) = rx.frequency() {
                values.insert("FREQ_RX".into(), super::adif::frequency(f));
            }
        }
        if let Some(iota) = super::lotw::own_iota(q, input.context.definitions)? {
            values.insert(
                "MY_IOTA".into(),
                super::lotw::station_value("IOTA", &iota, input.lotw),
            );
        }
        values
    };
    let mode = if input.service == Service::Lotw {
        super::lotw::normalize_mode(&q.mode, input.lotw)
            .ok()
            .or_else(|| {
                fields
                    .get("SUBMODE")
                    .or_else(|| fields.get("MODE"))
                    .cloned()
            })
    } else {
        fields.get("MODE").cloned()
    };
    let band = if input.service == Service::Lotw {
        input.lotw.band(&q.primary).ok()
    } else {
        q.primary.band().map(|b| b.label().to_ascii_uppercase())
    };
    let rx = q.primary.effective_receive(q.receive_override.as_ref());
    let receive_band = if input.service == Service::Lotw {
        input.lotw.band(rx).ok()
    } else {
        rx.band().map(|b| b.label().to_ascii_uppercase())
    };
    let basis = ConfirmationBasis {
        uninterpreted_band: if band.is_none() {
            q.primary.band().map(|b| b.label().to_owned())
        } else {
            None
        },
        uninterpreted_receive_band: if receive_band.is_none() {
            rx.band().map(|b| b.label().to_owned())
        } else {
            None
        },
        own_call: Some(q.own_station.callsign.clone()),
        worked_call: Some(q.contacted_station.callsign.clone()),
        start: Some(q.start),
        band: band.clone(),
        receive_band,
        frequency: q.primary.frequency(),
        receive_frequency: rx.frequency(),
        mode: mode.as_ref().map(|m| ModeReport {
            concrete: m.clone(),
            parent: fields.get("MODE").cloned(),
            group: if input.service == Service::Lotw {
                input.lotw.modes.get(m).cloned()
            } else {
                None
            },
        }),
    };
    if input.service == Service::Lotw {
        fields.retain(|key, _| {
            matches!(
                key.as_str(),
                "STATION_CALLSIGN"
                    | "CALL"
                    | "QSO_DATE"
                    | "TIME_ON"
                    | "FREQ"
                    | "BAND"
                    | "FREQ_RX"
                    | "BAND_RX"
                    | "MODE"
                    | "MY_IOTA"
            )
        });
        if let Some(mode) = mode {
            fields.insert("MODE".into(), mode);
        }
        if let Some(band) = band {
            fields.insert("BAND".into(), band);
        }
    }
    Ok(ImportedBaseline {
        native_end: if input.service == Service::Qrz {
            q.end
        } else {
            None
        },
        service: input.service,
        fields,
        own_location: if input.service == Service::Lotw {
            super::lotw::location_baseline(q.own_station.location.as_ref(), input.lotw)
        } else {
            None
        },
        confirmation: basis,
        coverage: input.coverage.clone(),
    })
}

/// Stable kind-qualified service identity used by the account record domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ServiceRecordId {
    /// Destination-wide maintained download coordination.
    Download(crate::ids::DownloadStateId),
    /// One recoverable query execution.
    Acquisition(crate::ids::AcquisitionId),
    /// Public collection descriptor.
    Destination(DestinationId),
    /// Imported positive claim.
    Assertion(AssertionId),
    /// Actual dispatch history.
    Attempt(AttemptId),
    /// Independent outcome evidence.
    Result(SubmissionResultId),
    /// Meaningful acquired provider report.
    Observation(ObservationId),
    /// Maintained unique external binding.
    Association(AssociationId),
    /// Observation review/processing state.
    Disposition(DispositionId),
    /// Known deletion suppression.
    Suppression(SuppressionId),
}

/// Service domain values without synchronization clock/version dependencies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServicePayload {
    /// Destination-wide download coordination.
    Download(download::DownloadControl),
    /// Query execution and completed-input coverage.
    Acquisition(download::AcquisitionState),
    /// Public destination namespace.
    Destination(Destination),
    /// Imported assertion and its self-contained baseline.
    Imported(Box<ImportedAssertion>),
    /// Actual or possible dispatch with frozen values.
    Attempt(SubmissionAttempt),
    /// Scope-qualified result for existing attempts.
    Result(SubmissionResult),
    /// Remote report, possibly unmatched or incompletely identified.
    Observation(Box<RemoteObservation>),
    /// Mutable unique binding.
    Association(ExternalAssociation),
    /// Processing decision or durable incoming review work.
    Disposition(ProcessingDisposition),
    /// Deleted-contact memory independent of query windows.
    Suppression(DeletionSuppression),
}

impl ServicePayload {
    /// Identify the domain record without display names or payload hashes.
    pub fn id(&self) -> ServiceRecordId {
        match self {
            Self::Download(v) => ServiceRecordId::Download(v.id),
            Self::Acquisition(v) => ServiceRecordId::Acquisition(v.id),
            Self::Destination(v) => ServiceRecordId::Destination(v.id),
            Self::Imported(v) => ServiceRecordId::Assertion(v.id),
            Self::Attempt(v) => ServiceRecordId::Attempt(v.id),
            Self::Result(v) => ServiceRecordId::Result(v.id),
            Self::Observation(v) => ServiceRecordId::Observation(v.id),
            Self::Association(v) => ServiceRecordId::Association(v.id),
            Self::Disposition(v) => ServiceRecordId::Disposition(v.id),
            Self::Suppression(v) => ServiceRecordId::Suppression(v.id),
        }
    }

    /// Return whether content is a retained immutable fact rather than maintained state.
    pub fn immutable(&self) -> bool {
        matches!(
            self,
            Self::Imported(_) | Self::Attempt(_) | Self::Result(_) | Self::Observation(_)
        )
    }

    /// Return known destination dependencies without requiring live access configuration.
    pub fn destination(&self) -> Option<DestinationId> {
        match self {
            Self::Download(v) => Some(v.destination),
            Self::Acquisition(v) => Some(v.query.destination),
            Self::Destination(v) => Some(v.id),
            Self::Imported(v) => v.destination,
            Self::Attempt(v) => Some(v.projection.destination()),
            Self::Result(v) => Some(v.destination),
            Self::Observation(v) => Some(v.destination),
            Self::Association(v) => Some(v.external.destination),
            Self::Suppression(v) => Some(v.external.destination),
            Self::Disposition(_) => None,
        }
    }

    /// Validate structural domain invariants before accepting deserialized evidence.
    pub fn validate(&self) -> anyhow::Result<()> {
        if let Self::Result(result) = self {
            result.identified_attempt()?;
        }
        match self {
            Self::Download(v) => v.validate()?,
            Self::Acquisition(v) => {
                anyhow::ensure!(
                    matches!(v.phase, download::AcquisitionPhase::Active)
                        == v.finished_at.is_none(),
                    "acquisition phase/close time mismatch"
                );
                anyhow::ensure!(
                    !v.complete || matches!(v.phase, download::AcquisitionPhase::Completed),
                    "incomplete request claims full coverage"
                );
            }
            Self::Destination(v) => {
                super::destination::validate_destinations(std::slice::from_ref(v))?
            }
            Self::Imported(v) => {
                if v.service != v.baseline.service || v.provenance.source.trim().is_empty() {
                    return Err(ServiceError::Invalid {
                        detail: format!("assertion {} has incompatible baseline/source", v.id),
                    }
                    .into());
                }
            }
            Self::Observation(v) => {
                if v.acquisition.query.destination != v.destination
                    || matches!(&v.identity,RemoteIdentity::Complete(k) if k.destination!=v.destination)
                {
                    return Err(ServiceError::Invalid {
                        detail: format!("observation {} crosses destination", v.id),
                    }
                    .into());
                }
                if let observation::Reported::Value(start) = v.facts.contact.start {
                    start.bounds()?;
                }
            }
            Self::Result(v)
                if (v.scope.attempts().is_empty()
                    || v.external
                        .as_ref()
                        .is_some_and(|k| k.destination != v.destination)) =>
            {
                return Err(ServiceError::Invalid {
                    detail: format!("result {} lacks members or crosses destination", v.id),
                }
                .into());
            }
            _ => {}
        }
        Ok(())
    }
}
