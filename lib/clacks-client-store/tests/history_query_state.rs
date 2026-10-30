//! Persist the operator's unfinished expression without changing current results.

use crate::support;
use clacks_client_store::{
    read::{
        self,
        contacts::ContactPageQuery,
        query_state::{self, QueryState},
    },
    save,
    store::{ClientStore, StoreSettings},
};
use clacks_model::{query, recognition::RecognitionCatalog};

#[tokio::test]
async fn history_query_state_restores_unfinished_text_and_current_results() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("query.sqlite");
    let store = ClientStore::create(&path, support::id(1), StoreSettings::default()).await?;
    let mut snapshot = read::snapshot(&store).await?;
    assert_eq!(snapshot.history_query_state().await?, QueryState::default());
    snapshot.close().await?;
    let state = QueryState {
        edited: "mode:CW OR".into(),
        applied: "mode:CW".into(),
    };
    query_state::save(&store, &state).await?;
    let qso = support::qso(support::id(10));
    save::save(
        &store,
        support::qso_key(qso.id()),
        None,
        support::contact(qso),
    )
    .await?;
    store.close().await?;

    let store = ClientStore::open(&path, support::id(1), StoreSettings::default()).await?;
    let mut snapshot = read::snapshot(&store).await?;
    let restored = snapshot.history_query_state().await?;
    assert_eq!(restored, state);
    let page = snapshot
        .contact_page(&ContactPageQuery {
            filter: query::parse(&restored.applied, RecognitionCatalog::adif_318()?)?,
            ..Default::default()
        })
        .await?;
    assert_eq!(page.matching_total, 1);
    assert_eq!(page.rows.first().unwrap().working.qso.id(), support::id(10));
    snapshot.close().await?;
    store.close().await?;
    Ok(())
}
