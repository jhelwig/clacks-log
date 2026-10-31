//! A retained competing recovery value is evidence for review, not recovered-prefix coverage.

use super::super::support::synchronization::{NativeAccount, qso};
use crate::store::{
    metadata::account,
    synchronization::{self, receipt},
};
use clacks_model::ids::QsoId;
use clacks_sync::{
    acceptance::{AcceptanceDecision, AcceptanceInput},
    record::{Observation, RecordId, RecordKey, RecordState, RecordValue},
    transfer::{
        WindowMode,
        discovery::{DiscoveryEnd, DiscoveryPage, DiscoveryStart},
        receipt::{ReceiptState, register_page},
    },
};

#[tokio::test]
async fn unresolved_return_does_not_certify_the_old_prefix() -> anyhow::Result<()> {
    let native = NativeAccount::create().await?;
    let base = qso(QsoId::new(), Default::default());
    let key = RecordKey {
        account: native.first.account,
        id: RecordId::Qso(base.id()),
    };
    let create = native
        .prepare(
            &native.first,
            key,
            RecordState::Live(RecordValue::Qso(Box::new(base.clone()))),
        )
        .await?;
    let AcceptanceDecision::Commit(original) =
        synchronization::accept_pending(&native.store, &native.first, &create).await?
    else {
        panic!("base")
    };
    let mut fields = base.clone().into_fields();
    fields.comment = Some("Before restore".into());
    let change = native
        .prepare(
            &native.first,
            key,
            RecordState::Live(RecordValue::Qso(Box::new(fields.try_into()?))),
        )
        .await?;
    let mut tx = native.store.pool.begin().await?;
    let before = account::load(&mut tx, key.account).await?;
    let membership = crate::enrollment::membership::snapshot(&mut tx, &before).await?;
    let interpretation = account::interpretation(&mut tx, key.account).await?;
    tx.commit().await?;
    let AcceptanceDecision::Commit(lost) = clacks_sync::acceptance::plan_acceptance(
        &change,
        &AcceptanceInput {
            account: &before,
            membership: &membership,
            current: &Observation::Accepted(original.accepted),
            references: &[],
            destination_registry: None,
            bindings: &[],
        },
        &interpretation.view(),
    )?
    else {
        panic!("pre-restore acceptance")
    };
    let source = synchronization::checkpoint(&lost.next_account);
    synchronization::recovery::negotiate(
        &native.store,
        &native.first,
        Some(&source),
        &interpretation.baselines,
        Some(&interpretation.lineage),
    )
    .await?;
    let mut fields = base.into_fields();
    fields.comment = Some("Competing restored edit".into());
    let change = native
        .prepare(
            &native.second,
            key,
            RecordState::Live(RecordValue::Qso(Box::new(fields.try_into()?))),
        )
        .await?;
    assert!(matches!(
        synchronization::accept_pending(&native.store, &native.second, &change).await?,
        AcceptanceDecision::Commit(_)
    ));
    let start = DiscoveryStart {
        source: source.clone(),
        mode: WindowMode::Full,
    };
    let page = DiscoveryPage {
        sequence: 0,
        requirements: vec![],
        accepted: vec![lost.accepted.clone()],
        support: vec![],
        baselines: interpretation.baselines,
        boundaries: vec![],
        units: vec![],
    };
    let received = receipt::receive(&native.store, &native.first, &page).await?;
    assert_eq!(received.reviews.len(), 1);
    let mut registered = register_page(&ReceiptState::begin(&start)?, &page)?;
    registered.staged = received
        .staged
        .iter()
        .map(clacks_sync::service::VersionReference::from)
        .collect();
    registered.completed_units = received.completed_units;
    assert!(
        !receipt::complete_return(
            &native.store,
            &native.first,
            &registered,
            &DiscoveryEnd {
                source: source.clone(),
                pages: 1
            }
        )
        .await?
    );
    let mut tx = native.store.pool.begin().await?;
    let after = account::load(&mut tx, key.account).await?;
    assert!(
        !after.installed.covers(
            source
                .coverage
                .entries()
                .map(|(timeline, revision)| clacks_sync::record::Position { timeline, revision })
                .next()
                .unwrap()
        )
    );
    tx.commit().await?;
    let mut discovery =
        crate::store::discovery::Source::start(&native.store, &native.first, None).await?;
    let mut historical = Vec::new();
    let mut heads = Vec::new();
    while let Some(page) = discovery.page(&native.store, &native.first, 64).await? {
        heads.extend(page.accepted);
        historical.extend(page.support);
    }
    assert!(historical.contains(&lost.accepted));
    assert!(!heads.contains(&lost.accepted));
    assert!(heads.iter().any(|value| value.key() == key));
    Ok(())
}

#[tokio::test]
async fn accepted_tail_witness_forks_without_certifying_its_prefix() -> anyhow::Result<()> {
    let native = NativeAccount::create().await?;
    let contact = qso(QsoId::new(), Default::default());
    let key = RecordKey {
        account: native.first.account,
        id: RecordId::Qso(contact.id()),
    };
    let change = native
        .prepare(
            &native.first,
            key,
            RecordState::Live(RecordValue::Qso(Box::new(contact))),
        )
        .await?;
    let mut tx = native.store.pool.begin().await?;
    let before = account::load(&mut tx, key.account).await?;
    let membership = crate::enrollment::membership::snapshot(&mut tx, &before).await?;
    let interpretation = account::interpretation(&mut tx, key.account).await?;
    tx.commit().await?;
    let current = Observation::Absent {
        key,
        checkpoint: synchronization::checkpoint(&before),
    };
    let AcceptanceDecision::Commit(lost) = clacks_sync::acceptance::plan_acceptance(
        &change,
        &AcceptanceInput {
            account: &before,
            membership: &membership,
            current: &current,
            references: &[],
            destination_registry: None,
            bindings: &[],
        },
        &interpretation.view(),
    )?
    else {
        panic!("pre-restore acceptance")
    };
    let page = DiscoveryPage {
        sequence: 0,
        requirements: vec![],
        accepted: vec![lost.accepted.clone()],
        support: vec![],
        baselines: interpretation.baselines,
        boundaries: vec![],
        units: vec![],
    };
    let received = receipt::receive(&native.store, &native.first, &page).await?;
    assert_eq!(received.represented_count, 1);
    let mut tx = native.store.pool.begin().await?;
    let restored = account::load(&mut tx, key.account).await?;
    assert_ne!(restored.head.timeline, before.head.timeline);
    assert!(!restored.installed.covers(lost.accepted.stamp.accepted_at()));
    assert!(
        super::accepted_heads(&mut tx, key.account)
            .await?
            .contains(&lost.accepted)
    );
    tx.commit().await?;
    Ok(())
}

#[tokio::test]
async fn exact_historical_support_does_not_replace_deleted_current_heads() -> anyhow::Result<()> {
    use crate::store::tests::support::synchronization::{NativeAccount, qso};
    use clacks_model::{
        ids::{AssociationId, DestinationId, OperationId, QsoId},
        service::{
            destination::{Destination, ExternalRecordKey, RemoteCollection, RemoteRecordId},
            evidence::{
                ServicePayload,
                download::DownloadControl,
                processing::{AssociationState, ExternalAssociation},
            },
        },
    };
    use clacks_sync::{
        acceptance::AcceptanceDecision,
        record::{AcceptedRecord, RecordId, RecordKey, RecordState, RecordValue},
        service::{
            ServiceRecord, ServiceSupport, SupportRequirement, VersionReference,
            operation::CoupledRequirement,
        },
        transfer::discovery::DiscoveryPage,
    };

    async fn accept(
        native: &NativeAccount,
        key: RecordKey,
        state: RecordState,
    ) -> anyhow::Result<AcceptedRecord> {
        let change = native.prepare(&native.first, key, state).await?;
        let AcceptanceDecision::Commit(commit) =
            crate::store::synchronization::accept_pending(&native.store, &native.first, &change)
                .await?
        else {
            anyhow::bail!("valid fixture proposal was not accepted");
        };
        Ok(commit.accepted)
    }

    let native = NativeAccount::create().await?;
    let qso_id = QsoId::new();
    let qso_key = RecordKey {
        account: native.first.account,
        id: RecordId::Qso(qso_id),
    };
    let old_qso = accept(
        &native,
        qso_key,
        RecordState::Live(RecordValue::Qso(Box::new(qso(qso_id, Default::default())))),
    )
    .await?;
    let destination = Destination {
        id: DestinationId::new(),
        collection: RemoteCollection::Qrz {
            book_id: Some("7".parse()?),
        },
        description: "QRZ logbook".into(),
    };
    let association = ExternalAssociation {
        id: AssociationId::new(),
        external: ExternalRecordKey {
            destination: destination.id,
            record: RemoteRecordId::Qrz("8".parse()?),
        },
        qso: qso_id,
        state: AssociationState::Active,
    };
    let payload = ServicePayload::Association(association.clone());
    let association_key = RecordKey {
        account: native.first.account,
        id: RecordId::Service(payload.id()),
    };
    let old_association = accept(
        &native,
        association_key,
        RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
            payload,
            support: ServiceSupport {
                destination: Some(destination.clone()),
                ..Default::default()
            },
        }))),
    )
    .await?;
    accept(&native, qso_key, RecordState::Deleted).await?;
    let deleted_association = accept(
        &native,
        association_key,
        RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
            payload: ServicePayload::Association(ExternalAssociation {
                state: AssociationState::DeletedQso,
                ..association
            }),
            support: ServiceSupport {
                destination: Some(destination.clone()),
                ..Default::default()
            },
        }))),
    )
    .await?;

    let payload = ServicePayload::Download(DownloadControl::new(destination.id)?);
    let control_key = RecordKey {
        account: native.first.account,
        id: RecordId::Service(payload.id()),
    };
    let control = accept(
        &native,
        control_key,
        RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
            payload,
            support: ServiceSupport {
                destination: Some(destination),
                required: vec![SupportRequirement::ExactFact(VersionReference::from(
                    &old_association,
                ))],
                ..Default::default()
            },
        }))),
    )
    .await?;
    let mut read = native.store.pool.begin().await?;
    let interpretation =
        crate::store::metadata::account::interpretation(&mut read, native.first.account).await?;
    read.commit().await?;
    let operation = OperationId::new();
    let result = receipt::receive(
        &native.store,
        &native.first,
        &DiscoveryPage {
            sequence: 0,
            requirements: vec![],
            accepted: vec![old_association.clone(), control],
            support: vec![],
            baselines: interpretation.baselines,
            boundaries: vec![],
            units: vec![CoupledRequirement {
                id: operation,
                members: [&old_qso, &old_association]
                    .into_iter()
                    .map(VersionReference::from)
                    .collect(),
            }],
        },
    )
    .await?;
    assert!(
        result.staged.is_empty(),
        "an old effect covered by deletion must not wait on its historical active value"
    );
    assert!(result.completed_units.contains(&operation));
    assert_eq!(result.represented_count, 2);
    assert!(result.reviews.is_empty());
    let mut read = native.store.pool.begin().await?;
    let heads = super::accepted_heads(&mut read, native.first.account).await?;
    assert!(heads.contains(&deleted_association));
    assert!(
        heads
            .iter()
            .any(|row| row.key() == qso_key && matches!(row.state(), RecordState::Deleted))
    );
    read.commit().await?;
    Ok(())
}
