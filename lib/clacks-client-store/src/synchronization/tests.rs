use super::*;
use crate::{identity::EnrollmentBinding, save, store::StoreSettings};
use clacks_model::tag::LogbookTag;
use clacks_sync::{
    acceptance::{AcceptanceDecision, AcceptanceInput},
    account::{AccountMetadata, InstalledCheckpoint},
    client::{Preparation, ReviewReason},
    membership::{ClientMembership, MembershipSnapshot},
    record::{RecordId, RecordValue},
};

fn id<T: std::str::FromStr>(value: u128) -> T
where
    T::Err: std::fmt::Debug,
{
    ulid::Ulid::from(value).to_string().parse().unwrap()
}
fn state(name: &str) -> RecordState {
    RecordState::Live(RecordValue::Tag(Box::new(LogbookTag::new(
        id(10),
        name.into(),
    ))))
}

#[tokio::test]
async fn frozen_restart_receipt_later_edit_and_rejection_preserve_their_actual_roles()
-> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("workspace.sqlite");
    let store = ClientStore::create(&file, id(1), StoreSettings::default()).await?;
    let binding = EnrollmentBinding {
        account: id(1),
        actor: id(9),
        server: "https://example.invalid".into(),
        credential_reference: "protected-store-reference".into(),
    };
    identity::bind(&store, None, binding.clone()).await?;
    let unknown_key = RecordKey {
        account: id(1),
        id: RecordId::Tag(id(20)),
    };
    let new_tag = |tag_id, name: &str| {
        RecordState::Live(RecordValue::Tag(Box::new(LogbookTag::new(
            tag_id,
            name.into(),
        ))))
    };
    save::save(&store, unknown_key, None, new_tag(id(20), "before receipt")).await?;
    let (account, baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
    let membership = MembershipSnapshot {
        account: id(1),
        generation: 0,
        clients: vec![ClientMembership {
            actor: id(9),
            created_at: account.head,
            closed_at: None,
            acknowledged: None,
        }],
    };
    let checkpoint = InstalledCheckpoint {
        account: id(1),
        baseline: baseline.id,
        coverage: account.installed.clone(),
    };
    // Actual initial server interpretation and complete empty source view.
    let mut transaction = store.write().await?;
    lineage::insert_baseline(&mut transaction, &baseline).await?;
    lineage::set_lineage(&mut transaction, &baseline.lineage).await?;
    lineage::set_checkpoint(&mut transaction, &checkpoint).await?;
    transaction.commit().await?;
    assert!(observe_account(&store, None, &account, &membership).await?);
    assert!(!observe_account(&store, None, &account, &membership).await?);
    let absent_key = RecordKey {
        account: id(1),
        id: RecordId::Tag(id(21)),
    };
    save::save(&store, absent_key, None, new_tag(id(21), "after receipt")).await?;
    assert!(matches!(
        prepared(&store, unknown_key).await?,
        Preparation::Review { .. }
    ));
    assert!(matches!(
        prepared(&store, absent_key).await?,
        Preparation::Ready { .. }
    ));
    let key = RecordKey {
        account: id(1),
        id: RecordId::Tag(id(10)),
    };
    let SaveOutcome::Saved(saved) = save::save(&store, key, None, state("first")).await? else {
        panic!("new save")
    };
    let Preparation::Ready {
        record,
        counter,
        change,
    } = prepared(&store, key).await?
    else {
        panic!("ready")
    };
    assert_eq!(counter.high_water, 1);
    let SaveOutcome::Saved(later) = save::save(&store, key, Some(&record), state("later")).await?
    else {
        panic!("later save")
    };
    assert_eq!(later.pending().unwrap().prepared.as_ref(), Some(&change));
    store.close().await?;
    let store = ClientStore::open(&file, id(1), StoreSettings::default()).await?;
    let Preparation::Retry { change: retry, .. } = prepared(&store, key).await? else {
        panic!("retry")
    };
    assert_eq!(retry, change);
    let context = {
        let mut tx = store.pool.begin().await?;
        evidence(&mut tx, id(1)).await?
    };
    let AcceptanceDecision::Commit(accepted) = acceptance::plan_acceptance(
        &change,
        &AcceptanceInput {
            destination_registry: None,
            bindings: &[],
            account: &account,
            membership: &membership,
            current: saved.latest(),
            references: &[],
        },
        &context.view(),
    )?
    else {
        panic!("accepted")
    };
    let updated = receive(&store, &accepted.accepted).await?;
    assert_eq!(updated.record.working_state(), Some(&state("later")));
    let Preparation::Ready { change: next, .. } = prepared(&store, key).await? else {
        panic!("second ready")
    };
    assert_eq!(
        next.vector()
            .component(clacks_sync::clock::ClockActor::Client(id(9))),
        2
    );
    let rejected = reject(&store, key, &RejectionReason::ActorUnknown { actor: id(9) }).await?;
    assert_eq!(
        rejected.review.as_ref().unwrap().reason,
        ReviewReason::ExcludedActor(id(9))
    );
    let next_binding = EnrollmentBinding {
        actor: id(19),
        ..binding.clone()
    };
    identity::bind(&store, Some(&binding), next_binding).await?;
    assert_eq!(
        save::load(&store, key).await?,
        Some(rejected.record.clone())
    );
    store.close().await?;
    let store = ClientStore::open(&file, id(1), StoreSettings::default()).await?;
    let mut tx = store.pool.begin().await?;
    assert_eq!(review::load(&mut tx, key).await?, rejected.review);
    assert_eq!(client::counter(&mut tx, key, id(9)).await?.high_water, 2);
    tx.commit().await?;
    let shown = rejected.review.as_ref().unwrap();
    let SaveOutcome::Saved(changed) =
        save::save(&store, key, Some(&rejected.record), state("intervening")).await?
    else {
        panic!("save")
    };
    assert_eq!(
        resolve(&store, &rejected.record, shown, state("chosen")).await?,
        SaveOutcome::Stale {
            current: Some(changed.clone())
        }
    );
    assert!(matches!(
        resolve(&store, &changed, shown, state("chosen")).await?,
        SaveOutcome::Stale { .. }
    ));
    let Preparation::Review { record, input } = prepared(&store, key).await? else {
        panic!("actor review")
    };
    assert!(matches!(
        resolve(&store, &record, &input, state("chosen")).await?,
        SaveOutcome::Saved(_)
    ));

    let mut forked = baseline.lineage.clone();
    forked.add_boundary(clacks_sync::lineage::TimelineBoundary {
        at: clacks_sync::record::Position {
            timeline: id(4),
            revision: 0,
        },
        includes: clacks_sync::lineage::Coverage::from(account.head),
        kind: clacks_sync::lineage::BoundaryKind::ForkOrigin,
    })?;
    crate::retention::install_interpretation(&store, &checkpoint, &[], &forked).await?;
    let alternative = clacks_sync::record::AcceptedRecord::new(
        key,
        state("restored branch"),
        clacks_sync::record::VersionStamp::new(
            clacks_sync::record::Position {
                timeline: id(4),
                revision: 1,
            },
            baseline.id,
            clacks_sync::clock::RecordVector::from_entries([(
                clacks_sync::clock::ClockActor::Server(id(4)),
                1,
            )])?,
        )
        .expect("valid accepted fixture context"),
    )?;
    let update = receive(&store, &alternative).await?;
    assert!(!update.record.retained().is_empty());
    store.close().await?;
    let reopened = ClientStore::open(&file, id(1), StoreSettings::default()).await?;
    let Preparation::Review { input, .. } = prepared(&reopened, key).await? else {
        panic!("retained alternative needs review")
    };
    assert!(!input.retained.is_empty());
    reopened.close().await?;
    Ok(())
}

async fn prepared(store: &ClientStore, key: RecordKey) -> anyhow::Result<Preparation> {
    let PrepareOutcome::Core(value) = prepare(store, key).await? else {
        panic!("enrolled fixture")
    };
    Ok(*value)
}
