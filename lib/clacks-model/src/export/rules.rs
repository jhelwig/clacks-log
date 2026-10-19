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
