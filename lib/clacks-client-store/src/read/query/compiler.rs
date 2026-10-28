//! Relational query structure with closed expressions and bound selection inputs.

use super::predicate;
use crate::{
    logbooks::selection::SelectedView,
    read::contacts::{ContactColumn, ContactPageQuery, SortDirection},
};
use clacks_model::{ids::AccountId, query::Query};
use sqlx::{QueryBuilder, Sqlite};

/// Start a runtime query against the consistent local-working relation.
pub(super) fn start(select: &'static str) -> QueryBuilder<Sqlite> {
    let mut builder = QueryBuilder::new(select);
    builder.push(include_str!("../../../sql/read/query/from.sql"));
    builder
}

/// Apply the account and selected logbook without changing the expression.
pub(super) fn view(builder: &mut QueryBuilder<Sqlite>, account: AccountId, view: SelectedView) {
    builder
        .push(" WHERE q.account = ")
        .push_bind(account.to_string());
    match view {
        SelectedView::AllContacts => {}
        SelectedView::Untagged => {
            builder.push(
                " AND NOT EXISTS (SELECT 1 FROM qso_tag t WHERE t.payload_id = q.payload_id)",
            );
        }
        SelectedView::Logbook(tag) => {
            builder.push(" AND EXISTS (SELECT 1 FROM qso_tag t WHERE t.payload_id = q.payload_id AND t.tag_id = ").push_bind(tag.to_string()).push(")");
        }
    }
}

/// Produce the same-snapshot counts, with hidden meaning outside the applied filter.
pub(super) fn counts(
    account: AccountId,
    query: &ContactPageQuery,
) -> anyhow::Result<QueryBuilder<Sqlite>> {
    let mut builder = QueryBuilder::new("WITH matches AS (SELECT q.record_id, ");
    predicate::expression(&mut builder, &query.filter)?;
    builder
        .push(" AS matches_filter, q.record_id IN (SELECT value FROM json_each(")
        .push_bind(serde_json::to_string(&query.selected_ids)?)
        .push(")) AS selected ")
        .push(include_str!("../../../sql/read/query/from.sql"));
    view(&mut builder, account, query.view);
    builder.push(") SELECT COALESCE(SUM(matches_filter),0) AS matching_total, COALESCE(SUM(selected),0) AS selected_total, COALESCE(SUM(selected AND NOT matches_filter),0) AS selected_outside_filter FROM matches");
    Ok(builder)
}

/// Select a stable bounded page, including selected-only inspection when requested.
pub(super) fn page(
    account: AccountId,
    query: &ContactPageQuery,
) -> anyhow::Result<QueryBuilder<Sqlite>> {
    let mut builder = start("SELECT q.record_id, q.payload_id, q.pending ");
    view(&mut builder, account, query.view);
    builder.push(" AND (");
    if query.show_selected {
        builder
            .push("q.record_id IN (SELECT value FROM json_each(")
            .push_bind(serde_json::to_string(&query.selected_ids)?)
            .push("))");
    } else {
        predicate::expression(&mut builder, &query.filter)?;
    }
    builder.push(") ORDER BY ");
    let column = order(query.order.column);
    builder
        .push(column)
        .push(" IS NULL, ")
        .push(column)
        .push(if query.order.direction == SortDirection::Descending {
            " DESC"
        } else {
            " ASC"
        })
        .push(", q.record_id ASC LIMIT ")
        .push_bind(i64::from(query.limit) + 1)
        .push(" OFFSET ")
        .push_bind(i64::from(query.offset));
    Ok(builder)
}

/// Select only the requested query families; presence of derived Booleans is unconditional.
pub(super) fn needs(query: &Query) -> super::derived::Needs {
    use clacks_model::query::{field::Field, value::Value};
    match query {
        Query::All => Default::default(),
        Query::Not(q) => needs(q),
        Query::And(a, b) | Query::Or(a, b) => needs(a).join(needs(b)),
        Query::Predicate(p) => super::derived::Needs {
            station: matches!(p.field, Field::Name | Field::OwnGrid | Field::ContactGrid),
            services: matches!(
                p.field,
                Field::LotwUploaded
                    | Field::LotwConfirmed
                    | Field::QrzUploaded
                    | Field::QrzConfirmed
            ) && !matches!(p.value, Value::Present),
            all_positions: false,
        },
    }
}

/// Closed table-column ordering; references use stable native association order.
fn order(column: ContactColumn) -> &'static str {
    use ContactColumn::*;
    match column {
        Callsign => "q.worked_call",
        Start => "q.start",
        End => "q.end",
        OwnCall => "q.own_call",
        Band => "q.band",
        Mode => "q.mode_label",
        Frequency => "q.frequency",
        Sent => "p.sent_report",
        Received => "p.received_report",
        Grid => "f.contact_grid",
        OwnGrid => "f.own_grid",
        Status => "CASE WHEN review.record_id IS NOT NULL THEN 2 WHEN q.pending THEN 1 ELSE 0 END",
        OwnReferences => {
            "(SELECT group_concat(value, ', ') FROM (SELECT COALESCE(g.name,t.group_id) || CASE WHEN t.identifier IS NULL THEN '' ELSE '/' || t.identifier END AS value FROM station_reference t LEFT JOIN local_working w ON w.account = q.account AND w.kind = 'group' AND w.record_id = t.group_id LEFT JOIN group_payload g ON g.payload_id = w.payload_id WHERE t.station_id = p.own_station ORDER BY t.group_id,t.identifier))"
        }
        ContactReferences => {
            "(SELECT group_concat(value, ', ') FROM (SELECT COALESCE(g.name,t.group_id) || CASE WHEN t.identifier IS NULL THEN '' ELSE '/' || t.identifier END AS value FROM station_reference t LEFT JOIN local_working w ON w.account = q.account AND w.kind = 'group' AND w.record_id = t.group_id LEFT JOIN group_payload g ON g.payload_id = w.payload_id WHERE t.station_id = p.contacted_station ORDER BY t.group_id,t.identifier))"
        }
    }
}
