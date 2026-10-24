#[path = "support/process.rs"]
mod process;
mod support;
use clacks_client_store::{
    identity::{self, EnrollmentBinding},
    save,
    store::{ClientStore, StoreSettings},
    synchronization::{self, PrepareOutcome},
    transfer,
};
use clacks_sync::{
    acceptance::{AcceptanceDecision, AcceptanceInput, ServerChange, plan_server_acceptance},
    account::{AccountMetadata, EvidenceView, InstalledCheckpoint},
    client::{Preparation, VersionCondition},
    membership::MembershipSnapshot,
    record::{Observation, RecordState, RecordValue},
    service::{VersionReference, operation::CoupledRequirement},
    transfer::{RecordRequirement, WindowManifest, WindowMode},
};

fn pair() -> anyhow::Result<(WindowManifest, Vec<clacks_sync::record::AcceptedRecord>)> {
    let (mut account, baseline) =
        AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
    let bases = [baseline.clone()];
    let evidence = EvidenceView {
        lineage: &baseline.lineage,
        baselines: &bases,
    };
    let membership = MembershipSnapshot {
        account: account.account,
        generation: 0,
        clients: vec![],
    };
    let mut accepted = vec![];
    for state in [
        support::contact(support::qso(support::id(10))),
        RecordState::Live(RecordValue::Tag(Box::new(
            clacks_model::tag::LogbookTag::new(support::id(20), "Fixture tag".into()),
        ))),
    ] {
        let RecordState::Live(value) = &state else {
            panic!("live fixture")
        };
        let key = support::key(value);
        let current = Observation::Absent {
            key,
            checkpoint: InstalledCheckpoint {
                account: account.account,
                baseline: baseline.id,
                coverage: account.installed.clone(),
            },
        };
        let change = ServerChange {
            key,
            state,
            expected: VersionCondition::from_observation(&current)?,
            dependencies: vec![],
        };
        let AcceptanceDecision::Commit(commit) = plan_server_acceptance(
            &change,
            &AcceptanceInput {
                destination_registry: None,
                bindings: &[],
                account: &account,
                membership: &membership,
                current: &current,
                references: &[],
            },
            &evidence,
        )?
        else {
            panic!("source commit")
        };
        account = commit.next_account;
        accepted.push(commit.accepted);
    }
    let source = InstalledCheckpoint {
        account: account.account,
        baseline: baseline.id,
        coverage: account.installed,
    };
    let manifest = WindowManifest::new(
        source.account,
        source,
        WindowMode::Full,
        accepted
            .iter()
            .map(|value| RecordRequirement {
                key: value.key,
                stamp: value.stamp.clone(),
                deleted: false,
            })
            .collect(),
        vec![],
        bases.to_vec(),
        vec![CoupledRequirement {
            id: support::id(90),
            members: accepted.iter().map(VersionReference::from).collect(),
        }],
    )?;
    Ok((manifest, accepted))
}

#[test]
#[ignore = "invoked only by the parent interruption test"]
fn interruption_child() -> anyhow::Result<()> {
    let file = std::path::PathBuf::from(
        std::env::var_os("CLACKS_TEST_DATABASE")
            .ok_or_else(|| anyhow::anyhow!("parent did not supply a database"))?,
    );
    let phase = std::env::var("CLACKS_TEST_PHASE")?;
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let store =
                ClientStore::create(&file, support::id(1), StoreSettings::default()).await?;
            if phase == "saved" {
                save::save(
                    &store,
                    support::qso_key(support::id(10)),
                    None,
                    support::contact(support::qso(support::id(10))),
                )
                .await?;
            } else if phase == "prepared" {
                identity::bind(
                    &store,
                    None,
                    EnrollmentBinding {
                        account: support::id(1),
                        actor: support::id(9),
                        server: "https://example.invalid".into(),
                        credential_reference: "protected-reference".into(),
                    },
                )
                .await?;
                let (account, baseline) =
                    AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
                let source = InstalledCheckpoint {
                    account: account.account,
                    baseline: baseline.id,
                    coverage: account.installed,
                };
                let manifest = WindowManifest::new(
                    source.account,
                    source.clone(),
                    WindowMode::Full,
                    vec![],
                    vec![],
                    vec![baseline],
                    vec![],
                )?;
                transfer::remember(&store, &manifest).await?;
                transfer::finish(&store, &source, &WindowMode::Full).await?;
                save::save(
                    &store,
                    support::qso_key(support::id(10)),
                    None,
                    support::contact(support::qso(support::id(10))),
                )
                .await?;
                let PrepareOutcome::Core(value) =
                    synchronization::prepare(&store, support::qso_key(support::id(10))).await?
                else {
                    panic!("enrolled")
                };
                assert!(matches!(*value, Preparation::Ready { .. }));
            } else {
                let (manifest, accepted) = pair()?;
                transfer::remember(&store, &manifest).await?;
                transfer::install(
                    &store,
                    &manifest.source,
                    &manifest.mode,
                    std::slice::from_ref(accepted.first().unwrap()),
                )
                .await?;
                if phase == "installed" {
                    transfer::install(
                        &store,
                        &manifest.source,
                        &manifest.mode,
                        std::slice::from_ref(accepted.last().unwrap()),
                    )
                    .await?;
                }
            }
            std::fs::write(file.with_extension("ready"), b"committed")?;
            std::future::pending::<()>().await;
            Ok::<_, anyhow::Error>(())
        })
}

#[test]
fn process_death_preserves_committed_values_frozen_proposals_and_window_boundaries()
-> anyhow::Result<()> {
    for phase in ["saved", "prepared", "staged", "installed"] {
        let directory = tempfile::tempdir()?;
        let file = directory.path().join("workspace.sqlite");
        let mut child = process::InterruptedChild::start("interruption_child", &file, phase)?;
        child.wait_ready(&file.with_extension("ready"))?;
        child.kill()?;
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async {
                let store =
                    ClientStore::open(&file, support::id(1), StoreSettings::default()).await?;
                let row = save::load(&store, support::qso_key(support::id(10))).await?;
                match phase {
                    "saved" => assert!(row.unwrap().pending.is_some()),
                    "prepared" => {
                        let prepared = row.unwrap().pending.unwrap().prepared.unwrap();
                        let PrepareOutcome::Core(result) =
                            synchronization::prepare(&store, prepared.key).await?
                        else {
                            panic!("enrolled")
                        };
                        let Preparation::Retry { change, .. } = *result else {
                            panic!("retry retained proposal")
                        };
                        assert_eq!(change, prepared);
                    }
                    "staged" | "installed" => {
                        let (manifest, accepted) = pair()?;
                        let progress =
                            transfer::progress(&store, &manifest.source, &manifest.mode).await?;
                        let mut snapshot = clacks_client_store::read::snapshot(&store).await?;
                        assert!(snapshot.checkpoint().await?.is_none());
                        snapshot.close().await?;
                        if phase == "staged" {
                            assert!(row.is_none());
                            assert_eq!(progress.staged.len(), 1);
                            transfer::install(
                                &store,
                                &manifest.source,
                                &manifest.mode,
                                std::slice::from_ref(accepted.last().unwrap()),
                            )
                            .await?;
                        } else {
                            assert!(row.is_some());
                            assert!(progress.staged.is_empty());
                            assert_eq!(progress.completed_units.len(), 1);
                        }
                        assert_eq!(
                            transfer::finish(&store, &manifest.source, &manifest.mode).await?,
                            manifest.source
                        );
                    }
                    _ => unreachable!(),
                }
                store.close().await?;
                Ok::<_, anyhow::Error>(())
            })?;
    }
    Ok(())
}
