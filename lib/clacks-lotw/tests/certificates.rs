mod support;

use clacks_lotw::certificate::inspect_certificate;
use time::{Date, Month};

#[test]
fn interprets_station_identity_and_full_coverage() -> anyhow::Result<()> {
    let metadata = inspect_certificate(&support::fixture("1024/station.der"))?;
    assert_eq!(metadata.identity.callsign.as_str(), "N0CALL");
    assert_eq!(metadata.identity.dxcc, "291");
    assert_eq!(
        metadata.serial,
        vec![
            0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x90, 0xab,
            0xcd, 0xef
        ]
    );
    assert_eq!(
        metadata.qso_not_before,
        Date::from_calendar_date(2000, Month::January, 1)?
    );
    assert_eq!(
        metadata.qso_not_after,
        Date::from_calendar_date(2029, Month::December, 31)?
    );
    assert_eq!(metadata.not_after.unix_timestamp(), 1907755200);
    assert!(!metadata.issuer_der.is_empty());
    Ok(())
}

#[test]
fn rejects_ambiguous_missing_or_malformed_metadata() {
    for name in [
        "common-name-only.der",
        "duplicate-callsign.der",
        "duplicate-coverage.der",
        "reversed-coverage.der",
        "invalid-date.der",
        "malformed-entity.der",
        "missing-coverage.der",
        "ec.der",
    ] {
        assert!(
            inspect_certificate(&support::fixture(name)).is_err(),
            "{name}"
        );
    }
    let mut trailing = support::fixture("1024/station.der");
    trailing.push(0);
    assert!(inspect_certificate(&trailing).is_err());
}
