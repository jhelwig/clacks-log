//! Account form interactions that depend on more than one record or a refreshed basis.

use super::*;
use crate::ui::tests::fixtures;
use clacks_sync::record::RecordValue;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};

struct Host {
    draft: AccountDraft,
    snapshot: SettingsSnapshot,
    section: super::super::Section,
    sender: mpsc::Sender<SettingsRequest>,
    commands: mpsc::Sender<SessionCommand>,
}

fn mount() -> (Harness<'static, Host>, mpsc::Receiver<SettingsRequest>) {
    let (sender, requests) = mpsc::channel(8);
    let (commands, _) = mpsc::channel(1);
    let mut snapshot = fixtures::snapshot();
    snapshot
        .logbooks
        .tags
        .push(fixtures::row(RecordValue::Tag(Box::new(LogbookTag::new(
            TagId::new(),
            "Home log".into(),
        )))));
    let ui = Harness::builder()
        .allow_missing_glyphs()
        .with_size(egui::vec2(1100., 1100.))
        .build_ui_state(
            |ui, host: &mut Host| match host.section {
                super::super::Section::Logbooks => {
                    host.draft.logbooks(ui, &host.snapshot, &host.sender)
                }
                super::super::Section::StationProfiles => host.draft.station_profiles(
                    ui,
                    &host.snapshot,
                    &DefinitionSet::default(),
                    &host.commands,
                    &GeographyStatus::Opening,
                ),
                super::super::Section::Equipment => host.draft.equipment_configurations(
                    ui,
                    &host.snapshot,
                    &DefinitionSet::default(),
                ),
                _ => {}
            },
            Host {
                draft: Default::default(),
                snapshot,
                section: super::super::Section::Logbooks,
                sender,
                commands,
            },
        );
    (ui, requests)
}

#[test]
fn tag_deletion_cancel_preserves_edits_and_confirmation_only_stages() -> anyhow::Result<()> {
    let (mut ui, mut requests) = mount();
    ui.get_by_label("Home log").click();
    ui.run();
    ui.get_by_label("Logbook name")
        .set_text_value_accesskit("Field log");
    ui.run();
    let original = ui
        .state()
        .snapshot
        .logbooks
        .tags
        .first()
        .expect("fixture contains the Home log")
        .clone();

    for confirm in [false, true] {
        ui.get_by_label("Delete logbook").click();
        ui.step();
        let request = requests.try_recv()?;
        let SettingsAction::Logbooks(LogbookAction::PreviewDeletion { id }) = request.action else {
            anyhow::bail!("expected a read of this logbook's impact")
        };
        assert_eq!(RecordId::Tag(id), original.key().id);
        request
            .reply
            .send(SettingsResponse::Logbooks(
                LogbookResponse::DeletionPreview(Arc::new(TagDeletionPreview {
                    tag: original.clone(),
                    members: Vec::new(),
                    counts: TagDeletionCounts {
                        affected: 0,
                        also_tagged_elsewhere: 0,
                        unique: 0,
                    },
                })),
            ))
            .expect("the review owns the reply while open");
        ui.run();
        assert!(
            ui.get_by_label("Confirm deletion")
                .accesskit_node()
                .is_disabled()
        );
        if confirm {
            ui.get_by_label("Leave all contacts (delete 0, preserve 0)")
                .click();
            ui.run();
            ui.get_by_label("Confirm deletion").click();
        } else {
            ui.key_press(egui::Key::Escape);
        }
        ui.run();
        assert!(
            requests.try_recv().is_err(),
            "confirmation and dismissal cannot write to the store"
        );
        let host = ui.state_mut();
        let changes = host.draft.submit(&host.snapshot, &Default::default());
        if confirm {
            let review = changes
                .tag_deletions
                .first()
                .expect("explicit confirmation stages the deletion");
            assert_eq!(review.id, id);
            assert!(changes.tags.is_empty());
        } else {
            assert!(changes.tag_deletions.is_empty());
            let edit = changes
                .tags
                .get(&id)
                .expect("dismissing the confirmation preserves the rename");
            assert_eq!(edit.next.name, "Field log");
        }
    }
    let mut rejected = AccountApplyResult::default();
    rejected.outcomes.insert(
        original.key().id,
        AccountOutcome::Rejected("The affected contacts changed.".into()),
    );
    ui.state_mut().draft.settled(rejected);
    ui.run();
    assert!(!ui.state().draft.dirty());
    ui.get_by_label("Review updated deletion").click();
    ui.step();
    let read = requests.try_recv()?;
    assert!(matches!(
        read.action,
        SettingsAction::Logbooks(LogbookAction::PreviewDeletion { .. })
    ));
    read.reply
        .send(SettingsResponse::Logbooks(
            LogbookResponse::DeletionPreview(Arc::new(TagDeletionPreview {
                tag: original,
                members: Vec::new(),
                counts: TagDeletionCounts {
                    affected: 0,
                    also_tagged_elsewhere: 0,
                    unique: 0,
                },
            })),
        ))
        .expect("the updated review is open");
    ui.run();
    assert!(
        !ui.get_by_label("Confirm deletion")
            .accesskit_node()
            .is_disabled(),
        "the previous treatment is still applicable"
    );
    assert!(
        !ui.state().draft.dirty(),
        "a stale deletion requires another explicit confirmation"
    );
    ui.get_by_label("Confirm deletion").click();
    ui.run();
    assert!(ui.state().draft.dirty());
    Ok(())
}

#[test]
fn profile_accepts_new_equipment_and_keeps_its_original_guard_after_a_refresh() -> anyhow::Result<()>
{
    let (mut ui, _) = mount();
    ui.state_mut().section = super::super::Section::Equipment;
    ui.run();
    ui.get_by_label("New configuration").click();
    ui.run();
    ui.get_by_label("Configuration name")
        .set_text_value_accesskit("Portable station");
    ui.run();
    ui.state_mut().section = super::super::Section::StationProfiles;
    ui.run();
    ui.get_by_label_contains("Home · W7HEX").click();
    ui.run();
    ui.get_by_label("Associated default equipment configuration")
        .click();
    ui.run();
    ui.get_all_by_label_contains("Portable station ·")
        .last()
        .expect("the staged configuration is available as a default")
        .click();
    ui.run();

    let host = ui.state_mut();
    let original = host
        .snapshot
        .profiles
        .first()
        .expect("the fixture contains Home")
        .clone();
    let p = profile(&original).expect("Home is a live profile");
    let profile_id = p.id();
    let mut replacement = p.clone().into_fields();
    replacement.name = "Refreshed home".into();
    host.snapshot.profiles = vec![fixtures::row(RecordValue::Profile(Box::new(
        replacement.try_into()?,
    )))];
    ui.run();

    let host = ui.state_mut();
    let changes = host.draft.submit(&host.snapshot, &Default::default());
    let equipment = changes
        .equipment
        .values()
        .next()
        .expect("the new configuration belongs to the shared draft");
    let edit = changes
        .profiles
        .get(&profile_id)
        .expect("selection stages a change to Home");
    assert_eq!(edit.next.default_equipment(), Some(equipment.next.id()));
    assert_eq!(edit.expected.as_ref(), Some(&original));
    assert_eq!(edit.next.name(), "Home");
    Ok(())
}
