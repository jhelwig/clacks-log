//! Independently authored WS completion frames for production receiver scenarios.

/// Wrap a hand-authored body using the documented schema-3 binary envelope.
fn frame(kind: u32, body: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0xad, 0xbc, 0xcb, 0xda, 0, 0, 0, 3];
    bytes.extend_from_slice(&kind.to_be_bytes());
    bytes.extend_from_slice(&[0, 0, 0, 2, b'W', b'S']);
    bytes.extend_from_slice(body);
    bytes
}

/// Append one UTF-8 byte array according to the published Qt wire contract.
fn text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u32).to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

/// Status prefix sufficient to reach DX call, with unrelated appended bytes.
pub(crate) fn status(call: &str) -> Vec<u8> {
    let mut body = 0_u64.to_be_bytes().to_vec();
    text(&mut body, "unknown-mode");
    text(&mut body, call);
    body.extend_from_slice(&[0, 1, 2]);
    frame(1, &body)
}

/// Structured completion at Unix second zero on 14.074 MHz, using FT4's leaf label.
pub(crate) fn structured(call: &str) -> Vec<u8> {
    let mut body = vec![0, 0, 0, 0, 0, 0x25, 0x3d, 0x8c, 0, 0, 3, 0xe8, 1];
    text(&mut body, call);
    text(&mut body, "CN87");
    body.extend_from_slice(&14_074_000_u64.to_be_bytes());
    for value in [
        "FT4",
        "-10",
        "-07",
        "5",
        "structured-only",
        "Structured name",
    ] {
        text(&mut body, value);
    }
    body.extend_from_slice(&[0, 0, 0, 0, 0, 0x25, 0x3d, 0x8c, 0, 0, 0, 0, 1]);
    for value in ["", "W7ABC", ""] {
        text(&mut body, value);
    }
    frame(5, &body)
}

/// ADIF counterpart, with distinct optional text and higher-precision supplied power.
pub(crate) fn adif(call: &str) -> Vec<u8> {
    let body = format!(
        "<CALL:{}>{call}<QSO_DATE:8>19700101<TIME_ON:6>000000<FREQ:6>14.074<MODE:4>MFSK<SUBMODE:3>FT4<STATION_CALLSIGN:5>W7ABC<TX_PWR:6>5.0007<COMMENT:15>packet-private!<EOR>",
        call.len()
    );
    let mut payload = vec![];
    text(&mut payload, &body);
    frame(12, &payload)
}
