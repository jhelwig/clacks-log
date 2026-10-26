use super::*;

#[test]
fn main_screen_offers_real_logbook_selection_and_management() {
    let mut window = Window::new();
    window.ready_main_screen();
    let output = window.main_frame(vec![]);
    let text = main_screen::labels(&output);
    assert!(text.contains(&"Logbook"));
    assert!(text.contains(&"All contacts"));
    assert!(text.contains(&"Manage logbooks…"));
    assert!(!text.contains(&"Contact history"));
}

use super::logbooks_support::LiveWindow;
use clacks_client_store::logbooks::selection::SelectedView;
use clacks_sync::record::{RecordState, RecordValue};

#[tokio::test(flavor = "multi_thread")]
async fn manage_logbooks_save_does_not_select_and_cancel_does_not_save() -> anyhow::Result<()> {
    let mut app = LiveWindow::new().await?;
    let operating = app.snapshot().await?.operating;
    app.click("Manage logbooks…");
    app.click("New logbook");
    app.text("Main");
    app.click("Forbid");
    app.now += 0.4;
    app.frame(vec![]);
    app.click("Save logbook");
    app.settle().await?;
    let snapshot = app.snapshot().await?;
    assert_eq!(snapshot.logbooks.selected_view, SelectedView::AllContacts);
    assert_eq!(snapshot.operating, operating);
    let row = snapshot.logbooks.tags.first().unwrap();
    let RecordState::Live(RecordValue::Tag(tag)) = row.working_state().unwrap() else {
        panic!("tag");
    };
    let id = tag.id;
    assert_eq!(
        tag.automatic_upload.lotw,
        clacks_model::tag::AutomaticUploadPolicy::Neither
    );
    assert_eq!(
        tag.automatic_upload.qrz,
        clacks_model::tag::AutomaticUploadPolicy::Forbid
    );
    app.click("Edit logbook");
    app.text("Renamed");
    app.click("Save logbook");
    app.settle().await?;
    app.click("Edit logbook");
    app.text("Cancelled name");
    app.click("Cancel edits");
    let snapshot = app.snapshot().await?;
    assert!(snapshot.logbooks.tags.iter().any(|row| matches!(row.working_state(), Some(RecordState::Live(RecordValue::Tag(tag))) if tag.id==id && tag.name=="Renamed")));
    app.click("Close manager");
    app.click("All contacts");
    app.click("Renamed");
    app.settle().await?;
    assert_eq!(
        app.snapshot().await?.logbooks.selected_view,
        SelectedView::Logbook(id)
    );
    assert!(!main_screen::labels(&app.output).contains(&"Named logbooks"));
    app.click("Renamed");
    app.click("Untagged");
    app.settle().await?;
    assert_eq!(
        app.snapshot().await?.logbooks.selected_view,
        SelectedView::Untagged
    );
    let reopened = crate::workspace::open(&app.paths).await?;
    assert_eq!(reopened.settings.logbooks, app.snapshot().await?.logbooks);
    reopened.store.close().await?;
    app.close().await
}

#[tokio::test(flavor = "multi_thread")]
async fn escape_discards_tag_edits_without_changing_saved_values() -> anyhow::Result<()> {
    let mut app = LiveWindow::new().await?;
    app.click("Manage logbooks…");
    app.click("New logbook");
    app.text("Main");
    app.click("Save logbook");
    app.settle().await?;
    let before = app.snapshot().await?;
    app.click("Edit logbook");
    app.text("Discard me");
    app.frame(vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    app.frame(vec![]);
    assert!(!main_screen::labels(&app.output).contains(&"Logbook name"));
    assert_eq!(app.snapshot().await?, before);
    app.click("Manage logbooks…");
    app.click("Edit logbook");
    assert!(main_screen::labels(&app.output).contains(&"Main"));
    assert!(!main_screen::labels(&app.output).contains(&"Discard me"));
    app.close().await
}

#[tokio::test(flavor = "multi_thread")]
async fn builtins_and_same_named_tag_remain_distinct_choices() -> anyhow::Result<()> {
    use clacks_client_store::logbooks::tags;
    use clacks_model::tag::LogbookTag;
    let mut app = LiveWindow::new().await?;
    let tag = LogbookTag::new(Default::default(), "All contacts".into());
    tags::save(&app.store, None, &tag).await?;
    app.settle().await?;
    app.click("All contacts");
    app.frame(vec![]);
    assert!(main_screen::labels(&app.output).contains(&"Built-in views"));
    assert!(main_screen::labels(&app.output).contains(&"Named logbooks"));
    app.click("All contacts"); // Last occurrence belongs to the named group.
    app.settle().await?;
    assert_eq!(
        app.snapshot().await?.logbooks.selected_view,
        SelectedView::Logbook(tag.id)
    );
    app.click("All contacts");
    app.frame(vec![]);
    let position = app
        .output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.text() == "All contacts" => {
                Some(text.visual_bounding_rect().center())
            }
            _ => None,
        })
        .nth(1)
        .unwrap(); // First popup occurrence, after the closed caption.
    for pressed in [true, false] {
        app.frame(vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
    }
    app.settle().await?;
    assert_eq!(
        app.snapshot().await?.logbooks.selected_view,
        SelectedView::AllContacts
    );
    app.close().await
}

#[tokio::test(flavor = "multi_thread")]
async fn saved_management_stays_in_the_application_menu_without_changing_operation()
-> anyhow::Result<()> {
    let mut app = LiveWindow::new().await?;
    let before = app.snapshot().await?;
    app.frame(vec![]);
    assert!(!main_screen::labels(&app.output).contains(&"Station profiles"));
    app.click("Saved settings");
    app.click("Station profiles");
    app.frame(vec![]);
    assert!(main_screen::labels(&app.output).contains(&"New profile"));
    app.click("Back");
    app.click("Saved settings");
    app.click("Equipment configurations");
    app.frame(vec![]);
    assert!(main_screen::labels(&app.output).contains(&"New configuration"));
    app.click("Back");
    assert_eq!(app.snapshot().await?, before);
    app.close().await
}
