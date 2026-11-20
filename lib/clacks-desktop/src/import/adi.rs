//! Read ADI records, identify the exporter, and locate problems in the source file.

#[cfg(test)]
mod tests;

use super::{
    evidence,
    mapping::{self, PreparedRow, input::RowInput},
};
use clacks_model::group::DefinitionSet;
use qso_io::adif::adi::parser::{
    framing::{field::PermissiveField, header::PermissiveHeader},
    options::{LengthUnit, ParseEncoding, ParseOpts},
    permissive::{PermissiveParser, PermissiveSection},
};
use std::{collections::BTreeMap, ops::Range, sync::Arc};

/// An ADI warning or terminal framing error with zero-based byte offsets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Diagnostic {
    pub message: String,
    /// Zero-based byte range in the original input.
    pub span: Range<usize>,
}

/// The ADIF version, program ID, and program version declared in the file header.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SourceMetadata {
    pub adif_version: Option<String>,
    pub program: Option<String>,
    pub program_version: Option<String>,
}

impl SourceMetadata {
    /// Select the Unicode-scalar length convention for the known LOG4OM build.
    fn log4om_scalar_lengths(&self) -> bool {
        self.adif_version.as_deref() == Some("3.1.0")
            && self.program.as_deref() == Some("LOG4OM2")
            && self.program_version.as_deref() == Some("2.42.1.0")
    }

    /// Recognize a QRZ export by its header program ID.
    pub(crate) fn qrz_export(&self) -> bool {
        self.program.as_deref().is_some_and(|name| {
            name.eq_ignore_ascii_case("qrzlog") || name.eq_ignore_ascii_case("QRZLogbook")
        })
    }

    fn provenance(&self) -> Arc<str> {
        let mut fields = vec!["Built-in ADI mapping".to_owned()];
        fields.extend(
            [
                ("ADIF_VER", &self.adif_version),
                ("PROGRAMID", &self.program),
                ("PROGRAMVERSION", &self.program_version),
            ]
            .into_iter()
            .filter_map(|(name, value)| Some(format!("{name} {}", value.as_ref()?))),
        );
        fields.join(", ").into()
    }
}

/// Mapped rows, warnings, and any error that stopped parsing.
#[derive(Debug)]
pub(crate) struct PreparedSource {
    /// Readable rows in original file order.
    pub rows: Vec<PreparedRow>,
    /// Number of readable records affected by each unsupported source field.
    pub unsupported: BTreeMap<String, usize>,
    /// Terminal diagnostic for the unreadable suffix, if any.
    pub terminal: Option<Diagnostic>,
    /// Decode failures in header fields or preamble.
    pub header_warnings: Vec<Diagnostic>,
    pub metadata: SourceMetadata,
}

#[cfg(test)]
pub(crate) fn prepare_source(bytes: &[u8], definitions: &DefinitionSet) -> PreparedSource {
    prepare_source_until(bytes, definitions, || false)
}

/// Map complete records up to the first framing error or cancellation.
/// An undecodable or invalid field can warn without ending the record scan.
pub(super) fn prepare_source_until(
    bytes: &[u8],
    definitions: &DefinitionSet,
    cancelled: impl Fn() -> bool,
) -> PreparedSource {
    let default = ParseOpts::default();
    let leading = PermissiveParser::new(bytes, default).next();
    let metadata = match leading {
        Some(Ok(PermissiveSection::Header(header))) => header_metadata(&header),
        _ => SourceMetadata::default(),
    };
    let options = if metadata.log4om_scalar_lengths() {
        ParseOpts {
            encoding: ParseEncoding::Utf8,
            field_length: LengthUnit::UnicodeScalars,
        }
    } else {
        default
    };
    let provenance = metadata.provenance();
    let mut prepared = PreparedSource {
        rows: Vec::new(),
        unsupported: BTreeMap::new(),
        terminal: None,
        header_warnings: Vec::new(),
        metadata,
    };
    let mut parser = PermissiveParser::new(bytes, options);
    while !cancelled()
        && let Some(section) = parser.next()
    {
        match section {
            Ok(PermissiveSection::Header(header)) => {
                if let Err(error) = header.text() {
                    prepared.header_warnings.push(Diagnostic {
                        message: format!("Header preamble: {error}"),
                        span: header.text_span(),
                    });
                }
                prepared
                    .header_warnings
                    .extend(decode_diagnostics(header.fields()));
            }
            Ok(PermissiveSection::Record(record)) => {
                let mut row = RowInput::new(
                    record.fields(),
                    record.span(),
                    prepared.rows.len() + 1,
                    Arc::clone(&provenance),
                );
                mapping::prepare(&mut row, definitions);
                evidence::prepare(&mut row, &prepared.metadata);
                for name in row.unsupported() {
                    *prepared.unsupported.entry(name).or_default() += 1;
                }
                prepared.rows.push(row.finish());
            }
            Err(error) => {
                prepared.terminal = Some(Diagnostic {
                    message: error.to_string(),
                    span: error.span(),
                });
                break;
            }
        }
    }
    prepared
}

/// Report each failed decode at its value's byte range, even for unsupported fields.
pub(super) fn decode_diagnostics(
    fields: &[PermissiveField<'_>],
) -> impl Iterator<Item = Diagnostic> {
    fields.iter().filter_map(|field| {
        let error = field.text().err()?;
        let span = field.value_span();
        Some(Diagnostic {
            message: format!(
                "{} with bytes {:02X?} at offsets {}–{} · {error}",
                field.name(),
                field.raw_data(),
                span.start,
                span.end,
            ),
            span,
        })
    })
}

/// Use a header value only if its field occurs once and decodes.
fn header_metadata(header: &PermissiveHeader<'_>) -> SourceMetadata {
    let text = |name| {
        header
            .field(name)
            .ok()
            .flatten()
            .and_then(|field| field.text().ok())
            .map(str::to_owned)
    };
    SourceMetadata {
        adif_version: text("ADIF_VER"),
        program: text("PROGRAMID"),
        program_version: text("PROGRAMVERSION"),
    }
}

/// Source text and byte range for a review alternative or warning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Alternative {
    /// Field or application property names shown during review.
    pub source: String,
    /// Byte offsets from the first field descriptor to the end of the last value.
    pub span: Range<usize>,
    pub value: String,
}

impl Alternative {
    pub(super) fn from_field(field: &PermissiveField<'_>, value: &str) -> Self {
        Self {
            source: field.name().to_ascii_uppercase(),
            span: field.descriptor_span().start..field.value_span().end,
            value: value.to_owned(),
        }
    }

    /// Use the smallest byte range that covers both contributing fields.
    pub(super) fn join(&self, other: &Self) -> Self {
        Self {
            source: format!("{} / {}", self.source, other.source),
            span: self.span.start.min(other.span.start)..self.span.end.max(other.span.end),
            value: format!("{} / {}", self.value, other.value),
        }
    }
}
