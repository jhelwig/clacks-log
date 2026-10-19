use crate::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportError {
    UnknownProfile { id: OutputProfileId },
    UnknownTarget { id: GroupId },
    InvalidResolution { qso_id: QsoId, reason: String },
    ConflictingBindings { group_id: GroupId, field: String },
    Definition(DefinitionError),
    NotReady { qso_id: QsoId, reason: String },
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownProfile { id } => write!(f, "unknown output profile {id}"),
            Self::UnknownTarget { id } => write!(f, "unknown target group {id}"),
            Self::InvalidResolution { qso_id, reason } => {
                write!(f, "invalid export choice for {qso_id}: {reason}")
            }
            Self::ConflictingBindings { group_id, field } => {
                write!(f, "group {group_id} conflicts on output field {field}")
            }
            Self::Definition(e) => e.fmt(f),
            Self::NotReady { qso_id, reason } => {
                write!(f, "QSO {qso_id} is not ready for output: {reason}")
            }
        }
    }
}

impl std::error::Error for ExportError {}

impl From<DefinitionError> for ExportError {
    fn from(e: DefinitionError) -> Self {
        Self::Definition(e)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContextResolution {
    ChooseTarget(GroupId),
    OmitReported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportDecision {
    AmbiguousContextField {
        qso_id: QsoId,
        field: String,
        candidates: BTreeMap<GroupId, String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportWarning {
    OmittedContextField {
        qso_id: QsoId,
        field: String,
        candidates: BTreeMap<GroupId, String>,
    },
    UnsupportedGroup {
        qso_id: QsoId,
        group_id: GroupId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectedGroupMapping {
    pub group_id: GroupId,
    pub plan: GroupExportPlan,
    pub sides: BTreeSet<StationSide>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MappingSelection {
    pub groups: Vec<SelectedGroupMapping>,
    pub decisions: Vec<ExportDecision>,
    pub warnings: Vec<ExportWarning>,
}

pub(crate) fn has_association(qso: &Qso, group: GroupId, side: StationSide) -> bool {
    let station = match side {
        StationSide::Own => &qso.own_station,
        StationSide::Contacted => &qso.contacted_station,
    };
    station.references.iter().any(|r| r.group_id == group)
}

pub(crate) fn evaluate_fields(
    compiled: &CompiledGroupDefinition,
    fields: &[FieldBinding],
    context: &ExpressionContext<'_>,
) -> Result<BTreeMap<String, String>, ExportError> {
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
                    });
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

pub fn select_mappings(
    qso: &Qso,
    context: &ExportContext,
    definitions: &DefinitionSet,
    profiles: &ProfileSet,
    resolution: Option<&ContextResolution>,
) -> Result<MappingSelection, ExportError> {
    let profile = profiles
        .get(&context.output_profile_id)
        .ok_or(ExportError::UnknownProfile {
            id: context.output_profile_id,
        })?;
    if let Some(id) = context.target_group_id
        && definitions.get(id).is_none()
    {
        return Err(ExportError::UnknownTarget { id });
    }
    if profile.purpose == OutputPurpose::Targeted && context.target_group_id.is_none() {
        return Err(ExportError::NotReady {
            qso_id: qso.id,
            reason: "targeted output requires a target group".into(),
        });
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
            OutputPurpose::Targeted => {
                if Some(group_id) != context.target_group_id {
                    continue;
                }
                definition
                    .export_mappings
                    .targeted
                    .get(&profile.id)
                    .cloned()
                    .ok_or_else(|| ExportError::NotReady {
                        qso_id: qso.id,
                        reason: "target has no mapping for output profile".into(),
                    })?
            }
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
        });
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
            });
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
        });
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
