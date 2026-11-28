//! Explicit-offset timestamp syntax used by queries and interchange adapters.

use jiff::fmt::temporal::Pieces;
use winnow::{
    Parser,
    error::ContextError,
    token::{one_of, take_while},
};

/// Parse an RFC 3339 timestamp with complete seconds and an explicit offset.
///
/// Returns civil components and an offset for callers that need the
/// full calendar range. Requires four-digit years and two-digit components;
/// fractional seconds are limited to nine digits and offset hours to 23. Jiff
/// checks calendar validity. Callers apply their own precision requirements.
/// Leap-second syntax must end a UTC month; Jiff exposes its preceding second.
pub fn parse(value: &str) -> anyhow::Result<Pieces<'_>> {
    /// Recognize the fixed-width decimal components of interchange timestamps.
    fn digits<const N: usize>(input: &mut &str) -> winnow::ModalResult<(), ContextError> {
        take_while(N..=N, |c: char| c.is_ascii_digit())
            .void()
            .parse_next(input)
    }

    (
        (digits::<4>, '-', digits::<2>, '-', digits::<2>),
        one_of(['T', 't', ' ']),
        (digits::<2>, ':', digits::<2>, ':', digits::<2>),
        winnow::combinator::opt(('.', take_while(1..=9, |c: char| c.is_ascii_digit()))),
        winnow::combinator::alt((
            one_of(['Z', 'z']).void(),
            (one_of(['+', '-']), digits::<2>, ':', digits::<2>).void(),
        )),
    )
        .parse(value)
        .map_err(|error| {
            anyhow::anyhow!("Expected a complete timestamp with an explicit offset: {error}")
        })?;
    let parsed = Pieces::parse(value)?;
    if let Some(offset) = parsed.to_numeric_offset() {
        anyhow::ensure!(
            offset.seconds().unsigned_abs() < 24 * 60 * 60,
            "Offset hours must be less than 24"
        );
        if value.get(17..19) == Some("60") {
            let time = parsed
                .time()
                .ok_or_else(|| anyhow::anyhow!("Supply a complete timestamp"))?;
            let utc = parsed
                .date()
                .to_datetime(time)
                .checked_sub(jiff::SignedDuration::from_secs(i64::from(offset.seconds())))?;
            anyhow::ensure!(
                utc.hour() == 23 && utc.minute() == 59 && utc.day() == utc.date().days_in_month(),
                "A leap second must end a UTC calendar month"
            );
        }
    }
    Ok(parsed)
}
