//! Bound relational history queries on one ordinary consistent read snapshot.

mod compiler;
mod derived;
mod predicate;

use super::{
    ReadSnapshot,
    contacts::{ContactPage, ContactPageQuery, ContactRow},
    history,
    projection::{self, ProjectionNeeds},
};
use crate::rows::contact::WorkingContactRow;
use sqlx::Acquire;
use std::collections::BTreeMap;

/// Raw aggregate values returned by the relational query before checked conversion.
#[derive(sqlx::FromRow)]
struct Counts {
    /// Active contacts matching the applied expression within the view.
    matching_total: i64,
    /// Selected active identities within the view.
    selected_total: i64,
    /// Selected identities excluded by the expression, independent of paging.
    selected_outside_filter: i64,
}

impl ReadSnapshot {
    /// Read a bounded working-contact page and complete selection counts atomically.
    ///
    /// Expression errors are rejected before this boundary. No provider requests or
    /// contact changes occur; native-derived facts live only for this operation.
    #[tracing::instrument(skip_all, name = "client_store.contact_query", level = "debug", fields(account_id = %self.account, limit = query.limit, offset = query.offset))]
    pub async fn contact_page(&mut self, query: &ContactPageQuery) -> anyhow::Result<ContactPage> {
        anyhow::ensure!(
            (1..=1000).contains(&query.limit),
            "Contact page size must be between 1 and 1000"
        );
        let mut transaction = self.transaction.begin().await?;
        derived::populate(&mut transaction, self.account, query).await?;
        let counts: Counts = compiler::counts(self.account, query)?
            .build_query_as()
            .fetch_one(&mut *transaction)
            .await?;
        let mut rows: Vec<WorkingContactRow> = compiler::page(self.account, query)?
            .build_query_as()
            .fetch_all(&mut *transaction)
            .await?;
        let has_more = rows.len() > usize::try_from(query.limit)?;
        rows.truncate(usize::try_from(query.limit)?);
        let rows = history::working_contacts(&mut transaction, self.account, rows).await?;
        let ids: Vec<_> = rows.iter().map(|r| r.qso.id()).collect();
        let mut projected = BTreeMap::new();
        for ids in ids.chunks(256) {
            for facts in projection::load(
                &mut transaction,
                self.account,
                ids,
                ProjectionNeeds {
                    station: true,
                    services: true,
                },
            )
            .await?
            {
                projected.insert(facts.id, facts.facts);
            }
        }
        let rows = rows
            .into_iter()
            .map(|working| {
                let facts = projected.remove(&working.qso.id()).ok_or_else(|| {
                    anyhow::anyhow!("Native projection omitted a selected working contact")
                })?;
                Ok(ContactRow { working, facts })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        transaction.rollback().await?;
        Ok(ContactPage {
            rows,
            has_more,
            matching_total: u64::try_from(counts.matching_total)?,
            selected_total: u64::try_from(counts.selected_total)?,
            selected_outside_filter: u64::try_from(counts.selected_outside_filter)?,
        })
    }
}
