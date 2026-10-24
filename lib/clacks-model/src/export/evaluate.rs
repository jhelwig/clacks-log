//! Pure reference-output planning with deterministic partitions and scoped row expansion.

use super::{rules::reference_identifiers, select::evaluate_fields};
use crate::contact::Qso;
use crate::export::rules::{
    CompiledGroupDefinition, ExpressionContext, ExpressionValue, PartitionRule,
};
use crate::export::select::{
    ContextResolution, ExportDecision, ExportError, ExportWarning, SelectedGroupMapping,
    select_mappings_with_compiled,
};
use crate::export::{ExportContext, OutputPurpose, ProfileSet};
use crate::group::DefinitionSet;
use crate::ids::{GroupId, QsoId};
use crate::reference::StationSide;
use anyhow::Context;
use std::collections::BTreeMap;

/// In-memory reference output plus decisions and diagnostics for a selected contact snapshot.
///
/// This is not a serialized ADIF file or proof of submission eligibility. A
/// caller must resolve outstanding decisions and enforce publication safeguards.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReferenceExportPlan {
    /// Planned partitions and their in-memory reference rows.
    pub files: Vec<ReferenceExportFile>,
    /// Unresolved contextual choices that must be handled before publication.
    pub decisions: Vec<ExportDecision>,
    /// Reportable omitted fields or unsupported groups.
    pub warnings: Vec<ExportWarning>,
    /// Unique native contact identities supplied, distinct from expanded output-entry count.
    pub source_contact_count: usize,
}

/// Entries sharing one planned output partition, without a filesystem path or open file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceExportFile {
    /// Ordered logical partition dimensions; not a safe filesystem path by itself.
    pub partition_key: Vec<String>,
    /// Reference-output rows assigned to this partition.
    pub entries: Vec<ReferenceExportEntry>,
}

/// One output row linked to its original native contact identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceExportEntry {
    /// Original contact identity shared by any reference-expanded rows.
    pub qso_id: QsoId,
    /// Selected reference mappings only; common contact-field formatting belongs to an adapter.
    pub reference_fields: BTreeMap<String, String>,
}

/// Selected partition components and reference bindings for one expanded output row.
#[derive(Clone, Default)]
struct Row {
    /// Logical partition dimensions selected for this row.
    key: Vec<String>,
    /// Reference values available to field expressions in this row.
    bindings: BTreeMap<String, String>,
}

/// Create a contact-scoped readiness diagnostic for missing required export input.
fn not_ready(qso: &Qso, reason: impl Into<String>) -> anyhow::Error {
    ExportError::NotReady {
        qso_id: qso.id,
        reason: reason.into(),
    }
    .into()
}

/// Expand rows over one group’s selected-side identifiers, optionally adding partition keys.
///
/// When no identifiers exist, retain a row only if the rule explicitly permits it.
fn extend_references(
    qso: &Qso,
    group: GroupId,
    rows: Vec<Row>,
    side: StationSide,
    binding: &str,
    partition: bool,
    keep_one: bool,
) -> anyhow::Result<Vec<Row>> {
    let identifiers = reference_identifiers(qso, group, side);
    if identifiers.is_empty() {
        if keep_one {
            return Ok(rows);
        }
        return Err(not_ready(
            qso,
            format!("required reference missing for {binding}"),
        ));
    }
    let mut result = Vec::new();
    for row in rows {
        for identifier in &identifiers {
            let mut row = row.clone();
            row.bindings.insert(binding.into(), identifier.clone());
            if partition {
                row.key.push(identifier.clone());
            }
            result.push(row);
        }
    }
    Ok(result)
}

/// Plan one selected group’s reference-specific partitions, expansions and fields for a contact.
fn targeted_entries(
    qso: &Qso,
    selected: &SelectedGroupMapping,
    definition: &CompiledGroupDefinition<'_>,
) -> anyhow::Result<Vec<(Vec<String>, ReferenceExportEntry)>> {
    let plan = &selected.plan;
    let mut rows = vec![Row::default()];
    for partition in &plan.file_partition {
        match partition {
            PartitionRule::EachReference { side, binding } => {
                rows = extend_references(qso, selected.group_id, rows, *side, binding, true, false)?
            }
            PartitionRule::ByValue(expression) => {
                for row in &mut rows {
                    let context = ExpressionContext {
                        qso: Some(qso),
                        current_identifier: None,
                        bindings: &row.bindings,
                    };
                    match definition.evaluate(expression, &context)? {
                        ExpressionValue::Scalar(value) => row.key.push(value),
                        ExpressionValue::Absent => row.key.push(String::new()),
                        _ => return Err(not_ready(qso, "required file-partition value missing")),
                    }
                }
            }
        }
    }
    for expansion in &plan.entry_expansion {
        rows = extend_references(
            qso,
            selected.group_id,
            rows,
            expansion.side,
            &expansion.binding,
            false,
            expansion.keep_one_if_empty,
        )?;
    }
    rows.into_iter()
        .map(|row| {
            let context = ExpressionContext {
                qso: Some(qso),
                current_identifier: None,
                bindings: &row.bindings,
            };
            let reference_fields = evaluate_fields(definition, &plan.fields, &context)?;
            Ok((
                row.key,
                ReferenceExportEntry {
                    qso_id: qso.id,
                    reference_fields,
                },
            ))
        })
        .collect()
}

/// Plan reference fields for an explicitly supplied, consistent local contact snapshot.
///
/// Ordinary output emits one entry per unique QSO; targeted rules may expand
/// only the chosen group’s references into partitions/entries. Identical repeated
/// IDs coalesce; differing versions of one ID fail rather than choosing a winner.
/// Invalid records or unusable mappings return errors with their original causes.
///
/// Inspect returned decisions and warnings before publication. This operation
/// does not fetch providers, take a storage snapshot, create files, validate a
/// complete program submission, or replace destinations on failure/cancellation.
#[tracing::instrument(skip_all, name = "model.reference_export.plan", level = "debug")]
pub fn plan_reference_export(
    qsos: &[Qso],
    context: &ExportContext,
    definitions: &DefinitionSet,
    profiles: &ProfileSet,
    resolutions: &BTreeMap<QsoId, ContextResolution>,
) -> anyhow::Result<ReferenceExportPlan> {
    let profile = profiles
        .get(&context.output_profile_id)
        .ok_or(ExportError::UnknownProfile {
            id: context.output_profile_id,
        })?;
    let mut unique: BTreeMap<QsoId, &Qso> = BTreeMap::new();
    for qso in qsos {
        qso.validate().map_err(|error| {
            let reason = error.to_string();
            error.context(ExportError::NotReady {
                qso_id: qso.id,
                reason,
            })
        })?;
        if let Some(previous) = unique.insert(qso.id, qso)
            && previous != qso
        {
            return Err(not_ready(
                qso,
                "multiple working versions for one contact identity",
            ));
        }
    }
    let mut plan = ReferenceExportPlan {
        source_contact_count: unique.len(),
        ..Default::default()
    };
    let mut files: BTreeMap<Vec<String>, Vec<ReferenceExportEntry>> = BTreeMap::new();
    let mut compiled_definitions = BTreeMap::new();
    for qso in unique.values() {
        let selected = select_mappings_with_compiled(
            qso,
            context,
            definitions,
            profiles,
            resolutions.get(&qso.id),
            &mut compiled_definitions,
        )?;
        plan.decisions.extend(selected.decisions);
        plan.warnings.extend(selected.warnings);
        let mut base_key = vec![profile.id.to_string()];
        match profile.purpose {
            OutputPurpose::OrdinaryInterchange => {
                let mut reference_fields = BTreeMap::new();
                let bindings = BTreeMap::new();
                let expression_context = ExpressionContext {
                    qso: Some(qso),
                    current_identifier: None,
                    bindings: &bindings,
                };
                for group in selected.groups {
                    let compiled =
                        compiled_definitions.get(&group.group_id).with_context(|| {
                            format!("group {} has no prepared export definition", group.group_id)
                        })?;
                    for (field, value) in
                        evaluate_fields(compiled, &group.plan.fields, &expression_context)?
                    {
                        if let Some(previous) =
                            reference_fields.insert(field.clone(), value.clone())
                            && previous != value
                        {
                            return Err(ExportError::ConflictingBindings {
                                group_id: group.group_id,
                                field,
                            }
                            .into());
                        }
                    }
                }
                files
                    .entry(base_key)
                    .or_default()
                    .push(ReferenceExportEntry {
                        qso_id: qso.id,
                        reference_fields,
                    });
            }
            OutputPurpose::Targeted => {
                base_key.push(
                    context
                        .target_group_id
                        .ok_or_else(|| not_ready(qso, "targeted output requires a target group"))?
                        .to_string(),
                );
                for group in selected.groups {
                    let compiled =
                        compiled_definitions.get(&group.group_id).with_context(|| {
                            format!("group {} has no prepared export definition", group.group_id)
                        })?;
                    for (partition, entry) in targeted_entries(qso, &group, compiled)? {
                        let mut key = base_key.clone();
                        key.extend(partition);
                        files.entry(key).or_default().push(entry);
                    }
                }
            }
        }
    }
    plan.files = files
        .into_iter()
        .map(|(partition_key, entries)| ReferenceExportFile {
            partition_key,
            entries,
        })
        .collect();
    let output_entry_count = plan
        .files
        .iter()
        .map(|file| file.entries.len())
        .sum::<usize>();
    let readiness = if plan.decisions.is_empty() {
        "ready"
    } else {
        "review"
    };
    tracing::debug!(
        event = "reference_export.planned",
        readiness,
        input_count = qsos.len(),
        source_contact_count = plan.source_contact_count,
        output_entry_count,
        partition_count = plan.files.len(),
        decision_count = plan.decisions.len(),
        warning_count = plan.warnings.len(),
    );
    Ok(plan)
}
