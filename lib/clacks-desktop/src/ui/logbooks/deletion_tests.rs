use super::*;
use crate::ui::tests::logbooks_support::{LiveWindow, seed_contacts};
use clacks_client_store::{logbooks::deletion, save};
use clacks_model::tag::TagDeletionChoice;

fn visible(app: &LiveWindow, value: &str) -> bool {
    app.output.shapes.iter().any(|shape| matches!(&shape.shape, egui::epaint::Shape::Text(text) if text.galley.text().contains(value)))
}

#[tokio::test(flavor = "multi_thread")]
async fn deletion_review_uses_distinct_counts_and_requires_an_explicit_choice() -> anyhow::Result<()>
{
    let mut app = LiveWindow::new().await?;
    let id = seed_contacts(&mut app).await?;
    app.click("Manage logbooks…");
    app.click("Delete logbook");
    app.settle().await?;
    app.frame(vec![egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    app.frame(vec![egui::Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert!(
        !visible(&app, "Confirm deletion"),
        "initial Enter must activate Cancel, not a destructive action"
    );
    assert_eq!(deletion::preview(&app.store, id).await?.counts.affected, 2);
    app.click("Delete logbook");
    app.settle().await?;
    assert!(visible(&app, "2 contacts"));
    assert!(visible(&app, "1 also tagged elsewhere"));
    assert!(visible(&app, "Leave all contacts (delete 0)"));
    assert!(visible(&app, "Delete all contacts (delete 2)"));
    assert!(visible(&app, "Delete unique contacts (delete 1)"));
    let before = app.snapshot().await?;
    app.click("Confirm deletion"); // Disabled: no choice was made.
    app.settle().await?;
    assert_eq!(app.snapshot().await?, before);
    app.click("Cancel deletion");
    assert_eq!(deletion::preview(&app.store, id).await?.counts.affected, 2);
    app.close().await
}

#[tokio::test(flavor = "multi_thread")]
async fn stale_deletion_preview_requires_new_confirmation() -> anyhow::Result<()> {
    let mut app = LiveWindow::new().await?;
    let id = seed_contacts(&mut app).await?;
    app.click("Manage logbooks…");
    app.click("Delete logbook");
    app.settle().await?;
    app.click("Delete all contacts (delete 2)");
    let preview = deletion::preview(&app.store, id).await?;
    let row = preview.members.first().unwrap();
    let Some(RecordState::Live(RecordValue::Qso(qso))) = row.working_state() else {
        panic!("qso");
    };
    let mut qso = qso.clone();
    qso.comment = Some("Intervening fact edit".into());
    save::save(
        &app.store,
        row.key,
        Some(row),
        RecordState::Live(RecordValue::Qso(qso)),
    )
    .await?;
    app.click("Confirm deletion");
    app.settle().await?;
    assert!(visible(&app, "changed"));
    assert_eq!(deletion::preview(&app.store, id).await?.counts.affected, 2);
    app.click("Reload deletion review");
    app.settle().await?;
    app.click("Confirm deletion"); // Reload must not retain implicit confirmation.
    app.settle().await?;
    assert_eq!(deletion::preview(&app.store, id).await?.counts.affected, 2);
    app.click("Delete unique contacts (delete 1)");
    app.click("Confirm deletion");
    app.settle().await?;
    assert!(deletion::preview(&app.store, id).await.is_err());
    assert!(visible(&app, "Deleted 1 contacts"));
    app.close().await
}

#[tokio::test(flavor = "multi_thread")]
async fn every_deletion_choice_preserves_the_committed_result() -> anyhow::Result<()> {
    for (choice, label, count) in [
        (
            TagDeletionChoice::LeaveAll,
            "Leave all contacts (delete 0)",
            0,
        ),
        (
            TagDeletionChoice::DeleteAll,
            "Delete all contacts (delete 2)",
            2,
        ),
        (
            TagDeletionChoice::DeleteUnique,
            "Delete unique contacts (delete 1)",
            1,
        ),
    ] {
        let mut app = LiveWindow::new().await?;
        let id = seed_contacts(&mut app).await?;
        app.click("All contacts");
        app.click("Main");
        app.settle().await?;
        app.click("Manage logbooks…");
        app.click("Delete logbook");
        app.settle().await?;
        let reviewed = deletion::preview(&app.store, id).await?;
        app.click(label);
        app.click("Confirm deletion");
        app.settle().await?;
        assert_eq!(
            app.snapshot().await?.logbooks.selected_view,
            SelectedView::AllContacts,
            "{choice:?}"
        );
        assert!(visible(&app, &format!("Deleted {count} contacts")));
        assert!(visible(&app, "was deleted. Showing All contacts."));
        assert!(deletion::preview(&app.store, id).await.is_err());
        for before in reviewed.members {
            let Some(RecordState::Live(RecordValue::Qso(qso))) = before.working_state() else {
                panic!("reviewed QSO")
            };
            let deleted = choice == TagDeletionChoice::DeleteAll
                || (choice == TagDeletionChoice::DeleteUnique && qso.tags.len() == 1);
            let after = save::load(&app.store, before.key).await?.unwrap();
            if deleted {
                assert_eq!(after.working_state(), Some(&RecordState::Deleted));
            } else {
                let mut expected = qso.clone();
                expected.tags.remove(&id);
                assert_eq!(
                    after.working_state(),
                    Some(&RecordState::Live(RecordValue::Qso(expected)))
                );
            }
        }
        app.close().await?;
    }
    Ok(())
}
