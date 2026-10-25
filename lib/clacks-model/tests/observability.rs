#[path = "observability/export.rs"]
mod export;
mod support;

use clacks_model::export::ExportContext;
use support::telemetry::capture;

#[test]
fn tag_deletion_preview_counts_are_not_deletions() {
    use clacks_model::ids::TagId;
    use clacks_model::tag::{TagDeletionChoice, plan_tag_deletion};
    let tag = support::id::<TagId>(1);
    let other = support::id::<TagId>(2);
    let mut a = support::qso(support::id(1));
    a.tags.insert(tag);
    let mut b = support::qso(support::id(2));
    b.tags.extend([tag, other]);
    let mut c = support::qso(support::id(3));
    c.tags.insert(other);
    let contacts = vec![a, b, c];
    let original = contacts.clone();
    let captured = capture(|| plan_tag_deletion(tag, TagDeletionChoice::DeleteUnique, &contacts));
    assert_eq!(
        captured
            .span("model.tag.deletion.plan")
            .fields
            .get("deletion_choice"),
        Some(&format!("{:?}", TagDeletionChoice::DeleteUnique))
    );
    assert_eq!(captured.result.affected.len(), 2);
    assert_eq!(captured.result.mark_deleted.len(), 1);
    assert_eq!(contacts, original);

    for (kind, n) in [
        ("input", 3.),
        ("affected", 2.),
        ("proposed_delete", 1.),
        ("remove_membership", 1.),
        ("shared", 1.),
    ] {
        captured.event_count("tag.deletion.planned", kind, n);
    }
}

#[test]
fn lotw_readiness_keeps_unfamiliar_values_out_of_telemetry() -> anyhow::Result<()> {
    use clacks_model::recognition::mode::{Mode, ModeIdentity};
    use clacks_model::service::{
        destination::{Destination, RemoteCollection},
        error::ProjectionDecision,
        lotw::{self, LotwConfiguration, LotwProjectionInput, PublicSigningIdentity},
    };
    let mut q = support::qso(support::id(1));
    let secret = "CANARY-UNFAMILIAR-MODE";
    q.mode = Mode::from_recorded_identity(ModeIdentity::Parent {
        label: secret.into(),
    })?;
    let d = Destination {
        id: support::id(1),
        collection: RemoteCollection::Lotw {
            account: "canary-account".into(),
        },
        description: "canary-description".into(),
    };
    let signing = PublicSigningIdentity {
        callsign: q.own_station.callsign.clone(),
        dxcc: "291".into(),
    };
    let config = LotwConfiguration::bundled()?;
    let defs = support::definitions();
    let captured = capture(|| {
        lotw::project(&LotwProjectionInput {
            qso: &q,
            destination: &d,
            signing: &signing,
            configuration: &config,
            definitions: &defs,
        })
    });

    let ProjectionDecision::NotReady(issues) = captured.result.as_ref().unwrap() else {
        panic!("expected readiness")
    };
    assert!(issues.iter().any(|i| i.reason.contains(secret)));
    captured.counter(
        "clacks_service_projection_readiness_total",
        &[("service", "lotw"), ("readiness", "not_ready")],
        1,
    );
    captured.event_count("projection.prepared", "readiness_issue", 1.);
    assert_eq!(
        captured
            .event("projection.conversion_unavailable")
            .fields
            .get("reason")
            .map(String::as_str),
        Some("unmapped_mode")
    );
    assert_eq!(
        captured
            .event("projection.prepared")
            .fields
            .get("readiness")
            .unwrap(),
        "not_ready"
    );
    assert!(!format!("{:?}{:?}", captured.observations, captured.metrics).contains(secret));
    Ok(())
}

#[test]
fn profile_and_reference_entry_preserve_semantics_and_safe_errors() -> anyhow::Result<()> {
    use clacks_model::{
        contact::RecordError,
        group::normalize_reference,
        profile::{
            CurrentOperatingValues, EquipmentConfiguration, SavedLocationProfile, load_profile,
        },
    };
    let q = support::qso(support::id(1));
    let current = CurrentOperatingValues {
        station: q.own_station.clone(),
        operator: None,
        transmit_power: None,
        equipment: None,
    };
    let mut profile = SavedLocationProfile {
        id: support::id(1),
        name: "CANARY-PROFILE".into(),
        station: q.own_station,
        default_equipment: None,
    };
    let captured = capture(|| load_profile(&current, &profile, None));
    assert_eq!(captured.result.as_ref().unwrap().station, profile.station);
    profile.default_equipment = Some(support::id(2));
    let captured = capture(|| load_profile(&current, &profile, None));
    assert!(
        captured
            .result
            .as_ref()
            .unwrap_err()
            .downcast_ref::<RecordError>()
            .is_some()
    );
    assert_eq!(
        captured.span("model.profile.load").fields.get("profile_id"),
        Some(&profile.id.to_string())
    );
    assert!(captured.observations.events.is_empty());
    assert!(!format!("{:?}", captured.observations).contains("CANARY-PROFILE"));
    let equipment = EquipmentConfiguration {
        id: support::id(2),
        name: "CANARY-EQUIPMENT-NAME".into(),
        values: Default::default(),
    };
    let captured = capture(|| load_profile(&current, &profile, Some(&equipment)));
    assert_eq!(
        captured.result.as_ref().unwrap().equipment,
        Some(equipment.values.clone())
    );
    let event = captured.event("profile.equipment_selected");
    assert_eq!(
        event.fields.get("equipment_id"),
        Some(&equipment.id.to_string())
    );
    assert_eq!(event.parent, captured.span("model.profile.load").id);
    assert!(!format!("{:?}", captured.observations).contains("CANARY-EQUIPMENT-NAME"));
    let association = support::reference(support::id(1), Some("us-1234"));
    let captured = capture(|| normalize_reference(&association, None));
    assert_eq!(captured.result.as_ref().unwrap(), &association);
    let defs = support::definitions();
    let captured = capture(|| normalize_reference(&association, defs.get(association.group_id)));
    assert_eq!(
        captured.result.as_ref().unwrap().identifier.as_deref(),
        Some("US-1234")
    );
    Ok(())
}

#[test]
fn catalog_outcomes_and_invalid_definition_keep_diagnostics_private() -> anyhow::Result<()> {
    use clacks_model::{
        catalog::{CatalogFinding, lookup_reference},
        export::rules::compile_definition,
        group::DefinitionSet,
    };
    let (_, association, defs, catalogs, _) = support::catalog_case();
    let captured = capture(|| lookup_reference(&association, &DefinitionSet::default(), &catalogs));
    assert_eq!(
        captured.result.as_ref().unwrap(),
        &CatalogFinding::NotConfigured
    );
    let mut bad = defs.get(association.group_id).unwrap().clone();
    bad.identifier.syntax = Some("[CANARY-INVALID".into());
    let captured = capture(|| compile_definition(&bad));
    assert!(
        captured
            .result
            .as_ref()
            .unwrap_err()
            .to_string()
            .contains("identifier.syntax")
    );
    assert!(!format!("{:?}", captured.observations).contains("CANARY-INVALID"));
    Ok(())
}

#[test]
fn qrz_conversion_warnings_and_loaded_definitions_have_bounded_reporting() -> anyhow::Result<()> {
    use clacks_model::{
        effective::EligibleStationFacts,
        recognition::RecognitionCatalog,
        service::{
            adif::ProjectionContext,
            destination::{Destination, RemoteCollection},
            error::ProjectionDecision,
            qrz::{self, QrzProjectionInput},
        },
    };
    let mut q = support::qso(support::id(1));
    q.comment = Some("CANARY-ä-non-ascii".into());
    let defs = support::definitions();
    let profiles = support::profiles();
    let resolutions = Default::default();
    let export = ExportContext {
        output_profile_id: support::id(1),
        target_group_id: None,
    };
    let context = ProjectionContext {
        recognition: RecognitionCatalog::adif_318()?,
        definitions: &defs,
        profiles: &profiles,
        export: &export,
        resolutions: &resolutions,
    };
    let destination = Destination {
        id: support::id(1),
        collection: RemoteCollection::Qrz {
            book_id: Some("42".parse().unwrap()),
        },
        description: "CANARY-DESCRIPTION".into(),
    };
    let captured = capture(|| {
        qrz::project(&QrzProjectionInput {
            qso: &q,
            destination: &destination,
            eligible: EligibleStationFacts::default(),
            context: &context,
        })
    });
    let ProjectionDecision::Ready(result) = captured.result.as_ref().unwrap() else {
        panic!("ready")
    };
    assert!(!result.warnings.is_empty());
    assert_eq!(
        captured
            .event("projection.text_converted")
            .fields
            .get("field")
            .map(String::as_str),
        Some("COMMENT")
    );
    assert!(
        captured
            .metrics
            .iter()
            .all(|m| !m.name.starts_with("clacks_reference_export_"))
    );
    captured.counter(
        "clacks_service_projection_readiness_total",
        &[("service", "qrz"), ("readiness", "ready")],
        1,
    );
    captured.event_count(
        "projection.prepared",
        "warning",
        result.warnings.len() as f64,
    );
    assert!(!format!("{:?}{:?}", captured.observations, captured.metrics).contains("CANARY"));
    Ok(())
}

#[test]
fn publication_reports_the_destination_resolved_by_domain_logic() {
    use clacks_model::service::{
        applicability::{PublicationCoverage, PublicationCoverageInput, publication_coverage},
        destination::Service,
        evidence::{
            ImportedBaseline,
            submission::{ConfirmationBasis, SubmissionProjection},
        },
        qrz::QrzSubmission,
    };
    let current = SubmissionProjection::Qrz(QrzSubmission {
        destination: support::id(2),
        fields: Default::default(),
    });
    let source = ImportedBaseline {
        native_end: None,
        service: Service::Qrz,
        fields: Default::default(),
        own_location: None,
        confirmation: ConfirmationBasis::default(),
        coverage: Default::default(),
    };
    let captured = capture(|| {
        publication_coverage(&PublicationCoverageInput {
            qso: support::id(1),
            service: Service::Qrz,
            destination: None,
            current: Some(&current),
            source_current: &source,
            assertions: &[],
            attempts: &[],
            results: &[],
        })
    });
    assert_eq!(
        *captured.result.as_ref().unwrap(),
        PublicationCoverage::Pending
    );
    let event = captured.event("publication.coverage_decided");
    assert_eq!(
        event.fields.get("destination_id"),
        Some(&current.destination().to_string())
    );
    assert_eq!(
        event.fields.get("coverage").map(String::as_str),
        Some("Pending")
    );
    assert_eq!(event.parent, captured.span("model.publication.coverage").id);
}

#[test]
fn publication_coverage_identifies_support_without_exposing_submission_values() -> anyhow::Result<()>
{
    use clacks_model::service::{
        applicability::{PublicationCoverage, PublicationCoverageInput, publication_coverage},
        destination::Service,
        evidence::{
            ImportProvenance, ImportedAssertion, ImportedBaseline, ImportedClaim,
            submission::{
                DispatchPhase, ResultScope, SubmissionAttempt, SubmissionOutcome,
                SubmissionProjection, SubmissionResult,
            },
        },
        qrz::QrzSubmission,
    };
    let current = SubmissionProjection::Qrz(QrzSubmission {
        destination: support::id(2),
        fields: [("COMMENT".into(), "CANARY-SUBMISSION".into())].into(),
    });
    let source = ImportedBaseline {
        native_end: None,
        service: Service::Qrz,
        fields: clacks_model::service::applicability::submitted_fields(&current),
        own_location: None,
        confirmation: Default::default(),
        coverage: Default::default(),
    };
    let input = PublicationCoverageInput {
        qso: support::id(1),
        service: Service::Qrz,
        destination: Some(current.destination()),
        current: Some(&current),
        source_current: &source,
        assertions: &[],
        attempts: &[],
        results: &[],
    };
    let attempt = SubmissionAttempt {
        id: support::id(3),
        qso: input.qso,
        projection: current.clone(),
        phase: DispatchPhase::Dispatched,
        at: clacks_model::scalar::UtcTimestamp::new(0, 0)?,
        batch: None,
    };
    let result = SubmissionResult {
        id: support::id(4),
        destination: current.destination(),
        scope: ResultScope::IndividualAttempt(attempt.id),
        outcome: SubmissionOutcome::Success,
        phase: DispatchPhase::Dispatched,
        at: attempt.at,
        diagnostic: Some("CANARY-PROVIDER".into()),
        external: None,
        external_attempt: None,
    };
    let results = [result];
    let missing = capture(|| {
        publication_coverage(&PublicationCoverageInput {
            results: &results,
            ..input
        })
    });
    assert_eq!(
        missing.result.as_ref().unwrap(),
        &PublicationCoverage::NotEstablished
    );
    assert_eq!(
        missing
            .event("publication.support_missing")
            .fields
            .get("attempt_id"),
        Some(&attempt.id.to_string())
    );
    let attempts = [attempt];
    let covered = capture(|| {
        publication_coverage(&PublicationCoverageInput {
            attempts: &attempts,
            results: &results,
            ..input
        })
    });
    assert_eq!(
        covered.result.as_ref().unwrap(),
        &PublicationCoverage::Covered
    );
    assert_eq!(
        covered
            .event("publication.coverage_decided")
            .fields
            .get("result_id"),
        Some(&results.first().unwrap().id.to_string())
    );
    let assertions = [ImportedAssertion {
        id: support::id(5),
        qso: input.qso,
        service: Service::Qrz,
        destination: None,
        claim: ImportedClaim::Uploaded,
        date: None,
        baseline: source.clone(),
        provenance: ImportProvenance {
            source: "CANARY-SOURCE".into(),
            record: "1".into(),
            mapping: "standard".into(),
        },
    }];
    let imported = capture(|| {
        publication_coverage(&PublicationCoverageInput {
            assertions: &assertions,
            ..input
        })
    });
    assert_eq!(
        imported.result.as_ref().unwrap(),
        &PublicationCoverage::Covered
    );
    assert_eq!(
        imported
            .event("publication.coverage_decided")
            .fields
            .get("assertion_id"),
        Some(&assertions.first().unwrap().id.to_string())
    );
    for captured in [missing, covered, imported] {
        assert!(!format!("{:?}", captured.observations).contains("CANARY"));
    }
    Ok(())
}
