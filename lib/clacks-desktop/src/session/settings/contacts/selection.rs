//! Enumerate applied history matches through the existing consistent read API.

use clacks_client_store::{
    read::{self, contacts::ContactPageQuery},
    store::ClientStore,
};
use clacks_model::ids::QsoId;
use std::collections::BTreeSet;

/// Read every live match without turning the current viewport into a scope boundary.
pub(super) async fn matching_ids(
    store: &ClientStore,
    mut query: ContactPageQuery,
) -> anyhow::Result<BTreeSet<QsoId>> {
    query.offset = 0;
    query.limit = 100;
    query.order = Default::default();
    query.selected_ids.clear();
    query.show_selected = false;
    let mut snapshot = read::snapshot(store).await?;
    let mut ids = BTreeSet::new();
    loop {
        let page = snapshot.contact_page(&query).await?;
        ids.extend(page.rows.into_iter().map(|row| row.working.qso.id()));
        if !page.has_more {
            break;
        }
        query.offset = query
            .offset
            .checked_add(query.limit)
            .ok_or_else(|| anyhow::anyhow!("Contact selection exceeds supported query range"))?;
    }
    snapshot.close().await?;
    Ok(ids)
}
