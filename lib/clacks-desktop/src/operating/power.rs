//! Exact decimal-watts entry shared by full and compact operating controls.

use clacks_model::scalar::Milliwatts;

/// Parse optional positive watts without rounding beyond the recorded milliwatt resolution.
pub(crate) fn parse_watts(text: &str) -> Result<Option<Milliwatts>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if (whole.is_empty() && fraction.is_empty())
        || !whole.bytes().all(|c| c.is_ascii_digit())
        || fraction.len() > 3
        || !fraction.bytes().all(|c| c.is_ascii_digit())
    {
        return Err("Enter positive watts with at most three decimal places".into());
    }
    let whole = if whole.is_empty() {
        0
    } else {
        whole
            .parse::<u64>()
            .map_err(|_| "Transmit power is too large".to_owned())?
    };
    let fraction = fraction
        .bytes()
        .fold(0_u64, |value, digit| value * 10 + u64::from(digit - b'0'))
        * 10_u64.pow(3 - u32::try_from(fraction.len()).map_err(|error| error.to_string())?);
    let value = whole
        .checked_mul(1000)
        .and_then(|value| value.checked_add(fraction))
        .filter(|value| *value <= i64::MAX as u64)
        .ok_or_else(|| "Transmit power is too large".to_owned())?;
    Milliwatts::new(value)
        .map(Some)
        .map_err(|error| error.to_string())
}

/// Format optional power as exact editable watts without insignificant trailing zeroes.
pub(crate) fn format_watts(value: Option<Milliwatts>) -> String {
    let Some(value) = value.map(Milliwatts::get) else {
        return String::new();
    };
    if value.is_multiple_of(1000) {
        return (value / 1000).to_string();
    }
    format!("{}.{:03}", value / 1000, value % 1000)
        .trim_end_matches('0')
        .to_owned()
}
