//! Staged record dependencies and overlapping deletion effects in a real workspace.

use super::*;
use clacks_client_store::save;
use clacks_model::profile::{EquipmentConfigurationFields, SavedLocationProfileFields};

async fn workspace() -> anyhow::Result<(tempfile::TempDir, ClientStore)> {
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("settings.sqlite"),
        Default::default(),
        Default::default(),
    )
    .await?;
    Ok((directory, store))
}

fn rig() -> anyhow::Result<EquipmentConfiguration> {
    EquipmentConfigurationFields {
        id: Default::default(),
        name: "Field kit".into(),
        values: clacks_model::equipment::EquipmentValuesFields {
            equipment_description: Some("KX3".into()),
            ..Default::default()
        },
    }
    .try_into()
}

fn home(default_equipment: EquipmentConfigurationId) -> anyhow::Result<SavedLocationProfile> {
    SavedLocationProfileFields {
        id: Default::default(),
        name: "Home".into(),
        station: clacks_model::contact::StationSnapshot {
            callsign: clacks_model::scalar::RecordedCallsign::new("W7HEX")?,
            location: None,
            references: Default::default(),
        },
        default_equipment: Some(default_equipment),
    }
    .try_into()
}

#[tokio::test]
async fn settings_apply_orders_staged_equipment_and_profiles() -> anyhow::Result<()> {
    let (_directory, store) = workspace().await?;
    let tag = LogbookTag::new(Default::default(), "Old name".into());
    let SaveOutcome::Saved(tag_basis) = tags::save(&store, None, &tag).await? else {
        anyhow::bail!("fixture logbook should be new")
    };
    for stale in [false, true] {
        let equipment = rig()?;
        let profile = home(equipment.id())?;
        let mut changes = AccountChanges::default();
        let mut expected = None;
        if stale {
            let SaveOutcome::Saved(created) =
                settings::save_equipment(&store, None, &equipment).await?
            else {
                anyhow::bail!("fixture equipment should be new")
            };
            let preview = settings::preview_equipment_deletion(&store, equipment.id()).await?;
            settings::delete_equipment(&store, &preview, &[]).await?;
            expected = Some(created);
            changes.tags.insert(
                tag.id,
                Edit {
                    expected: Some(tag_basis.clone()),
                    next: LogbookTag {
                        name: "New name".into(),
                        ..tag.clone()
                    },
                },
            );
        }
        changes.equipment.insert(
            equipment.id(),
            Edit {
                expected,
                next: equipment.clone(),
            },
        );
        changes.profiles.insert(
            profile.id(),
            Edit {
                expected: None,
                next: profile.clone(),
            },
        );

        let result = execute(&store, changes).await;
        let diag = result
            .outcomes
            .get(&RecordId::Profile(profile.id()))
            .and_then(AccountOutcome::diagnostic);
        assert_eq!(
            diag.is_some(),
            stale,
            "only a missing equipment default requires repair"
        );
        let snapshot = settings::snapshot(&store).await?;
        assert!(snapshot.profiles.iter().any(|r| matches!(r.working_state(),
            Some(RecordState::Live(RecordValue::Profile(v))) if **v == profile)));
        if stale {
            assert!(
                snapshot
                    .logbooks
                    .tags
                    .iter()
                    .any(|r| matches!(r.working_state(),
                Some(RecordState::Live(RecordValue::Tag(v))) if v.name == "New name"))
            );
        }
    }
    store.close().await
}

#[tokio::test]
async fn settings_apply_revalidates_overlapping_deletion_reviews() -> anyhow::Result<()> {
    let (_directory, store) = workspace().await?;
    for changed_after_review in [false, true] {
        let a = LogbookTag::new(Default::default(), "Home".into());
        let b = LogbookTag::new(Default::default(), "Portable".into());
        for tag in [&a, &b] {
            tags::save(&store, None, tag).await?;
        }
        clacks_client_store::logbooks::selection::select(&store, SelectedView::Logbook(b.id))
            .await?;
        let qso = crate::directory::lookup::tests::support::contact(Default::default(), "W7HEX");
        let mut fields = qso.into_fields();
        fields.tags = [a.id, b.id].into();
        let SaveOutcome::Saved(original) =
            save::contacts::save(&store, None, fields.try_into()?, None).await?
        else {
            anyhow::bail!("fixture contact should be new")
        };
        let mut changes = AccountChanges::default();
        for (tag, choice) in [
            (&a, TagDeletionChoice::LeaveAll),
            (&b, TagDeletionChoice::DeleteUnique),
        ] {
            let review =
                changes.review_tag(&Arc::new(deletion::preview(&store, tag.id).await?), choice)?;
            if tag.id == b.id {
                assert_eq!(review.counts.unique, 1);
            }
            changes.tag_deletions.push(review);
        }
        if changed_after_review {
            let Some(RecordState::Live(RecordValue::Qso(qso))) = original.working_state() else {
                anyhow::bail!("the newly created tag member should be a live QSO")
            };
            let mut fields = (**qso).clone().into_fields();
            fields.comment = Some("Operator's later correction".into());
            save::contacts::save(&store, Some(&original), fields.try_into()?, None).await?;
            let independent = LogbookTag::new(Default::default(), "A separate logbook".into());
            changes.tags.insert(
                independent.id,
                Edit {
                    expected: None,
                    next: independent.clone(),
                },
            );

            let result = execute(&store, changes).await;
            for tag in [&a, &b] {
                assert!(matches!(
                    result.outcomes.get(&RecordId::Tag(tag.id)),
                    Some(AccountOutcome::Rejected(_))
                ));
            }
            let snapshot = settings::snapshot(&store).await?;
            assert_eq!(snapshot.logbooks.selected_view, SelectedView::Logbook(b.id));
            let separate = snapshot
                .logbooks
                .tags
                .iter()
                .find(|r| r.key().id == RecordId::Tag(independent.id));
            assert!(matches!(
                separate.and_then(ClientRecord::working_state),
                Some(RecordState::Live(RecordValue::Tag(saved))) if **saved == independent
            ));
            let live = save::load(&store, original.key())
                .await?
                .expect("rejecting both deletions preserves their contact");
            let Some(RecordState::Live(RecordValue::Qso(saved))) = live.working_state() else {
                anyhow::bail!("stale deletion reviews must preserve the edited contact")
            };
            assert_eq!(*saved.tags(), [a.id, b.id].into());
            assert_eq!(
                saved.comment().as_deref(),
                Some("Operator's later correction")
            );
            continue;
        }
        execute(&store, changes).await;
        assert!(matches!(
            save::load(&store, original.key())
                .await?
                .and_then(|r| r.working_state().cloned()),
            Some(RecordState::Deleted)
        ));
        assert_eq!(
            settings::snapshot(&store).await?.logbooks.selected_view,
            SelectedView::AllContacts
        );
    }
    store.close().await
}

#[tokio::test]
async fn settings_apply_repairs_staged_profiles_and_guards_selected_replacements()
-> anyhow::Result<()> {
    let (_directory, store) = workspace().await?;
    for stale in [false, true] {
        let target = rig()?;
        let replacement = rig()?;
        settings::save_equipment(&store, None, &target).await?;
        let SaveOutcome::Saved(original) =
            settings::save_equipment(&store, None, &replacement).await?
        else {
            anyhow::bail!("fixture replacement should be new")
        };
        let profile = home(target.id())?;
        let mut changes = AccountChanges::default();
        changes.profiles.insert(
            profile.id(),
            Edit {
                expected: None,
                next: profile.clone(),
            },
        );
        let snapshot = settings::snapshot(&store).await?;
        let mut review = changes.review_equipment(&snapshot, target.id())?;
        assert_eq!(review.profiles.get(&profile.id()), Some(&profile));
        review.replacements.push(ProfileEquipmentReplacement {
            profile: profile.id(),
            equipment: Some(replacement.id()),
        });
        changes.equipment_deletions.push(review);
        if stale {
            let mut fields = replacement.clone().into_fields();
            fields.name = "Changed after the review".into();
            settings::save_equipment(&store, Some(&original), &fields.try_into()?).await?;
        }

        let result = execute(&store, changes).await;
        assert_eq!(
            matches!(
                result.outcomes.get(&RecordId::Equipment(target.id())),
                Some(AccountOutcome::Rejected(_))
            ),
            stale
        );
        let after = settings::snapshot(&store).await?;
        let row = after
            .profiles
            .iter()
            .find(|r| r.key().id == RecordId::Profile(profile.id()))
            .expect("the independent profile save should commit in either case");
        let Some(RecordState::Live(RecordValue::Profile(saved))) = row.working_state() else {
            anyhow::bail!("the saved station profile should remain live")
        };
        assert_eq!(
            saved.default_equipment(),
            Some(if stale { target.id() } else { replacement.id() })
        );
        let equipment = after
            .equipment
            .iter()
            .find(|r| r.key().id == RecordId::Equipment(target.id()))
            .expect("the equipment has a durable live or deletion record");
        assert_eq!(
            matches!(equipment.working_state(), Some(RecordState::Deleted)),
            !stale
        );
    }
    store.close().await
}
