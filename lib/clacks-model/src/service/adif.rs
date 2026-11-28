//! Canonical semantic ADIF fields shared by service projections.

use super::error::{ProjectionResult, ProjectionWarning, ServiceError};
use crate::{
    export::{ExportContext, ProfileSet, select::ContextResolution},
    group::DefinitionSet,
    ids::QsoId,
    position::Coordinates,
    recognition::{
        RecognitionCatalog,
        mode::{Mode, ModeIdentity, ModeInterpretation},
    },
    scalar::{Hertz, Milliwatts, UtcSecond},
};
use std::collections::BTreeMap;

/// Canonical emitted semantic values, excluding ADI field framing and encoding.
pub type AdifFields = BTreeMap<String, String>;

/// Read-only recognition/reference configuration used for ordinary service output.
pub struct ProjectionContext<'a> {
    /// Current recognition metadata; never rewrites historical values.
    pub recognition: &'a RecognitionCatalog,
    /// Account reference definitions.
    pub definitions: &'a DefinitionSet,
    /// Account output profiles.
    pub profiles: &'a ProfileSet,
    /// Ordinary interchange context, not POTA-expanded output.
    pub export: &'a ExportContext,
    /// Explicit per-contact ambiguity decisions.
    pub resolutions: &'a BTreeMap<QsoId, ContextResolution>,
}

/// Convert a native start/end instant to ADIF date and time; reject unrepresentable years.
pub fn contact_time(value: UtcSecond) -> anyhow::Result<(String, String)> {
    let t = jiff::civil::date(1970, 1, 1)
        .at(0, 0, 0, 0)
        .checked_add(jiff::SignedDuration::from_secs(value.unix_seconds()))?;
    if !(1..=9999).contains(&t.year()) {
        tracing::debug!(
            event = "projection.conversion_unavailable",
            reason = "year_unrepresentable"
        );
        return Err(ServiceError::Invalid {
            detail: format!("ADIF year {} is outside 1..=9999", t.year()),
        }
        .into());
    }
    Ok((
        format!("{:04}{:02}{:02}", t.year(), t.month(), t.day()),
        format!("{:02}{:02}{:02}", t.hour(), t.minute(), t.second()),
    ))
}

/// Format whole hertz as exact decimal megahertz, preserving all six fractional digits.
pub fn frequency(value: Hertz) -> String {
    let n = value.get();
    format!("{}.{:06}", n / 1_000_000, n % 1_000_000)
}

/// Format recorded milliwatts as exact decimal watts with three fractional digits.
pub fn power(value: Milliwatts) -> String {
    let n = value.get();
    format!("{}.{:03}", n / 1000, n % 1000)
}

/// Format one signed coordinate at ADIF's thousandth-minute precision.
fn angle(value: i32, positive: char, negative: char) -> String {
    let n = u64::from(value.unsigned_abs());
    let minutes = (n * 60_000 + 500_000) / 1_000_000;
    format!(
        "{}{:03} {:02}.{:03}",
        if value < 0 { negative } else { positive },
        minutes / 60_000,
        (minutes % 60_000) / 1000,
        minutes % 1000
    )
}

/// Emit point coordinates with deterministic nearest rounding and hemisphere from the source sign.
pub fn coordinates(value: Coordinates) -> (String, String) {
    (
        angle(value.latitude_microdegrees(), 'N', 'S'),
        angle(value.longitude_microdegrees(), 'E', 'W'),
    )
}

/// Project a preserved hierarchy to MODE/SUBMODE using current recognition without modifying it.
pub fn mode(value: &Mode, catalog: &RecognitionCatalog) -> AdifFields {
    let (parent, sub) = match value.interpret(catalog) {
        ModeInterpretation::Known(m) => {
            (Some(m.mode.code.clone()), m.submode.map(|s| s.code.clone()))
        }
        ModeInterpretation::Other(ModeIdentity::Parent { label }) => (Some(label), None),
        ModeInterpretation::Other(ModeIdentity::Specialization { label, parent }) => {
            (parent, Some(label))
        }
    };
    let mut fields = AdifFields::new();
    if let Some(v) = parent {
        fields.insert("MODE".into(), v);
    }
    if let Some(v) = sub {
        fields.insert("SUBMODE".into(), v);
    }
    fields
}

/// Transliterate optional ADI text, reporting any loss and omitting meaningless output.
pub fn text(field: &str, value: &str) -> ProjectionResult<Option<String>> {
    let mut converted = String::new();
    let mut changed = false;
    for c in value.chars() {
        if c.is_ascii() && (!c.is_control() || matches!(c, '\n' | '\r' | '\t')) {
            converted.push(c);
        } else if let Some(s) = deunicode::deunicode_char(c) {
            converted.push_str(s);
            changed = true;
        } else {
            changed = true;
        }
    }
    let value = (!converted.trim().is_empty()).then_some(converted);
    ProjectionResult {
        value,
        warnings: if changed {
            vec![ProjectionWarning {
                field: field.into(),
                detail: "text transliterated or unsupported characters omitted".into(),
            }]
        } else {
            Vec::new()
        },
    }
}
