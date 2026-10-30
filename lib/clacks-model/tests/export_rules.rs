use crate::support;

use clacks_model::export::rules::{
    ExpressionContext, ExpressionValue, ValueExpression, compile_definition,
};
use clacks_model::group::{DefinitionError, GroupDefinition};
use clacks_model::ids::{GroupId, QsoId};
use clacks_model::reference::StationSide;
use std::collections::BTreeMap;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn definition_errors_are_not_record_errors() {
    let defs = support::definitions();
    let base = defs.get(support::id::<GroupId>(1)).unwrap();
    for expr in [
        ValueExpression::BoundIdentifier("undeclared".into()),
        ValueExpression::Capture {
            value: Box::new(ValueExpression::CurrentIdentifier),
            pattern: "[".into(),
            name: "park".into(),
        },
        ValueExpression::Capture {
            value: Box::new(ValueExpression::CurrentIdentifier),
            pattern: "[A-Z]+".into(),
            name: "park".into(),
        },
    ] {
        let mut d = base.clone().into_fields();
        d.catalog.as_mut().unwrap().lookup_key = expr;
        assert!(matches!(
            GroupDefinition::try_from(d),
            Err(DefinitionError::InvalidDefinition { .. })
        ));
    }
    let mut d = base.clone().into_fields();
    d.export_mappings.ordinary_interchange[0].value =
        ValueExpression::Identifiers(StationSide::Own);
    assert!(GroupDefinition::try_from(d).is_err());
}

#[test]
fn full_match_and_named_capture() -> TestResult {
    let defs = support::definitions();
    let mut d = defs
        .get(support::id::<GroupId>(1))
        .unwrap()
        .clone()
        .into_fields();
    let expression = ValueExpression::Capture {
        value: Box::new(ValueExpression::CurrentIdentifier),
        pattern: "(?P<park>[A-Z0-9]{1,4}-[0-9]{4,5})(?:@(?P<location>[A-Z]{2}-[A-Z0-9]{1,3}))?"
            .into(),
        name: "park".into(),
    };
    d.catalog.as_mut().unwrap().lookup_key = expression.clone();
    let d = GroupDefinition::try_from(d)?;
    let compiled = compile_definition(&d)?;
    let bindings = BTreeMap::new();
    let context = ExpressionContext {
        qso: None,
        current_identifier: Some("US-1234@US-WY"),
        bindings: &bindings,
    };
    assert_eq!(
        compiled.evaluate(&expression, &context)?,
        ExpressionValue::Scalar("US-1234".into())
    );
    let bad = ExpressionContext {
        current_identifier: Some("junkUS-1234@US-WYjunk"),
        ..context
    };
    assert_eq!(
        compiled.evaluate(&expression, &bad)?,
        ExpressionValue::Absent
    );
    Ok(())
}

#[test]
fn bindings_are_group_scoped() -> TestResult {
    let defs = support::definitions();
    let d = defs.get(support::id::<GroupId>(1)).unwrap();
    let compiled = compile_definition(d)?;
    let mut q = support::qso(support::id::<QsoId>(1));
    support::edit(&mut q, |q| {
        q.own_station
            .references
            .insert(support::reference(d.id(), Some("US-1234")));

        Ok(())
    });
    support::edit(&mut q, |q| {
        q.own_station.references.insert(support::reference(
            support::id::<GroupId>(999),
            Some("US-9999"),
        ));

        Ok(())
    });
    let bindings = BTreeMap::new();
    let context = ExpressionContext {
        qso: Some(&q),
        current_identifier: None,
        bindings: &bindings,
    };
    assert_eq!(
        compiled.evaluate(&ValueExpression::Identifiers(StationSide::Own), &context)?,
        ExpressionValue::Collection(vec!["US-1234".into()])
    );
    Ok(())
}

#[test]
fn excessive_expression_depth_is_rejected_before_acceptance() {
    let mut fields = support::definitions()
        .iter()
        .next()
        .unwrap()
        .clone()
        .into_fields();
    let mut expression = ValueExpression::Literal("value".into());
    for _ in 0..40 {
        expression = ValueExpression::Capture {
            value: Box::new(expression),
            pattern: "(?P<value>.*)".into(),
            name: "value".into(),
        };
    }
    fields
        .export_mappings
        .ordinary_interchange
        .first_mut()
        .unwrap()
        .value = expression;
    assert!(matches!(GroupDefinition::try_from(fields),
        Err(DefinitionError::InvalidDefinition { reason, .. }) if reason == "expression nesting exceeds 32"));
}
