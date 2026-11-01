use crate::support;
use clacks_client_store::{
    save::{self, SaveOutcome},
    store::{ClientStore, StoreSettings},
};

#[tokio::test]
async fn replaced_working_values_survive_workspace_reopen() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, support::id(1), StoreSettings::default()).await?;
    let key = support::qso_key(support::id(10));
    let SaveOutcome::Saved(row) = save::save(
        &store,
        key,
        None,
        support::contact(support::qso(support::id(10))),
    )
    .await?
    else {
        panic!("save")
    };
    let mut qso = support::qso(support::id(10));
    support::edit(&mut qso, |qso| {
        qso.comment = Some("Keep pending values".into());
        Ok(())
    });
    let SaveOutcome::Saved(next) =
        save::save(&store, key, Some(&row), support::contact(qso)).await?
    else {
        panic!("save")
    };
    assert_eq!(save::load(&store, key).await?, Some(next.clone()));
    store.close().await?;
    let store = ClientStore::open(&file, support::id(1), StoreSettings::default()).await?;
    assert_eq!(save::load(&store, key).await?, Some(next));
    store.close().await?;
    Ok(())
}

mod baselines;
mod rollback;

/// Supply received interpretation through an ordinary incremental page.
async fn receive_interpretation(
    store: &ClientStore,
    checkpoint: &clacks_sync::account::InstalledCheckpoint,
    baselines: &[clacks_sync::account::Baseline],
    lineage: &clacks_sync::lineage::Lineage,
) -> anyhow::Result<()> {
    let mut receipt = clacks_client_store::transfer::begin(
        store,
        &clacks_sync::transfer::discovery::DiscoveryStart {
            source: checkpoint.clone(),
            mode: clacks_sync::transfer::WindowMode::Incremental {
                after: checkpoint.coverage.clone(),
            },
        },
    )
    .await?;
    clacks_client_store::transfer::install_page(
        store,
        &mut receipt,
        &clacks_sync::transfer::discovery::DiscoveryPage {
            sequence: 0,
            requirements: vec![],
            accepted: vec![],
            support: vec![],
            baselines: baselines.to_vec(),
            boundaries: lineage.boundaries().to_vec(),
            units: vec![],
        },
    )
    .await
}
