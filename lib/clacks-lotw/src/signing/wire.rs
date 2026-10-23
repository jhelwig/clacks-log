//! Byte-counted GAbbI records without compression or transport.

use base64::{Engine, engine::general_purpose::STANDARD};
use clacks_model::service::adif::AdifFields;

/// Frame one field and terminate its line without duplicating a base64 newline.
fn field(out: &mut Vec<u8>, name: &str, kind: Option<char>, value: &[u8]) {
    let header = if let Some(kind) = kind {
        format!("<{name}:{}:{kind}>", value.len())
    } else {
        format!("<{name}:{}>", value.len())
    };
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(value);
    if !value.ends_with(b"\n") {
        out.push(b'\n');
    }
}

/// Encode protocol base64 with 64-column wrapping and a final newline.
fn base64_lines(value: &[u8]) -> Vec<u8> {
    let encoded = STANDARD.encode(value);
    let mut output = Vec::new();
    for chunk in encoded.as_bytes().chunks(64) {
        output.extend_from_slice(chunk);
        output.push(b'\n');
    }
    output
}

/// Construct the truthful application/configuration identification field.
pub(super) fn identification(application: &str, version: &str, release: &str) -> Vec<u8> {
    let mut out = Vec::new();
    field(
        &mut out,
        "TQSL_IDENT",
        None,
        format!("{application} {version} Compatibility: TQSL V2.8.6 Config: V{release}").as_bytes(),
    );
    out
}

/// Append one original station certificate and its output-local reference.
pub(super) fn certificate(out: &mut Vec<u8>, uid: u64, der: &[u8]) {
    field(out, "Rec_Type", None, b"tCERT");
    field(out, "CERT_UID", None, uid.to_string().as_bytes());
    field(out, "CERTIFICATE", None, &base64_lines(der));
    out.extend_from_slice(b"<eor>\n");
}

/// Append station values linked to one output-local certificate.
pub(super) fn station(out: &mut Vec<u8>, uid: u64, certificate: u64, fields: &AdifFields) {
    field(out, "Rec_Type", None, b"tSTATION");
    field(out, "STATION_UID", None, uid.to_string().as_bytes());
    field(out, "CERT_UID", None, certificate.to_string().as_bytes());
    for (name, value) in fields {
        if !value.is_empty() {
            field(out, name, None, value.as_bytes());
        }
    }
    out.extend_from_slice(b"<eor>\n");
}

/// Append contact values and the exact signature/input for its station.
pub(super) fn contact(
    out: &mut Vec<u8>,
    station: u64,
    fields: &AdifFields,
    signature_field: &str,
    signature: &[u8],
    input: &[u8],
) {
    field(out, "Rec_Type", None, b"tCONTACT");
    field(out, "STATION_UID", None, station.to_string().as_bytes());
    for (name, value) in fields {
        if !value.is_empty() {
            field(out, name, None, value.as_bytes());
        }
    }
    field(out, signature_field, Some('6'), &base64_lines(signature));
    field(out, "SIGNDATA", None, input);
    out.extend_from_slice(b"<eor>\n");
}
