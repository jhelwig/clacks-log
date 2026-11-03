//! Recovery across an actual older PostgreSQL archive and newer persisted client stores.

mod database;

use super::super::support::synchronization::transfer::{bind, publish, pull, return_state, save};
use super::super::support::synchronization::{NativeAccount, qso};
use crate::{
    enrollment::ApprovalPurpose,
    store::{AccountStore, metadata::account, synchronization},
};
use clacks_client_store::{store::ClientStore, synchronization as client_sync};
use clacks_model::ids::QsoId;
use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};

#[tokio::test]
#[ignore = "requires an exclusively owned CLACKS_RESTORE_CONTAINER and PostgreSQL snapshot tools; run alone"]
async fn retention_recovers_from_older_database() -> anyhow::Result<()> {
    let database = database::Database::selected()?;
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("before-retention.dump");
    let mut native = NativeAccount::create().await?;
    let owner = native.first.account;

    let first = ClientStore::create(
        &directory.path().join("first.sqlite"),
        owner,
        Default::default(),
    )
    .await?;
    let second = ClientStore::create(
        &directory.path().join("second.sqlite"),
        owner,
        Default::default(),
    )
    .await?;
    bind(&first, &native.first, None).await?;
    bind(&second, &native.second, None).await?;
    pull(&native.store, &native.first, &first).await?;

    let contact = qso(QsoId::new(), Default::default());
    let key = RecordKey {
        account: owner,
        id: RecordId::Qso(contact.id()),
    };
    save(
        &first,
        key,
        RecordState::Live(RecordValue::Qso(Box::new(contact.clone()))),
    )
    .await?;
    publish(&native.store, &native.first, &first, key).await?;
    let directory_key = RecordKey {
        account: owner,
        id: RecordId::Directory(contact.id()),
    };
    let earlier_directory = super::directory::directory(
        contact.id(),
        clacks_model::directory::values::DirectoryProvider::Qrz,
        10,
        Some("Before backup"),
    )?;
    clacks_client_store::directory::snapshots::apply(
        &first,
        contact.id(),
        earlier_directory.snapshots().next().unwrap().clone(),
    )
    .await?;
    publish(&native.store, &native.first, &first, directory_key).await?;
    let first_ack = pull(&native.store, &native.first, &first).await?;

    let other = qso(QsoId::new(), Default::default());
    let other_key = RecordKey {
        account: owner,
        id: RecordId::Qso(other.id()),
    };
    save(
        &first,
        other_key,
        RecordState::Live(RecordValue::Qso(Box::new(other))),
    )
    .await?;
    publish(&native.store, &native.first, &first, other_key).await?;
    let second_ack = pull(&native.store, &native.second, &second).await?;
    assert_ne!(first_ack, second_ack);

    native.store.close().await;
    database.snapshot(&archive)?;
    native.store = reopen().await?;

    let third_authority = native
        .authorize("Third desktop", ApprovalPurpose::New)
        .await?;
    pull(&native.store, &native.first, &first).await?;

    let mut revoked_accepted = None;
    for _ in 0..2 {
        let contact = qso(QsoId::new(), Default::default());
        let key = RecordKey {
            account: owner,
            id: RecordId::Qso(contact.id()),
        };
        save(
            &first,
            key,
            RecordState::Live(RecordValue::Qso(Box::new(contact))),
        )
        .await?;
        revoked_accepted = Some(publish(&native.store, &native.first, &first, key).await?);
    }
    let revoked_accepted = revoked_accepted.unwrap();
    let newer_directory = super::directory::directory(
        contact.id(),
        clacks_model::directory::values::DirectoryProvider::HamQth,
        20,
        None,
    )?;
    clacks_client_store::directory::snapshots::apply(
        &first,
        contact.id(),
        newer_directory.snapshots().next().unwrap().clone(),
    )
    .await?;
    let directory_accepted = publish(&native.store, &native.first, &first, directory_key).await?;

    let third_path = directory.path().join("third.sqlite");
    let third = ClientStore::create(&third_path, owner, Default::default()).await?;
    let old_binding = bind(&third, &third_authority, None).await?;
    pull(&native.store, &third_authority, &third).await?;

    crate::enrollment::revoke_in_store(&native.store.pool, &native.principal, native.first.actor)
        .await?;

    pull(&native.store, &native.second, &second).await?;
    pull(&native.store, &third_authority, &third).await?;

    let imported = imported_status(&contact)?;
    let imported_key = RecordKey {
        account: owner,
        id: imported.id(),
    };
    save(&third, imported_key, RecordState::Live(imported)).await?;
    let imported_accepted = publish(&native.store, &third_authority, &third, imported_key).await?;

    // Freeze a real offline proposal, then accept a conflicting edit through another client.
    let mut local_fields = contact.clone().into_fields();
    local_fields.comment = Some("offline correction".into());
    save(
        &second,
        key,
        RecordState::Live(RecordValue::Qso(Box::new(local_fields.try_into()?))),
    )
    .await?;
    client_sync::prepare(&second, key).await?;
    let pending_before = clacks_client_store::save::load(&second, key)
        .await?
        .unwrap()
        .pending()
        .cloned();

    let mut remote_fields = contact.into_fields();
    remote_fields.comment = Some("accepted correction".into());
    save(
        &third,
        key,
        RecordState::Live(RecordValue::Qso(Box::new(remote_fields.try_into()?))),
    )
    .await?;
    let corrected = publish(&native.store, &third_authority, &third, key).await?;

    for _ in 0..5 {
        let contact = qso(QsoId::new(), Default::default());
        let key = RecordKey {
            account: owner,
            id: RecordId::Qso(contact.id()),
        };
        save(
            &third,
            key,
            RecordState::Live(RecordValue::Qso(Box::new(contact))),
        )
        .await?;
        publish(&native.store, &third_authority, &third, key).await?;
    }

    crate::store::retention::collect(
        &native.store,
        owner,
        3,
        &tokio_util::sync::CancellationToken::new(),
    )
    .await?;

    let newer = pull(&native.store, &third_authority, &third).await?;
    assert_ne!(newer.baseline, second_ack.baseline);

    pull(&native.store, &native.second, &second).await?;

    let mut review = clacks_client_store::read::snapshot(&second).await?;
    let conflict_before = review.review_record(key).await?.unwrap();
    review.close().await?;

    assert!(conflict_before.review.is_some());
    let pending = conflict_before.record.pending().unwrap();
    let before = pending_before.as_ref().unwrap();
    assert_eq!(pending.original, before.original);
    assert_eq!(pending.working, before.working);
    assert!(pending.witness.is_some());

    clacks_client_store::retention::reclaim_completed_cycle(&second).await?;

    // A post-backup actor has unpublished work which must not become accepted after reauthorization.
    let unsent = qso(QsoId::new(), Default::default());
    let unsent_key = RecordKey {
        account: owner,
        id: RecordId::Qso(unsent.id()),
    };
    save(
        &third,
        unsent_key,
        RecordState::Live(RecordValue::Qso(Box::new(unsent))),
    )
    .await?;
    client_sync::prepare(&third, unsent_key).await?;
    let unsent_before = clacks_client_store::save::load(&third, unsent_key)
        .await?
        .unwrap();

    third.close().await?;

    native.store.close().await;
    database.restore(&archive)?;
    native.store = reopen().await?;

    let third = ClientStore::open(&third_path, owner, Default::default()).await?;
    assert!(
        synchronization::acknowledge(&native.store, &third_authority, &newer)
            .await
            .is_err()
    );

    let replacement = native
        .authorize(
            "Recovered third desktop",
            ApprovalPurpose::ReplaceClosed {
                actor: third_authority.actor,
            },
        )
        .await?;
    assert_ne!(replacement.actor, third_authority.actor);

    bind(&third, &replacement, Some(&old_binding)).await?;

    crate::enrollment::revoke_in_store(&native.store.pool, &native.principal, native.first.actor)
        .await?;

    assert!(matches!(
        client_sync::prepare(&third, unsent_key).await?,
        client_sync::PrepareOutcome::AuthorizationReview { .. }
    ));
    assert_eq!(
        clacks_client_store::save::load(&third, unsent_key)
            .await?
            .unwrap()
            .pending(),
        unsent_before.pending()
    );

    let knowledge = client_sync::state(&third).await?;
    synchronization::recovery::negotiate(
        &native.store,
        &replacement,
        knowledge.checkpoint.as_ref(),
        &knowledge.baselines,
        knowledge.lineage.as_ref(),
    )
    .await?;

    let mut connection = native.store.pool.acquire().await?;
    let forked = account::load(&mut connection, owner).await?;
    assert_ne!(forked.head.timeline, corrected.stamp.accepted_at().timeline);
    drop(connection);

    assert!(!return_state(&native.store, &replacement, &third, true).await?);

    let mut connection = native.store.pool.acquire().await?;
    assert!(
        !account::load(&mut connection, owner)
            .await?
            .installed
            .covers(corrected.stamp.accepted_at()),
        "an interrupted return cannot certify the missing tail"
    );
    drop(connection);

    assert!(!return_state(&native.store, &replacement, &third, false).await?);

    let reviews = client_sync::state(&third).await?.pending;
    let mut resolved = 0;

    for row in reviews {
        let Some(review) = row.review else {
            continue;
        };

        if review.reason != clacks_sync::client::ReviewReason::ContradictoryCoverage {
            continue;
        }

        assert!(matches!(
            client_sync::resolve(&third, &row.record, &review, review.local.clone()).await?,
            clacks_client_store::save::SaveOutcome::Saved(_)
        ));
        publish(&native.store, &replacement, &third, row.record.key()).await?;
        resolved += 1;
    }
    assert_eq!(
        resolved, 1,
        "the reused old revision needs explicit reconciliation"
    );
    assert!(return_state(&native.store, &replacement, &third, false).await?);

    let mut connection = native.store.pool.acquire().await?;
    let recovered = account::load(&mut connection, owner).await?;
    assert!(recovered.installed.covers(corrected.stamp.accepted_at()));
    let meanings = account::interpretation(&mut connection, owner).await?;
    for baseline in &knowledge.baselines {
        assert!(
            meanings.baselines.contains(baseline),
            "a baseline ID must retain its original meaning"
        );
    }

    let clacks_sync::record::Observation::Accepted(returned) =
        synchronization::context::observation(&mut connection, &recovered, key).await?
    else {
        anyhow::bail!("corrected QSO not recovered");
    };
    assert_eq!(returned.state(), corrected.state());
    assert_eq!(returned.stamp.accepted_at(), corrected.stamp.accepted_at());
    let clacks_sync::record::Observation::Accepted(directory_return) =
        synchronization::context::observation(&mut connection, &recovered, directory_key).await?
    else {
        anyhow::bail!("Retired author's directory history was not recovered")
    };
    assert_eq!(
        directory_return.state(),
        directory_accepted.state(),
        "restore must preserve original provider lookup tuple and complete winner"
    );

    let clacks_sync::record::Observation::Accepted(imported_return) =
        synchronization::context::observation(&mut connection, &recovered, imported_key).await?
    else {
        anyhow::bail!("post-backup imported evidence not recovered");
    };
    assert_eq!(imported_return.state(), imported_accepted.state());
    assert_eq!(
        imported_return.stamp.accepted_at(),
        imported_accepted.stamp.accepted_at()
    );

    assert!(matches!(
        synchronization::context::observation(&mut connection, &recovered, unsent_key).await?,
        clacks_sync::record::Observation::Absent { .. }
    ));

    let clacks_sync::record::Observation::Accepted(original_return) =
        synchronization::context::observation(&mut connection, &recovered, revoked_accepted.key())
            .await?
    else {
        anyhow::bail!("revoked author's accepted contribution disappeared");
    };
    assert_eq!(original_return.state(), revoked_accepted.state());
    assert_eq!(
        original_return.stamp.accepted_at(),
        revoked_accepted.stamp.accepted_at()
    );
    drop(connection);

    let mut review = clacks_client_store::read::snapshot(&second).await?;
    assert_eq!(review.review_record(key).await?.unwrap(), conflict_before);
    review.close().await?;

    first.close().await?;
    second.close().await?;

    third.close().await?;

    native.store.close().await;

    Ok(())
}

/// Reopen the restricted runtime pool without inventing a new server identity.
async fn reopen() -> anyhow::Result<AccountStore> {
    Ok(AccountStore::new(
        sqlx::postgres::PgPoolOptions::new()
            .max_connections(4)
            .connect(&std::env::var("CLACKS_TEST_RUNTIME_DATABASE_URL")?)
            .await?,
    ))
}

/// A source-only positive claim, without fabricated dispatch or provider response history.
fn imported_status(contact: &clacks_model::contact::Qso) -> anyhow::Result<RecordValue> {
    use clacks_model::{
        ids::AssertionId,
        service::{
            adif,
            destination::Service,
            evidence::{
                ImportProvenance, ImportedAssertion, ImportedBaseline, ImportedClaim,
                ServicePayload, submission::ConfirmationBasis,
            },
        },
    };

    let (date, time) = adif::contact_time(contact.start())?;
    let fields: adif::AdifFields = [
        (
            "CALL".into(),
            contact.contacted_station().callsign.as_str().into(),
        ),
        (
            "STATION_CALLSIGN".into(),
            contact.own_station().callsign.as_str().into(),
        ),
        ("QSO_DATE".into(), date),
        ("TIME_ON".into(), time),
    ]
    .into();

    Ok(RecordValue::Service(Box::new(
        clacks_sync::service::ServiceRecord {
            payload: ServicePayload::Imported(Box::new(ImportedAssertion {
                id: AssertionId::new(),
                qso: contact.id(),
                service: Service::Qrz,
                destination: None,
                claim: ImportedClaim::Uploaded,
                date: None,
                provenance: ImportProvenance {
                    source: "source.adi".into(),
                    record: "1".into(),
                    mapping: "ADIF 3.1.8".into(),
                },
                baseline: ImportedBaseline {
                    native_end: contact.end(),
                    service: Service::Qrz,
                    coverage: fields.keys().cloned().collect(),
                    fields,
                    own_location: None,
                    confirmation: ConfirmationBasis {
                        own_call: Some(contact.own_station().callsign.clone()),
                        worked_call: Some(contact.contacted_station().callsign.clone()),
                        start: Some(contact.start()),
                        ..Default::default()
                    },
                },
            })),
            support: Default::default(),
        },
    )))
}
