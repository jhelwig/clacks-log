//! Recognized ADI service statuses, source dates, and source-only assertion baselines.

use super::{
    adi::{Alternative, SourceMetadata},
    mapping::{Edit, RowProjection, input::RowInput, radio},
};
use clacks_model::{
    ids::{AssertionId, QsoId},
    scalar::UtcSecond,
    service::{
        adif::ProjectionContext,
        destination::Service,
        evidence::{ImportProvenance, ImportedAssertion, ImportedClaim},
        lotw::LotwConfiguration,
    },
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;
use time::{Date, OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

mod baseline;

const LOG4OM: &str = "APP_L4ONG_QSO_CONFIRMATIONS";

/// A service date as supplied by the source, either a calendar day or a timestamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SourceDate {
    /// Calendar day with no supplied time.
    Day(Date),
    /// Parsed timestamp and the number of fractional decimal digits supplied.
    Timestamp {
        instant: OffsetDateTime,
        fractional_digits: u8,
    },
}

impl SourceDate {
    /// Return a timestamp only if the source specifies an exact whole second.
    fn whole_second(self) -> Option<UtcSecond> {
        match self {
            Self::Timestamp { instant, .. } if instant.nanosecond() == 0 => {
                Some(UtcSecond::from_unix_seconds(instant.unix_timestamp()))
            }
            _ => None,
        }
    }
}

/// An upload or confirmation status or date, selected independently in review.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ClaimFact {
    /// Whether the source reports an upload or confirmation as complete.
    Status {
        service: Service,
        claim: ImportedClaim,
        positive: bool,
        sources: Vec<Alternative>,
    },
    /// The date supplied for a completed upload or confirmation.
    Date {
        service: Service,
        claim: ImportedClaim,
        date: SourceDate,
        sources: Vec<Alternative>,
    },
}

/// Status and date values reported for one upload or confirmation, with their source fields.
#[derive(Default)]
struct Facts {
    statuses: Vec<(Alternative, Status)>,
    /// Dates that can be attributed to a positive status in that source.
    dates: Vec<(Alternative, SourceDate)>,
}

/// A service status reported in the import file.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Yes,
    No,
    /// The source values changed after a prior upload.
    Modified,
}

/// A JSON entry inside a framed LOG4OM service array.
#[derive(Deserialize)]
#[serde(untagged)]
enum Log4omEntry {
    /// An object with a service category. Keep status and date values untyped so an
    /// invalid property can be reported without discarding other properties.
    Service {
        #[serde(rename = "CT")]
        service: String,
        #[serde(rename = "S")]
        sent: Option<Value>,
        #[serde(rename = "R")]
        received: Option<Value>,
        #[serde(rename = "SD")]
        sent_at: Option<Value>,
        #[serde(rename = "RD")]
        received_at: Option<Value>,
    },
    /// An entry with no usable service object or category.
    Malformed(Value),
}

/// Map upload and confirmation independently. Use exporter authority only
/// for fields whose meaning and origin are established.
pub(super) fn prepare(row: &mut RowInput<'_, '_>, metadata: &SourceMetadata) {
    let [lotw_sent, lotw_received, qrz_sent, mut qrz_received] = log4om(row);

    let mut upload = standard(row, "LOTW_QSL_SENT", "LOTW_QSLSDATE", lotw_upload);
    upload.corroborate(lotw_sent);
    upload.record(row, Service::Lotw, ImportedClaim::Uploaded);

    let mut confirmation = standard(row, "LOTW_QSL_RCVD", "LOTW_QSLRDATE", lotw_confirm);
    confirmation.corroborate(lotw_received);
    confirmation.record(row, Service::Lotw, ImportedClaim::Confirmed);

    let mut upload = standard(
        row,
        "QRZCOM_QSO_UPLOAD_STATUS",
        "QRZCOM_QSO_UPLOAD_DATE",
        qrz_upload,
    );
    if upload
        .statuses
        .iter()
        .any(|(_, status)| *status == Status::Modified)
    {
        for (source, _) in &qrz_sent.statuses {
            row.warn(
                source,
                "Historical sent status does not establish acceptance of the changed QSO",
            );
        }
    } else {
        upload.corroborate(qrz_sent);
    }
    upload.record(row, Service::Qrz, ImportedClaim::Uploaded);

    let mut confirmation = standard(row, "APP_QRZLOG_STATUS", "APP_QRZLOG_QSLDATE", qrz_confirm);
    let mut received_dates = Vec::new();
    if confirmation.statuses.is_empty() {
        confirmation = qrz_received;
    } else {
        // QRZ's first confirmation date and LOG4OM's receipt date describe different events.
        received_dates = std::mem::take(&mut qrz_received.dates);
        if metadata.qrz_export() {
            qrz_received.statuses.retain(|(source, reported)| {
                let agrees = confirmation
                    .statuses
                    .iter()
                    .any(|(_, authored)| authored == reported);
                if !agrees {
                    row.warn(
                        source,
                        "QRZ's authored export status supplies the current confirmation state",
                    );
                }
                agrees
            });
            if !qrz_received
                .statuses
                .iter()
                .any(|(_, status)| *status == Status::Yes)
            {
                for (source, date) in received_dates.drain(..) {
                    warn_unrepresentable_date(row, &source, date);
                }
            }
        }
        confirmation.extend(qrz_received);
    }
    confirmation.record_with_distinct_dates(
        row,
        Service::Qrz,
        ImportedClaim::Confirmed,
        received_dates,
    );
}

/// A date is usable only when this source reports the corresponding positive status.
fn standard(
    row: &mut RowInput<'_, '_>,
    status_field: &str,
    date_field: &str,
    parse: fn(&str) -> Option<Status>,
) -> Facts {
    let mut result = Facts::default();
    for source in row.values(status_field) {
        let value = source.value.trim().to_ascii_uppercase();
        if let Some(status) = parse(&value) {
            result.statuses.push((source, status));
        } else if !value.is_empty() {
            row.warn(&source, "Unrecognized service status");
        }
    }
    for source in row.values(date_field) {
        if source.value.trim().is_empty() {
            continue;
        }
        match radio::date(&source.value) {
            Ok(date)
                if result
                    .statuses
                    .iter()
                    .any(|(_, status)| *status == Status::Yes) =>
            {
                result.dates.push((source, SourceDate::Day(date)));
            }
            Ok(_) => {}
            Err(error) => row.warn(&source, error),
        }
    }
    result
}

fn lotw_upload(value: &str) -> Option<Status> {
    match value {
        "Y" => Some(Status::Yes),
        "N" | "R" | "Q" | "I" => Some(Status::No),
        _ => None,
    }
}

fn lotw_confirm(value: &str) -> Option<Status> {
    match value {
        "Y" | "V" => Some(Status::Yes),
        "N" | "R" | "I" => Some(Status::No),
        _ => None,
    }
}

fn qrz_upload(value: &str) -> Option<Status> {
    match value {
        "Y" => Some(Status::Yes),
        "N" => Some(Status::No),
        "M" => Some(Status::Modified),
        _ => None,
    }
}

fn qrz_confirm(value: &str) -> Option<Status> {
    match value {
        "C" => Some(Status::Yes),
        "N" | "2" | "S" | "R" => Some(Status::No),
        _ => None,
    }
}

/// Report malformed properties per entry and continue with usable service claims.
fn log4om(row: &mut RowInput<'_, '_>) -> [Facts; 4] {
    let mut result: [Facts; 4] = Default::default();
    for source in row.values(LOG4OM) {
        if source.value.trim().is_empty() {
            continue;
        }
        let entries = match serde_json::from_str::<Vec<Log4omEntry>>(&source.value) {
            Ok(entries) => entries,
            Err(error) => {
                row.warn(&source, format!("Invalid service entries: {error}"));
                continue;
            }
        };
        let [lotw_sent, lotw_received, qrz_sent, qrz_received] = &mut result;
        for (index, entry) in entries.into_iter().enumerate() {
            match entry {
                Log4omEntry::Service {
                    service,
                    sent,
                    received,
                    sent_at,
                    received_at,
                } => {
                    let (upload, confirmation) = match service.trim().to_ascii_uppercase().as_str()
                    {
                        "LOTW" => (&mut *lotw_sent, &mut *lotw_received),
                        "QRZCOM" => (&mut *qrz_sent, &mut *qrz_received),
                        other => {
                            row.disclose(format!("{LOG4OM} ({other})"));
                            continue;
                        }
                    };
                    observe(
                        row,
                        upload,
                        &source,
                        &service,
                        ImportedClaim::Uploaded,
                        sent,
                        sent_at,
                    );
                    observe(
                        row,
                        confirmation,
                        &source,
                        &service,
                        ImportedClaim::Confirmed,
                        received,
                        received_at,
                    );
                }
                Log4omEntry::Malformed(value) => row.warn(
                    &Alternative {
                        source: format!("{LOG4OM} (entry {})", index + 1),
                        span: source.span.clone(),
                        value: value.to_string(),
                    },
                    "Expected a service object with a string CT and unique service properties",
                ),
            }
        }
    }
    result
}

fn observe(
    row: &mut RowInput<'_, '_>,
    facts: &mut Facts,
    source: &Alternative,
    provider: &str,
    claim: ImportedClaim,
    status: Option<Value>,
    date: Option<Value>,
) {
    let (status_key, date_key) = match claim {
        ImportedClaim::Uploaded => ("S", "SD"),
        ImportedClaim::Confirmed => ("R", "RD"),
    };
    let mut input = |key, value: Value| {
        let (value, is_text) = match value {
            Value::String(value) => (value, true),
            value => (value.to_string(), false),
        };
        let observed = Alternative {
            source: format!("{LOG4OM} ({provider}.{key})"),
            span: source.span.clone(),
            value,
        };
        if is_text {
            Some(observed)
        } else {
            row.warn(&observed, "Expected a string");
            None
        }
    };
    let status = status.and_then(|value| input(status_key, value));
    let date = date.and_then(|value| input(date_key, value));
    let Some(status) = status else { return };

    let interpretation = match status.value.trim().to_ascii_lowercase().as_str() {
        "yes" => Status::Yes,
        "no" => Status::No,
        _ => {
            row.warn(&status, "Unrecognized service status");
            return;
        }
    };
    facts.statuses.push((status, interpretation));
    if interpretation == Status::Yes
        && let Some(date) = date
    {
        match timestamp(&date.value) {
            Ok(value) => facts.dates.push((date, value)),
            Err(error) => row.warn(&date, error),
        }
    }
}

/// Retain the fractional digit count separately from the timestamp value.
fn timestamp(value: &str) -> anyhow::Result<SourceDate> {
    let value = value.trim();
    let instant = OffsetDateTime::parse(value, &Rfc3339)?;
    let digits = value.split_once('.').map_or(0, |(_, fraction)| {
        fraction.bytes().take_while(u8::is_ascii_digit).count()
    });
    anyhow::ensure!(digits <= 9, "Timestamp exceeds nine fractional digits");
    Ok(SourceDate::Timestamp {
        instant,
        fractional_digits: u8::try_from(digits)?,
    })
}

impl Facts {
    fn extend(&mut self, other: Self) {
        self.statuses.extend(other.statuses);
        self.dates.extend(other.dates);
    }

    /// Treat secondary data as corroboration only when a primary field supplies status.
    fn corroborate(&mut self, other: Self) {
        if !self.statuses.is_empty() {
            self.extend(other);
        }
    }

    fn record(self, row: &mut RowInput<'_, '_>, service: Service, claim: ImportedClaim) {
        self.record_with_distinct_dates(row, service, claim, Vec::new());
    }

    /// Keep dates for different events separate even when their UTC day agrees.
    fn record_with_distinct_dates(
        self,
        row: &mut RowInput<'_, '_>,
        service: Service,
        claim: ImportedClaim,
        distinct_dates: Vec<(Alternative, SourceDate)>,
    ) {
        let mut positives = Vec::new();
        let mut negatives = Vec::new();
        for (source, status) in self.statuses {
            if status == Status::Yes {
                positives.push(source);
            } else {
                negatives.push(source);
            }
        }
        let label = match (service, claim) {
            (Service::Lotw, ImportedClaim::Uploaded) => "LoTW upload",
            (Service::Lotw, ImportedClaim::Confirmed) => "LoTW confirmation",
            (Service::Qrz, ImportedClaim::Uploaded) => "QRZ upload",
            (Service::Qrz, ImportedClaim::Confirmed) => "QRZ confirmation",
        };
        let statuses = [(true, positives), (false, negatives)]
            .into_iter()
            .filter_map(|(positive, sources)| {
                let source = describe(&sources)?;
                Some((
                    source,
                    Edit::Claim(ClaimFact::Status {
                        service,
                        claim,
                        positive,
                        sources,
                    }),
                ))
            })
            .collect();
        row.record(label, statuses);
        for (source, date) in self.dates.iter().chain(&distinct_dates) {
            warn_unrepresentable_date(row, source, *date);
        }
        let dates = merge_dates(self.dates)
            .into_iter()
            .chain(merge_dates(distinct_dates))
            .filter_map(|(date, sources)| {
                let source = describe(&sources)?;
                Some((
                    source,
                    Edit::Claim(ClaimFact::Date {
                        service,
                        claim,
                        date,
                        sources,
                    }),
                ))
            })
            .collect::<Vec<_>>();
        row.record(format!("{label} date"), dates);
    }
}

/// Report a calendar-only date or timestamp with nonzero fractional seconds.
fn warn_unrepresentable_date(row: &mut RowInput<'_, '_>, source: &Alternative, date: SourceDate) {
    if date.whole_second().is_none() {
        let reason = match date {
            SourceDate::Day(_) => {
                "The calendar date supplies no time and cannot be stored as a timestamp"
            }
            SourceDate::Timestamp { .. } => {
                "The timestamp supplies fractional seconds and cannot be stored at whole-second resolution"
            }
        };
        row.warn(source, reason);
    }
}

fn describe(sources: &[Alternative]) -> Option<Alternative> {
    sources
        .iter()
        .cloned()
        .reduce(|joined, next| joined.join(&next))
}

/// Corroborate timestamps with matching calendar days. Keep distinct instants
/// as separate alternatives and retain the source fields for each.
fn merge_dates(values: Vec<(Alternative, SourceDate)>) -> Vec<(SourceDate, Vec<Alternative>)> {
    let mut candidates = values.iter().map(|(_, date)| *date).collect::<Vec<_>>();
    candidates.sort_by_key(|date| std::cmp::Reverse(precision(*date)));
    let mut distinct = Vec::new();
    for date in candidates {
        if distinct.iter().all(|saved| !agrees(*saved, date)) {
            distinct.push(date);
        }
    }
    distinct
        .into_iter()
        .map(|date| {
            let sources = values
                .iter()
                .filter(|(_, supplied)| agrees(*supplied, date))
                .map(|(source, _)| source.clone())
                .collect();
            (date, sources)
        })
        .collect()
}

/// Match timestamps by exact instant and calendar dates by UTC day.
fn agrees(a: SourceDate, b: SourceDate) -> bool {
    match (a, b) {
        (SourceDate::Day(a), b) => calendar(b) == a,
        (a, SourceDate::Day(b)) => calendar(a) == b,
        (SourceDate::Timestamp { instant: a, .. }, SourceDate::Timestamp { instant: b, .. }) => {
            a == b
        }
    }
}

fn precision(date: SourceDate) -> u8 {
    match date {
        SourceDate::Day(_) => 0,
        SourceDate::Timestamp {
            fractional_digits, ..
        } => 1 + fractional_digits,
    }
}

fn calendar(date: SourceDate) -> Date {
    match date {
        SourceDate::Day(date) => date,
        SourceDate::Timestamp { instant, .. } => instant.to_offset(UtcOffset::UTC).date(),
    }
}

impl RowProjection {
    /// A selected positive status contributes an assertion, with or without a date.
    pub(crate) fn has_assertions(&self) -> bool {
        self.edits
            .iter()
            .any(|edit| matches!(edit, Edit::Claim(ClaimFact::Status { positive: true, .. })))
    }

    /// Turn selected source claims into evidence attached to the assigned account contact.
    ///
    /// Use only the selected source values when recording what the service claim covers.
    pub(crate) fn assertions(
        &self,
        qso: QsoId,
        source: &str,
        lotw: &LotwConfiguration,
        context: &ProjectionContext<'_>,
    ) -> anyhow::Result<Vec<ImportedAssertion>> {
        let mut result = Vec::new();
        for edit in &self.edits {
            let Edit::Claim(ClaimFact::Status {
                service,
                claim,
                positive: true,
                sources,
            }) = edit
            else {
                continue;
            };
            let date = self.edits.iter().find_map(|edit| match edit {
                Edit::Claim(ClaimFact::Date {
                    service: s,
                    claim: c,
                    date,
                    sources,
                }) if s == service && c == claim => Some((*date, sources)),
                _ => None,
            });
            let provenance_fields = sources
                .iter()
                .chain(date.into_iter().flat_map(|(_, sources)| sources));
            let mapping = provenance_fields
                .map(|field| {
                    format!(
                        "{} at bytes {}–{}",
                        field.source, field.span.start, field.span.end
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let coverage = self
                .present()
                .into_iter()
                .map(|field| field.label())
                .collect::<BTreeSet<_>>();
            let baseline = baseline::capture(self, qso, *service, coverage, lotw, context)?;
            result.push(ImportedAssertion {
                id: AssertionId::new(),
                qso,
                service: *service,
                destination: None,
                claim: *claim,
                date: date.and_then(|(value, _)| value.whole_second()),
                provenance: ImportProvenance {
                    source: source.to_owned(),
                    record: format!(
                        "Record {} at bytes {}–{}",
                        self.number, self.span.start, self.span.end
                    ),
                    mapping: format!("{}; {mapping}", self.provenance),
                },
                baseline,
            });
        }
        Ok(result)
    }
}
