//! Single-record ADI parsing by declared byte length, including WS Latin-1 text.
use super::IncomingFacts;
use clacks_model::scalar::UtcSecond;
use std::collections::BTreeMap;
use winnow::{Parser, error::ContextError};

/// Parse the supported ADIF UTC date/time spelling without local-zone inference.
fn timestamp(date: &str, clock: &str) -> anyhow::Result<UtcSecond> {
    anyhow::ensure!(
        date.len() == 8
            && matches!(clock.len(), 4 | 6)
            && date
                .bytes()
                .chain(clock.bytes())
                .all(|c| c.is_ascii_digit()),
        "Invalid ADI date/time."
    );
    let clock = if clock.len() == 4 {
        format!("{clock}00")
    } else {
        clock.to_owned()
    };
    let format =
        time::format_description::parse_borrowed::<2>("[year][month][day][hour][minute][second]")?;
    let date = time::PrimitiveDateTime::parse(&format!("{date}{clock}"), &format)?.assume_utc();
    Ok(UtcSecond::from_unix_seconds(date.unix_timestamp()))
}

/// Parse one length-framed record with optional header; reject truncated/conflicting fields.
pub(super) fn decode(mut input: &[u8]) -> anyhow::Result<IncomingFacts> {
    let mut values = BTreeMap::new();
    let mut header = false;
    let mut ended = false;
    while !input.is_empty() {
        let start = input
            .iter()
            .position(|byte| *byte == b'<')
            .ok_or_else(|| anyhow::anyhow!("Missing ADI field marker."))?;
        let (_, rest) = input.split_at(start);
        let close = rest
            .iter()
            .position(|byte| *byte == b'>')
            .ok_or_else(|| anyhow::anyhow!("Truncated ADI field descriptor."))?;
        let (descriptor, rest) = rest.split_at(close + 1);
        let descriptor = descriptor
            .get(1..descriptor.len() - 1)
            .ok_or_else(|| anyhow::anyhow!("Invalid ADI field."))?;
        let descriptor = std::str::from_utf8(descriptor)
            .map_err(|_| anyhow::anyhow!("Invalid ADI descriptor."))?;
        let mut parts = descriptor.split(':');
        let name = parts.next().unwrap_or_default().trim().to_ascii_uppercase();
        input = rest;
        match name.as_str() {
            "EOH" => {
                anyhow::ensure!(!header, "Multiple ADI headers.");
                header = true;
                values.clear();
                continue;
            }
            "EOR" => {
                anyhow::ensure!(
                    input.iter().all(u8::is_ascii_whitespace),
                    "Multiple ADI records or trailing data."
                );
                ended = true;
                break;
            }
            _ => {}
        }
        let length = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("Missing ADI field length."))?
            .parse::<usize>()?;
        let bytes: &[u8] = winnow::token::take::<_, _, ContextError>(length)
            .parse_next(&mut input)
            .map_err(|_| anyhow::anyhow!("Truncated ADI value."))?;
        let value: String = bytes.iter().map(|byte| char::from(*byte)).collect();
        if let Some(previous) = values.insert(name, value.clone()) {
            anyhow::ensure!(previous == value, "Conflicting duplicate ADI field.");
        }
    }
    anyhow::ensure!(ended, "Missing ADI record end.");
    let start = timestamp(
        values.get("QSO_DATE").map_or("", String::as_str),
        values.get("TIME_ON").map_or("", String::as_str),
    )?;
    let mut warnings = vec![];
    let end = if values.contains_key("TIME_OFF") || values.contains_key("QSO_DATE_OFF") {
        match timestamp(
            values
                .get("QSO_DATE_OFF")
                .or_else(|| values.get("QSO_DATE"))
                .map_or("", String::as_str),
            values.get("TIME_OFF").map_or("", String::as_str),
        ) {
            Ok(value) => Some(value),
            Err(_) => {
                warnings.push("TIME_OFF".into());
                None
            }
        }
    } else {
        None
    };
    Ok(IncomingFacts {
        values,
        start,
        end,
        warnings,
    })
}
