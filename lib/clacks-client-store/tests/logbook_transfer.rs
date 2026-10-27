use crate::logbook_support;
use crate::support;

use clacks_client_store::{
    logbooks::{
        selection::{self, SelectedView},
        tags,
    },
    save::{self, SaveOutcome},
    settings, synchronization, transfer,
};
use clacks_model::tag::LogbookTag;
use clacks_sync::{
    account::{AccountMetadata, Baseline, InstalledCheckpoint},
    lineage::Coverage,
    record::{AcceptedRecord, RecordState, RecordValue},
    transfer::{RecordRequirement, WindowManifest, WindowMode},
};
use logbook_support::Workspace;
use support::id;

async fn remember(
    store: &clacks_client_store::store::ClientStore,
    accepted: &AcceptedRecord,
    baseline: Baseline,
) -> anyhow::Result<InstalledCheckpoint> {
    let source = InstalledCheckpoint {
        account: id(1),
        baseline: baseline.id,
        coverage: Coverage::from(accepted.stamp.accepted_at),
    };
    let manifest = WindowManifest::new(
        id(1),
        source.clone(),
        WindowMode::Full,
        vec![RecordRequirement {
            key: accepted.key,
            stamp: accepted.stamp.clone(),
            deleted: matches!(accepted.state, RecordState::Deleted),
        }],
        vec![],
        vec![baseline],
        vec![],
    )?;
    transfer::remember(store, &manifest).await?;
    Ok(source)
}

#[tokio::test]
async fn installed_and_received_deletion_preserve_live_alternatives_until_resolution()
-> anyhow::Result<()> {
    for pending in [false, true] {
        for receipt in [false, true] {
            let workspace = Workspace::new().await?;
            let (_, baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
            let mut tag = LogbookTag::new(id(20), "Main".into());
            let live = support::accepted(
                1,
                RecordState::Live(RecordValue::Tag(Box::new(tag.clone()))),
            );
            let source = remember(&workspace.store, &live, baseline.clone()).await?;
            transfer::install(
                &workspace.store,
                &source,
                &WindowMode::Full,
                std::slice::from_ref(&live),
            )
            .await?;
            transfer::finish(&workspace.store, &source, &WindowMode::Full).await?;
            selection::select(&workspace.store, SelectedView::Logbook(tag.id)).await?;
            if pending {
                let row = save::load(&workspace.store, live.key).await?.unwrap();
                tag.name = "Local alternative".into();
                tags::save(&workspace.store, Some(&row), &tag).await?;
            }
            let mut qso = support::qso(id(100));
            qso.tags = [tag.id].into();
            let SaveOutcome::Saved(qso_row) = save::save(
                &workspace.store,
                support::qso_key(qso.id),
                None,
                support::contact(qso),
            )
            .await?
            else {
                panic!("qso save");
            };
            let mut deleted =
                support::accepted(2, RecordState::Live(RecordValue::Tag(Box::new(tag))));
            deleted.state = RecordState::Deleted;
            let source = remember(&workspace.store, &deleted, baseline).await?;
            // Merely staging an advertised deletion has no selected-view effect.
            assert_eq!(
                settings::snapshot(&workspace.store)
                    .await?
                    .logbooks
                    .selected_view,
                SelectedView::Logbook(id(20))
            );
            if receipt {
                synchronization::receive(&workspace.store, &deleted).await?;
            } else {
                transfer::install(
                    &workspace.store,
                    &source,
                    &WindowMode::Full,
                    std::slice::from_ref(&deleted),
                )
                .await?;
                transfer::install(
                    &workspace.store,
                    &source,
                    &WindowMode::Full,
                    std::slice::from_ref(&deleted),
                )
                .await?;
            }
            assert_eq!(
                settings::snapshot(&workspace.store)
                    .await?
                    .logbooks
                    .selected_view,
                if pending {
                    SelectedView::Logbook(id(20))
                } else {
                    SelectedView::AllContacts
                }
            );
            assert_eq!(
                save::load(&workspace.store, qso_row.key).await?,
                Some(qso_row)
            );
            if pending {
                let row = save::load(&workspace.store, deleted.key).await?.unwrap();
                let mut snapshot = clacks_client_store::read::snapshot(&workspace.store).await?;
                let reviews = snapshot
                    .review(&clacks_client_store::read::review::ReviewQuery::default())
                    .await?;
                snapshot.close().await?;
                let review = reviews
                    .iter()
                    .find(|review| review.record.key == deleted.key)
                    .unwrap();
                let shown = review.review.as_ref().unwrap();
                assert!(matches!(
                    synchronization::resolve(&workspace.store, &row, shown, RecordState::Deleted)
                        .await?,
                    SaveOutcome::Saved(_)
                ));
                assert_eq!(
                    settings::snapshot(&workspace.store)
                        .await?
                        .logbooks
                        .selected_view,
                    SelectedView::AllContacts
                );
            }
            workspace.store.close().await?;
        }
    }
    Ok(())
}
