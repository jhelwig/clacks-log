//! Committed transfer staging counts exclude replayed inputs.

use crate::store::{ClientStore, StoreSettings};
use crate::test_support as support;

#[test]
fn staged_input_counts_new_commits_without_counting_retries() -> anyhow::Result<()> {
    use clacks_sync::{
        account::{AccountMetadata, InstalledCheckpoint},
        lineage::Coverage,
        service::{VersionReference, operation::CoupledRequirement},
        transfer::{RecordRequirement, WindowMode},
    };
    let recorder = metrics_util::debugging::DebuggingRecorder::new();
    let snapshot = recorder.snapshotter();
    let _metrics = metrics::set_default_local_recorder(&recorder);

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let directory = tempfile::tempdir()?;
            let store = ClientStore::create(
                &directory.path().join("workspace.sqlite"),
                support::id(1),
                StoreSettings::default(),
            )
            .await?;
            let (_, baseline) =
                AccountMetadata::initialize(support::id(1), support::id(2), support::id(3));
            let first = support::accepted(1, support::contact(support::qso(support::id(10))));
            let second = support::accepted(2, support::contact(support::qso(support::id(11))));
            let source = InstalledCheckpoint {
                account: support::id(1),
                baseline: baseline.id,
                coverage: Coverage::from(second.stamp.accepted_at()),
            };
            let manifest = crate::incoming::Source {
                start: clacks_sync::transfer::discovery::DiscoveryStart {
                    source: source.clone(),
                    mode: WindowMode::Full,
                },
                page: clacks_sync::transfer::discovery::DiscoveryPage {
                    sequence: 0,
                    requirements: [&first, &second]
                        .into_iter()
                        .map(|row| RecordRequirement {
                            key: row.key(),
                            stamp: row.stamp.clone(),
                            deleted: false,
                        })
                        .collect(),
                    boundaries: vec![],
                    baselines: vec![baseline],
                    units: vec![CoupledRequirement {
                        id: support::id(90),
                        members: [&first, &second]
                            .into_iter()
                            .map(VersionReference::from)
                            .collect(),
                    }],
                    accepted: vec![],
                    support: vec![],
                },
            };
            let mut incoming = crate::incoming::Incoming::begin(&store, &manifest).await?;
            incoming.page(&store, std::slice::from_ref(&first)).await?;
            assert!(crate::save::load(&store, first.key()).await?.is_none());
            for input in [std::slice::from_ref(&first), &[]] {
                incoming.page(&store, input).await?;
                assert!(crate::save::load(&store, first.key()).await?.is_none());
            }
            incoming.page(&store, &[second]).await?;
            assert!(crate::save::load(&store, first.key()).await?.is_some());
            incoming.finish(&store).await?;
            store.close().await?;
            Ok::<(), anyhow::Error>(())
        })?;
    let metrics = snapshot.snapshot().into_vec();
    for (name, expected) in [
        ("clacks_client_sync_records_staged_total", 1),
        ("clacks_client_sync_records_installed_total", 2),
    ] {
        let (key, _, _, value) = metrics
            .iter()
            .find(|(key, _, _, _)| key.key().name() == name)
            .ok_or_else(|| anyhow::anyhow!("missing committed counter {name}"))?;
        assert_eq!(key.key().labels().count(), 0);
        assert!(
            matches!(value, metrics_util::debugging::DebugValue::Counter(count) if *count == expected)
        );
    }
    Ok(())
}
