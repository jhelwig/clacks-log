use super::{IdentityGap, IdentityMatch, compatible_mode, time_difference};
use clacks_model::{
    recognition::mode::{Mode, ModeIdentity},
    scalar::UtcSecond,
};

#[test]
fn import_compares_both_times_at_the_lower_precision_across_midnight() {
    let before_midnight = UtcSecond::from_unix_seconds(86_399);
    let after_midnight = UtcSecond::from_unix_seconds(88_200);
    assert_eq!(
        time_difference(before_midnight, after_midnight, true),
        31 * 60
    );
    assert_eq!(
        time_difference(after_midnight, before_midnight, true),
        31 * 60
    );
    let within = UtcSecond::from_unix_seconds(88_199);
    assert_eq!(time_difference(before_midnight, within, true), 30 * 60);
    assert_eq!(time_difference(within, before_midnight, true), 30 * 60);
    assert_eq!(time_difference(before_midnight, within, false), 30 * 60);
}

#[test]
fn import_unknown_submode_requires_review_when_only_one_parent_is_supplied() -> anyhow::Result<()> {
    let catalog = clacks_model::recognition::RecognitionCatalog::adif_318()?;
    let mode = |parent: Option<&str>| {
        Mode::from_recorded_identity(ModeIdentity::Specialization {
            label: "EXAMPLE".into(),
            parent: parent.map(ToOwned::to_owned),
        })
    };
    let unparented = mode(None)?;
    let ssb = mode(Some("SSB"))?;
    let fm = mode(Some("FM"))?;
    assert_eq!(compatible_mode(&ssb, &ssb, catalog), IdentityMatch::Equal);
    assert_eq!(
        compatible_mode(&unparented, &ssb, catalog),
        IdentityMatch::Incomplete(IdentityGap::SubmodeParent)
    );
    assert_eq!(
        compatible_mode(&ssb, &unparented, catalog),
        IdentityMatch::Incomplete(IdentityGap::SubmodeParent)
    );
    assert_eq!(
        compatible_mode(&ssb, &fm, catalog),
        IdentityMatch::Different
    );
    Ok(())
}
