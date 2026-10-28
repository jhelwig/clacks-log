//! Worked Before follows the live call prefix and selected view, not history filters.

use crate::support;
use clacks_client_store::{
    logbooks::selection::SelectedView,
    read::{self, worked_before::WorkedBeforeQuery},
    save,
    store::{ClientStore, StoreSettings},
};
use clacks_model::scalar::RecordedCallsign;

#[tokio::test]
async fn worked_before_follows_prefix_scope_and_working_state() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("worked.sqlite"),
        support::id(1),
        StoreSettings::default(),
    )
    .await?;
    for (n, call) in [(10, "W7HEX"), (11, "W7HEX/P"), (12, "OK2CQR")] {
        let mut qso = support::qso(support::id(n));
        qso.contacted_station.callsign = RecordedCallsign::new(call)?;
        qso.own_station.callsign = RecordedCallsign::new(if n == 10 { "W1AW" } else { "W2AW" })?;
        if n == 10 {
            qso.tags.insert(support::id(20));
        }
        save::save(
            &store,
            support::qso_key(qso.id),
            None,
            support::contact(qso),
        )
        .await?;
    }
    let mut snapshot = read::snapshot(&store).await?;
    let query = WorkedBeforeQuery {
        call_prefix: " w7hex ".into(),
        ..Default::default()
    };
    let page = snapshot.worked_before(&query).await?;
    assert_eq!(page.matching_total, 2);
    assert_eq!(
        snapshot
            .worked_before(&WorkedBeforeQuery {
                call_prefix: "W7HEX/P".into(),
                ..query.clone()
            })
            .await?
            .matching_total,
        1
    );
    assert_eq!(
        snapshot
            .worked_before(&WorkedBeforeQuery {
                view: SelectedView::Logbook(support::id(20)),
                ..query.clone()
            })
            .await?
            .matching_total,
        1
    );
    assert!(
        snapshot
            .worked_before(&WorkedBeforeQuery::default())
            .await?
            .rows
            .is_empty()
    );
    snapshot.close().await?;

    let key = support::qso_key(support::id(10));
    let original = save::load(&store, key).await?.unwrap();
    let mut qso = support::qso(support::id(10));
    qso.contacted_station.callsign = RecordedCallsign::new("W7HEX/QRP")?;
    save::save(&store, key, Some(&original), support::contact(qso)).await?;
    let mut snapshot = read::snapshot(&store).await?;
    let page = snapshot.worked_before(&query).await?;
    assert_eq!(page.matching_total, 2);
    assert!(
        page.rows
            .iter()
            .any(|r| r.working.qso.contacted_station.callsign.as_str() == "W7HEX/QRP")
    );
    snapshot.close().await?;
    let current = save::load(&store, key).await?.unwrap();
    save::save(
        &store,
        key,
        Some(&current),
        clacks_sync::record::RecordState::Deleted,
    )
    .await?;
    let mut snapshot = read::snapshot(&store).await?;
    assert_eq!(snapshot.worked_before(&query).await?.matching_total, 1);
    snapshot.close().await?;
    store.close().await?;
    Ok(())
}
