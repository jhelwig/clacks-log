//! Convert ADI fields and review choices into proposed edits to a QSO.

pub(super) mod input;
pub(super) mod radio;
mod station;

use super::{
    adi::{Alternative, Diagnostic},
    evidence::ClaimFact,
};
use crate::review::fields::{FieldId, QsoField, StationField};
use clacks_model::{
    contact::{
        BandFrequency, OperatorInformation, OperatorInformationFields, OwnOperatingContext,
        StationSnapshot, input::QsoFields,
    },
    equipment::{EquipmentValues, EquipmentValuesFields},
    group::DefinitionSet,
    ids::{GroupId, QsoId},
    location::{CqZone, ItuZone, RecordedLocation, RecordedLocationFields},
    position::Position,
    recognition::{
        RecognitionCatalog,
        band::Band,
        mode::{Mode, ModeIdentity},
    },
    reference::{ReferenceAssociation, StationSide},
    scalar::{Milliwatts, RecordedCallsign, UtcSecond, optional_text},
};
use input::RowInput;
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
    sync::Arc,
};

pub(super) const IOTA_GROUP_ID: &str = "01M5QZFPZZ9GPDY819J1WMTJAK";
pub(super) const POTA_GROUP_ID: &str = "01M5QZFPZZ8TTM3MCPAT3MA7MA";
pub(super) const SOTA_GROUP_ID: &str = "01M5QZFPZZ8QTCY3F399Y282AR";

/// Operator decision for a single conflicting source fact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Choice {
    /// Use the named alternative by its zero-based index.
    Use(usize),
    /// Contribute no edit or status for this fact.
    Omit,
}

/// Conflicting values for a single fact that the operator can resolve or omit.
#[derive(Clone, Debug)]
pub(crate) struct Dispute {
    pub fact: String,
    /// Display values in the same order as the corresponding typed edits.
    pub alternatives: Vec<Alternative>,
    /// Skip this dispute if the selected submission already resolves this park's qualifier.
    unless_submission_selects: Option<SubmissionPark>,
}

#[derive(Clone, Debug)]
struct SubmissionPark {
    dispute: usize,
    park: String,
}

/// One readable source record with undisputed edits and unresolved alternatives.
#[derive(Debug)]
pub(crate) struct PreparedRow {
    /// One-based index among readable source records.
    pub number: usize,
    /// Source byte range of the complete record.
    pub span: Range<usize>,
    /// Optional invalid values, corrections or unsupported interpretations.
    pub warnings: Vec<Diagnostic>,
    provenance: Arc<str>,
    pub disputes: Vec<Dispute>,
    edits: Vec<Edit>,
    /// Typed edits indexed first by dispute, then by that dispute's alternative index.
    alternatives: Vec<Vec<Edit>>,
}

impl PreparedRow {
    /// Return a window for each usable worked-callsign and start-time pair,
    /// including alternatives available during review.
    pub(super) fn candidate_windows(&self) -> Vec<(RecordedCallsign, i64, i64)> {
        let edits = || self.edits.iter().chain(self.alternatives.iter().flatten());
        let starts = edits()
            .filter_map(|edit| match edit {
                Edit::Start(start, minute) => Some(super::matching::bounds(*start, *minute)),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let calls = edits().filter_map(|edit| match edit {
            Edit::Call(StationSide::Contacted, call) => Some(call),
            _ => None,
        });
        calls
            .flat_map(|call| {
                starts
                    .iter()
                    .map(move |&(lower, upper)| (call.clone(), lower, upper))
            })
            .collect()
    }

    /// An explicit omission resolves a dispute. Only currently applicable disputes count.
    pub(crate) fn source_resolved(&self, choices: &BTreeMap<usize, Choice>) -> bool {
        self.active_disputes(choices)
            .all(|(index, _)| choices.contains_key(&index))
    }

    /// Yield the disputes that require a choice given the selected POTA submission.
    pub(crate) fn active_disputes<'a>(
        &'a self,
        choices: &'a BTreeMap<usize, Choice>,
    ) -> impl Iterator<Item = (usize, &'a Dispute)> {
        self.disputes.iter().enumerate().filter(move |(_, dispute)| {
            let Some(submission) = &dispute.unless_submission_selects else {
                return true;
            };
            match choices.get(&submission.dispute) {
                None => false,
                Some(Choice::Omit) => true,
                Some(Choice::Use(selected)) => !matches!(
                    self.alternatives.get(submission.dispute).and_then(|edits| edits.get(*selected)),
                    Some(Edit::Reference(_, reference))
                        if station::park_identity(reference) == Some(submission.park.as_str())
                ),
            }
        })
    }

    /// Project all undisputed values and the selected dispute alternatives.
    ///
    /// Missing choices remain unresolved and contribute no edit.
    pub(crate) fn resolve(&self, choices: &BTreeMap<usize, Choice>) -> RowProjection {
        let edits: Vec<_> = self
            .edits
            .iter()
            .chain(self.active_disputes(choices).filter_map(|(index, _)| {
                let Choice::Use(value) = choices.get(&index)? else {
                    return None;
                };
                self.alternatives.get(index)?.get(*value)
            }))
            .cloned()
            .collect();
        let mut projection = RowProjection {
            number: self.number,
            span: self.span.clone(),
            provenance: Arc::clone(&self.provenance),
            edits,
            worked_call: None,
            own_call: None,
            start: None,
            minute_precision: false,
            primary: None,
            mode: None,
        };
        for edit in &projection.edits {
            match edit {
                Edit::Call(StationSide::Own, call) | Edit::StationOperator(call) => {
                    projection.own_call = Some(call.clone())
                }
                Edit::Call(StationSide::Contacted, call) => {
                    projection.worked_call = Some(call.clone())
                }
                Edit::Start(start, minute) => {
                    projection.start = Some(*start);
                    projection.minute_precision = *minute;
                }
                Edit::Primary(pair, _) => projection.primary = Some(pair.clone()),
                Edit::Mode(mode) => projection.mode = Some(mode.clone()),
                _ => {}
            }
        }
        projection.edits.retain_mut(|edit| match edit {
            Edit::RelativeEnd {
                clock,
                minute_precision,
            } => {
                let Some(end) = projection
                    .start
                    .and_then(|start| radio::relative_end(start, *clock, *minute_precision))
                else {
                    return false;
                };
                *edit = Edit::End(Some(end), *minute_precision);
                true
            }
            Edit::End(Some(end), minute_precision) => {
                if let Some(start) = projection.start
                    && *end < start
                {
                    *end = radio::keep_seconds(*end, start, *minute_precision);
                    return *end >= start;
                }
                true
            }
            _ => true,
        });
        projection
    }
}

/// What the row contributes under the current review choices.
#[derive(Clone, Debug)]
pub(crate) struct RowProjection {
    pub(super) number: usize,
    pub(super) span: Range<usize>,
    pub(super) provenance: Arc<str>,
    pub worked_call: Option<RecordedCallsign>,
    pub own_call: Option<RecordedCallsign>,
    pub start: Option<UtcSecond>,
    /// Whether the supplied time omitted seconds.
    pub minute_precision: bool,
    pub primary: Option<BandFrequency>,
    pub mode: Option<Mode>,
    pub(super) edits: Vec<Edit>,
}

impl RowProjection {
    /// Return contact fields that have a proposed value or explicit clear.
    pub(crate) fn present(&self) -> BTreeSet<FieldId> {
        let mut fields: BTreeSet<_> = self
            .edits
            .iter()
            .filter_map(Edit::field)
            .map(FieldId::Qso)
            .collect();
        if self
            .edits
            .iter()
            .any(|edit| matches!(edit, Edit::StationOperator(_)))
        {
            fields.insert(FieldId::Qso(QsoField::OwnOperatorCall));
        }
        fields
    }

    /// Build editable QSO fields from the selected facts. Use the explicit historical
    /// own-station callsign only if the source did not supply one.
    ///
    /// Return an error if required facts are missing. The caller assigns the destination tag.
    pub(crate) fn create(
        &self,
        id: QsoId,
        historical_own_call: Option<RecordedCallsign>,
    ) -> anyhow::Result<QsoFields> {
        let station = |callsign| StationSnapshot {
            callsign,
            location: None,
            references: BTreeSet::new(),
        };
        let mut value = QsoFields {
            id,
            start: self
                .start
                .ok_or_else(|| anyhow::anyhow!("Enter a valid source start date and time"))?,
            end: None,
            own_station: station(
                self.own_call
                    .clone()
                    .or(historical_own_call)
                    .ok_or_else(|| anyhow::anyhow!("Assign the historical station callsign"))?,
            ),
            contacted_station: station(
                self.worked_call
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("Enter a valid worked station callsign"))?,
            ),
            own_context: OwnOperatingContext::default(),
            contacted_operator: None,
            primary: self
                .primary
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Enter a frequency or band"))?,
            receive_override: None,
            mode: self
                .mode
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Enter a mode"))?,
            sent_report: None,
            received_report: None,
            comment: None,
            tags: BTreeSet::new(),
        };
        self.apply(&mut value)?;
        Ok(value)
    }

    /// Apply source edits to the editable QSO without validating the combined QSO.
    ///
    /// Missing, invalid and omitted disputed facts leave the target field untouched.
    /// Preserve compatible saved seconds, exact frequencies, and park qualifiers.
    pub(crate) fn apply(&self, target: &mut QsoFields) -> anyhow::Result<()> {
        if let Some(start) = self.start {
            target.start = radio::keep_seconds(start, target.start, self.minute_precision);
        }
        let mut own_location = target
            .own_station
            .location
            .clone()
            .map(RecordedLocation::into_fields)
            .unwrap_or_default();
        let mut worked_location = target
            .contacted_station
            .location
            .clone()
            .map(RecordedLocation::into_fields)
            .unwrap_or_default();
        let mut own_operator = target
            .own_context
            .operator
            .clone()
            .map(OperatorInformation::into_fields)
            .unwrap_or_default();
        let mut worked_operator = target
            .contacted_operator
            .clone()
            .map(OperatorInformation::into_fields)
            .unwrap_or_default();
        let mut equipment = target
            .own_context
            .equipment
            .clone()
            .map(EquipmentValues::into_fields)
            .unwrap_or_default();
        for edit in &self.edits {
            let side = edit.field().and_then(|field| match field {
                QsoField::Station { side, .. } => Some(side),
                _ => None,
            });
            let (station, location) = match side {
                Some(StationSide::Own) => (&mut target.own_station, &mut own_location),
                _ => (&mut target.contacted_station, &mut worked_location),
            };
            match edit {
                Edit::Call(_, value) => station.callsign = value.clone(),
                Edit::StationOperator(value) => {
                    station.callsign = value.clone();
                    own_operator.callsign = Some(value.clone());
                }
                Edit::Position(..)
                | Edit::ClearPosition(..)
                | Edit::Dxcc(..)
                | Edit::State(..)
                | Edit::County(..)
                | Edit::Cq(..)
                | Edit::Itu(..) => {
                    edit.apply_location(location);
                }
                Edit::Contest(value, _) => {
                    station.references.insert(value.clone());
                }
                Edit::Reference(_, value) => {
                    let references = &mut station.references;
                    if let Some(park) = station::park_identity(value) {
                        if value.identifier().as_deref() == Some(park)
                            && references
                                .iter()
                                .any(|saved| station::park_identity(saved) == Some(park))
                        {
                            continue;
                        }
                        references.retain(|saved| station::park_identity(saved) != Some(park));
                    }
                    references.insert(value.clone());
                }
                Edit::ClearReference(_, group) => station
                    .references
                    .retain(|reference| reference.group_id != *group),
                Edit::Start(..) | Edit::RelativeEnd { .. } => {}
                Edit::End(value, minute_precision) => {
                    target.end = value.map(|incoming| {
                        let end = if incoming < target.start {
                            radio::keep_seconds(incoming, target.start, *minute_precision)
                        } else {
                            incoming
                        };
                        target.end.map_or(end, |saved| {
                            radio::keep_seconds(end, saved, *minute_precision)
                        })
                    });
                }
                Edit::Primary(value, clear_frequency) => {
                    target.primary = merge_pair(value, &target.primary, *clear_frequency)?;
                }
                Edit::Receive(value, clear_frequency) => {
                    let pair = match &target.receive_override {
                        Some(saved) => merge_pair(value, saved, *clear_frequency)?,
                        None => value.clone(),
                    };
                    target.receive_override = Some(pair);
                }
                Edit::ClearRadio { receive, clear } => {
                    if *receive {
                        if let Some(saved) = &target.receive_override {
                            target.receive_override = clear.remaining(saved)?;
                        }
                    } else if let Some(pair) = clear.remaining(&target.primary)? {
                        target.primary = pair;
                    }
                }
                Edit::Mode(value) => target.mode = value.clone(),
                Edit::ClearSubmode => {
                    if let ModeIdentity::Specialization {
                        parent: Some(label),
                        ..
                    } = target.mode.identity()
                    {
                        target.mode = Mode::from_recorded_identity(ModeIdentity::Parent { label })?;
                    }
                }
                Edit::OperatorCall(StationSide::Own, value) => {
                    own_operator.callsign.clone_from(value)
                }
                Edit::OperatorCall(StationSide::Contacted, value) => {
                    worked_operator.callsign.clone_from(value)
                }
                Edit::OperatorName(StationSide::Own, value) => {
                    own_operator.recorded_name.clone_from(value)
                }
                Edit::OperatorName(StationSide::Contacted, value) => {
                    worked_operator.recorded_name.clone_from(value)
                }
                Edit::Power(value) => target.own_context.transmit_power = *value,
                Edit::Sent(value) => target.sent_report.clone_from(value),
                Edit::Received(value) => target.received_report.clone_from(value),
                Edit::Comment(value) => target.comment.clone_from(value),
                Edit::Equipment(value) => equipment.equipment_description.clone_from(value),
                Edit::Antenna(value) => equipment.antenna_description.clone_from(value),
                Edit::Claim(_) => {}
            }
        }
        target.own_station.location = own_location
            .normalized()
            .map(TryInto::try_into)
            .transpose()?;
        target.contacted_station.location = worked_location
            .normalized()
            .map(TryInto::try_into)
            .transpose()?;
        target.own_context.operator = operator(own_operator)?;
        target.contacted_operator = operator(worked_operator)?;
        target.own_context.equipment = (equipment != EquipmentValuesFields::default())
            .then_some(equipment)
            .map(TryInto::try_into)
            .transpose()?;
        Ok(())
    }
}

/// Keep the saved frequency when the source supplies a compatible band
/// and leaves frequency unspecified.
fn merge_pair(
    incoming: &BandFrequency,
    saved: &BandFrequency,
    clear_frequency: bool,
) -> anyhow::Result<BandFrequency> {
    let Some(band) = incoming
        .band()
        .filter(|_| incoming.frequency().is_none() && !clear_frequency)
    else {
        return Ok(incoming.clone());
    };
    let matches = |other: &Band| other.label().eq_ignore_ascii_case(band.label());
    let derived = saved.frequency().and_then(|frequency| {
        RecognitionCatalog::adif_318()
            .ok()
            .and_then(|catalog| Band::for_frequency(frequency, catalog))
    });
    if !derived
        .as_ref()
        .or_else(|| saved.band())
        .is_some_and(matches)
    {
        return Ok(incoming.clone());
    }
    if saved.band().is_some_and(matches) {
        return Ok(saved.clone());
    }
    BandFrequency::from_recorded_pair(saved.frequency(), Some(band.clone()))
}

fn operator(fields: OperatorInformationFields) -> anyhow::Result<Option<OperatorInformation>> {
    fields.normalized().map(TryInto::try_into).transpose()
}

/// The kind of saved position an explicit empty source field can clear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PositionClear {
    /// Clear a recorded grid area.
    Grid,
    /// Clear a recorded point.
    Point,
    /// Clear either position representation.
    Both,
}

/// One change proposed by the source. An optional value of `None` requests a clear.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Edit {
    /// A source callsign used for contact identity on the indicated station side.
    Call(StationSide, RecordedCallsign),
    /// Operator callsign supplying both station and operator under ADIF's absent-station rule.
    StationOperator(RecordedCallsign),
    /// Required start instant and whether its source precision is minutes.
    Start(UtcSecond, bool),
    /// Optional end instant and whether the source time omitted seconds.
    End(Option<UtcSecond>, bool),
    /// End clock supplied without a date, relative to the reviewed source start.
    RelativeEnd {
        clock: jiff::civil::Time,
        /// Whether the source time omitted seconds.
        minute_precision: bool,
    },
    /// Required transmit pair and whether the source explicitly cleared the frequency.
    Primary(BandFrequency, bool),
    /// Supplied independent receive pair and whether the source explicitly cleared its frequency.
    Receive(BandFrequency, bool),
    /// Explicit empty radio fields when the source supplies no replacement value.
    ClearRadio {
        receive: bool,
        clear: radio::Clear,
    },
    Mode(Mode),
    /// Explicitly clear the specialization where a recorded parent can remain.
    ClearSubmode,
    OperatorCall(StationSide, Option<RecordedCallsign>),
    OperatorName(StationSide, Option<String>),
    Power(Option<Milliwatts>),
    Sent(Option<String>),
    Received(Option<String>),
    Comment(Option<String>),
    Equipment(Option<String>),
    Antenna(Option<String>),
    Position(StationSide, Position),
    /// Clear only the explicitly empty position inputs.
    ClearPosition(StationSide, PositionClear),
    Dxcc(StationSide, Option<String>),
    State(StationSide, Option<String>),
    County(StationSide, Option<String>),
    Cq(StationSide, Option<CqZone>),
    Itu(StationSide, Option<ItuZone>),
    Reference(StationSide, ReferenceAssociation),
    /// Own-station contest participation and its supplied interchange code.
    Contest(ReferenceAssociation, String),
    /// Clear one explicitly empty association group.
    ClearReference(StationSide, GroupId),
    Claim(ClaimFact),
}

impl Edit {
    /// The caller supplies the location for the edit's station side. A position
    /// clear affects only the saved position kind it names.
    pub(super) fn apply_location(&self, location: &mut RecordedLocationFields) {
        match self {
            Self::Position(_, value) => location.position = Some(value.clone()),
            Self::ClearPosition(_, clear) => {
                if matches!(
                    (*clear, location.position.as_ref()),
                    (PositionClear::Grid, Some(Position::GridArea(_)))
                        | (PositionClear::Point, Some(Position::ExplicitPoint { .. }))
                        | (PositionClear::Both, Some(_))
                ) {
                    location.position = None;
                }
            }
            Self::Dxcc(_, value) => location.dxcc.clone_from(value),
            Self::State(_, value) => location.primary_subdivision.clone_from(value),
            Self::County(_, value) => location.secondary_subdivision.clone_from(value),
            Self::Cq(_, value) => location.cq_zone = *value,
            Self::Itu(_, value) => location.itu_zone = *value,
            _ => {}
        }
    }

    pub(super) fn field(&self) -> Option<QsoField> {
        let station = |side, field| QsoField::Station { side, field };
        Some(match self {
            Self::Call(side, _) => station(*side, StationField::Callsign),
            Self::StationOperator(_) => station(StationSide::Own, StationField::Callsign),
            Self::Position(side, _) | Self::ClearPosition(side, _) => {
                station(*side, StationField::Position)
            }
            Self::Dxcc(side, _) => station(*side, StationField::Dxcc),
            Self::State(side, _) => station(*side, StationField::State),
            Self::County(side, _) => station(*side, StationField::County),
            Self::Cq(side, _) => station(*side, StationField::Cq),
            Self::Itu(side, _) => station(*side, StationField::Itu),
            Self::Reference(side, _) | Self::ClearReference(side, _) => {
                station(*side, StationField::References)
            }
            Self::Contest(..) => station(StationSide::Own, StationField::References),
            Self::OperatorCall(StationSide::Own, _) => QsoField::OwnOperatorCall,
            Self::OperatorCall(StationSide::Contacted, _) => QsoField::ContactOperatorCall,
            Self::OperatorName(StationSide::Own, _) => QsoField::OwnOperatorName,
            Self::OperatorName(StationSide::Contacted, _) => QsoField::ContactOperatorName,
            Self::Start(..) => QsoField::Start,
            Self::End(..) | Self::RelativeEnd { .. } => QsoField::End,
            Self::Primary(..) | Self::ClearRadio { receive: false, .. } => QsoField::Primary,
            Self::Receive(..) | Self::ClearRadio { receive: true, .. } => QsoField::Receive,
            Self::Mode(_) | Self::ClearSubmode => QsoField::Mode,
            Self::Power(_) => QsoField::Power,
            Self::Sent(_) => QsoField::Sent,
            Self::Received(_) => QsoField::Received,
            Self::Comment(_) => QsoField::Comment,
            Self::Equipment(_) => QsoField::Equipment,
            Self::Antenna(_) => QsoField::Antenna,
            Self::Claim(_) => return None,
        })
    }
}

fn optional_call(value: &str) -> anyhow::Result<Option<RecordedCallsign>> {
    optional_text(value)
        .map(|call| RecordedCallsign::new(call.trim()))
        .transpose()
        .map_err(Into::into)
}

fn decimal(value: &str) -> &str {
    let value = value.trim();
    let value = value
        .strip_prefix('+')
        .filter(|remaining| !remaining.is_empty())
        .unwrap_or(value);
    if value.contains('.') {
        value.trim_end_matches('0')
    } else {
        value
    }
}

pub(super) fn prepare(row: &mut RowInput<'_, '_>, definitions: &DefinitionSet) {
    row.map("CALL", |value| {
        Ok(Edit::Call(
            StationSide::Contacted,
            RecordedCallsign::new(value.trim())?,
        ))
    });
    let has_station_callsign = row.supplied("STATION_CALLSIGN");
    row.map("STATION_CALLSIGN", |value| {
        Ok(Edit::Call(
            StationSide::Own,
            RecordedCallsign::new(value.trim())?,
        ))
    });
    row.map("OPERATOR", |value| {
        let call = optional_call(value)?;
        Ok(match call {
            Some(call) if !has_station_callsign => Edit::StationOperator(call),
            call => Edit::OperatorCall(StationSide::Own, call),
        })
    });
    row.map("CONTACTED_OP", |value| {
        Ok(Edit::OperatorCall(
            StationSide::Contacted,
            optional_call(value)?,
        ))
    });
    row.map("MY_NAME", |value| {
        Ok(Edit::OperatorName(StationSide::Own, optional_text(value)))
    });
    row.map("NAME", |value| {
        Ok(Edit::OperatorName(
            StationSide::Contacted,
            optional_text(value),
        ))
    });
    row.map("RST_SENT", |value| Ok(Edit::Sent(optional_text(value))));
    row.map("RST_RCVD", |value| Ok(Edit::Received(optional_text(value))));
    row.map("COMMENT", |value| Ok(Edit::Comment(optional_text(value))));
    row.map("MY_RIG", |value| Ok(Edit::Equipment(optional_text(value))));
    row.map("MY_ANTENNA", |value| {
        Ok(Edit::Antenna(optional_text(value)))
    });
    row.map("TX_PWR", |value| {
        Ok(Edit::Power(
            crate::operating::power::parse_watts(decimal(value)).map_err(anyhow::Error::msg)?,
        ))
    });

    for field in ["ADIF_VER", "PROGRAMID", "PROGRAMVERSION"] {
        row.handle(field);
    }
    radio::prepare(row);
    station::prepare(row, StationSide::Own, definitions);
    station::prepare(row, StationSide::Contacted, definitions);
}
