use super::*;

#[test]
fn extracts_exact_existing_data_from_read_only_reference_sources() -> anyhow::Result<()> {
    let xml = include_str!("../../../../references/trustedqsl/upstream/src/config.xml");
    let source = include_str!("../../../../references/trustedqsl/upstream/src/location.cpp");
    let (model, signatures) = render(xml, source)?;
    assert_eq!(
        model,
        include_str!("../../../clacks-model/data/lotw-11.34.json")
    );
    assert_eq!(
        signatures,
        include_str!("../../../clacks-lotw/data/signatures-11.34.json")
    );
    Ok(())
}

#[test]
fn converts_decimal_band_boundaries_exactly_and_rejects_lossy_values() -> anyhow::Result<()> {
    for (input, hz) in [
        ("14000", 14_000_000),
        ("0.136", 136),
        ("1.25", 1250),
        ("1.0", 1000),
    ] {
        assert_eq!(hertz(input)?, hz);
    }
    for input in [
        "1.0001",
        "NaN",
        "1e3",
        "-1",
        "18446744073709551615",
        "",
        "1..2",
    ] {
        assert!(hertz(input).is_err(), "{input}");
    }
    Ok(())
}

#[test]
fn reports_missing_source_sections_and_malformed_zones() {
    assert!(render("<tqslconfig/>", "no table").is_err());
    assert!(zones(Some("8:not-a-zone")).is_err());
    assert!(zones(Some("8")).is_err());
}
