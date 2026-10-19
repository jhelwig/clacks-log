mod support;

use clacks_model::export::rules::{
    ExpressionContext, ExpressionValue, ValueExpression, compile_definition,
};
use clacks_model::group::DefinitionError;
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
        let mut d = base.clone();
        d.catalog.as_mut().unwrap().lookup_key = expr;
        assert!(matches!(
            compile_definition(&d),
            Err(DefinitionError::InvalidDefinition { .. })
        ));
    }
    assert!(serde_json::from_str::<ValueExpression>(r#"{"QsoField":"Nonexistent"}"#).is_err());
    let mut d = base.clone();
    d.export_mappings.ordinary_interchange[0].value =
        ValueExpression::Identifiers(StationSide::Own);
    assert!(compile_definition(&d).is_err());
    assert!(support::qso(support::id::<QsoId>(1)).validate().is_ok());
}

#[test]
fn full_match_and_named_capture() -> TestResult {
    let defs = support::definitions();
    let mut d = defs.get(support::id::<GroupId>(1)).unwrap().clone();
    let expression = ValueExpression::Capture {
        value: Box::new(ValueExpression::CurrentIdentifier),
        pattern: "(?P<park>[A-Z0-9]{1,4}-[0-9]{4,5})(?:@(?P<location>[A-Z]{2}-[A-Z0-9]{1,3}))?"
            .into(),
        name: "park".into(),
    };
    d.catalog.as_mut().unwrap().lookup_key = expression.clone();
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
    q.own_station
        .references
        .insert(support::reference(d.id, Some("US-1234")));
    q.own_station.references.insert(support::reference(
        support::id::<GroupId>(999),
        Some("US-9999"),
    ));
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
fn compiled_state_is_not_serialized() -> TestResult {
    let defs = support::definitions();
    let d = defs.get(support::id::<GroupId>(1)).unwrap();
    let before = serde_json::to_value(d)?;
    let compiled = compile_definition(d)?;
    assert_eq!(serde_json::to_value(compiled.definition())?, before);
    Ok(())
}

#[test]
fn excessive_expression_depth_is_rejected_before_recursive_clone() {
    const CHILD_FLAG: &str = "CLACKS_TEST_DEEP_EXPRESSION";
    if std::env::var_os(CHILD_FLAG).is_some() {
        let mut definition = support::definitions().iter().next().unwrap().clone();
        let mut expression = ValueExpression::Literal("value".into());
        for _ in 0..100_000 {
            expression = ValueExpression::Capture {
                value: Box::new(expression),
                pattern: "(?P<value>.*)".into(),
                name: "value".into(),
            };
        }
        definition
            .export_mappings
            .ordinary_interchange
            .first_mut()
            .unwrap()
            .value = expression;
        // The subprocess owns this intentionally leaked input: its recursive drop
        // must not obscure whether compilation itself rejects the excessive depth.
        let definition = Box::leak(Box::new(definition));
        assert!(matches!(
            compile_definition(definition),
            Err(DefinitionError::InvalidDefinition { reason, .. })
                if reason == "expression nesting exceeds 32"
        ));
        return;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "excessive_expression_depth_is_rejected_before_recursive_clone",
        ])
        .env(CHILD_FLAG, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "compiler subprocess failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
