//! Relational value-expression chains for group export rules.

use super::{read_side, side};
use crate::rows::definitions::ExpressionRow;
use anyhow::Context;
use clacks_model::export::rules::{QsoFieldPath, ValueExpression};
use sqlx::SqliteConnection;
use std::collections::BTreeSet;

/// Store a bounded unary expression chain bottom-up using same-payload child references.
pub(super) async fn insert(
    connection: &mut SqliteConnection,
    payload: i64,
    expression: &ValueExpression,
) -> anyhow::Result<i64> {
    let mut chain = Vec::new();
    let mut current = expression;
    loop {
        anyhow::ensure!(chain.len() <= 32, "expression nesting exceeds 32");
        chain.push(current);
        current = match current {
            ValueExpression::Capture { value, .. } => value,
            ValueExpression::Join { values, .. } => values,
            _ => break,
        };
    }
    let mut child = None;
    for value in chain.into_iter().rev() {
        let row = match value {
            ValueExpression::Literal(text) => ExpressionRow {
                kind: "literal".into(),
                text: Some(text.clone()),
                side: None,
                child,
                pattern: None,
                capture_name: None,
            },
            ValueExpression::QsoField(path) => ExpressionRow {
                kind: "qso_field".into(),
                text: Some(
                    match path {
                        QsoFieldPath::OwnCallsign => "own_call",
                        QsoFieldPath::ContactedCallsign => "contacted_call",
                        QsoFieldPath::OwnPrimarySubdivision => "own_primary",
                        QsoFieldPath::ContactedPrimarySubdivision => "contacted_primary",
                    }
                    .into(),
                ),
                side: None,
                child,
                pattern: None,
                capture_name: None,
            },
            ValueExpression::CurrentIdentifier => ExpressionRow {
                kind: "current".into(),
                text: None,
                side: None,
                child,
                pattern: None,
                capture_name: None,
            },
            ValueExpression::Identifiers(value) => ExpressionRow {
                kind: "identifiers".into(),
                text: None,
                side: Some(side(*value).into()),
                child,
                pattern: None,
                capture_name: None,
            },
            ValueExpression::BoundIdentifier(text) => ExpressionRow {
                kind: "bound".into(),
                text: Some(text.clone()),
                side: None,
                child,
                pattern: None,
                capture_name: None,
            },
            ValueExpression::Capture { pattern, name, .. } => ExpressionRow {
                kind: "capture".into(),
                text: None,
                side: None,
                child,
                pattern: Some(pattern.clone()),
                capture_name: Some(name.clone()),
            },
            ValueExpression::Join { separator, .. } => ExpressionRow {
                kind: "join".into(),
                text: Some(separator.clone()),
                side: None,
                child,
                pattern: None,
                capture_name: None,
            },
        };
        child = Some(
            sqlx::query_file!(
                "sql/export/expression_insert.sql",
                payload,
                row.kind,
                row.text,
                row.side,
                row.child,
                row.pattern,
                row.capture_name
            )
            .fetch_one(&mut *connection)
            .await?
            .expression_id,
        );
    }
    child.context("expression chain is empty")
}

/// Read a bounded chain, rejecting cycles, cross-payload children and variant fields.
pub(super) async fn load(
    connection: &mut SqliteConnection,
    payload: i64,
    root: i64,
) -> anyhow::Result<ValueExpression> {
    let mut chain = Vec::new();
    let mut seen = BTreeSet::new();
    let mut current = root;
    loop {
        anyhow::ensure!(
            chain.len() <= 32 && seen.insert(current),
            "expression cycle or excessive depth"
        );
        let row = sqlx::query_file_as!(
            ExpressionRow,
            "sql/export/expression_select.sql",
            current,
            payload
        )
        .fetch_one(&mut *connection)
        .await?;
        let next = row.child;
        chain.push(row);
        match next {
            Some(child) => current = child,
            None => break,
        }
    }
    let mut child = None;
    for row in chain.into_iter().rev() {
        let value = match row {
            ExpressionRow {
                kind,
                text: Some(text),
                side: None,
                pattern: None,
                capture_name: None,
                ..
            } if kind == "literal" && child.is_none() => ValueExpression::Literal(text),
            ExpressionRow {
                kind,
                text: Some(path),
                side: None,
                pattern: None,
                capture_name: None,
                ..
            } if kind == "qso_field" && child.is_none() => {
                ValueExpression::QsoField(match path.as_str() {
                    "own_call" => QsoFieldPath::OwnCallsign,
                    "contacted_call" => QsoFieldPath::ContactedCallsign,
                    "own_primary" => QsoFieldPath::OwnPrimarySubdivision,
                    "contacted_primary" => QsoFieldPath::ContactedPrimarySubdivision,
                    _ => anyhow::bail!("unknown QSO field path"),
                })
            }
            ExpressionRow {
                kind,
                text: None,
                side: None,
                pattern: None,
                capture_name: None,
                ..
            } if kind == "current" && child.is_none() => ValueExpression::CurrentIdentifier,
            ExpressionRow {
                kind,
                text: None,
                side: Some(side),
                pattern: None,
                capture_name: None,
                ..
            } if kind == "identifiers" && child.is_none() => {
                ValueExpression::Identifiers(read_side(&side)?)
            }
            ExpressionRow {
                kind,
                text: Some(text),
                side: None,
                pattern: None,
                capture_name: None,
                ..
            } if kind == "bound" && child.is_none() => ValueExpression::BoundIdentifier(text),
            ExpressionRow {
                kind,
                text: None,
                side: None,
                pattern: Some(pattern),
                capture_name: Some(name),
                ..
            } if kind == "capture" => ValueExpression::Capture {
                value: Box::new(child.take().context("invalid expression variant fields")?),
                pattern,
                name,
            },
            ExpressionRow {
                kind,
                text: Some(separator),
                side: None,
                pattern: None,
                capture_name: None,
                ..
            } if kind == "join" => ValueExpression::Join {
                values: Box::new(child.take().context("invalid expression variant fields")?),
                separator,
            },
            _ => anyhow::bail!("invalid expression variant fields"),
        };
        child = Some(value);
    }
    child.context("expression chain is empty")
}
