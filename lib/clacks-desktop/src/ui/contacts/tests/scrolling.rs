//! Continuous history scrolling, bounded requests and retry feedback.

use super::*;

#[test]
fn tall_viewport_loads_and_retains_every_visible_page() -> anyhow::Result<()> {
    use egui_kittest::kittest::Queryable;

    let (sender, mut requests) = mpsc::channel(8);
    let mut ui = Harness::builder()
        .with_context_setup(|ctx| crate::ui::theme::install(ctx, &Default::default()))
        // All 250 contacts fit at once, spanning three internal 100-row reads.
        .with_size(egui::vec2(1100.0, 10_000.0))
        .build_ui_state(
            move |ui, view: &mut ContactsUi| {
                view.show(
                    ui,
                    &fixtures::snapshot(),
                    &sender,
                    &DefinitionSet::default(),
                );
            },
            ContactsUi::default(),
        );

    let restore = requests.try_recv()?;
    assert!(matches!(
        restore.action,
        SettingsAction::Contacts(ContactAction::LoadQueryState)
    ));
    restore
        .reply
        .send(SettingsResponse::Contacts(ContactResponse::QueryState(
            Default::default(),
        )))
        .unwrap();
    ui.run_steps(2);

    let pages = [
        fixtures::contact_page(0, 100, 250),
        fixtures::contact_page(100, 100, 250),
        fixtures::contact_page(200, 50, 250),
    ];
    let mut loaded = 0;
    for (page, offset) in pages.into_iter().zip([0, 100, 200]) {
        let request = requests.try_recv()?;
        let SettingsAction::Contacts(ContactAction::Page { query }) = &request.action else {
            panic!("expected a viewport-driven contact read")
        };
        assert_eq!(query.offset, offset);
        assert_eq!(query.limit, 100);
        loaded += page.rows.len();
        request
            .reply
            .send(SettingsResponse::Contacts(ContactResponse::Page(page)))
            .unwrap();
        ui.run_steps(2);

        // Later reads must neither replace earlier visible pages nor hide
        // unloaded rows from the logical viewport.
        for index in 0..loaded {
            assert!(
                ui.query_by_label(&format!("K1T{index:03}")).is_some(),
                "visible contact {index} missing after accepting offset {offset}"
            );
        }
        assert_eq!(
            ui.query_all_by_label("Loading contacts…").count(),
            250 - loaded
        );
    }

    // Further frames must retain all three pages without rereading any of them.
    ui.run_steps(3);
    for index in 0..250 {
        assert!(ui.query_by_label(&format!("K1T{index:03}")).is_some());
    }
    assert!(ui.query_all_by_label("Loading contacts…").next().is_none());
    assert!(requests.try_recv().is_err());
    Ok(())
}

#[test]
fn history_counts_stay_at_the_pane_edge_with_empty_and_scrolling_rows() {
    use egui_kittest::kittest::Queryable;
    for height in [220.0, 450.0] {
        for count in [0, 40] {
            let page = ContactPage {
                rows: (0..count)
                    .map(|n| {
                        let mut qso = fixtures::contact();
                        crate::test_support::edit(&mut qso, |qso| {
                            qso.contacted_station.callsign =
                                clacks_model::scalar::RecordedCallsign::new(&format!("K1T{n:03}"))
                                    .unwrap();
                            Ok(())
                        });
                        clacks_client_store::read::contacts::ContactRow {
                            facts: Default::default(),
                            working: clacks_client_store::read::history::HistoryRow {
                                qso,
                                pending: true,
                                review: None,
                            },
                        }
                    })
                    .collect(),
                matching_total: count,
                selected_total: 0,
                selected_outside_filter: 0,
                has_more: true,
            };
            let last = page.rows.len().checked_sub(1);
            let view = ContactsUi {
                data: {
                    let mut data = table::data::TableData::default();
                    data.accept(0, page, true);
                    data
                },
                ..Default::default()
            };
            let (sender, _requests) = mpsc::channel(16);
            let mut ui = Harness::builder()
                .allow_missing_glyphs()
                .with_context_setup(|ctx| crate::ui::theme::install(ctx, &Default::default()))
                .with_size(egui::vec2(1000.0, height))
                .build_ui_state(
                    move |ui, view: &mut ContactsUi| {
                        view.show(
                            ui,
                            &fixtures::snapshot(),
                            &sender,
                            &DefinitionSet::default(),
                        )
                    },
                    view,
                );
            assert!(
                ui.query_by_role_and_label(egui::Role::Button, "Next")
                    .is_none()
            );
            let footer = ui
                .get_by_label(&format!("{count} matching · 0 selected · 0 outside filter"))
                .rect();
            let title = ui.get_by_label("Contact history").rect();
            assert!((footer.bottom() - (height - 8.0)).abs() < 1.0, "{footer:?}");
            if let Some(last) = last {
                assert!(ui.query_by_label("K1T039").is_none());
                ui.state_mut().pending_focus = Some(table::PendingFocus {
                    index: last,
                    owner: None,
                });
                ui.run_steps(6);
                assert!(ui.query_by_label("K1T039").is_some());
                assert_eq!(
                    ui.get_by_label(&format!("{count} matching · 0 selected · 0 outside filter"))
                        .rect(),
                    footer
                );
                assert_eq!(ui.get_by_label("Contact history").rect(), title);
            }
        }
    }
}

#[test]
fn viewport_reads_jump_directly_and_failed_reads_wait_for_retry() -> anyhow::Result<()> {
    let (sender, mut requests) = mpsc::channel(8);
    let mut view = ContactsUi::default();
    view.filter.desired = Some(clacks_model::query::Query::All);
    view.notice = Some("Changes saved locally".into());
    view.data
        .accept(0, fixtures::contact_page(0, 100, 1000), true);
    view.needed = [700].into();
    view.read_page(&sender);
    let request = requests.try_recv()?;
    let SettingsAction::Contacts(ContactAction::Page { query }) = &request.action else {
        panic!()
    };
    assert_eq!(query.offset, 700);
    assert_eq!(query.limit, 100);
    request
        .reply
        .send(SettingsResponse::Failed("Temporary read failure".into()))
        .unwrap();
    view.poll();
    view.read_page(&sender);
    assert!(
        requests.try_recv().is_err(),
        "do not retry a failed read every frame"
    );
    assert!(
        view.read_error
            .as_ref()
            .unwrap()
            .contains("Temporary read failure")
    );

    view.read_error = None;
    view.read_page(&sender);
    let request = requests.try_recv()?;
    let SettingsAction::Contacts(ContactAction::Page { query }) = &request.action else {
        panic!()
    };
    assert_eq!(query.offset, 700);
    request
        .reply
        .send(SettingsResponse::Contacts(ContactResponse::Page(
            fixtures::contact_page(700, 100, 1000),
        )))
        .unwrap();
    view.poll();
    assert_eq!(
        view.data
            .row(734)
            .unwrap()
            .working
            .qso
            .contacted_station()
            .callsign
            .as_str(),
        "K1T734"
    );
    assert!(
        view.data.row(0).is_none(),
        "offscreen rows must not accumulate"
    );
    assert!(!view.problem);
    assert!(view.read_error.is_none());
    assert_eq!(view.notice.as_deref(), Some("Changes saved locally"));
    Ok(())
}
