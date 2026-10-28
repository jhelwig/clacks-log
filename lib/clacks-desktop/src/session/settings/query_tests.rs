//! Query reporting and cancellation at the runtime's actual admission boundary.

use super::*;
use clacks_client_store::read::{
    contacts::ContactPageQuery, query_state::QueryState, worked_before::WorkedBeforeQuery,
};
use metrics_util::debugging::{DebugValue, DebuggingRecorder};

#[test]
fn query_metrics_count_completed_reads_and_only_read_failures() -> anyhow::Result<()> {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || -> anyhow::Result<()> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async {
                let directory = tempfile::tempdir()?;
                let store = ClientStore::create(
                    &directory.path().join("query.sqlite"),
                    Default::default(),
                    Default::default(),
                )
                .await?;
                let filter = clacks_model::query::parse(
                    "comment:PRIVATE_QUERY_SENTINEL",
                    clacks_model::recognition::RecognitionCatalog::adif_318()?,
                )?;
                for action in [
                    contacts::ContactAction::Page {
                        query: ContactPageQuery {
                            filter,
                            ..Default::default()
                        },
                    },
                    contacts::ContactAction::WorkedBefore {
                        query: WorkedBeforeQuery {
                            call_prefix: "PRIVATE_QUERY_SENTINEL".into(),
                            ..Default::default()
                        },
                    },
                    contacts::ContactAction::Page {
                        query: ContactPageQuery {
                            limit: 0,
                            ..Default::default()
                        },
                    },
                ] {
                    let (reply, _receiver) = oneshot::channel();
                    let completion = execute(
                        store.clone(),
                        SettingsRequest {
                            action: SettingsAction::Contacts(action),
                            reply,
                        },
                    )
                    .await
                    .unwrap();
                    assert!(completion.snapshot.is_none());
                }
                // Discarded reads are cancelled; an admitted write still commits after dismissal.
                let (reply, receiver) = oneshot::channel();
                drop(receiver);
                assert!(
                    execute(
                        store.clone(),
                        SettingsRequest {
                            action: SettingsAction::Contacts(contacts::ContactAction::Page {
                                query: Default::default()
                            }),
                            reply
                        }
                    )
                    .await
                    .is_none()
                );
                let state = QueryState {
                    edited: "unfinished private text".into(),
                    applied: String::new(),
                };
                let (reply, receiver) = oneshot::channel();
                drop(receiver);
                assert!(
                    execute(
                        store.clone(),
                        SettingsRequest {
                            action: SettingsAction::Contacts(
                                contacts::ContactAction::SaveQueryState {
                                    state: state.clone()
                                }
                            ),
                            reply
                        }
                    )
                    .await
                    .is_some()
                );
                let mut read = clacks_client_store::read::snapshot(&store).await?;
                assert_eq!(read.history_query_state().await?, state);
                read.close().await?;
                store.close().await?;
                Ok(())
            })
    })?;
    let metrics = snapshotter.snapshot().into_vec();
    let queries: Vec<_> = metrics
        .iter()
        .filter(|(key, _, _, _)| key.key().name().starts_with("client_store.contact_query."))
        .collect();
    assert_eq!(queries.len(), 3);
    for (key, _, _, value) in queries {
        let labels: Vec<_> = key.key().labels().map(|l| (l.key(), l.value())).collect();
        let kind = labels.first().unwrap().1;
        assert_eq!(labels, vec![("kind", kind)]);
        assert!(matches!(kind, "history" | "worked_before"));
        match value {
            DebugValue::Counter(count) => {
                assert_eq!(
                    key.key().name(),
                    "client_store.contact_query.failures_total"
                );
                assert_eq!(kind, "history");
                assert_eq!(*count, 1);
            }
            DebugValue::Histogram(values) => {
                assert_eq!(values.len(), if kind == "history" { 2 } else { 1 })
            }
            _ => panic!("unexpected query metric"),
        }
    }
    Ok(())
}
