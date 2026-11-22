//! Logbook components are checked at their native request/response boundary.

mod responses;

use super::*;
use crate::ui::tests::fixtures;
use egui_kittest::{Harness, kittest::Queryable};

struct Host {
    view: LogbooksUi,
    snapshot: LogbookSnapshot,
    sender: mpsc::Sender<SettingsRequest>,
    requests: mpsc::Receiver<SettingsRequest>,
}

fn mount() -> Harness<'static, Host> {
    let (sender, requests) = mpsc::channel(8);
    let tag = LogbookTag::new(Default::default(), "Main".into());
    Harness::builder()
        .allow_missing_glyphs()
        .with_size(egui::vec2(1100.0, 900.0))
        .build_ui_state(
            |ui, host| {
                host.view.observe(&host.snapshot, None);
                host.view.show(ui, &host.snapshot, &host.sender);
                host.view.feedback(ui);
            },
            Host {
                view: Default::default(),
                snapshot: LogbookSnapshot {
                    selected_view: SelectedView::AllContacts,
                    tags: vec![fixtures::row(RecordValue::Tag(Box::new(tag)))],
                },
                sender,
                requests,
            },
        )
}

#[test]
fn a_tag_named_like_a_builtin_selects_its_native_identity() {
    let mut ui = mount();
    let mut named = tag(ui.state().snapshot.tags.first().unwrap())
        .unwrap()
        .clone();
    named.name = "All contacts".into();
    ui.state_mut().snapshot.tags = vec![fixtures::row(RecordValue::Tag(Box::new(named.clone())))];
    ui.run();
    ui.get_by_label("Logbook").click();
    ui.run();
    ui.get_all_by_label("All contacts").last().unwrap().click();
    ui.step();
    let request = ui.state_mut().requests.try_recv().unwrap();
    assert!(
        matches!(request.action, SettingsAction::Logbooks(LogbookAction::SelectView { next: SelectedView::Logbook(id) }) if id == named.id)
    );
    assert_eq!(ui.state().snapshot.selected_view, SelectedView::AllContacts);
}
