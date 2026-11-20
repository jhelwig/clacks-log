//! Find eligible saved QSOs for the source rows and explain which need review.

#[cfg(test)]
mod tests;

use super::mapping::{PreparedRow, RowProjection};
use clacks_client_store::read::{ReadSnapshot, import::ImportIdentity};
use clacks_model::{
    contact::{BandFrequency, Qso},
    ids::{AccountId, QsoId},
    recognition::{
        RecognitionCatalog,
        band::Band,
        mode::{Mode, ModeIdentity, ModeInterpretation},
    },
    scalar::{RecordedCallsign, UtcSecond},
};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordKey, RecordState, RecordValue},
};
use std::collections::{BTreeMap, BTreeSet};
use tokio_util::sync::CancellationToken;

const WINDOW_SECONDS: i64 = 30 * 60;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IdentityMatch {
    Equal,
    Incomplete(IdentityGap),
    Different,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IdentityGap {
    OwnCall,
    Band,
    Mode,
    Specialization,
    SubmodeParent,
}

impl IdentityGap {
    pub fn description(self) -> &'static str {
        match self {
            Self::OwnCall => "Historical own callsign missing",
            Self::Band => "Band missing on one or both sides",
            Self::Mode => "Source mode missing",
            Self::Specialization => "One side supplies only the parent mode",
            Self::SubmodeParent => "Unfamiliar submode has a parent on one side only",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Candidate {
    pub id: QsoId,
    pub gaps: Vec<IdentityGap>,
    pub seconds_apart: u64,
}

impl Candidate {
    pub fn requires_selection(&self) -> bool {
        !self.gaps.is_empty() || self.seconds_apart != 0
    }
}

/// Find saved QSOs eligible under any source choice. Combine overlapping searches
/// for each worked callsign and load each eligible record once from the same snapshot.
pub(super) async fn read_candidates(
    snapshot: &mut ReadSnapshot,
    account: AccountId,
    source: &[PreparedRow],
) -> anyhow::Result<Vec<ClientRecord>> {
    let mut row_windows = Vec::with_capacity(source.len());
    let mut windows = BTreeSet::new();
    for row in source {
        tokio::task::consume_budget().await;
        let requests = row.candidate_windows();
        windows.extend(requests.iter().cloned());
        row_windows.push(requests);
    }
    let mut merged: Vec<(RecordedCallsign, i64, i64)> = Vec::new();
    for (call, lower, upper) in windows {
        tokio::task::consume_budget().await;
        if let Some((previous_call, _, previous_upper)) = merged.last_mut()
            && *previous_call == call
            && lower <= *previous_upper
        {
            *previous_upper = (*previous_upper).max(upper);
        } else {
            merged.push((call, lower, upper));
        }
    }

    let mut identities = BTreeMap::<_, Vec<ImportIdentity>>::new();
    for (call, lower, upper) in merged {
        let rows = snapshot.import_identities(&call, lower..=upper).await?;
        identities.entry(call).or_default().extend(rows);
    }

    let catalog = RecognitionCatalog::adif_318()?;
    let mut eligible = BTreeSet::new();
    for (row, windows) in source.iter().zip(row_windows) {
        tokio::task::consume_budget().await;
        if windows.is_empty() {
            continue;
        }
        // Review can omit any disputed secondary identity field. The undisputed
        // fields therefore determine whether any resolution can be compatible.
        let source = row.resolve(&BTreeMap::new());
        for (call, lower, upper) in windows {
            let Some(rows) = identities.get(&call) else {
                continue;
            };
            let first = rows.partition_point(|row| row.start.unix_seconds() < lower);
            for row in rows
                .iter()
                .skip(first)
                .take_while(|row| row.start.unix_seconds() <= upper)
            {
                if !compatibility(&source, &row.own_call, &row.primary, &row.mode, catalog)
                    .contains(&IdentityMatch::Different)
                {
                    eligible.insert(row.id);
                }
            }
        }
    }

    let mut records = Vec::with_capacity(eligible.len());
    for id in eligible {
        let key = RecordKey {
            account,
            id: RecordId::Qso(id),
        };
        let record = snapshot
            .record(key)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Import snapshot lost saved QSO {id}"))?;
        records.push(record);
    }
    Ok(records)
}

pub(crate) struct CandidateIndex {
    by_id: BTreeMap<QsoId, ClientRecord>,
    by_worked_call: BTreeMap<String, Vec<(i64, QsoId)>>,
    catalog: &'static RecognitionCatalog,
}

impl CandidateIndex {
    pub fn new(records: Vec<ClientRecord>, stop: &CancellationToken) -> anyhow::Result<Self> {
        let mut index = Self {
            by_id: BTreeMap::new(),
            by_worked_call: BTreeMap::new(),
            catalog: RecognitionCatalog::adif_318()?,
        };
        for record in records {
            anyhow::ensure!(!stop.is_cancelled(), "Import cancelled");
            let saved = working(&record)
                .ok_or_else(|| anyhow::anyhow!("Import candidate has no live contact"))?;
            let id = saved.id();
            index
                .by_worked_call
                .entry(saved.contacted_station().callsign.as_str().to_owned())
                .or_default()
                .push((saved.start().unix_seconds(), id));
            index.by_id.insert(id, record);
        }
        for entries in index.by_worked_call.values_mut() {
            anyhow::ensure!(!stop.is_cancelled(), "Import cancelled");
            entries.sort_unstable();
        }
        Ok(index)
    }

    pub fn saved(&self, id: QsoId) -> Option<(&ClientRecord, &Qso)> {
        let record = self.by_id.get(&id)?;
        Some((record, working(record)?))
    }

    pub fn candidates(&self, source: &RowProjection) -> Vec<Candidate> {
        let (Some(call), Some(start)) = (&source.worked_call, source.start) else {
            return vec![];
        };
        let Some(entries) = self.by_worked_call.get(call.as_str()) else {
            return vec![];
        };
        let (lower, upper) = bounds(start, source.minute_precision);
        let from = entries.partition_point(|(time, _)| *time < lower);
        let to = entries.partition_point(|(time, _)| *time <= upper);
        entries
            .iter()
            .take(to)
            .skip(from)
            .filter_map(|&(_, id)| self.saved(id))
            .filter_map(|(_, saved)| compare(source, saved, self.catalog))
            .collect()
    }
}

/// Return the inclusive UTC-second range within 30 minutes of the source start.
/// For minute-only input, include every second of the last eligible minute.
pub(super) fn bounds(start: UtcSecond, minute_precision: bool) -> (i64, i64) {
    let start_at = timestamp(start, minute_precision);
    (
        start_at.saturating_sub(WINDOW_SECONDS),
        start_at
            .saturating_add(WINDOW_SECONDS)
            .saturating_add(if minute_precision { 59 } else { 0 }),
    )
}

fn working(record: &ClientRecord) -> Option<&Qso> {
    match record.working_state()? {
        RecordState::Live(RecordValue::Qso(qso)) => Some(qso),
        _ => None,
    }
}

fn compare(source: &RowProjection, saved: &Qso, catalog: &RecognitionCatalog) -> Option<Candidate> {
    let identity = compatibility(
        source,
        &saved.own_station().callsign,
        saved.primary(),
        saved.mode(),
        catalog,
    );
    if identity.contains(&IdentityMatch::Different) {
        return None;
    }
    let seconds_apart = time_difference(source.start?, saved.start(), source.minute_precision);
    (seconds_apart <= WINDOW_SECONDS as u64).then_some(Candidate {
        id: saved.id(),
        gaps: identity
            .into_iter()
            .filter_map(|result| match result {
                IdentityMatch::Incomplete(gap) => Some(gap),
                _ => None,
            })
            .collect(),
        seconds_apart,
    })
}

fn compatibility(
    source: &RowProjection,
    own_call: &RecordedCallsign,
    primary: &BandFrequency,
    mode: &Mode,
    catalog: &RecognitionCatalog,
) -> [IdentityMatch; 3] {
    let own = compare_optional(
        source.own_call.as_ref(),
        Some(own_call),
        RecordedCallsign::eq,
        IdentityGap::OwnCall,
    );
    let source_band = source
        .primary
        .as_ref()
        .and_then(|radio| band(radio, catalog));
    let saved_band = band(primary, catalog);
    let bands = compare_optional(
        source_band.as_ref(),
        saved_band.as_ref(),
        |a, b| a.label().eq_ignore_ascii_case(b.label()),
        IdentityGap::Band,
    );
    let modes = source.mode.as_ref().map_or(
        IdentityMatch::Incomplete(IdentityGap::Mode),
        |source_mode| compatible_mode(source_mode, mode, catalog),
    );
    [own, bands, modes]
}

fn band(radio: &BandFrequency, catalog: &RecognitionCatalog) -> Option<Band> {
    radio
        .frequency()
        .and_then(|frequency| Band::for_frequency(frequency, catalog))
        .or_else(|| radio.band().cloned())
}

fn compare_optional<T: ?Sized>(
    left: Option<&T>,
    right: Option<&T>,
    equivalent: impl FnOnce(&T, &T) -> bool,
    gap: IdentityGap,
) -> IdentityMatch {
    match (left, right) {
        (Some(a), Some(b)) if equivalent(a, b) => IdentityMatch::Equal,
        (Some(_), Some(_)) => IdentityMatch::Different,
        _ => IdentityMatch::Incomplete(gap),
    }
}

fn canonical_mode(value: &Mode, catalog: &RecognitionCatalog) -> ModeIdentity {
    match value.interpret(catalog) {
        ModeInterpretation::Known(known) => match known.submode {
            Some(submode) => ModeIdentity::Specialization {
                label: submode.code.clone(),
                parent: Some(known.mode.code.clone()),
            },
            None => ModeIdentity::Parent {
                label: known.mode.code.clone(),
            },
        },
        ModeInterpretation::Other(identity) => identity,
    }
}

fn compatible_mode(left: &Mode, right: &Mode, catalog: &RecognitionCatalog) -> IdentityMatch {
    use ModeIdentity::{Parent, Specialization};
    match (
        canonical_mode(left, catalog),
        canonical_mode(right, catalog),
    ) {
        (Parent { label: a }, Parent { label: b }) if a == b => IdentityMatch::Equal,
        (
            Specialization {
                label: a,
                parent: first,
            },
            Specialization {
                label: b,
                parent: second,
            },
        ) if a == b => {
            if first == second {
                IdentityMatch::Equal
            } else {
                compare_optional(
                    first.as_deref(),
                    second.as_deref(),
                    str::eq,
                    IdentityGap::SubmodeParent,
                )
            }
        }
        (
            Parent { label },
            Specialization {
                parent: Some(parent),
                ..
            },
        )
        | (
            Specialization {
                parent: Some(parent),
                ..
            },
            Parent { label },
        ) if label == parent => IdentityMatch::Incomplete(IdentityGap::Specialization),
        _ => IdentityMatch::Different,
    }
}

fn timestamp(value: UtcSecond, minute_precision: bool) -> i64 {
    let seconds = value.unix_seconds();
    if minute_precision {
        seconds.saturating_sub(seconds.rem_euclid(60))
    } else {
        seconds
    }
}

fn time_difference(left: UtcSecond, right: UtcSecond, minute_precision: bool) -> u64 {
    timestamp(left, minute_precision).abs_diff(timestamp(right, minute_precision))
}
