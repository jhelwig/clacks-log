//! Bounded export reads through the working history relation.

use super::{compiler, derived};
use crate::{
    logbooks::selection::SelectedView,
    read::{
        ReadSnapshot,
        export::{ExportContact, ExportCounts, ExportInput, ExportScope},
        history::{self, HistoryRow},
        projection::{self, ProjectionNeeds, ReadContext},
    },
};
use clacks_model::{
    effective::effective_contacted_references,
    ids::{AccountId, QsoId},
    query::Query,
};
use sqlx::{Acquire, SqliteConnection};
use std::collections::BTreeSet;

/// Maximum number of working QSOs projected by one export read.
const BATCH_SIZE: u16 = 256;

/// Scope counts returned by the export SQL as signed SQLite integers.
#[derive(sqlx::FromRow)]
struct Counts {
    /// Active contacts among the explicit selection.
    selected: i64,
    /// Active selected contacts outside the applied view or filter.
    hidden: i64,
    /// Active contacts matching the applied view and filter.
    matching: i64,
    /// All active contacts in the account.
    total: i64,
}

impl ReadSnapshot {
    /// Count export scopes using the applied history view and expression.
    #[tracing::instrument(skip_all, name = "client_store.export_counts", level = "debug", fields(account_id = %self.account))]
    pub async fn export_counts(
        &mut self,
        view: SelectedView,
        filter: &Query,
        selected: &BTreeSet<QsoId>,
    ) -> anyhow::Result<ExportCounts> {
        let mut transaction = self.transaction.begin().await?;
        derived::populate(
            &mut transaction,
            self.account,
            view,
            compiler::needs(filter),
        )
        .await?;
        let counts: Counts = compiler::export_counts(self.account, view, filter, selected)?
            .build_query_as()
            .fetch_one(&mut *transaction)
            .await?;
        transaction.rollback().await?;

        Ok(ExportCounts {
            selected: u64::try_from(counts.selected)?,
            hidden: u64::try_from(counts.hidden)?,
            matching: u64::try_from(counts.matching)?,
            all: u64::try_from(counts.total)?,
        })
    }

    /// Capture a complete saved scope and its read-side facts in this snapshot.
    #[tracing::instrument(skip_all, name = "client_store.export_input", level = "debug", fields(account_id = %self.account))]
    pub async fn export_input(&mut self, scope: &ExportScope) -> anyhow::Result<ExportInput> {
        let mut transaction = self.transaction.begin().await?;
        let context = projection::context(&mut transaction, self.account).await?;
        let mut contacts = Vec::new();

        match scope {
            ExportScope::Selected(ids) => {
                let mut ids = ids.iter().copied();
                loop {
                    let batch = ids
                        .by_ref()
                        .take(usize::from(BATCH_SIZE))
                        .collect::<Vec<_>>();
                    if batch.is_empty() {
                        break;
                    }
                    let rows = projection::working_contacts(&mut transaction, self.account, &batch)
                        .await?;
                    contacts.extend(project(&mut transaction, self.account, rows, &context).await?);
                }
            }
            ExportScope::Matching { .. } | ExportScope::All => {
                let matching = if let ExportScope::Matching { view, filter } = scope {
                    derived::populate(
                        &mut transaction,
                        self.account,
                        *view,
                        compiler::needs(filter),
                    )
                    .await?;
                    Some((*view, filter))
                } else {
                    None
                };
                let mut after = None;
                loop {
                    let rows = compiler::export_batch(
                        self.account,
                        matching,
                        after,
                        i64::from(BATCH_SIZE),
                    )?
                    .build_query_as()
                    .fetch_all(&mut *transaction)
                    .await?;
                    let more = rows.len() == usize::from(BATCH_SIZE);
                    let rows =
                        history::working_contacts(&mut transaction, self.account, rows).await?;
                    after = rows.last().map(|row| row.qso.id());
                    contacts.extend(project(&mut transaction, self.account, rows, &context).await?);
                    if !more {
                        break;
                    }
                }
            }
        }

        // Drop temporary query facts while preserving the enclosing read snapshot.
        transaction.rollback().await?;
        tracing::debug!(
            contact_count = contacts.len(),
            "Captured working contacts for export"
        );
        Ok(ExportInput {
            contacts,
            definitions: context.definitions,
            profiles: context.profiles,
        })
    }
}

async fn project(
    connection: &mut SqliteConnection,
    account: AccountId,
    rows: Vec<HistoryRow>,
    context: &ReadContext,
) -> anyhow::Result<Vec<ExportContact>> {
    let projected = projection::project(
        connection,
        account,
        &rows,
        context,
        ProjectionNeeds {
            station: true,
            services: true,
        },
    )
    .await?;

    Ok(rows
        .into_iter()
        .zip(projected)
        .map(|(row, projection)| ExportContact {
            contacted_references: effective_contacted_references(
                row.qso.contacted_station(),
                projection.facts.contact_location.as_ref(),
                &context.definitions,
            ),
            qso: row.qso,
            facts: projection.facts,
            confirmation_dates: projection.confirmation_dates,
        })
        .collect())
}
