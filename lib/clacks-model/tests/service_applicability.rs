use clacks_model::{
    scalar::{RecordedCallsign, UtcSecond},
    service::{
        applicability::*,
        destination::Service,
        evidence::{
            observation::{ConfirmationStatus, ModeReport},
            submission::ConfirmationBasis,
        },
    },
};

fn basis() -> ConfirmationBasis {
    ConfirmationBasis {
        uninterpreted_band: None,
        uninterpreted_receive_band: None,
        own_call: Some(RecordedCallsign::new("W7HEX").unwrap()),
        worked_call: Some(RecordedCallsign::new("K1ABC").unwrap()),
        start: Some(UtcSecond::from_unix_seconds(43200)),
        band: Some("20M".into()),
        receive_band: Some("20M".into()),
        frequency: None,
        receive_frequency: None,
        mode: Some(ModeReport {
            concrete: "FT4".into(),
            parent: Some("MFSK".into()),
            group: Some("DATA".into()),
        }),
    }
}

#[test]
fn conservative_time_and_mode_group_are_independent() -> anyhow::Result<()> {
    let old = basis();
    let current = old.clone();
    let mut input = ConfirmationInput {
        service: Service::Lotw,
        current: &current,
        basis: &old,
        status: Some(ConfirmationStatus::Confirmed),
        counterpart: None,
        station: None,
    };
    assert_eq!(
        confirmation_applicability(&input)?,
        ConfirmationApplicability::Confirmed
    );
    let mut edited = current.clone();
    edited.start = Some(UtcSecond::from_unix_seconds(43080));
    input.current = &edited;
    assert_eq!(
        confirmation_applicability(&input)?,
        ConfirmationApplicability::NotEstablished
    );
    let mut other = old.clone();
    other.start = Some(UtcSecond::from_unix_seconds(44940));
    input.counterpart = Some(&other);
    assert_eq!(
        confirmation_applicability(&input)?,
        ConfirmationApplicability::HistoricalOnly
    );
    let mut changed = old.clone();
    changed.mode.as_mut().unwrap().concrete = "FT8".into();
    input.current = &changed;
    input.counterpart = None;
    assert_eq!(
        confirmation_applicability(&input)?,
        ConfirmationApplicability::Confirmed
    );
    assert!(!exact_mode_qualification(&input));
    Ok(())
}

#[test]
fn identity_changes_and_negative_status_do_not_fabricate_confirmation() -> anyhow::Result<()> {
    let old = basis();
    let mut current = old.clone();
    current.worked_call = Some(RecordedCallsign::new("K2ABC")?);
    let input = ConfirmationInput {
        service: Service::Qrz,
        current: &current,
        basis: &old,
        status: Some(ConfirmationStatus::Confirmed),
        counterpart: None,
        station: None,
    };
    assert_eq!(
        confirmation_applicability(&input)?,
        ConfirmationApplicability::HistoricalOnly
    );
    let input = ConfirmationInput {
        current: &old,
        status: Some(ConfirmationStatus::Unconfirmed),
        ..input
    };
    assert_eq!(
        confirmation_applicability(&input)?,
        ConfirmationApplicability::Unconfirmed
    );
    Ok(())
}

#[test]
fn imported_upload_and_confirmation_are_independent_and_baseline_specific() -> anyhow::Result<()> {
    use clacks_model::{
        ids::{AssertionId, DestinationId, QsoId},
        service::{
            evidence::{
                ImportProvenance, ImportedAssertion, ImportedBaseline, ImportedClaim,
                submission::SubmissionProjection,
            },
            qrz::QrzSubmission,
        },
    };
    let qso = QsoId::new();
    let destination = DestinationId::new();
    let fields = std::collections::BTreeMap::from([("MY_GRIDSQUARE".into(), "FN31".into())]);
    let baseline = ImportedBaseline {
        native_end: None,
        service: Service::Qrz,
        fields: fields.clone(),
        own_location: None,
        confirmation: basis(),
        coverage: Default::default(),
    };
    let current = SubmissionProjection::Qrz(QrzSubmission {
        destination,
        fields,
    });
    let claims = vec![ImportedAssertion {
        id: AssertionId::new(),
        qso,
        service: Service::Qrz,
        destination: None,
        claim: ImportedClaim::Confirmed,
        date: None,
        provenance: ImportProvenance {
            source: "file".into(),
            record: "1".into(),
            mapping: "standard".into(),
        },
        baseline: baseline.clone(),
    }];
    let input = PublicationCoverageInput {
        qso,
        current: Some(&current),
        service: current.service(),
        destination: Some(current.destination()),
        source_current: &baseline,
        assertions: &claims,
        attempts: &[],
        results: &[],
    };
    assert_eq!(publication_coverage(&input)?, PublicationCoverage::Pending);
    let mut uploaded = claims.clone();
    uploaded.first_mut().unwrap().claim = ImportedClaim::Uploaded;
    let input = PublicationCoverageInput {
        assertions: &uploaded,
        ..input
    };
    assert_eq!(publication_coverage(&input)?, PublicationCoverage::Covered);
    let edited = SubmissionProjection::Qrz(QrzSubmission {
        destination,
        fields: [("MY_GRIDSQUARE".into(), "FN32".into())].into(),
    });
    let input = PublicationCoverageInput {
        current: Some(&edited),
        ..input
    };
    assert_eq!(publication_coverage(&input)?, PublicationCoverage::Pending);
    Ok(())
}

#[test]
fn service_level_imported_upload_is_trusted_without_routing_or_projection() -> anyhow::Result<()> {
    use clacks_model::{
        ids::{AssertionId, QsoId},
        service::evidence::{ImportProvenance, ImportedAssertion, ImportedBaseline, ImportedClaim},
    };
    let qso = QsoId::new();
    let baseline = ImportedBaseline {
        native_end: None,
        service: Service::Lotw,
        fields: Default::default(),
        own_location: None,
        confirmation: basis(),
        coverage: Default::default(),
    };
    let assertion = ImportedAssertion {
        id: AssertionId::new(),
        qso,
        service: Service::Lotw,
        destination: None,
        claim: ImportedClaim::Uploaded,
        date: None,
        provenance: ImportProvenance {
            source: "file".into(),
            record: "1".into(),
            mapping: "standard".into(),
        },
        baseline: baseline.clone(),
    };
    let input = PublicationCoverageInput {
        service: Service::Lotw,
        destination: None,
        qso,
        current: None,
        source_current: &baseline,
        assertions: &[assertion],
        attempts: &[],
        results: &[],
    };
    assert_eq!(publication_coverage(&input)?, PublicationCoverage::Covered);
    Ok(())
}
