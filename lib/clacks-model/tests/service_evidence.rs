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
            service: Service::Lotw,
            fields: Default::default(),
            own_location: None,
            confirmation: ConfirmationBasis::default(),
            coverage: Default::default(),
        },
    };
    let value = ServicePayload::Imported(assertion.clone());
    value.validate()?;
    let json = serde_json::to_value(&value)?;
    assert_eq!(assertion.destination, None);
    assert!(!json.to_string().contains("attempt"));
    assert_eq!(serde_json::from_value::<ServicePayload>(json)?, value);
    Ok(())
}

mod support;
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
