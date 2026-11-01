//! Startup interpretation of durable contact-view preferences.

use super::*;
use clacks_model::{ids::AccountId, tag::LogbookTag};

#[tokio::test]
async fn restore_preserves_unknown_target_and_repairs_known_deletion() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("workspace.sqlite"),
        AccountId::new(),
        Default::default(),
    )
    .await?;

    let id = TagId::new();
    let selected = serde_json::to_string(&SelectedView::Logbook(id))?;

    let mut transaction = store.write().await?;
    local_state::set_in(&mut transaction, SELECTED_VIEW_KEY, &selected).await?;
    transaction.commit().await?;

    assert_eq!(restore(&store).await?, None);
    assert_eq!(
        crate::settings::snapshot(&store)
            .await?
            .logbooks
            .selected_view,
        SelectedView::Logbook(id)
    );

    let key = RecordKey {
        account: store.account,
        id: RecordId::Tag(id),
    };

    crate::save::save(
        &store,
        key,
        None,
        RecordState::Live(RecordValue::Tag(Box::new(LogbookTag::new(
            id,
            "Restored".into(),
        )))),
    )
    .await?;

    crate::save::save(
        &store,
        key,
        crate::save::load(&store, key).await?.as_ref(),
        RecordState::Deleted,
    )
    .await?;
    assert_eq!(
        crate::settings::snapshot(&store)
            .await?
            .logbooks
            .selected_view,
        SelectedView::AllContacts
    );

    // Startup may encounter a valid persisted preference whose tag is already deleted.

    let mut transaction = store.write().await?;
    local_state::set_in(&mut transaction, SELECTED_VIEW_KEY, &selected).await?;
    transaction.commit().await?;

    assert_eq!(restore(&store).await?, Some(id));
    assert_eq!(
        crate::settings::snapshot(&store)
            .await?
            .logbooks
            .selected_view,
        SelectedView::AllContacts
    );

    store.close().await?;

    Ok(())
}
