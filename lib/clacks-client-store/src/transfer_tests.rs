//! Committed transfer staging counts exclude replayed inputs.

use crate::test_support as support;
#[path = "../tests/support/telemetry.rs"]
mod telemetry;
use crate::store::{ClientStore, StoreSettings};

#[test]
fn staged_input_counts_new_commits_without_counting_retries() -> anyhow::Result<()> {
    use crate::transfer;
    use clacks_sync::{
        account::{AccountMetadata, InstalledCheckpoint},
        lineage::Coverage,
        service::{VersionReference, operation::CoupledRequirement},
        transfer::{RecordRequirement, WindowManifest, WindowMode},
    };
    let captured = telemetry::capture(|| -> anyhow::Result<()> {
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
                let manifest = WindowManifest::new(
                    source.account,
                    source.clone(),
                    WindowMode::Full,
                    [&first, &second]
                        .into_iter()
                        .map(|row| RecordRequirement {
                            key: row.key(),
                            stamp: row.stamp.clone(),
                            deleted: false,
                        })
                        .collect(),
                    vec![],
                    vec![baseline],
                    vec![CoupledRequirement {
                        id: support::id(90),
                        members: [&first, &second]
                            .into_iter()
                            .map(VersionReference::from)
                            .collect(),
                    }],
                )?;
                transfer::remember(&store, &manifest).await?;
                let staged = transfer::install(
                    &store,
                    &source,
                    &WindowMode::Full,
                    std::slice::from_ref(&first),
                )
                .await?;
                assert_eq!(staged.staged, vec![first.clone()]);
                for input in [std::slice::from_ref(&first), &[]] {
                    let retry =
                        transfer::install(&store, &source, &WindowMode::Full, input).await?;
                    assert_eq!(retry.staged, vec![first.clone()]);
                }
                let complete =
                    transfer::install(&store, &source, &WindowMode::Full, &[second]).await?;
                assert!(complete.staged.is_empty());
                assert_eq!(complete.accepted.len(), 2);
                transfer::finish(&store, &source, &WindowMode::Full).await?;
                store.close().await?;
                Ok(())
            })
    });
    captured
        .result
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error:#}"))?;
    captured.counter("clacks_client_sync_records_staged_total", &[], 1);
    captured.counter("clacks_client_sync_records_installed_total", &[], 2);
    Ok(())
}
