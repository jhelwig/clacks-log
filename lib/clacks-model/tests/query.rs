use clacks_model::{
    query::{
        self, Query,
        field::Field,
        value::{TextMatch, Value},
    },
    recognition::RecognitionCatalog,
};

#[test]
fn precedence_and_literal_tokens() -> anyhow::Result<()> {
    let catalog = RecognitionCatalog::adif_318()?;
    let parse = |text| query::parse(text, catalog);
    assert_eq!(parse("has:name")?, parse("HAS:NAME")?);
    assert_eq!(
        parse("comment:activation comment:rain -name:alice")?,
        parse("comment:activation AND comment:rain AND NOT name:alice")?
    );
    assert_ne!(
        parse("comment:a OR comment:b comment:c")?,
        parse("(comment:a OR comment:b) comment:c")?
    );
    assert_eq!(
        parse("comment:\">5\"")?,
        Query::Predicate(query::Predicate {
            field: Field::Comment,
            value: Value::Text {
                text: ">5".into(),
                matching: TextMatch::Contains
            }
        })
    );
    assert_eq!(
        parse("comment:=\">5\"")?,
        Query::Predicate(query::Predicate {
            field: Field::Comment,
            value: Value::Text {
                text: ">5".into(),
                matching: TextMatch::Whole
            }
        })
    );
    for text in [
        "no_such_field:x",
        "mode:SSB OR",
        "name:>alice",
        "(name:alice",
        "comment:\"unfinished",
    ] {
        assert!(parse(text).is_err(), "{text}");
    }
    let error = parse("name:Élodie nope:x").unwrap_err();
    assert_eq!("name:Élodie nope:x".get(error.span), Some("nope:x"));
    Ok(())
}

#[test]
fn boolean_keywords_are_case_insensitive_only_as_whole_unquoted_tokens() -> anyhow::Result<()> {
    let catalog = RecognitionCatalog::adif_318()?;
    let parse = |text| query::parse(text, catalog);
    let comment = |text: &str| {
        Query::Predicate(query::Predicate {
            field: Field::Comment,
            value: Value::Text {
                text: text.into(),
                matching: TextMatch::Contains,
            },
        })
    };
    let expected = Query::Or(
        Box::new(comment("activation")),
        Box::new(Query::And(
            Box::new(comment("rain")),
            Box::new(Query::Not(Box::new(Query::Predicate(query::Predicate {
                field: Field::Name,
                value: Value::Present,
            })))),
        )),
    );
    for text in [
        "comment:activation or comment:rain and not has:name",
        "comment:activation oR comment:rain aNd nOt has:name",
        "comment:activation OR (comment:rain -has:name)",
    ] {
        assert_eq!(parse(text)?, expected, "{text}");
    }

    for (text, literal) in [
        ("orange", "orange"),
        ("notebook", "notebook"),
        ("android", "android"),
        ("\"or\"", "or"),
        (r"\o\r", "or"),
    ] {
        assert_eq!(
            parse(text)?,
            Query::Predicate(query::Predicate {
                field: Field::ContactCall,
                value: Value::Text {
                    text: literal.into(),
                    matching: TextMatch::Prefix,
                },
            }),
            "{text}"
        );
    }
    assert_eq!(parse("comment:and")?, comment("and"));
    assert_eq!(parse("comment:\"NoT\"")?, comment("NoT"));
    for text in [
        "and comment:x",
        "comment:x oR",
        "(comment:x AnD)",
        "NoT",
        "comment:x OR AND comment:y",
        "comment:x)",
    ] {
        assert!(parse(text).is_err(), "{text}");
    }
    Ok(())
}

#[test]
fn typed_fields_dates_units_and_diagnostics() -> anyhow::Result<()> {
    let catalog = RecognitionCatalog::adif_318()?;
    let parse = |text| query::parse(text, catalog);
    assert_eq!(parse("frequency:14000kHz")?, parse("frequency:14MHz")?);
    assert_eq!(parse("power:.5W")?, parse("power:500mW")?);
    assert_eq!(parse("power:-.5W")?, parse("power:-500mW")?);
    assert!(parse("power:1.2.3W").is_err());
    let Query::Predicate(p) = parse("frequency:>14.0000005MHz")? else {
        panic!("predicate expected")
    };
    let Value::Integer(range) = p.value else {
        panic!("integer range expected")
    };
    assert_eq!(range.lower, Some(14_000_001));
    assert_eq!(range.upper, None);
    assert_eq!(
        parse("qso_start:>2026-10-27")?,
        parse("qso_start:>=2026-10-28T00:00:00Z")?
    );
    assert_eq!(
        parse("qso_start:2026-10-27T18:00:00Z")?,
        parse("qso_start:2026-10-27T11:00:00-07:00")?
    );
    assert!(parse("qso_start:2026-10-27T18:00:00.1Z").is_err());
    assert!(parse("qso_start:2026-10-27T18:00:00.0Z").is_err());
    assert!(parse("qso_start:2026-10-27T18:00:00").is_err());
    assert!(parse("qso_start:2026-1-27T18:00:00Z").is_err());
    assert!(parse("qso_start:2026-10-27T18:00Z").is_err());
    assert!(parse("qso_start:2026-1-2").is_err());
    assert!(parse(r#"qso_start:"2026- 1-01""#).is_err());
    assert!(parse("qso_start:2026-10-27T18:00:00+24:00").is_err());
    assert!(parse("qso_start:2016-12-31T23:59:60Z").is_err());
    assert_eq!(
        parse("qso_start:2026-10-27t18:00:00z")?,
        parse("qso_start:2026-10-27T18:00:00Z")?
    );
    assert_eq!(
        parse("qso_start:>=9999-12-31T23:59:59Z")?,
        parse("qso_start:>9999-12-31T23:59:58Z")?
    );
    assert!(parse("qso_time:2026-10-27").is_err());
    assert_eq!(parse("mode:SSB")?, parse("MODE:=SSB")?);
    let Query::Predicate(p) = parse(r#"contact.reference:Group\/Name/ID\/1"#)? else {
        panic!("predicate expected")
    };
    assert_eq!(
        p.value,
        Value::Reference {
            group: "Group/Name".into(),
            identifier: "ID/1".into()
        }
    );
    assert!(parse("power:>999999999999999999999999999W").is_ok());
    assert!(parse("comment:activation OR").is_err());
    Ok(())
}

#[test]
fn completion_replaces_the_current_value_without_damaging_neighbor_tokens() {
    use query::completion::{self, CompletionContext};
    let text = "comment:Élodie mode:SSB tag:Main";
    let caret = text.find("SSB").unwrap() + 2;
    let site = completion::site(text, caret);
    assert_eq!(site.context, CompletionContext::Value(Field::Mode));
    assert_eq!(text.get(site.replacement), Some("SSB"));
    assert_eq!(site.prefix, "SS");
    let quoted = "comment:\"rain (portable)\"";
    let site = completion::site(quoted, quoted.len() - 2);
    assert_eq!(site.context, CompletionContext::Value(Field::Comment));
    assert_eq!(quoted.get(site.replacement), Some("\"rain (portable)\""));
}

#[test]
fn completion_keeps_date_hyphens_and_named_presence_context() {
    use query::completion::{self, CompletionContext};
    let text = "-qso_start:>=2026-10-27";
    assert_eq!(
        completion::site(text, text.len()).context,
        CompletionContext::Value(Field::Start)
    );
    assert_eq!(
        completion::site("has:recorded.na", 15).context,
        CompletionContext::Presence
    );
}

#[test]
fn reference_delimiters_respect_component_and_whole_value_quotes() -> anyhow::Result<()> {
    let catalog = RecognitionCatalog::adif_318()?;
    for expression in [
        r#"contact.reference:"Group/Name"/"ID/1""#,
        r#"contact.reference:Group\/Name/ID\/1"#,
        r#"contact.reference:"Group\/Name/ID\/1""#,
    ] {
        let Query::Predicate(p) = query::parse(expression, catalog)? else {
            panic!("reference predicate")
        };
        assert_eq!(
            p.value,
            Value::Reference {
                group: "Group/Name".into(),
                identifier: "ID/1".into()
            }
        );
    }
    Ok(())
}
