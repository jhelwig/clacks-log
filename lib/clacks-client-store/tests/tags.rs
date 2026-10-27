use crate::support;
use clacks_client_store::{
    logbooks::deletion::{self, TagDeleteOutcome, TagDeletionCounts},
    save::{self, SaveOutcome},
    store::{ClientStore, StoreSettings},
};
use clacks_model::tag::{LogbookTag, TagDeletionChoice};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordKey, RecordState, RecordValue},
};
use support::{contact, id, qso, qso_key};

async fn saved(
    store: &ClientStore,
    state: RecordState,
    key: RecordKey,
) -> anyhow::Result<ClientRecord> {
    let SaveOutcome::Saved(record) = save::save(store, key, None, state).await? else {
        panic!("new save expected")
    };
    Ok(record)
}

#[tokio::test]
async fn all_tag_deletion_choices_apply_to_the_complete_membership_basis() -> anyhow::Result<()> {
    for choice in [
        TagDeletionChoice::LeaveAll,
        TagDeletionChoice::DeleteAll,
        TagDeletionChoice::DeleteUnique,
    ] {
        let dir = tempfile::tempdir()?;
        let store = ClientStore::create(
            &dir.path().join("workspace.sqlite"),
            id(1),
            StoreSettings::default(),
        )
        .await?;
        let tag_key = RecordKey {
            account: id(1),
            id: RecordId::Tag(id(20)),
        };
        let _tag = saved(
            &store,
            RecordState::Live(RecordValue::Tag(Box::new(LogbookTag::new(
                id(20),
                "Same name".into(),
            )))),
            tag_key,
        )
        .await?;
        let mut unique = qso(id(10));
        unique.tags = [id(20)].into();
        let unique = saved(&store, contact(unique), qso_key(id(10))).await?;
        let mut shared = qso(id(11));
        shared.tags = [id(20), id(21)].into();
        let shared = saved(&store, contact(shared), qso_key(id(11))).await?;
        let untagged = saved(&store, contact(qso(id(12))), qso_key(id(12))).await?;
        let mut deleted = qso(id(13));
        deleted.tags.insert(id(20));
        let deleted = saved(&store, contact(deleted), qso_key(id(13))).await?;
        save::save(&store, deleted.key, Some(&deleted), RecordState::Deleted).await?;
        let preview = deletion::preview(&store, id(20)).await?;
        assert_eq!(
            preview.counts,
            TagDeletionCounts {
                affected: 2,
                also_tagged_elsewhere: 1,
                unique: 1
            }
        );
        assert_eq!(preview.members, vec![unique, shared]);
        let TagDeleteOutcome::Applied { plan, .. } =
            deletion::delete_tag(&store, &preview, choice).await?
        else {
            panic!("tag deletion expected")
        };
        assert_eq!(plan.also_tagged_elsewhere, 1);
        assert_eq!(plan.affected.len(), 2);
        for n in [10, 11] {
            let row = save::load(&store, qso_key(id(n))).await?.unwrap();
            let deleted = matches!(choice, TagDeletionChoice::DeleteAll)
                || (matches!(choice, TagDeletionChoice::DeleteUnique) && n == 10);
            if deleted {
                assert_eq!(row.working_state(), Some(&RecordState::Deleted));
            } else {
                let Some(RecordState::Live(RecordValue::Qso(qso))) = row.working_state() else {
                    panic!("preserved contact expected")
                };
                assert!(!qso.tags.contains(&id(20)));
                assert_eq!(qso.tags.contains(&id(21)), n == 11);
            }
        }
        assert_eq!(save::load(&store, qso_key(id(12))).await?, Some(untagged));
        assert_eq!(
            save::load(&store, tag_key).await?.unwrap().working_state(),
            Some(&RecordState::Deleted)
        );
        store.close().await?;
    }
    Ok(())
}
