use super::*;
use egui_kittest::{Harness, kittest::Queryable};

#[test]
fn review_sections_request_independent_pages() {
    let (requests, mut received) = mpsc::channel(8);
    let mut view = ReviewUi::default();
    let cursor = clacks_sync::record::RecordId::Qso(Default::default());
    view.conflicts.next = Some(cursor);
    let mut ui = Harness::builder()
        .with_size(egui::vec2(1000.0, 650.0))
        .build_ui_state(|ui, view| view.show_contents(ui, &requests), view);
    ui.get_by_label("Load more conflicts").click();
    ui.step();
    let request = received.try_recv().expect("real read request");
    let SettingsAction::Review(ReviewAction::ReadPage { query }) = request.action else {
        panic!("section read")
    };
    assert_eq!(query.section, Some(ReviewSection::Conflicts));
    assert_eq!(query.after, Some(cursor));
    assert!(query.kind.is_none());
    assert!(ui.state().pending_sync.rows.is_empty());
}
