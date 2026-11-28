//! Validation and wire formatting of already frozen semantic submissions.

use super::definition::{Field, SignatureDefinitions};
use crate::{certificate::CertificateMetadata, error::SigningError};
use anyhow::ensure;
use clacks_model::service::{adif::AdifFields, lotw::LotwSubmission};

/// Owned protocol values and exact signing bytes for one checked contact.
pub(super) struct PreparedInput {
    /// Formatted contact field values, excluding framing.
    pub contact: AdifFields,
    /// Station values including the explicitly supplied signing identity.
    pub station: AdifFields,
    /// Protocol field identifying the selected signature definition.
    pub signature_field: String,
    /// Station/contact definition-order concatenation, uppercased.
    pub signing_bytes: Vec<u8>,
}

/// Require a supported printable protocol value without transliteration.
pub(super) fn text(field: &str, value: &str) -> anyhow::Result<String> {
    let value = value.trim_ascii();
    ensure!(
        value.bytes().all(|b| b.is_ascii_graphic() || b == b' '),
        SigningError::Field {
            field: field.into(),
            reason: "unsupported non-ASCII or control text"
        }
    );
    Ok(value.to_owned())
}

/// Validate frozen values and perform only lossless protocol formatting.
pub(super) fn prepare_input(
    submission: &LotwSubmission,
    metadata: &CertificateMetadata,
    definitions: &SignatureDefinitions,
) -> anyhow::Result<PreparedInput> {
    ensure!(
        submission.mapping_provenance() == &definitions.provenance,
        SigningError::Definition {
            reason: "submission mapping provenance differs from signature definitions".into()
        }
    );
    if submission.signing_identity() != &metadata.identity {
        tracing::debug!(event = "signing.input.rejected", reason = ?SigningError::IdentityMismatch);
        return Err(SigningError::IdentityMismatch.into());
    }
    let mut contact = AdifFields::new();
    let mut station = AdifFields::new();
    for (name, value) in submission.contact_fields() {
        ensure!(
            definitions.selected.contact.iter().any(|f| &f.name == name),
            SigningError::Field {
                field: name.clone(),
                reason: "field not represented by signature definition"
            }
        );
        contact.insert(name.clone(), text(name, value)?);
    }
    for (name, value) in submission.station_fields() {
        ensure!(
            definitions.selected.station.iter().any(|f| &f.name == name),
            SigningError::Field {
                field: name.clone(),
                reason: "field not represented by signature definition"
            }
        );
        station.insert(name.clone(), text(name, value)?);
    }
    let instant = jiff::civil::date(1970, 1, 1).at(0, 0, 0, 0).checked_add(
        jiff::SignedDuration::from_secs(submission.key().start.unix_seconds()),
    )?;
    contact.insert(
        "QSO_DATE".into(),
        format!(
            "{:04}-{:02}-{:02}",
            instant.year(),
            instant.month(),
            instant.day()
        ),
    );
    contact.insert(
        "QSO_TIME".into(),
        format!(
            "{:02}:{:02}:{:02}Z",
            instant.hour(),
            instant.minute(),
            instant.second()
        ),
    );
    let mut signing = String::new();
    append(&mut signing, &station, &definitions.selected.station)?;
    append(&mut signing, &contact, &definitions.selected.contact)?;
    station.insert(
        "CALL".into(),
        submission.signing_identity().callsign.as_str().into(),
    );
    station.insert("DXCC".into(), submission.signing_identity().dxcc.clone());
    Ok(PreparedInput {
        contact,
        station,
        signature_field: format!(
            "SIGN_{}_V{}",
            definitions.selected.name, definitions.selected.version
        ),
        signing_bytes: signing.to_ascii_uppercase().into_bytes(),
    })
}

/// Append each present definition-ordered value without framing or separators.
fn append(output: &mut String, values: &AdifFields, fields: &[Field]) -> anyhow::Result<()> {
    for field in fields {
        let value = values
            .get(&field.name)
            .map(String::as_str)
            .unwrap_or_default();
        ensure!(
            !field.required || !value.is_empty(),
            SigningError::Field {
                field: field.name.clone(),
                reason: "missing required signing value"
            }
        );
        output.push_str(value);
    }
    Ok(())
}
