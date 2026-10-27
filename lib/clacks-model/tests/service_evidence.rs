use clacks_model::{
    ids::{AssertionId, QsoId},
    service::{
        destination::Service,
        evidence::submission::ConfirmationBasis,
        evidence::{
            ImportProvenance, ImportedAssertion, ImportedBaseline, ImportedClaim, ServicePayload,
        },
    },
};

#[test]
fn imported_claim_has_no_fabricated_attempt_or_destination() -> anyhow::Result<()> {
    let assertion = ImportedAssertion {
        id: AssertionId::new(),
        qso: QsoId::new(),
        service: Service::Lotw,
        destination: None,
        claim: ImportedClaim::Uploaded,
        date: None,
        provenance: ImportProvenance {
            source: "file.adi".into(),
            record: "1".into(),
            mapping: "standard".into(),
        },
        baseline: ImportedBaseline {
            native_end: None,
            service: Service::Lotw,
            fields: Default::default(),
            own_location: None,
            confirmation: ConfirmationBasis::default(),
            coverage: Default::default(),
        },
    };
    let value = ServicePayload::Imported(Box::new(assertion.clone()));
    value.validate()?;
    let json = serde_json::to_value(&value)?;
    assert_eq!(assertion.destination, None);
    assert!(!json.to_string().contains("attempt"));
    Ok(())
}

use crate::support;
#[test]
fn source_baseline_ignores_unsubmitted_lotw_values() -> anyhow::Result<()> {
    use clacks_model::{
        export::ExportContext,
        ids::OutputProfileId,
        recognition::RecognitionCatalog,
        service::{
            adif::ProjectionContext,
            evidence::{ImportedBaselineInput, imported_baseline},
            lotw::LotwConfiguration,
        },
    };
    let mut q = support::qso(support::id(77));
    let defs = support::definitions();
    let profiles = support::profiles();
    let resolutions = Default::default();
    let export = ExportContext {
        output_profile_id: support::id::<OutputProfileId>(1),
        target_group_id: None,
    };
    let config = LotwConfiguration::bundled()?;
    let context = ProjectionContext {
        recognition: RecognitionCatalog::adif_318()?,
        definitions: &defs,
        profiles: &profiles,
        export: &export,
        resolutions: &resolutions,
    };
    let a = imported_baseline(&ImportedBaselineInput {
        qso: &q,
        service: Service::Lotw,
        coverage: Default::default(),
        lotw: &config,
        context: &context,
    })?;
    q.comment = Some("not uploaded to LoTW".into());
    let b = imported_baseline(&ImportedBaselineInput {
        qso: &q,
        service: Service::Lotw,
        coverage: Default::default(),
        lotw: &config,
        context: &context,
    })?;
    assert_eq!(a, b);
    assert_eq!(a.fields.get("MODE").map(String::as_str), Some("FT4"));
    assert_eq!(a.confirmation.own_call, Some(q.own_station.callsign));
    assert!(!a.fields.contains_key("MY_DXCC"));
    Ok(())
}

#[test]
fn explicitly_mapped_own_iota_is_part_of_imported_lotw_baseline() -> anyhow::Result<()> {
    use clacks_model::{
        export::{
            ExportContext,
            rules::{BindingCondition, FieldBinding, ValueExpression},
        },
        group::DefinitionSet,
        ids::{GroupId, OutputProfileId},
        recognition::RecognitionCatalog,
        reference::{ReferenceAssociation, StationSide},
        service::{
            adif::ProjectionContext,
            evidence::{ImportedBaselineInput, imported_baseline},
            lotw::LotwConfiguration,
        },
    };
    let original = support::definitions();
    let mut def = original.get(support::id::<GroupId>(1)).unwrap().clone();
    def.identifier.syntax = None;
    def.export_mappings.ordinary_interchange = vec![FieldBinding {
        field: "MY_IOTA".into(),
        value: ValueExpression::Join {
            values: Box::new(ValueExpression::Identifiers(StationSide::Own)),
            separator: ",".into(),
        },
        when: BindingCondition::AssociationPresent(StationSide::Own),
    }];
    let defs = DefinitionSet::new(vec![def])?;
    let profiles = support::profiles();
    let resolutions = Default::default();
    let export = ExportContext {
        output_profile_id: support::id::<OutputProfileId>(1),
        target_group_id: None,
    };
    let config = LotwConfiguration::bundled()?;
    let context = ProjectionContext {
        recognition: RecognitionCatalog::adif_318()?,
        definitions: &defs,
        profiles: &profiles,
        export: &export,
        resolutions: &resolutions,
    };
    let mut q = support::qso(support::id(4));
    q.own_station.references.insert(ReferenceAssociation {
        group_id: support::id(1),
        identifier: Some("na-001".into()),
    });
    let baseline = imported_baseline(&ImportedBaselineInput {
        qso: &q,
        service: Service::Lotw,
        coverage: Default::default(),
        lotw: &config,
        context: &context,
    })?;
    assert_eq!(
        baseline.fields.get("MY_IOTA").map(String::as_str),
        Some("NA-001")
    );
    Ok(())
}

#[test]
fn source_baselines_preserve_unrepresentable_native_times_and_unknown_band_changes()
-> anyhow::Result<()> {
    use clacks_model::{
        contact::BandFrequency,
        export::ExportContext,
        recognition::{RecognitionCatalog, band::Band},
        scalar::UtcSecond,
        service::{
            adif::ProjectionContext,
            applicability::{
                ConfirmationApplicability, ConfirmationInput, confirmation_applicability,
            },
            evidence::{ImportedBaselineInput, imported_baseline, observation::ConfirmationStatus},
            lotw::LotwConfiguration,
        },
    };
    let defs = support::definitions();
    let profiles = support::profiles();
    let resolutions = Default::default();
    let export = ExportContext {
        output_profile_id: support::id(1),
        target_group_id: None,
    };
    let cfg = LotwConfiguration::bundled()?;
    let context = ProjectionContext {
        recognition: RecognitionCatalog::adif_318()?,
        definitions: &defs,
        profiles: &profiles,
        export: &export,
        resolutions: &resolutions,
    };
    let mut q = support::qso(support::id(77));
    q.start = UtcSecond::from_unix_seconds(i64::MAX);
    let capture = |q: &clacks_model::contact::Qso, service| {
        imported_baseline(&ImportedBaselineInput {
            qso: q,
            service,
            coverage: Default::default(),
            lotw: &cfg,
            context: &context,
        })
    };
    for service in [Service::Lotw, Service::Qrz] {
        let b = capture(&q, service)?;
        assert_eq!(b.confirmation.start, Some(q.start));
        assert!(!b.fields.contains_key("QSO_DATE"));
    }
    q.start = UtcSecond::from_unix_seconds(1000);
    q.primary = BandFrequency::new(
        None,
        Some(Band::from_recorded_label("unknown-X")?),
        context.recognition,
    )?;
    let original = capture(&q, Service::Lotw)?;
    q.primary = BandFrequency::new(
        None,
        Some(Band::from_recorded_label("unknown-Y")?),
        context.recognition,
    )?;
    let changed = capture(&q, Service::Lotw)?;
    assert_ne!(original.confirmation, changed.confirmation);
    assert_ne!(
        confirmation_applicability(&ConfirmationInput {
            service: Service::Lotw,
            current: &changed.confirmation,
            basis: &original.confirmation,
            status: Some(ConfirmationStatus::Confirmed),
            counterpart: None,
            station: None
        })?,
        ConfirmationApplicability::Confirmed
    );
    Ok(())
}

#[test]
fn batch_returned_identity_needs_explicit_member_correlation() -> anyhow::Result<()> {
    use clacks_model::{
        scalar::UtcTimestamp,
        service::{
            destination::{ExternalRecordKey, RemoteRecordId},
            evidence::submission::*,
        },
    };
    let a = clacks_model::ids::AttemptId::new();
    let b = clacks_model::ids::AttemptId::new();
    let destination = clacks_model::ids::DestinationId::new();
    let mut result = SubmissionResult {
        external_attempt: None,
        id: clacks_model::ids::SubmissionResultId::new(),
        destination,
        scope: ResultScope::UploadBatch {
            batch: clacks_model::ids::SubmissionBatchId::new(),
            members: [a, b].into(),
        },
        outcome: SubmissionOutcome::Success,
        phase: DispatchPhase::Dispatched,
        at: UtcTimestamp::new(0, 0)?,
        diagnostic: None,
        external: Some(ExternalRecordKey {
            destination,
            record: RemoteRecordId::Qrz("7".parse().unwrap()),
        }),
    };
    assert_eq!(result.identified_attempt()?, None);
    result.external_attempt = Some(b);
    assert_eq!(result.identified_attempt()?, Some(b));
    result.external_attempt = Some(clacks_model::ids::AttemptId::new());
    assert!(result.identified_attempt().is_err());
    Ok(())
}

#[test]
fn imported_lotw_upload_compares_the_emitted_grid_not_the_exact_point() -> anyhow::Result<()> {
    use clacks_model::{
        export::ExportContext,
        location::RecordedLocation,
        position::{Coordinates, GridLength, Position},
        recognition::RecognitionCatalog,
        service::{
            adif::ProjectionContext,
            applicability::{PublicationCoverage, PublicationCoverageInput, publication_coverage},
            evidence::{ImportedBaselineInput, imported_baseline},
            lotw::LotwConfiguration,
        },
    };
    let mut q = support::qso(support::id(77));
    let defs = support::definitions();
    let profiles = support::profiles();
    let resolutions = Default::default();
    let export = ExportContext {
        output_profile_id: support::id(1),
        target_group_id: None,
    };
    let config = LotwConfiguration::bundled()?;
    let context = ProjectionContext {
        recognition: RecognitionCatalog::adif_318()?,
        definitions: &defs,
        profiles: &profiles,
        export: &export,
        resolutions: &resolutions,
    };
    let location = |latitude| -> anyhow::Result<RecordedLocation> {
        Ok(RecordedLocation {
            position: Some(Position::ExplicitPoint {
                coordinates: Coordinates::from_microdegrees(latitude, -122_100_000)?,
                grid_length: GridLength::new(6)?,
            }),
            ..Default::default()
        })
    };
    q.own_station.location = Some(location(47_100_000)?);
    let capture = |q: &_| {
        imported_baseline(&ImportedBaselineInput {
            qso: q,
            service: Service::Lotw,
            coverage: Default::default(),
            lotw: &config,
            context: &context,
        })
    };
    let assertion = ImportedAssertion {
        id: AssertionId::new(),
        qso: q.id,
        service: Service::Lotw,
        destination: None,
        claim: ImportedClaim::Uploaded,
        date: None,
        provenance: ImportProvenance {
            source: "file.adi".into(),
            record: "1".into(),
            mapping: "standard".into(),
        },
        baseline: capture(&q)?,
    };
    for (latitude, expected) in [
        (47_100_001, PublicationCoverage::Covered),
        (47_200_000, PublicationCoverage::Pending),
    ] {
        q.own_station.location = Some(location(latitude)?);
        let current = capture(&q)?;
        assert_eq!(
            publication_coverage(&PublicationCoverageInput {
                qso: q.id,
                service: Service::Lotw,
                destination: None,
                current: None,
                source_current: &current,
                assertions: std::slice::from_ref(&assertion),
                attempts: &[],
                results: &[]
            })?,
            expected
        );
    }
    q.own_station.location = Some(RecordedLocation {
        dxcc: Some("291".into()),
        primary_subdivision: Some("Washington".into()),
        secondary_subdivision: Some("King".into()),
        ..Default::default()
    });
    let before = capture(&q)?;
    let location = q.own_station.location.as_mut().unwrap();
    location.primary_subdivision = Some("WA".into());
    location.secondary_subdivision = Some("WA,King".into());
    assert_eq!(before, capture(&q)?);
    let location = q.own_station.location.as_mut().unwrap();
    location.primary_subdivision = Some("unrecognized historical subdivision".into());
    let unrecognized = capture(&q)?;
    assert_eq!(
        unrecognized
            .own_location
            .as_ref()
            .and_then(|l| l.primary_subdivision.as_deref()),
        Some("unrecognized historical subdivision")
    );
    Ok(())
}
