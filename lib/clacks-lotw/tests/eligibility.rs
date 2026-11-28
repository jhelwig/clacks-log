use crate::support;

use clacks_lotw::certificate::{
    eligibility::{CredentialState, EligibilityIssue, eligibility},
    inspect_certificate,
};
use jiff::{Timestamp, civil::date, tz::Offset};

#[test]
fn coverage_is_inclusive_and_expiry_uses_the_utc_day() -> anyhow::Result<()> {
    let c = inspect_certificate(&support::fixture("1024/station.der"))?;
    let last = Offset::UTC.to_timestamp(date(2030, 6, 15).at(23, 59, 59, 0))?;
    for date in [c.qso_not_before, c.qso_not_after] {
        assert!(eligibility(&c, &c.identity, date, CredentialState::Available, last).is_empty());
    }
    assert_eq!(
        eligibility(
            &c,
            &c.identity,
            c.qso_not_before.yesterday()?,
            CredentialState::Available,
            last
        ),
        vec![EligibilityIssue::BeforeCoverage]
    );
    assert_eq!(
        eligibility(
            &c,
            &c.identity,
            c.qso_not_after.tomorrow()?,
            CredentialState::Available,
            last
        ),
        vec![EligibilityIssue::AfterCoverage]
    );
    let next = Offset::UTC.to_timestamp(date(2030, 6, 16).at(0, 0, 0, 0))?;
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
    let early = Offset::UTC.to_timestamp(date(2010, 1, 1).at(0, 0, 0, 0))?;
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
    let now = Timestamp::from_second(2_000_000_000)?;
    assert_eq!(
        eligibility(
            &c,
            &identity,
            c.qso_not_before.yesterday()?,
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
            Offset::UTC.to_timestamp(date(2026, 10, 22).at(0, 0, 0, 0))?
        ),
        vec![EligibilityIssue::KeyUnavailable]
    );
    Ok(())
}
