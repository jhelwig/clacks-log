//! Interpretation of issued LoTW certificate fields.

use super::CertificateMetadata;
use crate::error::CertificateError;
use anyhow::{Context, ensure};
use clacks_model::{scalar::RecordedCallsign, service::lotw::PublicSigningIdentity};
use jiff::{
    SignedDuration,
    civil::{Date, date as civil_date},
};
use x509_parser::parse_x509_certificate;

/// Interpret exact DER certificate bytes as public LoTW metadata.
pub(super) fn inspect(der: &[u8]) -> anyhow::Result<CertificateMetadata> {
    let (rest, cert) = parse_x509_certificate(der)
        .map_err(|e| e.to_owned())
        .context("decode station certificate")?;
    ensure!(rest.is_empty(), "trailing certificate bytes");
    ensure!(
        cert.public_key().algorithm.algorithm.to_id_string() == "1.2.840.113549.1.1.1",
        "unsupported certificate key algorithm"
    );
    let mut calls = cert
        .subject()
        .iter_attributes()
        .filter(|a| a.attr_type().to_id_string() == "1.3.6.1.4.1.12348.1.1");
    let call = calls
        .next()
        .ok_or(CertificateError {
            field: "AROcallsign",
            reason: "missing",
        })?
        .as_str()
        .context("decode station callsign")?;
    ensure!(
        calls.next().is_none(),
        CertificateError {
            field: "AROcallsign",
            reason: "duplicate"
        }
    );
    ensure!(
        call.is_ascii()
            && call
                .trim_ascii()
                .bytes()
                .all(|b| b.is_ascii_graphic() || b == b' '),
        CertificateError {
            field: "AROcallsign",
            reason: "unsupported non-ASCII or control text"
        }
    );
    let value = |oid: &str, field| -> anyhow::Result<&str> {
        let mut found = cert
            .extensions()
            .iter()
            .filter(|e| e.oid.to_id_string() == oid);
        let ext = found.next().ok_or(CertificateError {
            field,
            reason: "missing",
        })?;
        ensure!(
            found.next().is_none(),
            CertificateError {
                field,
                reason: "duplicate"
            }
        );
        std::str::from_utf8(ext.value).context("decode certificate extension text")
    };
    let first = date(value("1.3.6.1.4.1.12348.1.2", "QSONotBeforeDate")?)?;
    let last = date(value("1.3.6.1.4.1.12348.1.3", "QSONotAfterDate")?)?;
    ensure!(
        first <= last,
        CertificateError {
            field: "QSO coverage",
            reason: "reversed range"
        }
    );
    let entity = value("1.3.6.1.4.1.12348.1.4", "DXCC")?;
    ensure!(
        !entity.is_empty() && entity.bytes().all(|b| b.is_ascii_digit()),
        CertificateError {
            field: "DXCC",
            reason: "expected decimal entity"
        }
    );
    ensure!(
        entity.parse::<u32>()? > 0,
        CertificateError {
            field: "DXCC",
            reason: "expected positive entity"
        }
    );
    let epoch = civil_date(1970, 1, 1).at(0, 0, 0, 0);
    let not_before = epoch.checked_add(SignedDuration::from_secs(
        cert.validity().not_before.timestamp(),
    ))?;
    let not_after = epoch.checked_add(SignedDuration::from_secs(
        cert.validity().not_after.timestamp(),
    ))?;
    ensure!(
        not_before <= not_after,
        CertificateError {
            field: "validity",
            reason: "reversed range"
        }
    );
    Ok(CertificateMetadata {
        identity: PublicSigningIdentity {
            callsign: RecordedCallsign::new(call)?,
            dxcc: entity.into(),
        },
        issuer_der: cert.issuer().as_raw().to_vec(),
        serial: cert.raw_serial().to_vec(),
        not_before,
        not_after,
        qso_not_before: first,
        qso_not_after: last,
    })
}

/// Decode the complete coverage date without permissive numeric conversion.
fn date(value: &str) -> anyhow::Result<Date> {
    let mut parts = value.split('-');
    let year = parts.next().context("missing coverage year")?;
    let month = parts.next().context("missing coverage month")?;
    let day = parts.next().context("missing coverage day")?;
    ensure!(
        year.len() == 4 && month.len() == 2 && day.len() == 2 && parts.next().is_none(),
        "invalid coverage date syntax"
    );
    ensure!(
        year.bytes()
            .chain(month.bytes())
            .chain(day.bytes())
            .all(|b| b.is_ascii_digit()),
        "nondecimal coverage date"
    );
    let year = year.parse::<i16>()?;
    ensure!((1..=9999).contains(&year), "coverage year out of range");
    Ok(Date::new(year, month.parse()?, day.parse()?)?)
}
