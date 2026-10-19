//! Declarative group mapping expressions, validation and bounded local evaluation.

use crate::contact::Qso;
use crate::group::{DefinitionError, GroupDefinition, TextNormalization};
use crate::ids::{GroupId, OutputProfileId};
use crate::reference::{ReferenceAssociation, StationSide};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A group’s ordinary interchange fields and separately selected submission plans.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupExportMappings {
    /// Reference fields used without targeted partitioning or expansion.
    pub ordinary_interchange: Vec<FieldBinding>,
    /// Submission plans keyed by the requesting output profile identity.
    pub targeted: BTreeMap<OutputProfileId, GroupExportPlan>,
}

/// Field bindings and ordered file/entry expansion rules for one group.
///
/// These rules describe in-memory output; they do not perform file publication.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupExportPlan {
    /// Conditional scalar field assignments evaluated for each output row.
    pub fields: Vec<FieldBinding>,
    /// Ordered partition dimensions; reference partitions also bind their selected value.
    pub file_partition: Vec<PartitionRule>,
    /// Ordered row expansions after partitioning, restricted to this group.
    pub entry_expansion: Vec<ExpansionRule>,
}

/// A serializable text-mapping expression.
///
/// The syntax represents local text mappings, with no script or network
/// operation. For validation and evaluation, see [`crate::export::rules::compile_definition`] and
/// [`crate::export::rules::CompiledGroupDefinition::evaluate`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueExpression {
    /// Fixed text.
    Literal(String),
    /// A recorded QSO field path.
    QsoField(QsoFieldPath),
    /// The single association identifier in the current lookup context.
    CurrentIdentifier,
    /// A group-scoped station-side identifier collection.
    Identifiers(StationSide),
    /// A named partition/expansion binding.
    BoundIdentifier(String),
    /// A named full-value regex capture over another expression.
    Capture {
        /// Input expression for capture extraction.
        value: Box<ValueExpression>,
        /// Whole-value regex declared in this definition.
        pattern: String,
        /// Required named capture to return from each match.
        name: String,
    },
    /// A collection-to-scalar join expression.
    Join {
        /// Expression required to produce a collection or absence.
        values: Box<ValueExpression>,
        /// Text inserted between collection elements.
        separator: String,
    },
}

/// Recorded contact fields available to mapping expressions without provider fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QsoFieldPath {
    /// Recorded own-station callsign.
    OwnCallsign,
    /// Recorded contacted-station callsign.
    ContactedCallsign,
    /// Recorded own-station primary subdivision, without provider fallback.
    OwnPrimarySubdivision,
    /// Recorded contacted-station primary subdivision, without provider fallback.
    ContactedPrimarySubdivision,
}

/// Conditional assignment of a scalar expression to a named output field.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldBinding {
    /// Nonblank output field name; format-specific adapters interpret it.
    pub field: String,
    /// Expression that must produce a scalar or absence for assignment.
    pub value: ValueExpression,
    /// Condition controlling whether the binding participates.
    pub when: BindingCondition,
}

/// The condition under which one output-field assignment is evaluated.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BindingCondition {
    /// An unconditional binding.
    Always,
    /// A presence requirement for the named row binding.
    BoundPresent(String),
    /// A group-participation requirement for the selected station side.
    AssociationPresent(StationSide),
}

/// An ordered contribution to the planned file partition key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartitionRule {
    /// An expression-valued partition dimension.
    ByValue(ValueExpression),
    /// A station-reference partition dimension with a named value.
    EachReference {
        /// Station side supplying this group’s partition references.
        side: StationSide,
        /// Unique name exposing the reference selected for this partition.
        binding: String,
    },
}

/// Per-reference output-entry expansion for the selected group and station side.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpansionRule {
    /// Station side whose identifiers supply rows for this group.
    pub side: StationSide,
    /// Unique name exposing the selected reference to field expressions.
    pub binding: String,
    /// Whether an empty identifier set permits one unbound output row.
    pub keep_one_if_empty: bool,
}

use std::collections::BTreeSet;

/// An evaluated mapping value that retains the distinction between absence and collections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExpressionValue {
    /// No text value.
    Absent,
    /// One text value.
    Scalar(String),
    /// An ordered sequence of text values.
    Collection(Vec<String>),
}

/// Read-only inputs available to an expression in its current association or export row.
///
/// Supply a QSO for contact-field/collection access, a current identifier for
/// association-local lookup, and named bindings for declared partition/expansion values.
pub struct ExpressionContext<'a> {
    /// Contact facts available for QSO field paths and station-side reference collections.
    pub qso: Option<&'a Qso>,
    /// Single association value used for catalog/identifier-local expressions.
    pub current_identifier: Option<&'a str>,
    /// Values already selected by declared partition or expansion rules.
    pub bindings: &'a BTreeMap<String, String>,
}

/// Validated group data with reusable, nonserialized expression support.
///
/// This owned definition snapshot is paired with evaluation state rather than
/// serialized as contact data. See [`crate::export::rules::compile_definition`], [`Self::evaluate`] and
/// [`Self::normalize_reference`] for its lifecycle and operations.
#[derive(Clone, Debug)]
pub struct CompiledGroupDefinition {
    /// Definition snapshot used for evaluation and group-scoped diagnostics.
    definition: GroupDefinition,
    /// Compiled full-value patterns keyed by validated definition paths.
    patterns: BTreeMap<String, regex::Regex>,
    /// Pattern text to compiled-path correspondence for expression evaluation.
    pattern_paths: BTreeMap<String, String>,
}

/// Static expression result category used to reject invalid mapping combinations.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// One text value, or absence at runtime.
    Scalar,
    /// A sequence of text values requiring explicit joining for scalar output.
    Collection,
}

impl CompiledGroupDefinition {
    /// Borrow the definition snapshot whose rules this compiled value evaluates.
    ///
    /// Changes to a separately held definition do not alter this compiled snapshot.
    pub fn definition(&self) -> &GroupDefinition {
        &self.definition
    }

    /// Attach this group’s identity and a definition location to a configuration diagnostic.
    fn error(&self, path: &str, reason: impl Into<String>) -> DefinitionError {
        DefinitionError::InvalidDefinition {
            group_id: self.definition.id,
            path: path.into(),
            reason: reason.into(),
        }
    }

    /// Borrow a compiled pattern or return a definition diagnostic for unavailable state.
    fn pattern(&self, path: &str) -> Result<&regex::Regex, DefinitionError> {
        self.patterns
            .get(path)
            .ok_or_else(|| self.error(path, "compiled pattern missing"))
    }

    /// Make a full-value regex available at a definition path for subsequent validation/evaluation.
    fn add_pattern(&mut self, path: &str, pattern: &str) -> Result<(), DefinitionError> {
        let regex = if let Some(previous) = self.pattern_paths.get(pattern) {
            self.pattern(previous)?.clone()
        } else {
            crate::group::full_regex(pattern).map_err(|e| self.error(path, e.to_string()))?
        };
        self.patterns.insert(path.into(), regex);
        self.pattern_paths
            .entry(pattern.into())
            .or_insert_with(|| path.into());
        Ok(())
    }

    /// Validate an expression’s required context and bindings and return its static result shape.
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
                if !self
                    .pattern(path)?
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

    /// Validate one output plan’s partition bindings, scalar fields and demonstrable assignment conflicts.
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

    /// Require a nonblank binding name that is unique within the current output plan.
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

    /// Evaluate a definition expression against explicit local inputs without mutating records.
    ///
    /// Use expressions belonging to this compiled definition so capture patterns
    /// are available. QSO fields and identifier collections require QSO context;
    /// `CurrentIdentifier` and named bindings use their corresponding context inputs.
    /// Missing or empty scalar values yield `Absent`; failed captures are omitted
    /// from collections, and joining an empty collection also yields `Absent`.
    /// Incompatible contexts, shapes or excessive nesting return `DefinitionError`.
    /// This operation does not select export context.
    pub fn evaluate(
        &self,
        expression: &ValueExpression,
        context: &ExpressionContext<'_>,
    ) -> Result<ExpressionValue, DefinitionError> {
        self.evaluate_depth(expression, context, 0)
    }

    /// Evaluate an expression with a bounded nesting budget and the supplied row/association context.
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
                let regex = self.pattern(path)?;
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

    /// Prepare one new or explicitly edited association using this compiled group’s rules.
    ///
    /// Require a matching group ID, apply configured normalization, then validate
    /// requiredness and whole-value syntax. Returns a prepared copy or a definition/
    /// identifier error. Do not use merely to reinterpret saved references after a refresh.
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

/// Collect recorded identifiers for one group and station side in deterministic set order.
///
/// Group-only associations have no identifier to emit; other groups are excluded.
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

/// Validate a supplied group definition for identifier, catalog and reference-export use.
///
/// Compile whole-value regexes, enforce expression/context/binding constraints,
/// and reject statically detectable mapping conflicts. Returns `DefinitionError`
/// with group and path context. Dynamic assignment conflicts remain evaluation errors.
/// No catalog download, native-record mutation or account provisioning occurs.
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
