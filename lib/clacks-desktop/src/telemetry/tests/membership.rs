//! Membership preparation, progress and cancellation under production telemetry.

use crate::{
    session::settings::contacts::{
        ContactAction, execute,
        membership::{self, MembershipSubmission},
    },
    workspace::WorkspacePaths,
};
use clacks_client_store::{
    save::{self, SaveOutcome},
    store::{ClientStore, StoreSettings},
};
use clacks_model::ids::{QsoId, TagId};
use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

/// Exercise captured bases, partial progress and cancellation through actual membership operations.
pub(super) async fn run(paths: &WorkspacePaths) -> anyhow::Result<()> {
    let store = ClientStore::create(
        &paths.database,
        Default::default(),
        StoreSettings::default(),
    )
    .await?;

    let a = super::operations::contact()?;
    let b = super::operations::contact()?;
    for qso in [&a, &b] {
        execute(
            &store,
            ContactAction::Save {
                directory: None,
                expected: None,
                next: Box::new(qso.clone()),
            },
            None,
            tokio::sync::watch::channel(()).0,
        )
        .await?;
    }

    let account = store.account_id();
    let ids = [a.id(), b.id()].into();
    let basis = membership::prepare(&store, &ids).await?;
    assert_eq!(basis.records.len(), 2);

    let first = basis.records.first().unwrap().clone();
    let Some(RecordState::Live(RecordValue::Qso(mut next))) = first.working_state().cloned() else {
        panic!("qso")
    };
    crate::test_support::edit(&mut next, |next| {
        next.comment = Some("newer fact".into());
        Ok(())
    });
    save::save(
        &store,
        first.key(),
        Some(&first),
        RecordState::Live(RecordValue::Qso(next)),
    )
    .await?;

    let (progress, receiver) = watch::channel(Default::default());
    let report = membership::apply(
        &store,
        MembershipSubmission {
            basis: basis.records,
            add: [TagId::new()].into(),
            remove: Default::default(),
            cancel: CancellationToken::new(),
            progress,
        },
    )
    .await?;

    assert_eq!(report.entries.len(), 2);
    assert!(matches!(
        report.entries.first().unwrap().outcome,
        SaveOutcome::Stale { .. }
    ));
    assert_eq!(receiver.borrow().changed, 1);
    assert_eq!(receiver.borrow().stale, 1);

    let basis = membership::prepare(&store, &ids).await?;
    let cancel = CancellationToken::new();
    cancel.cancel();

    let (progress, receiver) = watch::channel(Default::default());
    let report = membership::apply(
        &store,
        MembershipSubmission {
            basis: basis.records,
            add: [TagId::new()].into(),
            remove: Default::default(),
            cancel,
            progress,
        },
    )
    .await?;

    assert!(report.entries.is_empty());
    assert_eq!(report.unprocessed.len(), 2);
    assert_eq!(receiver.borrow().changed, 0);

    assert!(
        membership::prepare(&store, &[QsoId::new()].into())
            .await
            .is_err()
    );
    let b = save::load(
        &store,
        RecordKey {
            account,
            id: RecordId::Qso(b.id()),
        },
    )
    .await?
    .unwrap();
    save::save(&store, b.key(), Some(&b), RecordState::Deleted).await?;
    assert!(membership::prepare(&store, &ids).await.is_err());

    store.close().await?;
    Ok(())
}
