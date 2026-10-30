use crate::support;
use clacks_client_store::{
    save::{self, SaveOutcome},
    store::{ClientStore, StoreSettings},
};
use clacks_sync::{client::OriginalBasis, record::RecordState};
use support::{contact, id, qso, qso_key};

#[tokio::test]
async fn save_reopen_without_server_and_reject_stale_edits() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, id(1), StoreSettings::default()).await?;
    let original = qso(id(10));
    let key = qso_key(original.id());
    let SaveOutcome::Saved(first) =
        save::save(&store, key, None, contact(original.clone())).await?
    else {
        panic!("new local save expected")
    };
    assert!(matches!(
        first.pending().as_ref().unwrap().original,
        OriginalBasis::Unavailable { .. }
    ));
    assert!(first.pending().as_ref().unwrap().prepared.is_none());
    let mut changed = original.clone();
    support::edit(&mut changed, |changed| {
        changed.comment = Some("intervening edit".into());
        Ok(())
    });
    let SaveOutcome::Saved(second) =
        save::save(&store, key, Some(&first), contact(changed.clone())).await?
    else {
        panic!("edit expected")
    };
    let stale = save::save(&store, key, Some(&first), contact(original)).await?;
    assert_eq!(
        stale,
        SaveOutcome::Stale {
            current: Some(second.clone())
        }
    );
    assert_eq!(
        save::save(&store, key, Some(&second), contact(changed)).await?,
        SaveOutcome::Unchanged(second.clone())
    );
    store.close().await?;
    let store = ClientStore::open(&file, id(1), StoreSettings::default()).await?;
    assert_eq!(save::load(&store, key).await?, Some(second.clone()));
    let SaveOutcome::Saved(deleted) =
        save::save(&store, key, Some(&second), RecordState::Deleted).await?
    else {
        panic!("deletion expected")
    };
    assert_eq!(deleted.working_state(), Some(&RecordState::Deleted));
    assert!(matches!(
        save::save(&store, key, Some(&second), contact(qso(id(10)))).await?,
        SaveOutcome::Stale { .. }
    ));
    store.close().await?;
    Ok(())
}
