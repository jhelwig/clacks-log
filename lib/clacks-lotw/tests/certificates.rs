use crate::support;

use clacks_lotw::certificate::inspect_certificate;
use jiff::civil::{Date, date};

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
    assert_eq!(metadata.qso_not_before, Date::new(2000, 1, 1)?);
    assert_eq!(metadata.qso_not_after, Date::new(2029, 12, 31)?);
    assert_eq!(metadata.not_after, date(2030, 6, 15).at(12, 0, 0, 0));
    let no_expiry = inspect_certificate(&support::fixture("no-expiry.der"))?;
    assert_eq!(no_expiry.not_after, date(9999, 12, 31).at(23, 59, 59, 0));
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
