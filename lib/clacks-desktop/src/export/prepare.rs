//! Assemble reviewed ADI records from one immutable saved-contact snapshot.

use super::{
    ExportFormat, ExportWarning, PreparedExport, PreparedFile, PreparedRecord, ReadinessField,
    SkippedContact, mapping, pota,
};
use clacks_client_store::read::export::{ExportContact, ExportInput};
use clacks_model::{
    export::{
        ExportContext, OutputPurpose,
        evaluate::{ReferenceExportInput, plan_reference_export},
        rules::{ExpansionRule, PartitionRule, QsoFieldPath, ValueExpression},
        select::{ContextResolution, ExportWarning as ReferenceWarning},
    },
    ids::{GroupId, QsoId},
    recognition::RecognitionCatalog,
    reference::StationSide,
    service::adif::AdifFields,
};
use qso_io::adif::{DataType, StandardField};
use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

/// Installed profile for POTA-compatible ADIF.
const POTA_PROFILE: &str = "01M5QZFPZZM8HD710JY3Z4M36X";

/// Stable identity of the Parks on the Air group.
const POTA_GROUP: &str = "01M5QZFPZZ8TTM3MCPAT3MA7MA";

/// Preparation failures that prevent publishing the selected format.
#[derive(Debug, thiserror::Error)]
pub(crate) enum PrepareError {
    /// Installed configuration or selected bindings could not be evaluated.
    #[error("Could not prepare the export: {0}")]
    Configuration(#[from] anyhow::Error),
    /// A required ordinary field cannot represent the saved contact.
    #[error("{callsign}: {field}. Update the saved contact before exporting it.")]
    InvalidContact {
        /// Contact label for the operator's local error.
        callsign: String,
        /// Required field with an unrepresentable value.
        field: ReadinessField,
    },
}

/// Per-contact adapter inputs shared by all reference-expanded records.
struct ContactPlan<'a> {
    /// Saved contact and evidence for the local report.
    contact: &'a ExportContact,
    /// Common ADI field bindings.
    fields: AdifFields,
    /// Park readiness and contacted references for POTA only.
    pota: Option<pota::ContactReadiness<'a>>,
}

/// Plan selected saved contacts for ordinary ADI or POTA-compatible submission.
///
/// Inspect unresolved decisions before publication. POTA collects all ineligible
/// contacts and their required fields while retaining the eligible contacts.
/// Invalid account definitions or conflicting mappings fail the whole preparation.
pub(crate) fn prepare(
    input: &ExportInput,
    format: ExportFormat,
    resolutions: &BTreeMap<QsoId, ContextResolution>,
) -> Result<PreparedExport, PrepareError> {
    let context = context(input, format)?;
    let catalog = RecognitionCatalog::adif_318().map_err(anyhow::Error::from)?;
    let mut prepared = PreparedExport {
        format,
        ..Default::default()
    };
    let mut contacts = BTreeMap::new();
    let mut requested = BTreeSet::new();
    let today = jiff::tz::Offset::UTC
        .to_datetime(jiff::Timestamp::now())
        .date();
    for contact in &input.contacts {
        let qso_id = contact.qso.id();
        if !requested.insert(qso_id) {
            return Err(
                anyhow::anyhow!("The export snapshot contains repeated contact {qso_id}.").into(),
            );
        }
        let mut mapped = mapping::contact_fields(contact, format, catalog, today);
        let pota = context
            .target_group_id
            .map(|group| pota::assess(contact, group, &mut mapped.issues, &mut mapped.warnings))
            .transpose()?;
        if !mapped.issues.is_empty() {
            let callsign = contact.qso.contacted_station().callsign.as_str().to_owned();
            if format == ExportFormat::Pota {
                prepared.skipped.push(SkippedContact {
                    qso_id,
                    callsign,
                    missing: mapped.issues,
                });
                continue;
            }
            if let Some(field) = mapped.issues.into_iter().next() {
                return Err(PrepareError::InvalidContact { callsign, field });
            }
        }
        prepared.warnings.extend(mapped.warnings);
        contacts.insert(
            qso_id,
            ContactPlan {
                contact,
                fields: mapped.fields,
                pota,
            },
        );
    }
    prepared.requested_count = requested.len();
    if contacts.is_empty() {
        return Ok(prepared);
    }

    let reference_inputs = contacts
        .values()
        .map(|plan| ReferenceExportInput {
            qso: &plan.contact.qso,
            contacted_references: plan
                .pota
                .as_ref()
                .map(|pota| &pota.contacted_references)
                .unwrap_or(&plan.contact.contacted_references),
        })
        .collect::<Vec<_>>();
    let references = plan_reference_export(
        &reference_inputs,
        &context,
        &input.definitions,
        &input.profiles,
        resolutions,
    )?;
    prepared.decisions = references.decisions;
    for warning in references.warnings {
        let qso_id = match &warning {
            ReferenceWarning::OmittedContextField { qso_id, .. }
            | ReferenceWarning::UnsupportedGroup { qso_id, .. } => qso_id,
        };
        if let Some(plan) = contacts.get(qso_id) {
            prepared
                .warnings
                .push(ExportWarning::reference(plan.contact, warning));
        }
    }
    if format == ExportFormat::Ordinary {
        for plan in contacts.values() {
            warn_unmapped_groups(plan.contact, input, &mut prepared.warnings);
        }
    }

    let mut files = BTreeMap::<Vec<String>, PartitionRows>::new();
    for file in references.files {
        for entry in file.entries {
            let qso_id = entry.qso_id;
            let plan = contacts.get(&qso_id).ok_or_else(|| {
                anyhow::anyhow!(
                    "The reference plan included a contact outside the eligible selection."
                )
            })?;
            let mut fields = plan.fields.clone();
            let mut reference_fields = entry.reference_fields;
            let partition = if let Some(pota) = &plan.pota {
                let [_, _, park, subdivision] = file.partition_key.as_slice() else {
                    return Err(anyhow::anyhow!("The installed POTA mapping must partition by activation park and subdivision.").into());
                };
                let activation = pota.activations.get(park.as_str()).ok_or_else(|| {
                    anyhow::anyhow!(
                        "The installed POTA mapping did not select an own activation park."
                    )
                })?;
                if !pota::apply_partition(&mut reference_fields, activation, subdivision) {
                    return Err(anyhow::anyhow!(
                        "The installed POTA mapping supplies inconsistent park or location fields."
                    )
                    .into());
                }
                activation_partition(activation)
            } else {
                Vec::new()
            };
            for (tag, value) in reference_fields {
                merge_reference(
                    plan.contact,
                    format,
                    &mut fields,
                    tag,
                    value,
                    &mut prepared.warnings,
                )?;
            }
            let worked_park = match &plan.pota {
                Some(pota) => pota.planned_contacted_park(&fields)?,
                None => None,
            };
            match files
                .entry(partition)
                .or_default()
                .entry(qso_id)
                .or_default()
                .entry(worked_park)
            {
                Entry::Vacant(record) => {
                    record.insert(PreparedRecord { qso_id, fields });
                }
                Entry::Occupied(record) => {
                    if record.get().fields != fields {
                        return Err(anyhow::anyhow!("The reference plan produced conflicting records for the same contact and parks.").into());
                    }
                }
            }
        }
    }
    verify_pota_coverage(&contacts, &files)?;
    prepared.files = files
        .into_iter()
        .map(|(partition, records)| PreparedFile {
            partition,
            records: records
                .into_values()
                .flat_map(BTreeMap::into_values)
                .collect(),
        })
        .collect();
    Ok(prepared)
}

/// Select the authority for each format from the snapshot's installed profile records.
fn context(input: &ExportInput, format: ExportFormat) -> anyhow::Result<ExportContext> {
    let (output_profile_id, target_group_id) = match format {
        ExportFormat::Ordinary => {
            let id = input
                .profiles
                .iter()
                .find_map(|(id, profile)| {
                    (profile.id == *id && profile.purpose == OutputPurpose::OrdinaryInterchange)
                        .then_some(*id)
                })
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "The ordinary ADI output profile is unavailable in this workspace."
                    )
                })?;
            (id, None)
        }
        ExportFormat::Pota => {
            let id = POTA_PROFILE.parse()?;
            anyhow::ensure!(
                input.profiles.get(&id).is_some_and(
                    |profile| profile.id == id && profile.purpose == OutputPurpose::Targeted
                ),
                "The POTA output profile is unavailable in this workspace."
            );
            (id, Some(POTA_GROUP.parse::<GroupId>()?))
        }
    };
    if let Some(group_id) = target_group_id {
        let definition = input
            .definitions
            .get(group_id)
            .ok_or_else(|| anyhow::anyhow!("The POTA group is unavailable in this workspace."))?;
        let mapping = definition
            .export_mappings()
            .targeted
            .get(&output_profile_id)
            .ok_or_else(|| anyhow::anyhow!("The POTA group lacks its targeted ADI mapping."))?;
        let park_and_location = matches!(
            mapping.file_partition.as_slice(),
            [
                PartitionRule::EachReference {
                    side: StationSide::Own,
                    ..
                },
                PartitionRule::ByValue(ValueExpression::QsoField(
                    QsoFieldPath::OwnPrimarySubdivision
                ))
            ]
        );
        let contacted_parks = matches!(
            mapping.entry_expansion.as_slice(),
            [ExpansionRule {
                side: StationSide::Contacted,
                keep_one_if_empty: true,
                ..
            }]
        );
        anyhow::ensure!(
            park_and_location
                && contacted_parks
                && ["MY_SIG", "MY_SIG_INFO", "SIG", "SIG_INFO", "MY_STATE"]
                    .iter()
                    .all(|tag| mapping.fields.iter().any(|binding| &binding.field == tag)),
            "The installed POTA mapping lacks activation or contacted park bindings."
        );
    }
    Ok(ExportContext {
        output_profile_id,
        target_group_id,
    })
}

/// Report recognized group participation with no ordinary ADI field mapping.
fn warn_unmapped_groups(
    contact: &ExportContact,
    input: &ExportInput,
    warnings: &mut Vec<ExportWarning>,
) {
    let groups: BTreeSet<_> = contact
        .qso
        .own_station()
        .references
        .iter()
        .chain(&contact.contacted_references)
        .map(|reference| reference.group_id)
        .collect();
    for group in groups {
        if let Some(definition) = input.definitions.get(group)
            && definition.export_mappings().ordinary_interchange.is_empty()
        {
            warnings.push(ExportWarning::new(
                contact,
                "References",
                format!(
                    "Omitted participation in {}, which has no ordinary ADI mapping.",
                    definition.name()
                ),
            ));
        }
    }
}

/// Check for conflicting values, then omit unrepresentable reference values with a warning.
fn merge_reference(
    contact: &ExportContact,
    format: ExportFormat,
    fields: &mut AdifFields,
    tag: String,
    mut value: String,
    warnings: &mut Vec<ExportWarning>,
) -> anyhow::Result<()> {
    let standard = mapping::standard_record_field(&tag);
    let Some(tag) = standard.map(|field| field.as_str()).or_else(|| {
        [
            mapping::QRZ_CONFIRMATION_STATUS,
            mapping::QRZ_CONFIRMATION_DATE,
        ]
        .into_iter()
        .find(|candidate| candidate.eq_ignore_ascii_case(&tag))
    }) else {
        warnings.push(ExportWarning::new(
            contact,
            tag,
            "Omitted a reference without a supported ADI record field.",
        ));
        return Ok(());
    };
    if let Some(existing) = fields.get(tag) {
        anyhow::ensure!(
            *existing == value,
            "The export produces conflicting values for {tag} on {}.",
            contact.qso.contacted_station().callsign.as_str()
        );
        return Ok(());
    }

    let validation_field = match (standard, tag) {
        // The targeted adapter has already checked the receiver's location code.
        (Some(StandardField::MyState), _) if format == ExportFormat::Pota => None,
        (None, mapping::QRZ_CONFIRMATION_DATE) => Some(StandardField::QsoDate),
        (field, _) => field,
    };
    let valid = |value: &str| match validation_field {
        Some(field) => mapping::standard_value(field, value),
        None => mapping::printable(value),
    };
    if !valid(&value) {
        warnings.push(ExportWarning::new(
            contact,
            tag,
            "Omitted reference information unsupported by the ADI field type.",
        ));

        if !validation_field
            .is_some_and(|field| field.data_types().contains(&DataType::POTARefList))
        {
            return Ok(());
        }
        // A single park is a valid POTARefList, so the same check can salvage each member.
        value = value
            .split(',')
            .filter(|member| valid(member))
            .collect::<Vec<_>>()
            .join(",");
        if value.is_empty() {
            return Ok(());
        }
    }
    fields.insert(tag.into(), value);
    Ok(())
}

/// A partition's entries keyed by native contact and then by unqualified worked park.
type PartitionRows = BTreeMap<QsoId, BTreeMap<Option<String>, PreparedRecord>>;

/// Canonical park and location dimensions for a prepared export file.
fn activation_partition(activation: &pota::Activation<'_>) -> Vec<String> {
    vec![activation.park.to_owned(), activation.location.clone()]
}

/// Ensure the installed bindings cover each expected park on every activation.
///
/// Each row's park already passed the membership check, and duplicate aliases
/// share the same row key. An empty contacted set expects a single row.
fn verify_pota_coverage(
    contacts: &BTreeMap<QsoId, ContactPlan<'_>>,
    files: &BTreeMap<Vec<String>, PartitionRows>,
) -> anyhow::Result<()> {
    for (id, contact) in contacts {
        let Some(pota) = &contact.pota else { continue };
        let expected_rows = pota.contacted_parks.len().max(1);
        for activation in pota.activations.values() {
            let partition = activation_partition(activation);
            let rows = files.get(&partition).and_then(|rows| rows.get(id));
            anyhow::ensure!(
                rows.is_some_and(|rows| rows.len() == expected_rows),
                "The installed POTA mapping does not cover all contacted parks for each activation."
            );
        }
    }
    Ok(())
}
