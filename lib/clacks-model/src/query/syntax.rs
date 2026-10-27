//! Located expression grammar with case-insensitive Boolean operators.

use super::{
    Predicate, Query,
    diagnostic::QueryDiagnostic,
    field::Field,
    value::{self, TextMatch, Value},
};
use crate::recognition::RecognitionCatalog;
use std::ops::Range;
use winnow::{
    Parser,
    ascii::Caseless,
    combinator::{
        Infix, Prefix, alt, cut_err, delimited, eof, expression, not, peek, preceded, repeat,
        terminated,
    },
    error::{ContextError, ErrMode, FromExternalError},
    stream::LocatingSlice,
    token::{any, one_of, take_while},
};

/// Original query text with byte positions for diagnostics.
type Input<'a> = LocatingSlice<&'a str>;

/// Grammar failures retain their explanatory context and typed-value cause.
type Result<T> = winnow::ModalResult<T, ContextError<&'static str>>;

/// Parse the complete expression, retaining actionable syntax or value diagnostics.
pub(super) fn parse(
    text: &str,
    catalog: &RecognitionCatalog,
) -> std::result::Result<Query, QueryDiagnostic> {
    terminated(
        preceded(
            whitespace,
            alt((eof.value(Query::All), |input: &mut Input<'_>| {
                boolean(input, catalog)
            })),
        ),
        preceded(whitespace, eof).context("Unexpected token; check parentheses and operators"),
    )
    .parse(Input::new(text))
    .map_err(|error| {
        if let Some(diagnostic) = error
            .inner()
            .cause()
            .and_then(|cause| cause.downcast_ref::<QueryDiagnostic>())
        {
            return diagnostic.clone();
        }
        let start = error.offset();
        let end = text
            .get(start..)
            .and_then(|rest| rest.chars().next())
            .map_or(start, |c| start + c.len_utf8());
        let message = error
            .inner()
            .context()
            .next()
            .copied()
            .unwrap_or("Supply a field, callsign or grouped expression");
        QueryDiagnostic::new(start..end, message)
    })
}

/// Consume inter-expression whitespace, including non-ASCII whitespace.
fn whitespace(input: &mut Input<'_>) -> Result<()> {
    take_while(0.., char::is_whitespace)
        .void()
        .parse_next(input)
}

/// Match only a complete, unquoted keyword, leaving longer literal terms intact.
fn keyword<'a>(
    word: &'static str,
) -> impl Parser<Input<'a>, &'a str, ErrMode<ContextError<&'static str>>> {
    terminated(
        Caseless(word),
        not(one_of(|c: char| {
            !c.is_whitespace() && !matches!(c, '(' | ')')
        })),
    )
}

/// Recognize a predicate token without consuming Boolean operators or leading negation.
fn predicate_token<'a>(input: &mut Input<'a>) -> Result<&'a str> {
    preceded(
        not(alt((keyword("AND"), keyword("OR"), keyword("NOT"), "-"))),
        atom,
    )
    .parse_next(input)
}

/// Apply the language's NOT, AND, OR binding powers through winnow's expression parser.
fn boolean(input: &mut Input<'_>, catalog: &RecognitionCatalog) -> Result<Query> {
    expression(preceded(
        whitespace,
        alt((
            |input: &mut Input<'_>| group(input, catalog),
            |input: &mut Input<'_>| predicate(input, catalog),
        )),
    ))
    .prefix(
        preceded(whitespace, alt((keyword("NOT").void(), '-'.void())))
            .value(Prefix(5, |_, value| Ok(Query::Not(Box::new(value))))),
    )
    .infix(preceded(
        whitespace,
        alt((
            keyword("OR").value(Infix::Left(1, |_, left, right| {
                Ok(Query::Or(Box::new(left), Box::new(right)))
            })),
            keyword("AND").value(Infix::Left(3, |_, left, right| {
                Ok(Query::And(Box::new(left), Box::new(right)))
            })),
            // An adjacent operand is conjunction without consuming the operand itself.
            peek(alt((
                '('.void(),
                '-'.void(),
                keyword("NOT").void(),
                predicate_token.void(),
            )))
            .value(Infix::Left(3, |_, left, right| {
                Ok(Query::And(Box::new(left), Box::new(right)))
            })),
        )),
    ))
    .context("Supply an expression after the operator")
    .parse_next(input)
}

/// Require a complete grouped expression once its opening parenthesis is consumed.
fn group(input: &mut Input<'_>, catalog: &RecognitionCatalog) -> Result<Query> {
    delimited(
        '(',
        cut_err(|input: &mut Input<'_>| boolean(input, catalog)),
        cut_err(preceded(whitespace, ')')).context("Close this parenthesis"),
    )
    .parse_next(input)
}

/// Consume a quoted segment without interpreting its literal contents.
fn quoted<'a>(input: &mut Input<'a>) -> Result<&'a str> {
    (
        '"',
        cut_err((
            repeat::<_, _, (), _, _>(
                0..,
                alt((
                    ('\\', any).void(),
                    take_while(1.., |c| c != '"' && c != '\\').void(),
                )),
            ),
            '"',
        ))
        .context("Complete the quoted or escaped value"),
    )
        .take()
        .parse_next(input)
}

/// Consume a field token while retaining escaped/quoted delimiters.
fn atom<'a>(input: &mut Input<'a>) -> Result<&'a str> {
    repeat::<_, _, (), _, _>(
        1..,
        alt((
            quoted.void(),
            (
                '\\',
                cut_err(any).context("Complete the quoted or escaped value"),
            )
                .void(),
            take_while(1.., |c: char| {
                !c.is_whitespace() && !matches!(c, '(' | ')' | '"' | '\\')
            })
            .void(),
        )),
    )
    .take()
    .parse_next(input)
}

/// Bind a located syntactic operand to a typed field restriction.
fn predicate(input: &mut Input<'_>, catalog: &RecognitionCatalog) -> Result<Query> {
    let (raw, span) = predicate_token.with_span().parse_next(input)?;
    bind(raw, span, catalog)
        .map(Query::Predicate)
        .map_err(|error| ErrMode::Cut(ContextError::from_external_error(input, error)))
}

/// Interpret presence, named fields and unqualified callsign literals.
fn bind(
    raw: &str,
    span: Range<usize>,
    catalog: &RecognitionCatalog,
) -> std::result::Result<Predicate, QueryDiagnostic> {
    if let Some((operator, name)) = raw.split_once(':')
        && operator.eq_ignore_ascii_case("has")
    {
        let field = Field::named(name)
            .ok_or_else(|| QueryDiagnostic::new(span, "Use a supported field after has:"))?;
        return Ok(Predicate {
            field,
            value: Value::Present,
        });
    }
    // A colon in a quoted bare callsign is literal, not a named-field separator.
    let named = if raw.starts_with('"') {
        None
    } else {
        raw.split_once(':')
    };
    let (field, value) = if let Some((name, raw)) = named {
        let field = Field::named(name).ok_or_else(|| {
            QueryDiagnostic::new(
                span.clone(),
                "Unknown field; use a field from the suggestions",
            )
        })?;
        (field, value::parse(field, raw, span, catalog)?)
    } else {
        (
            Field::ContactCall,
            Value::Text {
                text: value::decode(raw).map_err(|message| QueryDiagnostic::new(span, message))?,
                matching: TextMatch::Prefix,
            },
        )
    };
    Ok(Predicate { field, value })
}
