//! Real account provisioning, accepted-state persistence and synchronization transactions.

mod recovery;
mod references;
mod restore;
mod retention;

use crate::auth::oidc::VerifiedIdentity;
use time::Duration;

#[tokio::test]
async fn tag_deletion_checks_current_reverse_references() -> anyhow::Result<()> {
    use super::support::synchronization::{NativeAccount, qso};
    use clacks_sync::{
        acceptance::AcceptanceDecision,
        record::{RecordId, RecordKey, RecordState, RecordValue},
    };
    let _cache_guard = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let output = tempfile::NamedTempFile::new()?;
    let subscriber = tracing_subscriber::fmt()
        .json()
        .without_time()
        .with_max_level(tracing::Level::INFO)
        .with_writer(output.reopen()?)
        .finish();
    let _tracing = tracing::subscriber::set_default(subscriber);
    let recorder = metrics_util::debugging::DebuggingRecorder::new();
    let snapshot = recorder.snapshotter();
    let _metrics = metrics::set_default_local_recorder(&recorder);
    let native = NativeAccount::create().await?;
    let tag =
        clacks_model::tag::LogbookTag::new(clacks_model::ids::TagId::new(), "Activation".into());
    let key = RecordKey {
        account: native.first.account,
        id: RecordId::Tag(tag.id),
    };
    let create = native
        .prepare(
            &native.first,
            key,
            RecordState::Live(RecordValue::Tag(Box::new(tag.clone()))),
        )
        .await?;
    assert!(matches!(
        super::super::synchronization::accept_pending(&native.store, &native.first, &create)
            .await?,
        AcceptanceDecision::Commit(_)
    ));

    let mut known = Vec::new();
    for _ in 0..3 {
        let contact = qso(clacks_model::ids::QsoId::new(), [tag.id].into());
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
        assert!(matches!(
            super::super::synchronization::accept_pending(&native.store, &native.first, &change)
                .await?,
            AcceptanceDecision::Commit(_)
        ));
        known.push(key);
    }
    let delete_tag = native
        .prepare(&native.first, key, RecordState::Deleted)
        .await?;
    let fourth = qso(clacks_model::ids::QsoId::new(), [tag.id].into());
    let fourth_key = RecordKey {
        account: native.second.account,
        id: RecordId::Qso(fourth.id()),
    };
    let add_fourth = native
        .prepare(
            &native.second,
            fourth_key,
            RecordState::Live(RecordValue::Qso(Box::new(fourth.clone()))),
        )
        .await?;
    assert!(matches!(
        super::super::synchronization::accept_pending(&native.store, &native.second, &add_fourth)
            .await?,
        AcceptanceDecision::Commit(_)
    ));
    for key in known {
        let change = native
            .prepare(&native.first, key, RecordState::Deleted)
            .await?;
        assert!(matches!(
            super::super::synchronization::accept_pending(&native.store, &native.first, &change)
                .await?,
            AcceptanceDecision::Commit(_)
        ));
    }

    let blocked =
        super::super::synchronization::accept_pending(&native.store, &native.first, &delete_tag)
            .await?;
    assert!(
        matches!(blocked, AcceptanceDecision::Rejected(clacks_sync::acceptance::RejectionReason::Referenced { ref referrers, total: 1, .. }) if referrers == &[fourth_key]),
        "the fourth QSO survives; deleting three known QSOs must not authorize deleting its tag"
    );

    let fifth = qso(clacks_model::ids::QsoId::new(), [tag.id].into());
    let fifth_key = RecordKey {
        account: native.second.account,
        id: RecordId::Qso(fifth.id()),
    };
    let late_membership = native
        .prepare(
            &native.second,
            fifth_key,
            RecordState::Live(RecordValue::Qso(Box::new(fifth))),
        )
        .await?;
    let mut fields = fourth.into_fields();
    fields.tags.clear();
    let remove_fourth = native
        .prepare(
            &native.second,
            fourth_key,
            RecordState::Live(RecordValue::Qso(Box::new(fields.try_into()?))),
        )
        .await?;
    assert!(matches!(
        super::super::synchronization::accept_pending(
            &native.store,
            &native.second,
            &remove_fourth
        )
        .await?,
        AcceptanceDecision::Commit(_)
    ));
    assert!(matches!(
        super::super::synchronization::accept_pending(&native.store, &native.first, &delete_tag)
            .await?,
        AcceptanceDecision::Commit(_)
    ));
    let mut transaction = native.store.pool.begin().await?;
    let before_retry = super::super::metadata::account::load(&mut transaction, key.account).await?;
    transaction.commit().await?;
    assert!(matches!(
        super::super::synchronization::accept_pending(&native.store, &native.first, &delete_tag)
            .await?,
        AcceptanceDecision::Covered(_)
    ));
    let mut transaction = native.store.pool.begin().await?;
    assert_eq!(
        super::super::metadata::account::load(&mut transaction, key.account).await?,
        before_retry
    );
    transaction.commit().await?;
    assert!(matches!(
        super::super::synchronization::accept_pending(
            &native.store,
            &native.second,
            &late_membership
        )
        .await?,
        AcceptanceDecision::Stale(_)
    ));
    let mut read = native.store.pool.begin().await?;
    let current = super::super::metadata::account::load(&mut read, key.account).await?;
    let interpretation =
        super::super::metadata::account::interpretation(&mut read, key.account).await?;
    let clacks_sync::record::Observation::Accepted(distributed) =
        super::super::synchronization::context::observation(&mut read, &current, key).await?
    else {
        anyhow::bail!("missing distributed deletion");
    };
    read.commit().await?;
    crate::enrollment::revoke_in_store(&native.store.pool, &native.principal, native.first.actor)
        .await?;
    assert!(
        super::super::synchronization::accept_pending(&native.store, &native.first, &delete_tag)
            .await
            .is_err()
    );
    let page = clacks_sync::transfer::discovery::DiscoveryPage {
        sequence: 0,
        requirements: vec![],
        accepted: vec![distributed.clone()],
        support: vec![],
        baselines: interpretation.baselines,
        boundaries: vec![],
        units: vec![],
    };
    let returned =
        super::super::synchronization::receipt::receive(&native.store, &native.second, &page)
            .await?;
    assert!(
        returned.represented_count == 1,
        "revoking a writer must not reject accepted state carried by another authorized client"
    );
    let captured = std::fs::read_to_string(output.path())?;
    let events = captured
        .lines()
        .map(serde_json::from_str::<serde_json::Value>)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        events
            .iter()
            .filter(|value| value["fields"]["event"] == "sync.accepted")
            .count(),
        10
    );
    assert!(
        events
            .iter()
            .any(|value| value["fields"]["event"] == "sync.proposal.rejected"
                && value["level"] == "INFO")
    );
    assert!(
        !captured.contains("Activation"),
        "tag payload must not be logged"
    );
    let observations = snapshot.snapshot().into_vec();
    let (_, _, _, count) = observations
        .iter()
        .find(|(key, _, _, _)| key.key().name() == "clacks_server_sync_records_accepted_total")
        .ok_or_else(|| anyhow::anyhow!("missing committed acceptance counter"))?;
    assert_eq!(*count, metrics_util::debugging::DebugValue::Counter(10));
    Ok(())
}

#[tokio::test]
async fn account_defaults_are_accepted_once() -> anyhow::Result<()> {
    use super::support::synchronization::NativeAccount;
    use crate::store::discovery::Source;
    let native = NativeAccount::create().await?;

    let mut source = Source::start(&native.store, &native.first, None).await?;
    let before = source.start.source.clone();
    let mut records = Vec::new();
    while let Some(page) = source.page(&native.store, &native.first, 64).await? {
        records.extend(page.accepted);
    }
    source.end()?;
    let definitions = clacks_model::defaults::account_definitions()?;
    assert_eq!(
        records.len(),
        definitions.groups.len() + definitions.output_profiles.len()
    );
    for record in &records {
        assert_eq!(record.key().account, native.first.account);
        assert!(before.coverage.covers(record.stamp.accepted_at()));
    }
    for group in definitions.groups {
        assert!(records.iter().any(|record| matches!(record.state(), clacks_sync::record::RecordState::Live(clacks_sync::record::RecordValue::Group(value)) if **value == group)));
    }

    let identity = native
        .store
        .identities(&native.principal)
        .await?
        .into_iter()
        .next()
        .unwrap();
    native
        .store
        .provision_login(
            VerifiedIdentity {
                issuer: identity.issuer,
                subject: identity.subject,
                display_name: identity.display_name,
            },
            Duration::days(14),
        )
        .await?;

    let mut source = Source::start(&native.store, &native.first, None).await?;
    assert_eq!(source.start.source, before);

    let mut repeated = Vec::new();
    while let Some(page) = source.page(&native.store, &native.first, 64).await? {
        repeated.extend(page.accepted);
    }
    source.end()?;

    assert_eq!(repeated, records);
    Ok(())
}

#[tokio::test]
async fn discovery_keeps_fixed_target_while_heads_change() -> anyhow::Result<()> {
    use super::support::synchronization::NativeAccount;
    use clacks_model::{ids::TagId, tag::LogbookTag};
    use clacks_sync::{
        acceptance::AcceptanceDecision,
        record::{RecordId, RecordKey, RecordState, RecordValue},
    };
    let native = NativeAccount::create().await?;
    let mut keys = Vec::new();
    for name in ["A", "B", "C"] {
        let tag = LogbookTag::new(TagId::new(), name.into());
        let key = RecordKey {
            account: native.first.account,
            id: RecordId::Tag(tag.id),
        };
        let change = native
            .prepare(
                &native.first,
                key,
                RecordState::Live(RecordValue::Tag(Box::new(tag))),
            )
            .await?;
        assert!(matches!(
            super::super::synchronization::accept_pending(&native.store, &native.first, &change)
                .await?,
            AcceptanceDecision::Commit(_)
        ));
        keys.push(key);
    }
    let mut source =
        super::super::discovery::Source::start(&native.store, &native.first, None).await?;
    let target = source.start.source.clone();
    let late = LogbookTag::new(
        ulid::Ulid::from(1).to_string().parse()?,
        "D behind cursor".into(),
    );
    let late_key = RecordKey {
        account: native.first.account,
        id: RecordId::Tag(late.id),
    };
    let mut received = Vec::new();
    while let Some(page) = source.page(&native.store, &native.first, 1).await? {
        let saw_a = page
            .accepted
            .iter()
            .any(|value| value.key() == *keys.first().unwrap());
        received.extend(page.accepted);
        if saw_a {
            for key in keys.iter().take(2) {
                let RecordId::Tag(id) = key.id else {
                    unreachable!()
                };
                let change = native
                    .prepare(
                        &native.second,
                        *key,
                        RecordState::Live(RecordValue::Tag(Box::new(LogbookTag::new(
                            id,
                            "Corrected".into(),
                        )))),
                    )
                    .await?;
                assert!(matches!(
                    super::super::synchronization::accept_pending(
                        &native.store,
                        &native.second,
                        &change
                    )
                    .await?,
                    AcceptanceDecision::Commit(_)
                ));
            }
            let key = *keys.last().unwrap();
            let change = native
                .prepare(&native.second, key, RecordState::Deleted)
                .await?;
            assert!(matches!(
                super::super::synchronization::accept_pending(
                    &native.store,
                    &native.second,
                    &change
                )
                .await?,
                AcceptanceDecision::Commit(_)
            ));
            let change = native
                .prepare(
                    &native.second,
                    late_key,
                    RecordState::Live(RecordValue::Tag(Box::new(late.clone()))),
                )
                .await?;
            assert!(matches!(
                super::super::synchronization::accept_pending(
                    &native.store,
                    &native.second,
                    &change
                )
                .await?,
                AcceptanceDecision::Commit(_)
            ));
        }
    }
    assert_eq!(source.end()?.source, target);
    for key in &keys {
        assert!(received.iter().any(|value| value.key() == *key));
    }
    assert!(
        received
            .iter()
            .any(|value| value.key() == *keys.last().unwrap()
                && matches!(value.state(), RecordState::Deleted))
    );
    let first = received
        .iter()
        .find(|value| value.key() == *keys.first().unwrap())
        .unwrap();
    assert!(target.coverage.covers(first.stamp.accepted_at()));
    let second = received
        .iter()
        .find(|value| value.key() == *keys.get(1).unwrap())
        .unwrap();
    assert!(!target.coverage.covers(second.stamp.accepted_at()));
    assert!(!received.iter().any(|value| value.key() == late_key));
    let mut journal =
        super::super::discovery::Source::start(&native.store, &native.first, Some(&target)).await?;
    assert!(matches!(
        journal.start.mode,
        clacks_sync::transfer::WindowMode::Incremental { .. }
    ));
    let mut changes = Vec::new();
    while let Some(page) = journal.page(&native.store, &native.first, 1).await? {
        changes.extend(page.requirements);
    }
    assert_eq!(changes.len(), 4);
    assert!(changes.iter().any(|value| value.key == late_key));
    let mut unknown = target.clone();
    unknown.baseline = clacks_model::ids::BaselineId::new();
    let fallback =
        super::super::discovery::Source::start(&native.store, &native.first, Some(&unknown))
            .await?;
    assert_eq!(fallback.start.mode, clacks_sync::transfer::WindowMode::Full);
    Ok(())
}

#[tokio::test]
async fn returned_coupled_effects_wait_for_support_and_keep_original_stamps() -> anyhow::Result<()>
{
    use super::support::synchronization::{NativeAccount, qso};
    use clacks_model::{
        ids::{OperationId, TagId},
        tag::LogbookTag,
    };
    use clacks_sync::{
        acceptance::{AcceptanceDecision, AcceptanceInput, ServerChange},
        client::VersionCondition,
        record::{Observation, RecordKey, RecordState, RecordValue},
        service::{VersionReference, operation::CoupledRequirement},
        transfer::discovery::DiscoveryPage,
    };
    let native = NativeAccount::create().await?;
    let owner = native.first.account;
    let mut read = native.store.pool.begin().await?;
    let mut account = super::super::metadata::account::load(&mut read, owner).await?;
    let restored = account.clone();
    let membership = crate::enrollment::membership::snapshot(&mut read, &account).await?;
    let interpretation = super::super::metadata::account::interpretation(&mut read, owner).await?;
    read.commit().await?;
    let tag = LogbookTag::new(TagId::new(), "Returned activation".into());
    let contact = qso(clacks_model::ids::QsoId::new(), [tag.id].into());
    let mut accepted = Vec::new();
    for value in [
        RecordValue::Tag(Box::new(tag)),
        RecordValue::Qso(Box::new(contact)),
    ] {
        let key = RecordKey {
            account: owner,
            id: value.id(),
        };
        let absent = Observation::Absent {
            key,
            checkpoint: super::super::synchronization::checkpoint(&account),
        };
        let references: Vec<_> = accepted
            .iter()
            .cloned()
            .map(Observation::Accepted)
            .collect();
        let dependencies = references
            .iter()
            .map(VersionCondition::from_observation)
            .collect::<anyhow::Result<Vec<_>>>()?;
        let change = ServerChange {
            key,
            state: RecordState::Live(value),
            expected: VersionCondition::from_observation(&absent)?,
            dependencies,
        };
        let input = AcceptanceInput {
            destination_registry: None,
            bindings: &[],
            account: &account,
            membership: &membership,
            current: &absent,
            references: &references,
        };
        let AcceptanceDecision::Commit(commit) = clacks_sync::acceptance::plan_server_acceptance(
            &change,
            &input,
            &interpretation.view(),
        )?
        else {
            anyhow::bail!("valid pre-restore fixture was not accepted");
        };
        accepted.push(commit.accepted);
        account = commit.next_account;
    }
    // The returned values are produced by the native acceptance planner. PostgreSQL still
    // contains the earlier state, modeling an older restored store without schema injection.
    let source = super::super::synchronization::checkpoint(&account);
    super::super::synchronization::recovery::negotiate(
        &native.store,
        &native.first,
        Some(&source),
        &interpretation.baselines,
        Some(&interpretation.lineage),
    )
    .await?;
    let unit = CoupledRequirement {
        id: OperationId::new(),
        members: accepted.iter().map(VersionReference::from).collect(),
    };
    let first = accepted.first().unwrap().clone();
    let second = accepted.last().unwrap().clone();
    let page = DiscoveryPage {
        sequence: 0,
        requirements: vec![],
        accepted: vec![first.clone()],
        support: vec![],
        baselines: interpretation.baselines.clone(),
        boundaries: vec![],
        units: vec![unit],
    };
    let partial =
        super::super::synchronization::receipt::receive(&native.store, &native.first, &page)
            .await?;
    assert_eq!(partial.represented_count, 0);
    assert_eq!(partial.staged, vec![first.clone()]);
    let mut read = native.store.pool.begin().await?;
    let current = super::super::metadata::account::load(&mut read, owner).await?;
    assert_ne!(current.head.timeline, restored.head.timeline);
    assert_eq!(current.head.revision, 0);
    assert!(matches!(
        super::super::synchronization::context::observation(&mut read, &current, first.key())
            .await?,
        Observation::Absent { .. }
    ));
    read.commit().await?;
    let mut remainder = page.clone();
    remainder.sequence = 1;
    remainder.accepted = vec![second.clone()];
    let completed =
        super::super::synchronization::receipt::receive(&native.store, &native.first, &remainder)
            .await?;
    assert!(completed.staged.is_empty());
    assert_eq!(completed.completed_units.len(), 1);
    assert_eq!(completed.represented_count, 2);
    let mut read = native.store.pool.begin().await?;
    let current = super::super::metadata::account::load(&mut read, owner).await?;
    assert_eq!(current.head.revision, 2);
    assert!(
        !current.installed.covers(second.stamp.accepted_at()),
        "individually returned facts do not certify the old complete prefix"
    );
    for value in [first, second] {
        assert_eq!(
            super::super::synchronization::context::observation(&mut read, &current, value.key())
                .await?,
            Observation::Accepted(value)
        );
    }
    read.commit().await?;
    Ok(())
}
