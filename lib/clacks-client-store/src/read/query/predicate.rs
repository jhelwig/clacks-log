//! Bound predicate lowering for the closed operator vocabulary.

use clacks_model::{
    position::{Locator, RationalDegree},
    query::{
        Predicate, Query,
        field::Field,
        value::{TextMatch, Value, literal_pattern},
    },
};
use sqlx::{QueryBuilder, Sqlite};

/// Return the scalar expression for a supported field, never a supplied identifier.
fn scalar(field: Field) -> Option<&'static str> {
    use Field::*;
    Some(match field {
        OwnCall => "q.own_call",
        ContactCall => "q.worked_call",
        Start => "q.start",
        End => "q.end",
        Name => "CASE WHEN f.qso_id IS NOT NULL THEN f.name ELSE p.contacted_operator_name END",
        RecordedName | ContactOperatorName => "p.contacted_operator_name",
        OwnOperatorName => "p.own_operator_name",
        Comment => "p.comment",
        Frequency => "p.frequency",
        Band => "p.band",
        Power => "p.power",
        ReceiveFrequency => {
            "CASE WHEN p.receive_frequency IS NOT NULL OR p.receive_band IS NOT NULL THEN p.receive_frequency ELSE p.frequency END"
        }
        ReceiveBand => {
            "CASE WHEN p.receive_frequency IS NOT NULL OR p.receive_band IS NOT NULL THEN p.receive_band ELSE p.band END"
        }
        RecordedReceiveFrequency => "p.receive_frequency",
        RecordedReceiveBand => "p.receive_band",
        Sent => "p.sent_report",
        Received => "p.received_report",
        Equipment => "e.equipment_description",
        Antenna => "e.antenna_description",
        EquipmentNotes => "e.notes",
        OwnOperatorCall => "p.own_operator_call",
        ContactOperatorCall => "p.contacted_operator_call",
        LotwUploaded => "COALESCE(f.lotw_uploaded,0)",
        LotwConfirmed => "COALESCE(f.lotw_confirmed,0)",
        QrzUploaded => "COALESCE(f.qrz_uploaded,0)",
        QrzConfirmed => "COALESCE(f.qrz_confirmed,0)",
        _ => return None,
    })
}

/// Recursively lower ordinary Boolean semantics with values exclusively bound.
pub(super) fn expression(builder: &mut QueryBuilder<Sqlite>, query: &Query) -> anyhow::Result<()> {
    match query {
        Query::All => {
            builder.push("1");
        }
        Query::Predicate(p) => predicate(builder, p)?,
        Query::Not(q) => {
            builder.push("NOT (");
            expression(builder, q)?;
            builder.push(")");
        }
        Query::And(a, b) | Query::Or(a, b) => {
            builder.push("(");
            expression(builder, a)?;
            builder.push(if matches!(query, Query::And(..)) {
                " AND "
            } else {
                " OR "
            });
            expression(builder, b)?;
            builder.push(")");
        }
    }
    Ok(())
}

/// Match one typed predicate without exposing SQLite's nullable Boolean semantics.
fn predicate(builder: &mut QueryBuilder<Sqlite>, predicate: &Predicate) -> anyhow::Result<()> {
    use Field::*;
    let field = predicate.field;
    if matches!(
        field,
        Tag | OwnActivity | ContactActivity | OwnReference | ContactReference
    ) {
        return collection(builder, predicate);
    }
    if matches!(
        field,
        OwnGrid | ContactGrid | RecordedOwnGrid | RecordedContactGrid
    ) {
        return grid(builder, field, &predicate.value);
    }
    if field == Mode {
        match &predicate.value {
            Value::Present => {
                builder.push("1");
            }
            Value::Mode {
                label, submodes, ..
            } => {
                builder.push("(");
                literal(builder, "q.mode_label", label, TextMatch::Whole);
                for submode in submodes {
                    builder.push(" OR ");
                    literal(builder, "q.mode_label", submode, TextMatch::Whole);
                }
                builder.push(")");
            }
            _ => anyhow::bail!("Mode predicate has an incompatible typed value"),
        }
        return Ok(());
    }
    let column =
        scalar(field).ok_or_else(|| anyhow::anyhow!("Field does not accept a scalar predicate"))?;
    match &predicate.value {
        Value::Present => {
            builder.push(column).push(" IS NOT NULL");
        }
        Value::Text { text, matching } => literal(builder, column, text, *matching),
        Value::Integer(range) => {
            if range.empty {
                builder.push("0");
                return Ok(());
            }
            builder.push("(").push(column).push(" IS NOT NULL");
            if let Some(lower) = range.lower {
                builder
                    .push(" AND ")
                    .push(column)
                    .push(" >= ")
                    .push_bind(lower);
            }
            if let Some(upper) = range.upper {
                builder
                    .push(" AND ")
                    .push(column)
                    .push(" <= ")
                    .push_bind(upper);
            }
            builder.push(")");
        }
        Value::Boolean(value) => {
            builder
                .push("(")
                .push(column)
                .push(" = ")
                .push_bind(*value)
                .push(")");
        }
        _ => anyhow::bail!("Scalar predicate has an incompatible typed value"),
    }
    Ok(())
}

/// Match Unicode literal text, making absent scalar values explicitly false.
fn literal(
    builder: &mut QueryBuilder<Sqlite>,
    column: &'static str,
    text: &str,
    matching: TextMatch,
) {
    builder
        .push("COALESCE((")
        .push(column)
        .push(" REGEXP ")
        .push_bind(literal_pattern(text, matching))
        .push("),0)");
}

/// Existential membership keeps repeated fields independent and output rows unique.
fn collection(builder: &mut QueryBuilder<Sqlite>, predicate: &Predicate) -> anyhow::Result<()> {
    use Field::*;
    let field = predicate.field;
    if field == Tag {
        builder.push("EXISTS (SELECT 1 FROM qso_tag t ");
        if !matches!(predicate.value, Value::Present) {
            builder.push("JOIN local_working w ON w.account = q.account AND w.kind = 'tag' AND w.record_id = t.tag_id JOIN tag_payload g ON g.payload_id = w.payload_id ");
        }
        builder.push("WHERE t.payload_id = q.payload_id");
    } else {
        builder.push("EXISTS (SELECT 1 FROM station_reference t ");
        if !matches!(predicate.value, Value::Present) {
            builder.push("JOIN local_working w ON w.account = q.account AND w.kind = 'group' AND w.record_id = t.group_id JOIN group_payload g ON g.payload_id = w.payload_id ");
        }
        builder.push("WHERE t.station_id = ").push(
            if matches!(field, OwnActivity | OwnReference) {
                "p.own_station"
            } else {
                "p.contacted_station"
            },
        );
    }
    match &predicate.value {
        Value::Present => {
            if matches!(field, OwnReference | ContactReference) {
                builder.push(" AND t.identifier IS NOT NULL");
            }
        }
        Value::Text { text, .. } => {
            builder.push(" AND ");
            literal(builder, "g.name", text, TextMatch::Whole);
        }
        Value::Reference { group, identifier } if field != Tag => {
            builder.push(" AND ");
            literal(builder, "g.name", group, TextMatch::Whole);
            builder.push(" AND ");
            literal(builder, "t.identifier", identifier, TextMatch::Whole);
        }
        _ => anyhow::bail!("Collection predicate has an incompatible typed value"),
    }
    builder.push(")");
    Ok(())
}

/// A selected whole position's scalar columns, all from the same source.
struct PositionColumns {
    /// Native area/point evidence form.
    kind: &'static str,
    /// Complete recorded grid area.
    locator: &'static str,
    /// Explicit point latitude in millionths of a degree.
    latitude: &'static str,
    /// Explicit longitude normalized at the date line like native coordinates.
    longitude: &'static str,
}

/// Select a whole native source without filling its absent fields from another.
fn position(field: Field) -> anyhow::Result<PositionColumns> {
    use Field::*;
    Ok(match field {
        RecordedOwnGrid => PositionColumns {
            kind: "ol.position_kind",
            locator: "ol.locator",
            latitude: "ol.latitude",
            longitude: "CASE ol.longitude WHEN 180000000 THEN -180000000 ELSE ol.longitude END",
        },
        RecordedContactGrid => PositionColumns {
            kind: "cl.position_kind",
            locator: "cl.locator",
            latitude: "cl.latitude",
            longitude: "CASE cl.longitude WHEN 180000000 THEN -180000000 ELSE cl.longitude END",
        },
        OwnGrid => PositionColumns {
            kind: "CASE WHEN f.qso_id IS NOT NULL THEN f.own_kind ELSE ol.position_kind END",
            locator: "CASE WHEN f.qso_id IS NOT NULL THEN f.own_locator ELSE ol.locator END",
            latitude: "CASE WHEN f.qso_id IS NOT NULL THEN f.own_latitude ELSE ol.latitude END",
            longitude: "CASE (CASE WHEN f.qso_id IS NOT NULL THEN f.own_longitude ELSE ol.longitude END) WHEN 180000000 THEN -180000000 ELSE (CASE WHEN f.qso_id IS NOT NULL THEN f.own_longitude ELSE ol.longitude END) END",
        },
        ContactGrid => PositionColumns {
            kind: "CASE WHEN f.qso_id IS NOT NULL THEN f.contact_kind ELSE cl.position_kind END",
            locator: "CASE WHEN f.qso_id IS NOT NULL THEN f.contact_locator ELSE cl.locator END",
            latitude: "CASE WHEN f.qso_id IS NOT NULL THEN f.contact_latitude ELSE cl.latitude END",
            longitude: "CASE (CASE WHEN f.qso_id IS NOT NULL THEN f.contact_longitude ELSE cl.longitude END) WHEN 180000000 THEN -180000000 ELSE (CASE WHEN f.qso_id IS NOT NULL THEN f.contact_longitude ELSE cl.longitude END) END",
        },
        _ => anyhow::bail!("Field does not select a position"),
    })
}

/// Compare a native microdegree coordinate with an exact rational grid edge.
fn edge(
    builder: &mut QueryBuilder<Sqlite>,
    column: &'static str,
    operator: &'static str,
    edge: RationalDegree,
) -> anyhow::Result<()> {
    builder
        .push("(")
        .push(column)
        .push(" * ")
        .push_bind(i64::try_from(edge.denominator)?)
        .push(") ")
        .push(operator)
        .push_bind(
            edge.numerator
                .checked_mul(1_000_000)
                .ok_or_else(|| anyhow::anyhow!("Grid edge exceeds integer comparison range"))?,
        );
    Ok(())
}

/// Match areas by containment and explicit points by exact native grid bounds.
fn grid(builder: &mut QueryBuilder<Sqlite>, field: Field, value: &Value) -> anyhow::Result<()> {
    let columns = position(field)?;
    if matches!(value, Value::Present) {
        builder.push(columns.kind).push(" IS NOT NULL");
        return Ok(());
    }
    let Value::Grid(locator) = value else {
        anyhow::bail!("Grid predicate has an incompatible typed value");
    };
    let bounds = Locator::bounds(locator);
    builder
        .push("COALESCE(((")
        .push(columns.kind)
        .push(" = 'grid' AND ")
        .push(columns.locator)
        .push(" LIKE ")
        .push_bind(format!("{locator}%"))
        .push(") OR (")
        .push(columns.kind)
        .push(" = 'point' AND ");
    edge(builder, columns.longitude, ">=", bounds.west)?;
    builder.push(" AND ");
    edge(builder, columns.longitude, "<", bounds.east)?;
    builder.push(" AND ");
    edge(builder, columns.latitude, ">=", bounds.south)?;
    builder.push(" AND (");
    edge(builder, columns.latitude, "<", bounds.north)?;
    if bounds.north.numerator == 90 * i64::try_from(bounds.north.denominator)? {
        builder
            .push(" OR ")
            .push(columns.latitude)
            .push(" = 90000000");
    }
    builder.push("))),0)");
    Ok(())
}
