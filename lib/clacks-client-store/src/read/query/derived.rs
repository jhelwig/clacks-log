//! Operation-local native facts, composed with SQLx's ordinary transaction lifetime.

use super::compiler;
use crate::read::{
    contacts::{ContactColumn, ContactPageQuery},
    projection::{self, ProjectionNeeds},
};
use clacks_model::ids::AccountId;
use sqlx::{Row, SqliteConnection};

/// Native families needed by this operation, not an account-wide cache configuration.
#[derive(Clone, Copy, Default)]
pub(super) struct Needs {
    /// Effective names and whole positions are used.
    pub(super) station: bool,
    /// Service-positive flags are used.
    pub(super) services: bool,
    /// Grid sorting needs every recorded point's native display locator.
    pub(super) all_positions: bool,
}

impl Needs {
    /// Combine independent expression branches without pushing restrictions through OR/NOT.
    pub(super) fn join(self, other: Self) -> Self {
        Self {
            station: self.station || other.station,
            services: self.services || other.services,
            all_positions: self.all_positions || other.all_positions,
        }
    }
}

/// Create this operation's temporary facts and fill only rows needing native evaluation.
pub(super) async fn populate(
    connection: &mut SqliteConnection,
    account: AccountId,
    query: &ContactPageQuery,
) -> anyhow::Result<()> {
    sqlx::query(include_str!("../../../sql/read/query/facts.sql"))
        .execute(&mut *connection)
        .await?;
    let mut needs = compiler::needs(&query.filter);
    if matches!(
        query.order.column,
        ContactColumn::Grid | ContactColumn::OwnGrid
    ) {
        needs.station = true;
        needs.all_positions = true;
    }
    if !needs.station && !needs.services {
        return Ok(());
    }
    let mut after: Option<String> = None;
    loop {
        let mut selection = compiler::start("SELECT q.record_id ");
        compiler::view(&mut selection, account, query.view);
        if let Some(after) = &after {
            selection
                .push(" AND q.record_id > ")
                .push_bind(after.clone());
        }
        if !needs.all_positions {
            // Without any retained provider evidence, ordinary values are exactly
            // recorded values and all service-positive flags are false.
            selection.push(" AND q.record_id IN (");
            if needs.services {
                selection.push("SELECT a.qso_id FROM local_working sw JOIN service_assertion a ON a.payload_id = sw.payload_id WHERE sw.kind = 'imported' AND sw.account = ").push_bind(account.to_string());
                selection.push(" UNION SELECT a.qso_id FROM local_working sw JOIN service_attempt a ON a.payload_id = sw.payload_id WHERE sw.kind = 'attempt' AND sw.account = ").push_bind(account.to_string());
                selection.push(" UNION ");
            }
            selection.push("SELECT a.qso_id FROM local_working sw JOIN service_association a ON a.payload_id = sw.payload_id WHERE sw.kind = 'association' AND sw.account = ").push_bind(account.to_string()).push(")");
        }
        selection.push(" ORDER BY q.record_id LIMIT 256");
        let rows = selection.build().fetch_all(&mut *connection).await?;
        let ids = rows
            .iter()
            .map(|r| {
                r.try_get::<String, _>("record_id")?
                    .parse()
                    .map_err(anyhow::Error::from)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        if ids.is_empty() {
            break;
        }
        after = ids.last().map(ToString::to_string);
        let facts = projection::load(
            connection,
            account,
            &ids,
            ProjectionNeeds {
                station: needs.station,
                services: needs.services,
            },
        )
        .await?;
        for value in facts {
            let own = value
                .facts
                .own_location
                .as_ref()
                .and_then(|s| s.value.as_ref())
                .and_then(|l| l.position().as_ref());
            let contact = value
                .facts
                .contact_location
                .as_ref()
                .and_then(|s| s.value.as_ref())
                .and_then(|l| l.position().as_ref());
            let own_grid = own.map(|p| p.display_locator().to_string());
            let contact_grid = contact.map(|p| p.display_locator().to_string());
            let own = crate::rows::contact::PositionFields::from(own);

            let contact = crate::rows::contact::PositionFields::from(contact);

            sqlx::query(include_str!("../../../sql/read/query/facts_insert.sql"))
                .bind(value.id.to_string())
                .bind(value.facts.name.map(|s| s.value))
                .bind(own.kind.as_deref())
                .bind(own.locator.as_deref())
                .bind(own.latitude)
                .bind(own.longitude)
                .bind(own_grid)
                .bind(contact.kind.as_deref())
                .bind(contact.locator.as_deref())
                .bind(contact.latitude)
                .bind(contact.longitude)
                .bind(contact_grid)
                .bind(value.facts.lotw.uploaded)
                .bind(value.facts.lotw.confirmed)
                .bind(value.facts.qrz.uploaded)
                .bind(value.facts.qrz.confirmed)
                .execute(&mut *connection)
                .await?;
            if !value.facts.diagnostics.is_empty() {
                tracing::warn!(event = "contact_query.evidence_incomplete", qso_id = %value.id, diagnostics = ?value.facts.diagnostics);
            }
        }
    }
    Ok(())
}
