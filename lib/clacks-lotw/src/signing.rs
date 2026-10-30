//! Definition-driven LoTW signing and standard-writer batch output.

pub mod definition;
mod input;
mod wire;

#[cfg(test)]
mod tests;

use crate::{
    certificate::{
        SigningCredential,
        eligibility::{CredentialState, eligibility},
    },
    error::SigningError,
};
use anyhow::{Context, ensure};
use clacks_model::service::{adif::AdifFields, lotw::LotwSubmission};
use definition::SignatureDefinitions;
use rand::{
    SeedableRng,
    rngs::{OsRng, StdRng},
};
use rsa::signature::{RandomizedSigner, SignatureEncoding};
use std::{collections::BTreeMap, io::Write};
use time::OffsetDateTime;

/// A frozen submission with its explicitly selected credential and current state.
pub struct BatchItem<'a> {
    /// Already normalized semantic values and external identity.
    pub submission: &'a LotwSubmission,
    /// Checked private credential used only on the server.
    pub credential: &'a SigningCredential,
    /// Adapter-supplied current signing availability.
    pub state: CredentialState,
}

/// Public producer identity placed in the artifact header.
pub struct ProducerInformation<'a> {
    /// Actual application name, not an impersonated reference executable.
    pub application: &'a str,
    /// Actual application version.
    pub version: &'a str,
}

/// Counts for a completely constructed artifact, without any upload claim.
#[derive(Debug, PartialEq, Eq)]
pub struct BatchSummary {
    /// Distinct certificate records emitted.
    pub certificates: usize,
    /// Distinct station records emitted.
    pub stations: usize,
    /// Contact records emitted, preserving supplied membership.
    pub contacts: usize,
    /// Bytes successfully written for this complete artifact.
    pub bytes_written: u64,
}

/// Construct a complete signed batch through a caller-owned standard writer.
///
/// Checks every item's representation and current eligibility before writing;
/// cryptographic and I/O errors may still leave a prefix in the sink. Discard
/// partial output on any error. On success, finalize caller-owned compression
/// before dispatch. This function neither flushes the sink nor dispatches output,
/// modifies frozen submissions, chooses credentials or records service success.
#[tracing::instrument(skip_all, name = "lotw.batch.write", level = "debug")]
pub fn write_batch<W: Write>(
    writer: &mut W,
    items: &[BatchItem<'_>],
    definitions: &SignatureDefinitions,
    producer: &ProducerInformation<'_>,
    now: OffsetDateTime,
) -> anyhow::Result<BatchSummary> {
    ensure!(!items.is_empty(), SigningError::EmptyBatch);
    let application = input::text("producer application", producer.application)?;
    let version = input::text("producer version", producer.version)?;
    ensure!(
        !application.is_empty() && !version.is_empty(),
        SigningError::Field {
            field: "producer".into(),
            reason: "empty application or version"
        }
    );
    let mut prepared = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let input = input::prepare_input(item.submission, item.credential.metadata(), definitions)
            .with_context(|| format!("prepare batch item {index}"))?;
        let date =
            OffsetDateTime::from_unix_timestamp(item.submission.key().start.unix_seconds())?.date();
        let issues = eligibility(
            item.credential.metadata(),
            item.submission.signing_identity(),
            date,
            item.state,
            now,
        );
        ensure!(
            issues.is_empty(),
            SigningError::Ineligible { index, issues }
        );
        prepared.push(input);
    }
    let mut randomness = StdRng::from_rng(OsRng).context("seed signing randomness")?;
    let header = wire::identification(&application, &version, &definitions.provenance.release);
    writer
        .write_all(&header)
        .context("write batch identification")?;
    let mut bytes_written = u64::try_from(header.len())?;
    let mut certificates: BTreeMap<&[u8], u64> = BTreeMap::new();
    let mut stations: BTreeMap<(u64, AdifFields), u64> = BTreeMap::new();
    for (index, (item, input)) in items.iter().zip(prepared).enumerate() {
        let mut records = Vec::new();
        let certificate = if let Some(id) = certificates.get(item.credential.certificate_der()) {
            *id
        } else {
            let id = u64::try_from(certificates.len())?
                .checked_add(1)
                .context("certificate reference overflow")?;
            wire::certificate(&mut records, id, item.credential.certificate_der());
            certificates.insert(item.credential.certificate_der(), id);
            id
        };
        let station_key = (certificate, input.station);
        let station = if let Some(id) = stations.get(&station_key) {
            *id
        } else {
            let id = u64::try_from(stations.len())?
                .checked_add(1)
                .context("station reference overflow")?;
            wire::station(&mut records, id, certificate, &station_key.1);
            stations.insert(station_key, id);
            id
        };
        let signature = item
            .credential
            .signing_key
            .try_sign_with_rng(&mut randomness, &input.signing_bytes)
            .with_context(|| format!("sign batch item {index}"))?;
        wire::contact(
            &mut records,
            station,
            &input.contact,
            &input.signature_field,
            &signature.to_bytes(),
            &input.signing_bytes,
        );
        writer
            .write_all(&records)
            .with_context(|| format!("write batch item {index}"))?;
        bytes_written = bytes_written
            .checked_add(u64::try_from(records.len())?)
            .context("batch length overflow")?;
    }
    let summary = BatchSummary {
        certificates: certificates.len(),
        stations: stations.len(),
        contacts: items.len(),
        bytes_written,
    };
    tracing::debug!(
        event = "signing.batch.constructed",
        certificate_count = summary.certificates,
        station_count = summary.stations,
        contact_count = summary.contacts,
        output_bytes = summary.bytes_written
    );
    Ok(summary)
}
