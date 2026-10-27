//! Typed literal matching, exact numeric thresholds and UTC date bounds.

use super::{
    diagnostic::QueryDiagnostic,
    field::{Field, FieldType},
};
use crate::{position::Locator, recognition::RecognitionCatalog};
use num_bigint::BigInt;
use num_integer::Integer;
use std::ops::Range;
use time::{Date, OffsetDateTime, format_description::well_known::Rfc3339};
use winnow::{
    Parser,
    ascii::{alpha0, digit0},
    combinator::{alt, opt, preceded},
    error::ContextError,
    token::one_of,
};

/// Literal text matching; metacharacters never become query operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextMatch {
    /// Literal substring anywhere in the value.
    Contains,
    /// Literal prefix, including slash components.
    Prefix,
    /// Entire value, without trimming or accent normalization.
    Whole,
}

/// Inclusive bounds on whole native units, restricted to present values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntegerRange {
    /// Inclusive minimum, absent when unrestricted.
    pub lower: Option<i64>,
    /// Inclusive maximum, absent when unrestricted.
    pub upper: Option<i64>,
    /// No native integer can satisfy this restriction.
    pub empty: bool,
}

/// A value's typed matching semantics, independent of database syntax.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    /// Presence according to this field's source binding.
    Present,
    /// Unicode case-insensitive literal text.
    Text {
        /// Preserved search literal.
        text: String,
        /// Anchoring determined by the field/operator.
        matching: TextMatch,
    },
    /// Exact comparison after converting units or calendar-day boundaries.
    Integer(IntegerRange),
    /// Recognized family or exact preserved mode label.
    Mode {
        /// Parent or specific mode term.
        label: String,
        /// Known specializations included when the term names a parent.
        submodes: Vec<String>,
        /// Whether the current catalog recognizes a parent family.
        family: bool,
    },
    /// Location must be known to lie within this Maidenhead area.
    Grid(Locator),
    /// One group/identifier pair, not independent collection predicates.
    Reference {
        /// Exact group name.
        group: String,
        /// Exact recorded identifier.
        identifier: String,
    },
    /// Current derived positive status, independent of credentials/job readiness.
    Boolean(bool),
}

/// Escape and anchor a literal for the shared Unicode simple-fold matcher.
///
/// Use only with the supplied matching mode; the literal is never treated as
/// user-provided regular expression syntax.
pub fn literal_pattern(text: &str, matching: TextMatch) -> String {
    let literal = regex::escape(text);
    match matching {
        TextMatch::Contains => format!("(?i:{literal})"),
        TextMatch::Prefix => format!(r"\A(?i:{literal})"),
        TextMatch::Whole => format!(r"\A(?i:{literal})\z"),
    }
}

/// Decode one quoted/escaped literal without changing internal whitespace.
pub(super) fn decode(raw: &str) -> Result<String, &'static str> {
    let mut decoded = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for c in raw.chars() {
        if escaped {
            decoded.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            quoted = !quoted;
        } else {
            decoded.push(c);
        }
    }
    if quoted || escaped {
        return Err("Complete the quoted or escaped value");
    }
    if decoded.is_empty() {
        return Err("Supply a field value");
    }
    Ok(decoded)
}

/// Convert one syntactic field value after retaining quoting and escaping.
pub(super) fn parse(
    field: Field,
    raw: &str,
    span: Range<usize>,
    catalog: &RecognitionCatalog,
) -> Result<Value, QueryDiagnostic> {
    let invalid = |message| QueryDiagnostic::new(span.clone(), message);
    let (raw, op) = opt::<_, _, ContextError, _>(alt((">=", "<=", ">", "<", "=")))
        .parse_peek(raw)
        .map_err(|_| invalid("Use a supported comparison prefix"))?;
    let op = op.unwrap_or("");
    let text = decode(raw).map_err(invalid)?;
    let ordered = matches!(op, ">" | ">=" | "<" | "<=");
    if ordered
        && !matches!(
            field.kind(),
            FieldType::Time | FieldType::Frequency | FieldType::Power
        )
    {
        return Err(invalid("Ordering is only available for times and numbers"));
    }
    match field.kind() {
        FieldType::Time => timestamp(&text, op).map(Value::Integer).map_err(invalid),
        FieldType::Frequency | FieldType::Power => number(&text, op, field.kind())
            .map(Value::Integer)
            .map_err(invalid),
        FieldType::Grid => Locator::parse(&text)
            .map(Value::Grid)
            .map_err(|_| invalid("Use a valid 2–12 character grid locator")),
        FieldType::Boolean => match text.as_str() {
            "true" => Ok(Value::Boolean(true)),
            "false" => Ok(Value::Boolean(false)),
            _ => Err(invalid("Use true or false")),
        },
        FieldType::Mode => {
            let parent = catalog
                .modes
                .iter()
                .find(|r| r.code.eq_ignore_ascii_case(&text));
            let label = parent.map_or(text.clone(), |r| r.code.clone());
            let submodes = if parent.is_some() {
                catalog
                    .submodes
                    .iter()
                    .filter(|r| r.parent == label)
                    .map(|r| r.code.clone())
                    .collect()
            } else {
                Vec::new()
            };
            Ok(Value::Mode {
                label,
                submodes,
                family: parent.is_some(),
            })
        }
        FieldType::Reference => {
            let (group, identifier) = reference_parts(raw)
                .ok_or_else(|| invalid("Use group/reference with an unescaped slash separator"))?;
            Ok(Value::Reference {
                group: decode(group).map_err(invalid)?,
                identifier: decode(identifier).map_err(invalid)?,
            })
        }
        kind => Ok(Value::Text {
            text,
            matching: if op == "=" || kind == FieldType::Label {
                TextMatch::Whole
            } else if kind == FieldType::Call {
                TextMatch::Prefix
            } else {
                TextMatch::Contains
            },
        }),
    }
}

/// Split component-quoted or whole-value-quoted references before decoding escapes.
fn reference_parts(raw: &str) -> Option<(&str, &str)> {
    let mut escaped = false;
    let mut quoted = false;
    for (index, c) in raw.char_indices() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            quoted = !quoted;
        } else if c == '/' && !quoted {
            return Some((raw.get(..index)?, raw.get(index + 1..)?));
        }
    }
    reference_parts(raw.strip_prefix('"')?.strip_suffix('"')?)
}

/// Lower a decimal-unit comparison without rounding its threshold.
fn number(text: &str, op: &str, kind: FieldType) -> Result<IntegerRange, &'static str> {
    let (whole, fraction, suffix) = (
        (opt(one_of::<_, _, ContextError>(['-', '+'])), digit0).take(),
        opt(preceded('.', digit0)).map(|value| value.unwrap_or("")),
        alpha0,
    )
        .verify(|(whole, fraction, _): &(&str, &str, &str)| {
            whole.chars().any(|c| c.is_ascii_digit()) || !fraction.is_empty()
        })
        .parse(text)
        .map_err(|_: winnow::error::ParseError<&str, ContextError>| "Supply a decimal number")?;
    let multiplier: i64 = match (kind, suffix.to_ascii_lowercase().as_str()) {
        (FieldType::Frequency, "" | "mhz") => 1_000_000,
        (FieldType::Frequency, "khz") => 1_000,
        (FieldType::Frequency, "hz") => 1,
        (FieldType::Power, "" | "w") => 1_000,
        (FieldType::Power, "mw") => 1,
        _ => return Err("Use Hz/kHz/MHz for frequency or W/mW for power"),
    };
    let joined = format!("{whole}{fraction}");
    let numerator = joined
        .parse::<BigInt>()
        .map_err(|_| "Supply a decimal number")?
        * multiplier;
    let scale = u32::try_from(fraction.len()).map_err(|_| "Number has too many digits")?;
    let denominator = BigInt::from(10).pow(scale);
    let floor = numerator.div_floor(&denominator);
    let ceil = numerator.div_ceil(&denominator);
    let (lower, upper) = match op {
        ">" => (Some(floor + 1), None),
        ">=" => (Some(ceil), None),
        "<" => (None, Some(ceil - 1)),
        "<=" => (None, Some(floor)),
        _ if floor != ceil => {
            return Ok(IntegerRange {
                lower: None,
                upper: None,
                empty: true,
            });
        }
        _ => (Some(floor.clone()), Some(floor)),
    };
    let empty = lower.as_ref().is_some_and(|v| v > &BigInt::from(i64::MAX))
        || upper.as_ref().is_some_and(|v| v < &BigInt::from(i64::MIN));
    Ok(IntegerRange {
        lower: lower.and_then(|v| i64::try_from(v).ok()),
        upper: upper.and_then(|v| i64::try_from(v).ok()),
        empty,
    })
}

/// Compare exact seconds or a complete UTC calendar day.
fn timestamp(text: &str, op: &str) -> Result<IntegerRange, &'static str> {
    let date_format = time::format_description::parse_borrowed::<2>("[year]-[month]-[day]")
        .map_err(|_| "Invalid date format")?;
    let (start, end) = if let Ok(date) = Date::parse(text, &date_format) {
        let start = date.midnight().assume_utc().unix_timestamp();
        (start, start + 86_399)
    } else {
        let time = OffsetDateTime::parse(text, &Rfc3339)
            .map_err(|_| "Use a UTC date or timestamp with an explicit offset")?;
        if time.nanosecond() != 0 || text.contains('.') {
            return Err("Contact times use whole seconds");
        }
        (time.unix_timestamp(), time.unix_timestamp())
    };
    let (lower, upper) = match op {
        ">" => (end.checked_add(1), None),
        ">=" => (Some(start), None),
        "<" => (None, start.checked_sub(1)),
        "<=" => (None, Some(end)),
        _ => (Some(start), Some(end)),
    };
    Ok(IntegerRange {
        lower,
        upper,
        empty: false,
    })
}
