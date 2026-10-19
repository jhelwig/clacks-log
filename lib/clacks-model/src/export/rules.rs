use crate::{OutputProfileId, StationSide};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupExportMappings {
    pub ordinary_interchange: Vec<FieldBinding>,
    pub targeted: BTreeMap<OutputProfileId, GroupExportPlan>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupExportPlan {
    pub fields: Vec<FieldBinding>,
    pub file_partition: Vec<PartitionRule>,
    pub entry_expansion: Vec<ExpansionRule>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueExpression {
    Literal(String),
    QsoField(QsoFieldPath),
    CurrentIdentifier,
    Identifiers(StationSide),
    BoundIdentifier(String),
    Capture {
        value: Box<ValueExpression>,
        pattern: String,
        name: String,
    },
    Join {
        values: Box<ValueExpression>,
        separator: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QsoFieldPath {
    OwnCallsign,
    ContactedCallsign,
    OwnPrimarySubdivision,
    ContactedPrimarySubdivision,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldBinding {
    pub field: String,
    pub value: ValueExpression,
    pub when: BindingCondition,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingCondition {
    Always,
    BoundPresent(String),
    AssociationPresent(StationSide),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartitionRule {
    ByValue(ValueExpression),
    EachReference { side: StationSide, binding: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpansionRule {
    pub side: StationSide,
    pub binding: String,
    pub keep_one_if_empty: bool,
}

use crate::{
    DefinitionError, GroupDefinition, GroupId, Qso, ReferenceAssociation, TextNormalization,
};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExpressionValue {
    Absent,
    Scalar(String),
    Collection(Vec<String>),
}

pub struct ExpressionContext<'a> {
    pub qso: Option<&'a Qso>,
    pub current_identifier: Option<&'a str>,
    pub bindings: &'a BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct CompiledGroupDefinition {
    definition: GroupDefinition,
    patterns: BTreeMap<String, regex::Regex>,
    pattern_paths: BTreeMap<String, String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Scalar,
    Collection,
}

impl CompiledGroupDefinition {
    pub fn definition(&self) -> &GroupDefinition {
        &self.definition
    }

    fn error(&self, path: &str, reason: impl Into<String>) -> DefinitionError {
        DefinitionError::InvalidDefinition {
            group_id: self.definition.id,
            path: path.into(),
            reason: reason.into(),
        }
    }

    fn add_pattern(&mut self, path: &str, pattern: &str) -> Result<(), DefinitionError> {
        let regex = if let Some(previous) = self.pattern_paths.get(pattern) {
            self.patterns[previous].clone()
        } else {
            crate::group::full_regex(pattern).map_err(|e| self.error(path, e.to_string()))?
        };
        self.patterns.insert(path.into(), regex);
        self.pattern_paths
            .entry(pattern.into())
            .or_insert_with(|| path.into());
        Ok(())
    }

    fn check_expression(
        &mut self,
        expression: &ValueExpression,
        path: &str,
        bindings: &BTreeSet<String>,
        association_local: bool,
        depth: usize,
    ) -> Result<Shape, DefinitionError> {
        if depth > 32 {
            return Err(self.error(path, "expression nesting exceeds 32"));
        }
        Ok(match expression {
            ValueExpression::Literal(_) => Shape::Scalar,
            ValueExpression::QsoField(_) => {
                if association_local {
                    return Err(self.error(path, "catalog has no QSO field context"));
                }
                Shape::Scalar
            }
            ValueExpression::CurrentIdentifier => {
                if !association_local {
                    return Err(self.error(path,"current identifier requires a single association context; use Identifiers or BoundIdentifier"));
                }
                Shape::Scalar
            }
            ValueExpression::Identifiers(_) => {
                if association_local {
                    return Err(self.error(path, "catalog has no station-side collection"));
                }
                Shape::Collection
            }
            ValueExpression::BoundIdentifier(name) => {
                if !bindings.contains(name) {
                    return Err(self.error(path, format!("undeclared binding {name}")));
                }
                Shape::Scalar
            }
            ValueExpression::Capture {
                value,
                pattern,
                name,
            } => {
                let shape = self.check_expression(
                    value,
                    &format!("{path}.value"),
                    bindings,
                    association_local,
                    depth + 1,
                )?;
                self.add_pattern(path, pattern)?;
                if !self.patterns[path]
                    .capture_names()
                    .flatten()
                    .any(|n| n == name)
                {
                    return Err(self.error(path, format!("missing capture {name}")));
                }
                shape
            }
            ValueExpression::Join { values, .. } => {
                if self.check_expression(
                    values,
                    &format!("{path}.values"),
                    bindings,
                    association_local,
                    depth + 1,
                )? != Shape::Collection
                {
                    return Err(self.error(path, "Join requires a collection"));
                }
                Shape::Scalar
            }
        })
    }

    fn check_plan(&mut self, plan: &GroupExportPlan, path: &str) -> Result<(), DefinitionError> {
        let mut bindings = BTreeSet::new();
        for (i, partition) in plan.file_partition.iter().enumerate() {
            match partition {
                PartitionRule::ByValue(value) => {
                    if self.check_expression(
                        value,
                        &format!("{path}.partition[{i}]"),
                        &bindings,
                        false,
                        0,
                    )? != Shape::Scalar
                    {
                        return Err(self.error(path, "partition value must be scalar"));
                    }
                }
                PartitionRule::EachReference { binding, .. } => {
                    self.declare_binding(&mut bindings, binding, path)?
                }
            }
        }
        for expansion in &plan.entry_expansion {
            self.declare_binding(&mut bindings, &expansion.binding, path)?;
        }
        let mut static_fields: BTreeMap<&str, &str> = BTreeMap::new();
        for (i, field) in plan.fields.iter().enumerate() {
            let location = format!("{path}.fields[{i}]");
            if field.field.trim().is_empty() {
                return Err(self.error(&location, "output field cannot be blank"));
            }
            if let BindingCondition::BoundPresent(name) = &field.when
                && !bindings.contains(name)
            {
                return Err(self.error(&location, format!("undeclared condition binding {name}")));
            }
            if self.check_expression(&field.value, &location, &bindings, false, 0)? != Shape::Scalar
            {
                return Err(self.error(
                    &location,
                    "field requires scalar or absence; join collections explicitly",
                ));
            }
            if field.when == BindingCondition::Always
                && let ValueExpression::Literal(value) = &field.value
                && let Some(previous) = static_fields.insert(&field.field, value)
                && previous != value
            {
                return Err(self.error(
                    &location,
                    format!("conflicting assignments to {}", field.field),
                ));
            }
        }
        Ok(())
    }

    fn declare_binding(
        &self,
        bindings: &mut BTreeSet<String>,
        name: &str,
        path: &str,
    ) -> Result<(), DefinitionError> {
        if name.trim().is_empty() || !bindings.insert(name.into()) {
            return Err(self.error(path, format!("blank or duplicate binding {name}")));
        }
        Ok(())
    }

    pub fn evaluate(
        &self,
        expression: &ValueExpression,
        context: &ExpressionContext<'_>,
    ) -> Result<ExpressionValue, DefinitionError> {
        self.evaluate_depth(expression, context, 0)
    }

    fn evaluate_depth(
        &self,
        expression: &ValueExpression,
        context: &ExpressionContext<'_>,
        depth: usize,
    ) -> Result<ExpressionValue, DefinitionError> {
        if depth > 32 {
            return Err(self.error("expression", "expression nesting exceeds 32"));
        }
        let scalar = |v: Option<String>| {
            v.filter(|s| !s.is_empty())
                .map(ExpressionValue::Scalar)
                .unwrap_or(ExpressionValue::Absent)
        };
        Ok(match expression {
            ValueExpression::Literal(v) => scalar(Some(v.clone())),
            ValueExpression::CurrentIdentifier => {
                scalar(context.current_identifier.map(str::to_owned))
            }
            ValueExpression::BoundIdentifier(name) => scalar(context.bindings.get(name).cloned()),
            ValueExpression::QsoField(path) => {
                let q = context
                    .qso
                    .ok_or_else(|| self.error("expression", "QSO context missing"))?;
                scalar(match path {
                    QsoFieldPath::OwnCallsign => Some(q.own_station.callsign.as_str().into()),
                    QsoFieldPath::ContactedCallsign => {
                        Some(q.contacted_station.callsign.as_str().into())
                    }
                    QsoFieldPath::OwnPrimarySubdivision => q
                        .own_station
                        .location
                        .as_ref()
                        .and_then(|l| l.primary_subdivision.clone()),
                    QsoFieldPath::ContactedPrimarySubdivision => q
                        .contacted_station
                        .location
                        .as_ref()
                        .and_then(|l| l.primary_subdivision.clone()),
                })
            }
            ValueExpression::Identifiers(side) => {
                let q = context
                    .qso
                    .ok_or_else(|| self.error("expression", "QSO context missing"))?;
                ExpressionValue::Collection(reference_identifiers(q, self.definition.id, *side))
            }
            ValueExpression::Capture {
                value,
                pattern,
                name,
            } => {
                let path = self.pattern_paths.get(pattern).ok_or_else(|| {
                    self.error("expression", "pattern not compiled in this definition")
                })?;
                let regex = &self.patterns[path];
                let capture = |v: String| {
                    regex
                        .captures(&v)
                        .and_then(|c| c.name(name).map(|m| m.as_str().to_owned()))
                };
                match self.evaluate_depth(value, context, depth + 1)? {
                    ExpressionValue::Absent => ExpressionValue::Absent,
                    ExpressionValue::Scalar(v) => scalar(capture(v)),
                    ExpressionValue::Collection(v) => {
                        ExpressionValue::Collection(v.into_iter().filter_map(capture).collect())
                    }
                }
            }
            ValueExpression::Join { values, separator } => {
                match self.evaluate_depth(values, context, depth + 1)? {
                    ExpressionValue::Collection(v) => {
                        if v.is_empty() {
                            ExpressionValue::Absent
                        } else {
                            ExpressionValue::Scalar(v.join(separator))
                        }
                    }
                    ExpressionValue::Absent => ExpressionValue::Absent,
                    _ => return Err(self.error("expression", "Join requires a collection")),
                }
            }
        })
    }

    pub fn normalize_reference(
        &self,
        association: &ReferenceAssociation,
    ) -> Result<ReferenceAssociation, DefinitionError> {
        let d = &self.definition;
        if association.group_id != d.id {
            return Err(self.error("identifier", "definition ID does not match association"));
        }
        let mut value = association.identifier.clone();
        for operation in &d.identifier.normalization {
            value = value.map(|v| match operation {
                TextNormalization::Trim => v.trim().into(),
                TextNormalization::Uppercase => v.to_uppercase(),
                TextNormalization::Lowercase => v.to_lowercase(),
            });
        }
        value = value.filter(|v| !v.trim().is_empty());
        if d.identifier.required && value.is_none() {
            return Err(DefinitionError::MissingIdentifier { group_id: d.id });
        }
        if let Some(v) = &value
            && let Some(pattern) = self.patterns.get("identifier.syntax")
            && !pattern.is_match(v)
        {
            return Err(DefinitionError::InvalidIdentifier {
                group_id: d.id,
                value: v.clone(),
            });
        }
        Ok(ReferenceAssociation {
            group_id: d.id,
            identifier: value,
        })
    }
}

pub(crate) fn reference_identifiers(qso: &Qso, group: GroupId, side: StationSide) -> Vec<String> {
    let station = match side {
        StationSide::Own => &qso.own_station,
        StationSide::Contacted => &qso.contacted_station,
    };
    station
        .references
        .iter()
        .filter(|r| r.group_id == group)
        .filter_map(|r| r.identifier.clone())
        .collect()
}

pub fn compile_definition(
    definition: &GroupDefinition,
) -> Result<CompiledGroupDefinition, DefinitionError> {
    let mut compiled = CompiledGroupDefinition {
        definition: definition.clone(),
        patterns: BTreeMap::new(),
        pattern_paths: BTreeMap::new(),
    };
    if definition.name.trim().is_empty() {
        return Err(compiled.error("name", "group name cannot be blank"));
    }
    if let Some(pattern) = &definition.identifier.syntax {
        compiled.add_pattern("identifier.syntax", pattern)?;
    }
    if let Some(catalog) = &definition.catalog {
        if catalog.display_name.trim().is_empty()
            || catalog
                .details
                .iter()
                .any(|d| d.field.trim().is_empty() || d.label.trim().is_empty())
        {
            return Err(compiled.error("catalog", "metadata keys/labels cannot be blank"));
        }
        if compiled.check_expression(
            &catalog.lookup_key,
            "catalog.lookup_key",
            &BTreeSet::new(),
            true,
            0,
        )? != Shape::Scalar
        {
            return Err(compiled.error("catalog.lookup_key", "key must be scalar"));
        }
    }
    compiled.check_plan(
        &GroupExportPlan {
            fields: definition.export_mappings.ordinary_interchange.clone(),
            ..Default::default()
        },
        "ordinary_interchange",
    )?;
    for (id, plan) in &definition.export_mappings.targeted {
        compiled.check_plan(plan, &format!("targeted.{id}"))?;
    }
    Ok(compiled)
}
