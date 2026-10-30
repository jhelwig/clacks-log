use crate::logbook_support;
use crate::support;
use crate::telemetry;

use clacks_client_store::{
    logbooks::{
        deletion,
        selection::{self, SelectedView},
        tags,
    },
    save::{self, SaveOutcome},
};
use clacks_model::tag::{LogbookTag, TagDeletionChoice};
use logbook_support::Workspace;
use support::{contact, id, qso, qso_key};

#[test]
fn tag_telemetry_counts_commits_without_counting_stale_or_unchanged_saves() -> anyhow::Result<()> {
    let capture = telemetry::capture_filtered(
        || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(async {
                    let workspace = Workspace::new().await?;
                    let mut tag = LogbookTag::new(id(20), "SECRET LOGBOOK NAME".into());
                    let SaveOutcome::Saved(first) =
                        tags::save(&workspace.store, None, &tag).await?
                    else {
                        panic!("create");
                    };
                    tag.name = "PRIVATE RENAMED LOGBOOK".into();
                    let SaveOutcome::Saved(updated) =
                        tags::save(&workspace.store, Some(&first), &tag).await?
                    else {
                        panic!("update");
                    };
                    assert!(matches!(
                        tags::save(&workspace.store, Some(&updated), &tag).await?,
                        SaveOutcome::Unchanged(_)
                    ));
                    assert!(matches!(
                        tags::save(&workspace.store, Some(&first), &tag).await?,
                        SaveOutcome::Stale { .. }
                    ));
                    selection::select(&workspace.store, SelectedView::Logbook(tag.id)).await?;
                    for n in [100, 101] {
                        let mut value = qso(id(n));
                        support::edit(&mut value, |value| {
                            value.comment = Some("SECRET ON-AIR COMMENT".into());
                            Ok(())
                        });
                        support::edit(&mut value, |value| {
                            value.tags = if n == 100 {
                                [tag.id].into()
                            } else {
                                [tag.id, id(21)].into()
                            };
                            Ok(())
                        });
                        save::save(&workspace.store, qso_key(value.id()), None, contact(value))
                            .await?;
                    }
                    let preview = deletion::preview(&workspace.store, tag.id).await?;
                    deletion::delete_tag(
                        &workspace.store,
                        &preview,
                        TagDeletionChoice::DeleteUnique,
                    )
                    .await?;
                    workspace.store.close().await?;
                    anyhow::Ok(())
                })
        },
        |metadata| *metadata.level() <= tracing::Level::INFO,
    );
    capture
        .result
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error:#}"))?;
    for change in ["create", "update", "delete"] {
        capture.counter(
            "clacks_client_tag_mutations_total",
            &[("change", change)],
            1,
        );
    }
    capture.counter(
        "clacks_client_qso_mutations_total",
        &[("change", "create")],
        2,
    );
    for change in ["update", "delete"] {
        capture.counter(
            "clacks_client_qso_mutations_total",
            &[("change", change)],
            1,
        );
    }
    let deleted = capture.event("tag.deleted");
    assert_eq!(
        deleted.fields.get("deleted_contacts").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        deleted
            .fields
            .get("changed_memberships")
            .map(String::as_str),
        Some("1")
    );
    assert!(deleted.fields.contains_key("fallback_from"));
    let selected = capture.event("logbook.view.selected");
    assert_eq!(
        selected.fields.get("account_id"),
        Some(&id::<clacks_model::ids::AccountId>(1).to_string())
    );
    let observations = format!("{:?}", capture.observations);
    for sensitive in [
        "SECRET LOGBOOK NAME",
        "PRIVATE RENAMED LOGBOOK",
        "SECRET ON-AIR COMMENT",
        "W7HEX/P",
        "K1ABC",
    ] {
        assert!(!observations.contains(sensitive), "leaked {sensitive}");
    }
    Ok(())
}

#[test]
fn stale_deletion_reports_tag_or_members_without_counting_deletions() -> anyhow::Result<()> {
    let capture = telemetry::capture(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(async {
                let workspace = Workspace::new().await?;
                let mut tag = LogbookTag::new(id(20), "PRIVATE TAG".into());
                let SaveOutcome::Saved(original_tag) =
                    tags::save(&workspace.store, None, &tag).await?
                else {
                    panic!("create tag");
                };
                let mut value = qso(id(100));
                support::edit(&mut value, |value| {
                    value.tags.insert(tag.id);
                    Ok(())
                });
                let SaveOutcome::Saved(original_qso) = save::save(
                    &workspace.store,
                    qso_key(value.id()),
                    None,
                    contact(value.clone()),
                )
                .await?
                else {
                    panic!("create contact");
                };

                let preview = deletion::preview(&workspace.store, tag.id).await?;
                tag.name = "PRIVATE RENAMED TAG".into();
                tags::save(&workspace.store, Some(&original_tag), &tag).await?;
                assert!(matches!(
                    deletion::delete_tag(&workspace.store, &preview, TagDeletionChoice::DeleteAll)
                        .await?,
                    deletion::TagDeleteOutcome::Stale { .. }
                ));

                let preview = deletion::preview(&workspace.store, tag.id).await?;
                support::edit(&mut value, |value| {
                    value.comment = Some("PRIVATE COMMENT".into());
                    Ok(())
                });
                save::save(
                    &workspace.store,
                    qso_key(value.id()),
                    Some(&original_qso),
                    contact(value),
                )
                .await?;
                assert!(matches!(
                    deletion::delete_tag(&workspace.store, &preview, TagDeletionChoice::DeleteAll)
                        .await?,
                    deletion::TagDeleteOutcome::Stale { .. }
                ));
                workspace.store.close().await?;
                anyhow::Ok(())
            })
    });
    capture
        .result
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error:#}"))?;

    let stale: Vec<_> = capture
        .observations
        .events
        .iter()
        .filter(|event| {
            event
                .fields
                .get("event")
                .is_some_and(|name| name == "tag.deletion_stale")
        })
        .collect();
    assert_eq!(stale.len(), 2);
    let reasons: std::collections::BTreeSet<_> = stale
        .iter()
        .map(|event| event.fields.get("reason").unwrap().as_str())
        .collect();
    assert_eq!(reasons, ["tag_changed", "members_changed"].into());
    for event in stale {
        assert_eq!(
            event.fields.get("account_id"),
            Some(&id::<clacks_model::ids::AccountId>(1).to_string())
        );
        assert_eq!(
            event.fields.get("tag_id"),
            Some(&id::<clacks_model::ids::TagId>(20).to_string())
        );
    }
    assert!(!capture.metrics.iter().any(|metric| {
        metric
            .labels
            .get("change")
            .is_some_and(|change| change == "delete")
    }));
    let observations = format!("{:?}", capture.observations);
    for private in ["PRIVATE TAG", "PRIVATE RENAMED TAG", "PRIVATE COMMENT"] {
        assert!(!observations.contains(private));
    }
    Ok(())
}
