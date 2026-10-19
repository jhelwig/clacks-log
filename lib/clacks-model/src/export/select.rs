//! Per-contact mapping selection with explicit resolution of contextual ambiguity.

use crate::contact::Qso;
use crate::export::rules::{
    BindingCondition, CompiledGroupDefinition, ExpressionContext, ExpressionValue, FieldBinding,
    GroupExportPlan, compile_definition,
};
use crate::export::{ExportContext, OutputPurpose, ProfileSet};
use crate::group::{DefinitionError, DefinitionSet};
use crate::ids::{GroupId, OutputProfileId, QsoId};
use crate::reference::StationSide;
use std::collections::{BTreeMap, BTreeSet};

/// A reference-export context, assignment or readiness failure.
///
/// Variants identify the affected output profile, group, field or contact.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ExportError {
    /// The requested output profile is unavailable.
    #[error("unknown output profile {id}")]
    UnknownProfile {
        /// Requested profile identity.
        id: OutputProfileId,
    },
    /// The selected group has no available definition.
    #[error("unknown target group {id}")]
    UnknownTarget {
        /// Requested group identity.
        id: GroupId,
    },
    /// A supplied ambiguity choice is not applicable to this contact/context.
    #[error("invalid export choice for {qso_id}: {reason}")]
    InvalidResolution {
        /// Contact needing a valid choice.
        qso_id: QsoId,
        /// Why the supplied resolution cannot be used.
        reason: String,
    },
    /// Selected mappings assign differing values to one output field.
    #[error("group {group_id} conflicts on output field {field}")]
    ConflictingBindings {
        /// Group responsible for the conflicting assignment.
        group_id: GroupId,
        /// Output field receiving incompatible values.
        field: String,
    },
    /// Required data or context prevents planning this contact’s output.
    #[error("QSO {qso_id} is not ready for output: {reason}")]
    NotReady {
        /// Contact that cannot currently produce the requested output.
        qso_id: QsoId,
        /// Missing input or invalid state requiring attention.
        reason: String,
    },
}

/// An explicit caller decision for ambiguous contextual fields on one contact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContextResolution {
    /// Select a candidate group for this contact’s contextual fields.
    ChooseTarget(GroupId),
    /// Omit ambiguous fields explicitly and report their candidate values.
    OmitReported,
}

/// A required operator choice that prevents unambiguous reference output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportDecision {
    /// Participating groups supply different values for one contextual field.
    AmbiguousContextField {
        /// Contact whose contextual field requires a decision.
        qso_id: QsoId,
        /// Output field with competing group values.
        field: String,
        /// Candidate value for each participating group.
        candidates: BTreeMap<GroupId, String>,
    },
}

/// A reported omission or unsupported association without altering the native contact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportWarning {
    /// The caller explicitly chose omission of an ambiguous field.
    OmittedContextField {
        /// Contact whose field was omitted.
        qso_id: QsoId,
        /// Omitted contextual field.
        field: String,
        /// Values omitted rather than chosen arbitrarily.
        candidates: BTreeMap<GroupId, String>,
    },
    /// No available definition can supply this association’s mappings.
    UnsupportedGroup {
        /// Contact retaining the unsupported participation.
        qso_id: QsoId,
        /// Unavailable group definition.
        group_id: GroupId,
    },
}

/// A group plan selected for this contact’s participation and export context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedGroupMapping {
    /// Participating group whose definition supplies this plan.
    pub group_id: GroupId,
    /// Context-selected field, partition and expansion rules.
    pub plan: GroupExportPlan,
    /// Station sides with recorded participation in this group.
    pub sides: BTreeSet<StationSide>,
}

/// Selected per-contact plans plus outstanding decisions and reportable warnings.
///
/// This value may contain unresolved decisions; it is not a publication authorization.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MappingSelection {
    /// Supported selected plans, without arbitrary competing contextual assignments.
    pub groups: Vec<SelectedGroupMapping>,
    /// Choices still required before publishing output.
    pub decisions: Vec<ExportDecision>,
    /// Explicit omissions and unsupported-group diagnostics for the export report.
    pub warnings: Vec<ExportWarning>,
}

/// Check group participation on a station side, including group-only associations.
pub(crate) fn has_association(qso: &Qso, group: GroupId, side: StationSide) -> bool {
    let station = match side {
        StationSide::Own => &qso.own_station,
        StationSide::Contacted => &qso.contacted_station,
    };
    station.references.iter().any(|r| r.group_id == group)
}

/// Evaluate enabled scalar assignments and reject conflicting values for one output field.
///
/// Absent expressions omit assignments; collection values require explicit joining.
pub(crate) fn evaluate_fields(
    compiled: &CompiledGroupDefinition,
    fields: &[FieldBinding],
    context: &ExpressionContext<'_>,
) -> anyhow::Result<BTreeMap<String, String>> {
    let group_id = compiled.definition().id;
    let mut output = BTreeMap::new();
    for binding in fields {
        let enabled = match &binding.when {
            BindingCondition::Always => true,
            BindingCondition::BoundPresent(name) => context.bindings.contains_key(name),
            BindingCondition::AssociationPresent(side) => context
                .qso
                .is_some_and(|q| has_association(q, group_id, *side)),
        };
        if !enabled {
            continue;
        }
        match compiled.evaluate(&binding.value, context)? {
            ExpressionValue::Absent => {}
            ExpressionValue::Scalar(value) => {
                if let Some(previous) = output.insert(binding.field.clone(), value.clone())
                    && previous != value
                {
                    return Err(ExportError::ConflictingBindings {
                        group_id,
                        field: binding.field.clone(),
                    }
                    .into());
                }
            }
            ExpressionValue::Collection(_) => {
                return Err(DefinitionError::InvalidDefinition {
                    group_id,
                    path: binding.field.clone(),
                    reason: "field requires joined collection".into(),
                }
                .into());
            }
        }
    }
    Ok(output)
}

/// Select reference mappings for one contact without changing its participation or values.
///
/// Ordinary output retains supported group mappings; a target limits contextual
/// fields. Targeted output uses only the selected group’s profile-specific plan.
/// Supply explicit resolutions for competing contextual values, or inspect returned
/// decisions before publication. Registry keys must match embedded profile IDs.
/// Unknown profiles/targets, invalid choices and unusable definitions return errors;
/// unknown associated groups are reported where applicable, never added or rewritten.
pub fn select_mappings(
    qso: &Qso,
    context: &ExportContext,
    definitions: &DefinitionSet,
    profiles: &ProfileSet,
    resolution: Option<&ContextResolution>,
) -> anyhow::Result<MappingSelection> {
    let profile = profiles
        .get(&context.output_profile_id)
        .ok_or(ExportError::UnknownProfile {
            id: context.output_profile_id,
        })?;
    if let Some(id) = context.target_group_id
        && definitions.get(id).is_none()
    {
        return Err(ExportError::UnknownTarget { id }.into());
    }
    if profile.purpose == OutputPurpose::Targeted && context.target_group_id.is_none() {
        return Err(ExportError::NotReady {
            qso_id: qso.id,
            reason: "targeted output requires a target group".into(),
        }
        .into());
    }
    let group_ids: BTreeSet<_> = qso
        .own_station
        .references
        .iter()
        .chain(&qso.contacted_station.references)
        .map(|r| r.group_id)
        .collect();
    let mut selection = MappingSelection::default();
    let mut candidates: BTreeMap<String, BTreeMap<GroupId, String>> = BTreeMap::new();
    let bindings = BTreeMap::new();
    let expression_context = ExpressionContext {
        qso: Some(qso),
        current_identifier: None,
        bindings: &bindings,
    };
    for group_id in group_ids {
        if profile.purpose == OutputPurpose::Targeted && Some(group_id) != context.target_group_id {
            continue;
        }
        let Some(definition) = definitions.get(group_id) else {
            selection.warnings.push(ExportWarning::UnsupportedGroup {
                qso_id: qso.id,
                group_id,
            });
            continue;
        };
        let compiled = compile_definition(definition)?;
        let mut plan = match profile.purpose {
            OutputPurpose::OrdinaryInterchange => GroupExportPlan {
                fields: definition.export_mappings.ordinary_interchange.clone(),
                ..Default::default()
            },
            OutputPurpose::Targeted => definition
                .export_mappings
                .targeted
                .get(&profile.id)
                .cloned()
                .ok_or_else(|| ExportError::NotReady {
                    qso_id: qso.id,
                    reason: "target has no mapping for output profile".into(),
                })?,
        };
        if context.target_group_id.is_some() && context.target_group_id != Some(group_id) {
            plan.fields
                .retain(|f| !profile.contextual_fields.contains(&f.field));
        }
        if profile.purpose == OutputPurpose::OrdinaryInterchange {
            let values = evaluate_fields(&compiled, &plan.fields, &expression_context)?;
            for (field, value) in values {
                if profile.contextual_fields.contains(&field) {
                    candidates.entry(field).or_default().insert(group_id, value);
                }
            }
        }
        let sides = [StationSide::Own, StationSide::Contacted]
            .into_iter()
            .filter(|side| has_association(qso, group_id, *side))
            .collect();
        selection.groups.push(SelectedGroupMapping {
            group_id,
            plan,
            sides,
        });
    }
    if profile.purpose == OutputPurpose::Targeted && selection.groups.is_empty() {
        return Err(ExportError::NotReady {
            qso_id: qso.id,
            reason: "contact does not participate in target group".into(),
        }
        .into());
    }
    let ambiguous: Vec<_> = candidates
        .into_iter()
        .filter(|(_, values)| values.values().collect::<BTreeSet<_>>().len() > 1)
        .collect();
    if let Some(ContextResolution::ChooseTarget(id)) = resolution {
        if context.target_group_id.is_some() || !ambiguous.iter().any(|(_, c)| c.contains_key(id)) {
            return Err(ExportError::InvalidResolution {
                qso_id: qso.id,
                reason: "choice is not an ambiguous candidate".into(),
            }
            .into());
        }
        return select_mappings(
            qso,
            &ExportContext {
                target_group_id: Some(*id),
                ..context.clone()
            },
            definitions,
            profiles,
            None,
        );
    }
    if resolution.is_some() && ambiguous.is_empty() {
        return Err(ExportError::InvalidResolution {
            qso_id: qso.id,
            reason: "no ambiguous field to omit".into(),
        }
        .into());
    }
    for (field, candidates) in ambiguous {
        for group in &mut selection.groups {
            group.plan.fields.retain(|f| f.field != field);
        }
        if resolution == Some(&ContextResolution::OmitReported) {
            selection.warnings.push(ExportWarning::OmittedContextField {
                qso_id: qso.id,
                field,
                candidates,
            });
        } else {
            selection
                .decisions
                .push(ExportDecision::AmbiguousContextField {
                    qso_id: qso.id,
                    field,
                    candidates,
                });
        }
    }
    Ok(selection)
}
