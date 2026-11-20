//! ADI start and end times, transmit and receive pairs, and mode hierarchy.

use super::{Edit, RowInput};
use crate::logging::radio::parse_mhz;
use clacks_model::{
    contact::BandFrequency,
    recognition::{
        RecognitionCatalog,
        band::Band,
        mode::{Mode, ModeIdentity, ModeInterpretation},
    },
    scalar::UtcSecond,
};
use time::{Date, Duration, Month, OffsetDateTime, Time};

pub(super) fn prepare(row: &mut RowInput<'_, '_>) {
    times(row);
    let catalog = match RecognitionCatalog::adif_318() {
        Ok(catalog) => catalog,
        Err(error) => {
            for name in ["FREQ", "BAND", "FREQ_RX", "BAND_RX", "SUBMODE", "MODE"] {
                for input in row.values(name) {
                    row.warn(&input, &error);
                }
            }
            return;
        }
    };
    pair(row, "FREQ", "BAND", false, catalog);
    pair(row, "FREQ_RX", "BAND_RX", true, catalog);
    mode(row, catalog);
}

pub(in crate::import) fn date(value: &str) -> anyhow::Result<Date> {
    let value = value.trim();
    anyhow::ensure!(
        value.len() == 8 && value.bytes().all(|byte| byte.is_ascii_digit()),
        "Expected UTC date in YYYYMMDD form"
    );
    let year = value
        .get(..4)
        .ok_or_else(|| anyhow::anyhow!("Missing year"))?
        .parse()?;
    let month = value
        .get(4..6)
        .ok_or_else(|| anyhow::anyhow!("Missing month"))?
        .parse::<u8>()?;
    let day = value
        .get(6..)
        .ok_or_else(|| anyhow::anyhow!("Missing day"))?
        .parse()?;
    anyhow::ensure!(year > 0, "Expected a positive calendar year");
    Ok(Date::from_calendar_date(
        year,
        Month::try_from(month)?,
        day,
    )?)
}

/// Return `true` in the second tuple element when the source omits seconds.
fn time(value: &str) -> anyhow::Result<(Time, bool)> {
    let value = value.trim();
    anyhow::ensure!(
        matches!(value.len(), 4 | 6) && value.bytes().all(|byte| byte.is_ascii_digit()),
        "Expected UTC time in HHMM or HHMMSS form"
    );
    let hour = value
        .get(..2)
        .ok_or_else(|| anyhow::anyhow!("Missing hour"))?
        .parse()?;
    let minute = value
        .get(2..4)
        .ok_or_else(|| anyhow::anyhow!("Missing minute"))?
        .parse()?;
    let second = value
        .get(4..)
        .filter(|text| !text.is_empty())
        .map(str::parse)
        .transpose()?
        .unwrap_or(0);
    Ok((Time::from_hms(hour, minute, second)?, value.len() == 4))
}

/// Construct source time alternatives. An undated end uses the selected start
/// for rollover, including when the start itself needs review.
fn times(row: &mut RowInput<'_, '_>) {
    let dates = row.parsed("QSO_DATE", date);
    let mut clocks = row.parsed("TIME_ON", time);
    let precise_clocks = clocks
        .iter()
        .filter(|(_, (_, minute))| !minute)
        .map(|(_, (clock, _))| (clock.hour(), clock.minute()))
        .collect::<std::collections::BTreeSet<_>>();
    clocks.retain(|(_, (clock, minute))| {
        !minute || !precise_clocks.contains(&(clock.hour(), clock.minute()))
    });
    let mut starts = std::collections::BTreeSet::new();
    let mut values = Vec::new();
    for (date_source, date) in &dates {
        for (time_source, (start, precision)) in &clocks {
            let start =
                UtcSecond::from_unix_seconds(date.with_time(*start).assume_utc().unix_timestamp());
            starts.insert(start);
            values.push((
                date_source.join(time_source),
                Edit::Start(start, *precision),
            ));
        }
    }
    row.record("UTC start", values);

    let end_dates = row.parsed_optional("QSO_DATE_OFF", date);
    let ends = row.parsed_optional("TIME_OFF", time);
    let end_date_omitted = !row.supplied("QSO_DATE_OFF");
    let infer_end_date = end_date_omitted || end_dates.iter().any(|(_, date)| date.is_none());
    let can_clear_end = row.absent_or_empty("QSO_DATE_OFF");
    if !ends.iter().any(|(_, end)| end.is_some()) {
        for (source, _) in end_dates.iter().filter(|(_, date)| date.is_some()) {
            row.warn(source, "An end date requires a usable end time");
        }
    }
    let mut values = Vec::new();
    for (end_source, end) in ends {
        let Some((end_time, minute_precision)) = end else {
            if can_clear_end {
                values.push((end_source, Edit::End(None, false)));
            }
            continue;
        };
        if infer_end_date {
            if starts.is_empty() {
                row.warn(
                    &end_source,
                    "An end time without its date needs a source start",
                );
            } else if starts.len() == 1
                && let Some(start) = starts.first()
            {
                if let Some(end) = relative_end(*start, end_time, minute_precision) {
                    values.push((end_source.clone(), Edit::End(Some(end), minute_precision)));
                } else {
                    row.warn(
                        &end_source,
                        "Inferred end time is outside the supported calendar",
                    );
                }
            } else {
                values.push((
                    end_source.clone(),
                    Edit::RelativeEnd {
                        clock: end_time,
                        minute_precision,
                    },
                ));
            }
        }
        for (source, date) in &end_dates {
            let Some(date) = date else { continue };
            let source = source.join(&end_source);
            let end = UtcSecond::from_unix_seconds(
                date.with_time(end_time).assume_utc().unix_timestamp(),
            );
            let precedes = |start: &UtcSecond| keep_seconds(end, *start, minute_precision) < *start;
            if starts.first().is_some_and(precedes) {
                row.warn(&source, "Contact end precedes start");
            } else {
                if starts.last().is_some_and(precedes) {
                    row.warn(&source, "Contact end precedes one of the disputed starts");
                }
                values.push((source, Edit::End(Some(end), minute_precision)));
            }
        }
    }
    let mut precise_end_minutes = std::collections::BTreeSet::new();
    let mut precise_clocks = std::collections::BTreeSet::new();
    for (_, edit) in &values {
        match edit {
            Edit::End(Some(end), false) => {
                precise_end_minutes.insert(end.unix_seconds().div_euclid(60));
            }
            Edit::RelativeEnd {
                clock,
                minute_precision: false,
            } => {
                precise_clocks.insert((clock.hour(), clock.minute()));
            }
            _ => {}
        }
    }
    values.retain(|(_, edit)| match edit {
        Edit::End(Some(end), true) => {
            !precise_end_minutes.contains(&end.unix_seconds().div_euclid(60))
        }
        Edit::RelativeEnd {
            clock,
            minute_precision: true,
        } => !precise_clocks.contains(&(clock.hour(), clock.minute())),
        _ => true,
    });
    row.record("UTC end", values);
}

/// Derive an end from its clock time and the reviewed historical start date.
pub(super) fn relative_end(
    start: UtcSecond,
    end: Time,
    minute_precision: bool,
) -> Option<UtcSecond> {
    let start_date = OffsetDateTime::from_unix_timestamp(start.unix_seconds())
        .ok()?
        .date();
    let same_day = start_date.with_time(end).assume_utc();
    let end_on_start_day = keep_seconds(
        UtcSecond::from_unix_seconds(same_day.unix_timestamp()),
        start,
        minute_precision,
    );
    if end_on_start_day >= start {
        return Some(end_on_start_day);
    }
    let end = same_day
        .date()
        .checked_add(Duration::days(1))?
        .with_time(end)
        .assume_utc();
    (end.year() <= 9999).then(|| UtcSecond::from_unix_seconds(end.unix_timestamp()))
}

/// Preserve the known instant when the source supplies the same UTC minute.
pub(super) fn keep_seconds(
    source: UtcSecond,
    known: UtcSecond,
    minute_precision: bool,
) -> UtcSecond {
    if minute_precision
        && source.unix_seconds().div_euclid(60) == known.unix_seconds().div_euclid(60)
    {
        known
    } else {
        source
    }
}

/// Recorded radio components explicitly supplied as empty.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::import) enum Clear {
    /// Clear only the recorded frequency.
    Frequency,
    /// Clear only the recorded band.
    Band,
    /// Clear both components.
    Both,
}

impl Clear {
    /// Return `None` if the clear removes the last recorded component.
    /// Preserve the remaining component's recorded value.
    pub(super) fn remaining(self, saved: &BandFrequency) -> anyhow::Result<Option<BandFrequency>> {
        let (frequency, band) = match self {
            Self::Frequency => (None, saved.band().cloned()),
            Self::Band => (saved.frequency(), None),
            Self::Both => (None, None),
        };
        if frequency.is_none() && band.is_none() {
            return Ok(None);
        }
        BandFrequency::from_recorded_pair(frequency, band).map(Some)
    }
}

/// A usable frequency supplies the band. A usable band-only input or an explicit
/// clear can contribute an edit when the source supplies no usable frequency.
fn pair(
    row: &mut RowInput<'_, '_>,
    frequency_field: &str,
    band_field: &str,
    receive: bool,
    catalog: &RecognitionCatalog,
) {
    let frequencies = row.parsed_optional(frequency_field, |value| {
        parse_mhz(super::decimal(value))?.ok_or_else(|| anyhow::anyhow!("Frequency is absent"))
    });
    let bands = row.parsed_optional(band_field, |value| Ok(Band::from_recorded_label(value)?));
    let mut values = Vec::new();
    for (source, frequency) in &frequencies {
        let Some(frequency) = frequency else { continue };
        let derived = Band::for_frequency(*frequency, catalog);
        for (band_source, band) in &bands {
            if let (Some(derived), Some(band)) = (&derived, band)
                && !derived.label().eq_ignore_ascii_case(band.label())
            {
                row.warn(
                    band_source,
                    format!("Frequency supplies band {}", derived.label()),
                );
            }
        }
        match BandFrequency::new(Some(*frequency), None, catalog) {
            Ok(value) => values.push((
                source.clone(),
                if receive {
                    Edit::Receive(value, false)
                } else {
                    Edit::Primary(value, false)
                },
            )),
            Err(error) => row.warn(source, error),
        }
    }
    let empty_frequency = frequencies
        .iter()
        .find_map(|(source, value)| value.is_none().then_some(source));
    if values.is_empty() {
        for (source, band) in &bands {
            if let Some(band) = band {
                match BandFrequency::new(None, Some(band.clone()), catalog) {
                    Ok(value) => values.push((
                        empty_frequency.map_or_else(|| source.clone(), |empty| empty.join(source)),
                        if receive {
                            Edit::Receive(value, empty_frequency.is_some())
                        } else {
                            Edit::Primary(value, empty_frequency.is_some())
                        },
                    )),
                    Err(error) => row.warn(source, error),
                }
            }
        }
    }
    if values.is_empty() {
        let empty_band = bands
            .iter()
            .find_map(|(source, value)| value.is_none().then_some(source));
        let clear = match (empty_frequency, empty_band) {
            (Some(frequency), Some(band)) => Some((frequency.join(band), Clear::Both)),
            (Some(frequency), None) => Some((frequency.clone(), Clear::Frequency)),
            (None, Some(band)) => Some((band.clone(), Clear::Band)),
            (None, None) => None,
        };
        if let Some((source, clear)) = clear {
            values.push((source, Edit::ClearRadio { receive, clear }));
        }
    }
    row.record(
        if receive {
            "Receive frequency and band"
        } else {
            "Primary frequency and band"
        },
        values,
    );
}

/// Interpret known submodes and preserve unknown parent and specialization labels.
fn mode(row: &mut RowInput<'_, '_>, catalog: &RecognitionCatalog) {
    let modes = row
        .values("MODE")
        .into_iter()
        .filter(|value| !value.value.trim().is_empty())
        .collect::<Vec<_>>();
    let subs = row.values("SUBMODE");
    let empty_submode = subs
        .iter()
        .find(|value| value.value.trim().is_empty())
        .cloned();
    let subs = subs
        .into_iter()
        .filter(|value| !value.value.trim().is_empty())
        .collect::<Vec<_>>();
    let mut values = Vec::new();
    if subs.is_empty() {
        for mode in &modes {
            match crate::logging::form::parse_mode(&mode.value, "") {
                Ok(value) => {
                    if matches!(value.interpret(catalog), ModeInterpretation::Other(_)) {
                        row.warn(mode, "Unfamiliar mode retained as supplied");
                    }
                    values.push((mode.clone(), Edit::Mode(value)));
                }
                Err(error) => row.warn(mode, error),
            }
        }
    } else {
        for sub in &subs {
            let label = sub.value.trim();
            let known = catalog
                .submodes
                .iter()
                .find(|entry| entry.code.eq_ignore_ascii_case(label));
            if let Some(known) = known {
                let reason = format!("Recognized submode supplies parent {}", known.parent);
                if modes.is_empty() {
                    row.warn(sub, &reason);
                }
                for parent in modes
                    .iter()
                    .filter(|parent| !parent.value.trim().eq_ignore_ascii_case(&known.parent))
                {
                    row.warn(&parent.join(sub), &reason);
                }
            }
            let parents = (modes.is_empty() || known.is_some())
                .then_some(None)
                .into_iter()
                .chain(modes.iter().filter(|_| known.is_none()).map(Some));
            for parent in parents {
                let source = parent.map_or_else(|| sub.clone(), |parent| parent.join(sub));
                if known.is_none() {
                    row.warn(&source, "Unfamiliar submode retained as supplied");
                }
                let identity = ModeIdentity::Specialization {
                    label: known.map_or_else(|| label.to_owned(), |entry| entry.code.clone()),
                    parent: known
                        .map(|entry| entry.parent.clone())
                        .or_else(|| parent.map(|parent| parent.value.trim().to_owned())),
                };
                match Mode::from_recorded_identity(identity) {
                    Ok(value) => values.push((source, Edit::Mode(value))),
                    Err(error) => row.warn(sub, error),
                }
            }
        }
    }
    if !row.supplied("MODE")
        && let Some(source) = empty_submode
    {
        values.push((source, Edit::ClearSubmode));
    }
    row.record("Mode", values);
}
