use super::*;
use crate::session::settings::contacts;
use clacks_client_store::store::{ClientStore, StoreSettings};
use clacks_model::{group::DefinitionSet, ids::QsoId, scalar::UtcSecond};

#[tokio::test]
async fn stale_editor_cannot_overwrite_or_resurrect_a_contact() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("edit.sqlite"),
        Default::default(),
        StoreSettings::default(),
    )
    .await?;
    let mut form = crate::logging::form::ContactForm::empty();
    form.own.callsign = "W7HEX".into();
    form.dx.callsign = "OK2CQR".into();
    form.mode = "SSB".into();
    form.primary.band = "20m".into();
    form.start =
        crate::logging::time::ContactTimestamp::recorded(Some(UtcSecond::from_unix_seconds(1)));
    let qso = form
        .build(QsoId::new(), &DefinitionSet::default())
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let result = contacts::execute(
        &store,
        ContactAction::Save {
            expected: None,
            next: Box::new(qso),
        },
    )
    .await?;
    let ContactResponse::Saved(SaveOutcome::Saved(original)) = result else {
        anyhow::bail!("create failed")
    };
    let context = egui::Context::default();
    let mut confirmation = deletion::DeletionConfirmation {
        expected: original.clone(),
        first_frame: true,
    };
    context
        .run_ui(egui::RawInput::default(), |ui| {
            deletion::show(ui.ctx(), &mut confirmation, false);
        })
        .drop_without_applying_deltas();
    let input = egui::RawInput {
        events: vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        ..Default::default()
    };
    let mut choice = deletion::DeletionAction::None;
    context
        .run_ui(input, |ui| {
            choice = deletion::show(ui.ctx(), &mut confirmation, false);
        })
        .drop_without_applying_deltas();
    assert!(
        matches!(choice, deletion::DeletionAction::Cancel),
        "Enter defaults to cancellation, not deletion"
    );
    let mut editor = editor::ContactEditor::from_record(original.clone())?;
    editor.form.comment = "My unsaved correction".into();
    contacts::execute(
        &store,
        ContactAction::Delete {
            expected: Box::new(original.clone()),
        },
    )
    .await?;
    let next = editor
        .form
        .build(editor.id()?, &DefinitionSet::default())
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let stale = contacts::execute(
        &store,
        ContactAction::Save {
            expected: Some(Box::new(original)),
            next: Box::new(next),
        },
    )
    .await?;
    let mut ui = ContactsUi {
        editor: Some(editor),
        ..Default::default()
    };
    ui.accept(stale);
    assert_eq!(
        ui.editor.as_ref().unwrap().form.comment,
        "My unsaved correction"
    );
    assert!(ui.notice.as_ref().unwrap().contains("changed"));
    store.close().await?;
    Ok(())
}
