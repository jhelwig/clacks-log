use super::ReleaseVersion;

#[test]
fn monthly_allocation_uses_the_highest_sequence_and_resets_at_one() -> anyhow::Result<()> {
    let allocated = ["26.10.65535", "26.11.1", "26.11.4", "25.12.99"]
        .into_iter()
        .map(ReleaseVersion::parse)
        .collect::<anyhow::Result<Vec<_>>>()?;

    assert_eq!(
        ReleaseVersion::next_for_month(26, 11, &allocated)?.to_string(),
        "26.11.5"
    );
    assert_eq!(
        ReleaseVersion::next_for_month(26, 12, &allocated)?.to_string(),
        "26.12.1"
    );
    assert_eq!(
        ReleaseVersion::next_for_month(27, 1, &allocated)?.to_string(),
        "27.1.1"
    );
    assert_eq!(
        ReleaseVersion::next_for_month(26, 11, &[])?.to_string(),
        "26.11.1"
    );
    Ok(())
}

#[test]
fn exhausted_month_does_not_wrap_or_select_another_month() -> anyhow::Result<()> {
    let allocated = [ReleaseVersion::parse("26.11.65535")?];
    assert!(ReleaseVersion::next_for_month(26, 11, &allocated).is_err());
    Ok(())
}
