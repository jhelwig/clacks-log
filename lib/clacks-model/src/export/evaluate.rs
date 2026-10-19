use super::{rules::reference_identifiers, select::evaluate_fields};
use crate::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReferenceExportPlan {
    pub files: Vec<ReferenceExportFile>,
    pub decisions: Vec<ExportDecision>,
    pub warnings: Vec<ExportWarning>,
    pub source_contact_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceExportFile {
    pub partition_key: Vec<String>,
    pub entries: Vec<ReferenceExportEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceExportEntry {
    pub qso_id: QsoId,
    pub reference_fields: BTreeMap<String, String>,
}

#[derive(Clone, Default)]
struct Row {
    key: Vec<String>,
    bindings: BTreeMap<String, String>,
}

fn not_ready(qso: &Qso, reason: impl Into<String>) -> ExportError {
    ExportError::NotReady {
        qso_id: qso.id,
        reason: reason.into(),
    }
}

fn extend_references(
    qso: &Qso,
    group: GroupId,
    rows: Vec<Row>,
    side: StationSide,
    binding: &str,
    partition: bool,
    keep_one: bool,
) -> Result<Vec<Row>, ExportError> {
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

fn targeted_entries(
    qso: &Qso,
    selected: &SelectedGroupMapping,
    definition: &CompiledGroupDefinition,
) -> Result<Vec<(Vec<String>, ReferenceExportEntry)>, ExportError> {
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

pub fn plan_reference_export(
    qsos: &[Qso],
    context: &ExportContext,
    definitions: &DefinitionSet,
    profiles: &ProfileSet,
    resolutions: &BTreeMap<QsoId, ContextResolution>,
) -> Result<ReferenceExportPlan, ExportError> {
    let profile = profiles
        .get(&context.output_profile_id)
        .ok_or(ExportError::UnknownProfile {
            id: context.output_profile_id,
        })?;
    let mut unique: BTreeMap<QsoId, &Qso> = BTreeMap::new();
    for qso in qsos {
        qso.validate().map_err(|e| not_ready(qso, e.to_string()))?;
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
    for qso in unique.values() {
        let selected = select_mappings(
            qso,
            context,
            definitions,
            profiles,
            resolutions.get(&qso.id),
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
                    let compiled = compile_definition(
                        definitions
                            .get(group.group_id)
                            .expect("selection retained definition"),
                    )?;
                    for (field, value) in
                        evaluate_fields(&compiled, &group.plan.fields, &expression_context)?
                    {
                        if let Some(previous) =
                            reference_fields.insert(field.clone(), value.clone())
                            && previous != value
                        {
                            return Err(ExportError::ConflictingBindings {
                                group_id: group.group_id,
                                field,
                            });
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
                        .expect("selection validated target")
                        .to_string(),
                );
                for group in selected.groups {
                    let compiled = compile_definition(
                        definitions
                            .get(group.group_id)
                            .expect("selection retained definition"),
                    )?;
                    for (partition, entry) in targeted_entries(qso, &group, &compiled)? {
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
    Ok(plan)
}
