//! Frequency/band input with exact whole-hertz conversion and edit authority.

use clacks_model::{
    contact::BandFrequency,
    recognition::{RecognitionCatalog, band::Band},
    scalar::Hertz,
};

/// Independently editable radio pair; no frequency is invented for band-only input.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RadioForm {
    /// Decimal MHz with at most six fractional places.
    pub frequency: String,
    /// Recorded band label, including unfamiliar labels.
    pub band: String,
}

impl RadioForm {
    /// Copy recorded labels and exact frequency without recognition changes.
    pub fn from_pair(pair: Option<&BandFrequency>) -> Self {
        Self {
            frequency: pair
                .and_then(BandFrequency::frequency)
                .map(format_mhz)
                .unwrap_or_default(),
            band: pair
                .and_then(BandFrequency::band)
                .map(|v| v.label().to_owned())
                .unwrap_or_default(),
        }
    }

    /// Apply frequency authority only after an explicit frequency edit.
    pub fn frequency_changed(&mut self) {
        if let Ok(Some(hz)) = parse_mhz(&self.frequency)
            && let Ok(catalog) = RecognitionCatalog::adif_318()
        {
            self.band = Band::for_frequency(hz, catalog)
                .map(|v| v.label().to_owned())
                .unwrap_or_default();
        }
    }

    /// A manually changed band removes any previously recorded exact frequency.
    pub fn band_changed(&mut self) {
        self.frequency.clear();
    }

    /// Validate an optional override or primary pair without reinterpreting untouched labels.
    pub fn build(&self) -> anyhow::Result<Option<BandFrequency>> {
        let frequency = parse_mhz(&self.frequency)?;
        let band = if self.band.trim().is_empty() {
            None
        } else {
            Some(Band::from_recorded_label(self.band.trim())?)
        };
        if frequency.is_none() && band.is_none() {
            return Ok(None);
        }
        Ok(Some(BandFrequency::from_recorded_pair(frequency, band)?))
    }
}

/// Parse positive decimal MHz with whole-hertz precision and SQLite-safe range.
pub(crate) fn parse_mhz(text: &str) -> anyhow::Result<Option<Hertz>> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    anyhow::ensure!(
        !whole.is_empty() || !fraction.is_empty(),
        "Enter a frequency in MHz"
    );
    anyhow::ensure!(
        whole.bytes().all(|v| v.is_ascii_digit())
            && fraction.bytes().all(|v| v.is_ascii_digit())
            && fraction.len() <= 6,
        "Use positive MHz with at most six decimal places"
    );
    let whole = if whole.is_empty() {
        0
    } else {
        whole.parse::<u64>()?
    };
    let mut fractional = fraction.to_owned();
    while fractional.len() < 6 {
        fractional.push('0');
    }
    let hz = whole
        .checked_mul(1_000_000)
        .and_then(|v| v.checked_add(fractional.parse::<u64>().ok()?))
        .ok_or_else(|| anyhow::anyhow!("Frequency is too large"))?;
    anyhow::ensure!(hz <= i64::MAX as u64, "Frequency is too large");
    Ok(Some(Hertz::new(hz)?))
}

/// Format whole hertz as exact MHz without unnecessary trailing zeroes.
fn format_mhz(value: Hertz) -> String {
    let hz = value.get();
    if hz.is_multiple_of(1_000_000) {
        return (hz / 1_000_000).to_string();
    }
    format!("{}.{:06}", hz / 1_000_000, hz % 1_000_000)
        .trim_end_matches('0')
        .to_owned()
}
