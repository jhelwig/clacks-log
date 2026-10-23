mod support;

use clacks_lotw::certificate::{
    eligibility::{CredentialState, EligibilityIssue, eligibility},
    inspect_certificate,
};
use time::{Date, Month, OffsetDateTime, UtcOffset};

fn day(y: i32, m: Month, d: u8) -> Date {
    Date::from_calendar_date(y, m, d).unwrap()
}

#[test]
fn coverage_is_inclusive_and_expiry_uses_the_utc_day() -> anyhow::Result<()> {
    let c = inspect_certificate(&support::fixture("1024/station.der"))?;
    let last = day(2030, Month::June, 15)
        .with_hms(23, 59, 59)?
        .assume_utc();
    for date in [c.qso_not_before, c.qso_not_after] {
        assert!(eligibility(&c, &c.identity, date, CredentialState::Available, last).is_empty());
    }
    assert_eq!(
        eligibility(
            &c,
            &c.identity,
            c.qso_not_before.previous_day().unwrap(),
            CredentialState::Available,
            last
        ),
        vec![EligibilityIssue::BeforeCoverage]
    );
    assert_eq!(
        eligibility(
            &c,
            &c.identity,
            c.qso_not_after.next_day().unwrap(),
            CredentialState::Available,
            last
        ),
        vec![EligibilityIssue::AfterCoverage]
    );
    let next = day(2030, Month::June, 16).midnight().assume_utc();
    assert_eq!(
        eligibility(
            &c,
            &c.identity,
            c.qso_not_before,
            CredentialState::Available,
            next
        ),
        vec![EligibilityIssue::Expired]
    );
    assert_eq!(
        eligibility(
            &c,
            &c.identity,
            c.qso_not_before,
            CredentialState::Available,
            next.to_offset(UtcOffset::from_hms(-7, 0, 0)?)
        ),
        vec![EligibilityIssue::Expired]
    );
    let early = day(2010, Month::January, 1).midnight().assume_utc();
    assert!(
        eligibility(
            &c,
            &c.identity,
            c.qso_not_before,
            CredentialState::Available,
            early
        )
        .is_empty()
    );
    Ok(())
}

#[test]
fn explains_all_relevant_identity_state_and_date_failures_in_stable_order() -> anyhow::Result<()> {
    let c = inspect_certificate(&support::fixture("1024/station.der"))?;
    let mut identity = c.identity.clone();
    identity.callsign = clacks_model::scalar::RecordedCallsign::new("N0CALL/P")?;
    identity.dxcc = "1".into();
    let now = OffsetDateTime::from_unix_timestamp(2_000_000_000)?;
    assert_eq!(
        eligibility(
            &c,
            &identity,
            c.qso_not_before.previous_day().unwrap(),
            CredentialState::Superseded,
            now
        ),
        vec![
            EligibilityIssue::CallsignMismatch,
            EligibilityIssue::EntityMismatch,
            EligibilityIssue::BeforeCoverage,
            EligibilityIssue::Expired,
            EligibilityIssue::Superseded
        ]
    );
    assert_eq!(
        eligibility(
            &c,
            &c.identity,
            c.qso_not_before,
            CredentialState::Removed,
            day(2026, Month::October, 22).midnight().assume_utc()
        ),
        vec![EligibilityIssue::KeyUnavailable]
    );
    Ok(())
}
