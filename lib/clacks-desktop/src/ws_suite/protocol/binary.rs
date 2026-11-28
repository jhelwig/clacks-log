//! Checked big-endian Qt primitives shared by schemas 2 and 3.
use super::IncomingFacts;
use clacks_model::scalar::UtcSecond;
use std::collections::BTreeMap;
use winnow::{Parser, error::ContextError};

/// Consume one big-endian unsigned 32-bit field.
pub(super) fn u32(input: &mut &[u8]) -> anyhow::Result<u32> {
    winnow::binary::be_u32::<_, ContextError>
        .parse_next(input)
        .map_err(|_| anyhow::anyhow!("Truncated WS integer."))
}

/// Consume one big-endian unsigned 64-bit field.
pub(super) fn u64(input: &mut &[u8]) -> anyhow::Result<u64> {
    winnow::binary::be_u64::<_, ContextError>
        .parse_next(input)
        .map_err(|_| anyhow::anyhow!("Truncated WS integer."))
}

/// Consume a nullable Qt byte array, checking its declared length before access.
pub(super) fn bytes<'a>(input: &mut &'a [u8]) -> anyhow::Result<&'a [u8]> {
    let length = u32(input)?;
    if length == u32::MAX {
        return Ok(&[]);
    }
    winnow::token::take::<_, _, ContextError>(length as usize)
        .parse_next(input)
        .map_err(|_| anyhow::anyhow!("Truncated WS byte array."))
}

/// Decode a structured UTF-8 field without guessing an alternative encoding.
pub(super) fn text(input: &mut &[u8]) -> anyhow::Result<String> {
    Ok(std::str::from_utf8(bytes(input)?)
        .map_err(|_| anyhow::anyhow!("Invalid UTF-8 in structured WS text."))?
        .into())
}

/// Decode Qt's represented UTC or fixed-offset timestamp; local/named zones are unsupported.
fn timestamp(input: &mut &[u8]) -> anyhow::Result<UtcSecond> {
    let julian = u64(input)? as i64;
    let milliseconds = u32(input)?;
    let spec = winnow::binary::u8::<_, ContextError>
        .parse_next(input)
        .map_err(|_| anyhow::anyhow!("Truncated WS timestamp."))?;
    anyhow::ensure!(milliseconds < 86_400_000, "Invalid WS time of day.");
    let offset = match spec {
        1 => 0,
        2 => i64::from(u32(input)? as i32),
        _ => anyhow::bail!("Unsupported WS timestamp time zone."),
    };
    let seconds = julian
        .checked_sub(2_440_588)
        .and_then(|days| days.checked_mul(86_400))
        .and_then(|seconds| seconds.checked_add(i64::from(milliseconds / 1000)))
        .and_then(|seconds| seconds.checked_sub(offset))
        .ok_or_else(|| anyhow::anyhow!("WS timestamp out of range."))?;
    // Qt uses a null/invalid date sentinel; reject it and unrepresentable calendar dates.
    jiff::civil::date(1970, 1, 1)
        .at(0, 0, 0, 0)
        .checked_add(jiff::SignedDuration::from_secs(seconds))
        .map_err(|_| anyhow::anyhow!("Invalid WS date."))?;
    Ok(UtcSecond::from_unix_seconds(seconds))
}

/// Decode the established type 5 body, accepting absent optional appended fields.
pub(super) fn structured(input: &mut &[u8]) -> anyhow::Result<IncomingFacts> {
    let end = timestamp(input)?;
    let mut values = BTreeMap::new();
    values.insert("CALL".into(), text(input)?);
    values.insert("GRIDSQUARE".into(), text(input)?);
    let frequency = u64(input)?;
    values.insert(
        "FREQ".into(),
        format!("{}.{:06}", frequency / 1_000_000, frequency % 1_000_000),
    );
    for field in ["MODE", "RST_SENT", "RST_RCVD", "TX_PWR", "COMMENT", "NAME"] {
        values.insert(field.into(), text(input)?);
    }
    let start = timestamp(input)?;
    for field in [
        "OPERATOR",
        "STATION_CALLSIGN",
        "MY_GRIDSQUARE",
        "STX_STRING",
        "SRX_STRING",
        "PROP_MODE",
        "SAT_NAME",
        "SAT_MODE",
        "FREQ_RX",
    ] {
        if input.is_empty() {
            break;
        }
        values.insert(field.into(), text(input)?);
    }
    Ok(IncomingFacts {
        values,
        start,
        end: Some(end),
        warnings: vec![],
    })
}
