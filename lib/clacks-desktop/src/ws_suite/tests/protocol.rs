use super::super::protocol;

#[test]
fn status_call_wire_fields() -> anyhow::Result<()> {
    // Schema 3/type 1, Id WS, zero dial frequency, blank mode, DX call W7HEX.
    let bytes = [
        0xad, 0xbc, 0xcb, 0xda, 0, 0, 0, 3, 0, 0, 0, 1, 0, 0, 0, 2, b'W', b'S', 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 5, b'W', b'7', b'H', b'E', b'X',
    ];
    let sender = "127.0.0.1:2237".parse()?;
    for schema in [2, 3] {
        let mut frame = bytes.to_vec();
        frame[7] = schema;
        for call in [Some("W7HEX"), Some(""), None] {
            frame.truncate(30);
            match call {
                Some(call) => {
                    frame.extend_from_slice(&(call.len() as u32).to_be_bytes());
                    frame.extend_from_slice(call.as_bytes());
                }
                None => frame.extend_from_slice(&u32::MAX.to_be_bytes()),
            }
            assert_eq!(
                protocol::decode(&frame, sender)?,
                Some(protocol::DecodedMessage::Status(protocol::StatusCall {
                    call: call.unwrap_or_default().into()
                }))
            );
            frame.extend_from_slice(&[0xff, 1, 2]);
            assert!(protocol::decode(&frame, sender)?.is_some());
        }
    }
    for length in 12..bytes.len() {
        assert!(
            protocol::decode(&bytes[..length], sender).is_err(),
            "truncated at {length}"
        );
    }
    let unknown_mode = crate::test_support::ws_suite::status("W7HEX");
    assert!(matches!(
        protocol::decode(&unknown_mode, sender)?,
        Some(protocol::DecodedMessage::Status(_))
    ));
    Ok(())
}

/// Authored binary wrapper around hand-checked ADI bytes, not a production encoder.
pub(crate) fn adif(body: &[u8]) -> Vec<u8> {
    let mut bytes = vec![
        0xad, 0xbc, 0xcb, 0xda, 0, 0, 0, 3, 0, 0, 0, 12, 0, 0, 0, 2, b'W', b'S',
    ];
    bytes.extend_from_slice(&(body.len() as u32).to_be_bytes());
    bytes.extend_from_slice(body);
    bytes
}

#[test]
fn adi_length_framing() -> anyhow::Result<()> {
    let body=b"<adif_ver:5>3.1.0<eoh><call:5>w7hex<qso_date:8>20261103<time_on:6>123456<freq:9>14.074000<mode:4>MFSK<submode:3>FT4<station_callsign:0><comment:7>x<EOR>y<name:4>Jos\xe9<eor>";
    let notice = super::completion(&adif(body), "127.0.0.1:9999".parse()?)?.unwrap();
    assert_eq!(notice.identity.contacted_callsign, "W7HEX");
    assert_eq!(notice.identity.mode, "FT4");
    assert_eq!(notice.facts.text("COMMENT"), Some("x<EOR>y"));
    assert_eq!(notice.facts.text("NAME"), Some("José"));
    assert_eq!(notice.facts.text("STATION_CALLSIGN"), None);
    let last =
        adif(b"<CALL:5>W7HEX<QSO_DATE:8>99991231<TIME_ON:6>235959<FREQ:6>14.074<MODE:3>FT8<EOR>");
    assert_eq!(
        super::completion(&last, "127.0.0.1:1".parse()?)?
            .expect("the complete ADI record has a supported date and time")
            .identity
            .start
            .unix_seconds(),
        253_402_300_799
    );
    let invalid =
        adif(b"<CALL:5>W7HEX<QSO_DATE:8>20261027<TIME_ON:6>235960<FREQ:6>14.074<MODE:3>FT8<EOR>");
    assert!(super::completion(&invalid, "127.0.0.1:1".parse()?).is_err());
    assert!(super::completion(&adif(b"<CALL:99>short"), "127.0.0.1:1".parse()?).is_err());
    assert!(
        super::completion(
            &adif(b"<CALL:5>W7HEX<EOR><CALL:5>W7ABC<EOR>"),
            "127.0.0.1:1".parse()?
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn completion_wire_fields() -> anyhow::Result<()> {
    // Schema 2/type 5; Julian 2440588, 1000 ms UTC ends at Unix second 1.
    // Start: Julian 2440588, 0 ms UTC. Frequency is exactly 14,074,000 Hz.
    let bytes = vec![
        0xad, 0xbc, 0xcb, 0xda, 0, 0, 0, 2, 0, 0, 0, 5, 0, 0, 0, 2, b'W', b'S', 0, 0, 0, 0, 0,
        0x25, 0x3d, 0x8c, 0, 0, 3, 0xe8, 1, 0, 0, 0, 5, b'W', b'7', b'H', b'E', b'X', 0, 0, 0, 4,
        b'C', b'N', b'8', b'7', 0, 0, 0, 0, 0, 0xd6, 0xc0, 0x90, 0, 0, 0, 3, b'F', b'T', b'4', 0,
        0, 0, 3, b'-', b'1', b'0', 0, 0, 0, 3, b'-', b'0', b'7', 0, 0, 0, 1, b'5', 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0x25, 0x3d, 0x8c, 0, 0, 0, 0, 1,
    ];
    let notice = super::completion(&bytes, "127.0.0.1:2237".parse()?)?.unwrap();
    assert_eq!(notice.identity.start.unix_seconds(), 0);
    assert_eq!(notice.facts.end.unwrap().unix_seconds(), 1);
    let mut last = bytes.clone();
    last.splice(18..26, 5_373_484_u64.to_be_bytes());
    last.splice(26..30, 86_399_000_u32.to_be_bytes());
    assert_eq!(
        super::completion(&last, "127.0.0.1:1".parse()?)?
            .expect("the binary completion has a supported final calendar day")
            .facts
            .end
            .expect("the completion supplies its end timestamp")
            .unix_seconds(),
        253_402_300_799
    );
    assert_eq!(notice.identity.mode, "FT4");
    assert_eq!(
        notice.identity.primary,
        protocol::PrimaryIdentity::Frequency(14_074_000)
    );
    let mut schema3 = bytes.clone();
    *schema3.get_mut(7).unwrap() = 3;
    assert_eq!(
        super::completion(&schema3, "127.0.0.1:1".parse()?)?
            .unwrap()
            .identity,
        notice.identity
    );
    let mut offset = bytes.clone();
    *offset.get_mut(30).unwrap() = 2;
    offset.splice(31..31, 3600_i32.to_be_bytes());
    assert_eq!(
        super::completion(&offset, "127.0.0.1:2".parse()?)?
            .unwrap()
            .facts
            .end
            .unwrap()
            .unix_seconds(),
        -3599
    );
    let mut local = bytes.clone();
    *local.get_mut(30).unwrap() = 0;
    assert!(super::completion(&local, "127.0.0.1:1".parse()?).is_err());
    let mut truncated = bytes.clone();
    truncated.push(0);
    assert!(super::completion(&truncated, "127.0.0.1:1".parse()?).is_err());
    Ok(())
}
