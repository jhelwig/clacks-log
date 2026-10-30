//! Native evidence fixtures shared by read-projection and query scenarios.

use super::id;
use clacks_model::{
    contact::Qso,
    defaults::account_definitions,
    export::{ExportContext, OutputPurpose, ProfileSet},
    group::DefinitionSet,
    recognition::RecognitionCatalog,
    service::{
        adif::ProjectionContext,
        destination::Service,
        evidence::{
            ImportProvenance, ImportedAssertion, ImportedBaselineInput, ImportedClaim,
            ServicePayload, imported_baseline,
        },
        lotw::LotwConfiguration,
    },
};
use clacks_sync::{
    record::{RecordState, RecordValue},
    service::ServiceRecord,
};

/// Capture a source-only imported assertion without manufacturing a submission.
pub fn imported(
    qso: &Qso,
    assertion: u128,
    service: Service,
    claim: ImportedClaim,
) -> anyhow::Result<RecordState> {
    let seed = account_definitions()?;
    let definitions = DefinitionSet::new(seed.groups)?;
    let profiles: ProfileSet = seed
        .output_profiles
        .into_iter()
        .map(|p| (p.id, p))
        .collect();
    let ordinary = profiles
        .values()
        .find(|p| p.purpose == OutputPurpose::OrdinaryInterchange)
        .unwrap();
    let context = ProjectionContext {
        recognition: RecognitionCatalog::adif_318()?,
        definitions: &definitions,
        profiles: &profiles,
        export: &ExportContext {
            output_profile_id: ordinary.id,
            target_group_id: None,
        },
        resolutions: &Default::default(),
    };
    let baseline = imported_baseline(&ImportedBaselineInput {
        qso,
        service,
        coverage: Default::default(),
        lotw: &LotwConfiguration::bundled()?,
        context: &context,
    })?;
    Ok(RecordState::Live(RecordValue::Service(Box::new(
        ServiceRecord {
            payload: ServicePayload::Imported(Box::new(ImportedAssertion {
                id: id(assertion),
                qso: qso.id(),
                service,
                destination: None,
                claim,
                date: None,
                provenance: ImportProvenance {
                    source: "independent ADIF source".into(),
                    record: "1".into(),
                    mapping: "ordinary".into(),
                },
                baseline,
            })),
            support: Default::default(),
        },
    ))))
}
