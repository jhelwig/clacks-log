//! Source-derived ordered signature definitions.

use crate::error::SigningError;
use anyhow::{Context, ensure};
use clacks_model::service::lotw::{LotwConfiguration, LotwMappingProvenance};
use serde::Deserialize;
use std::collections::BTreeSet;

/// Validated source-derived signature definitions and their selected version.
///
/// Load bundled or supplied data before constructing a batch. Selection follows
/// numeric version order and excludes deprecated definitions; unsupported newest
/// definitions fail instead of silently selecting an older version.
pub struct SignatureDefinitions {
    /// Mapping identity compatible with these signature fields.
    pub(super) provenance: LotwMappingProvenance,
    /// Highest supported nondeprecated definition.
    pub(super) selected: Definition,
}

/// One source-specified signing field in its declared position.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Field {
    /// Protocol field name.
    pub name: String,
    /// Whether a nonempty value is required.
    pub required: bool,
    /// Source numeric representation hint, already applied by model projection.
    pub method: Option<String>,
}

/// One source-defined signing protocol version.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Definition {
    /// Protocol name, currently LOTW.
    pub name: String,
    /// Numeric major/minor version spelling used in the output field.
    pub version: String,
    /// Source declaration excluding selection.
    pub deprecated: bool,
    /// Ordered station signing fields.
    pub station: Vec<Field>,
    /// Ordered contact signing fields.
    pub contact: Vec<Field>,
}

/// Serialized source/configuration envelope.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Data {
    /// Mapping release paired with this data.
    release: String,
    /// Pinned source commit.
    upstream_commit: String,
    /// Available protocol definitions.
    definitions: Vec<Definition>,
}

impl SignatureDefinitions {
    /// Load explicit signature data and reject unsupported or ambiguous definitions.
    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        let data: Data = serde_json::from_str(json)?;
        let config = LotwConfiguration::bundled()?;
        ensure!(
            data.release == config.release && data.upstream_commit == config.upstream_commit,
            SigningError::Definition {
                reason: "incompatible mapping provenance".into()
            }
        );
        let mut unique = BTreeSet::new();
        let mut rows = Vec::new();
        for definition in data.definitions {
            let version = version(&definition.version)?;
            ensure!(
                definition.name == "LOTW" && unique.insert((definition.name.clone(), version)),
                SigningError::Definition {
                    reason: "unsupported name or duplicate name/version".into()
                }
            );
            validate_fields(&definition.station, |n| {
                config.station_fields.iter().any(|f| f == n)
            })?;
            validate_fields(&definition.contact, |n| {
                matches!(
                    n,
                    "BAND"
                        | "BAND_RX"
                        | "CALL"
                        | "FREQ"
                        | "FREQ_RX"
                        | "MODE"
                        | "PROP_MODE"
                        | "QSO_DATE"
                        | "QSO_TIME"
                        | "SAT_NAME"
                )
            })?;
            rows.push((version, definition));
        }
        let (_, selected) = rows
            .into_iter()
            .filter(|(_, d)| !d.deprecated)
            .max_by_key(|(v, _)| *v)
            .context("no nondeprecated signature definition")?;
        ensure!(
            matches!(selected.version.as_str(), "1.0" | "2.0"),
            SigningError::Definition {
                reason: format!("unsupported selected version {}", selected.version)
            }
        );
        Ok(Self {
            provenance: LotwMappingProvenance {
                release: data.release,
                upstream_commit: data.upstream_commit,
            },
            selected,
        })
    }

    /// Load the pinned signing definitions without accessing a network or user store.
    pub fn bundled() -> anyhow::Result<Self> {
        Self::from_json(include_str!("../../data/signatures-11.34.json"))
    }
}

/// Decode numeric version components without floating-point ordering.
fn version(text: &str) -> anyhow::Result<(u16, u16)> {
    let (major, minor) = text
        .split_once('.')
        .context("signature version must have major.minor form")?;
    ensure!(
        !major.is_empty()
            && !minor.is_empty()
            && major
                .bytes()
                .chain(minor.bytes())
                .all(|b| b.is_ascii_digit()),
        SigningError::Definition {
            reason: "non-numeric version".into()
        }
    );
    Ok((major.parse()?, minor.parse()?))
}

/// Reject fields whose order or interpretation would be ambiguous.
fn validate_fields(fields: &[Field], supported: impl Fn(&str) -> bool) -> anyhow::Result<()> {
    let mut names = BTreeSet::new();
    ensure!(
        !fields.is_empty(),
        SigningError::Definition {
            reason: "empty signing field list".into()
        }
    );
    for f in fields {
        ensure!(
            supported(&f.name)
                && names.insert(&f.name)
                && f.method.as_deref().is_none_or(|m| m == "int"),
            SigningError::Definition {
                reason: format!("unsupported or duplicate field {}", f.name)
            }
        );
    }
    Ok(())
}
