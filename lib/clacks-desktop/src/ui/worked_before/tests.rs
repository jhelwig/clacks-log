//! Prefix correlation and UTC Today are local presentation decisions.

use super::model::WorkedBeforeModel;
use crate::{
    session::settings::{
        SettingsAction, SettingsResponse,
        contacts::{ContactAction, ContactResponse},
    },
    ui::tests::fixtures,
};
use clacks_client_store::{
    logbooks::selection::SelectedView,
    read::{
        contacts::{ContactPage, ContactRow},
        history::HistoryRow,
    },
};
use clacks_model::scalar::UtcSecond;
use eframe::egui;
use tokio::sync::mpsc;

#[test]
fn loading_stays_inside_the_table_without_moving_its_headers() {
    use egui_kittest::{Harness, kittest::Queryable};

    let mut ui = Harness::builder()
        .allow_missing_glyphs()
        .with_size(egui::vec2(1000.0, 350.0))
        .build_ui_state(
            |ui, view: &mut super::WorkedBeforeUi| {
                view.show(ui);
            },
            super::WorkedBeforeUi::default(),
        );
    let header = ui.get_by_label("Callsign").rect();
    let footer = ui.get_by_role_and_label(egui::Role::Button, "Next").rect();
    ui.state_mut().model.query.call_prefix = "W7HEX".into();
    ui.run();
    let loading = ui.get_by_label("Loading prior contacts…").rect();
    assert!(loading.top() >= header.bottom());
    assert_eq!(ui.get_by_label("Callsign").rect(), header);
    assert_eq!(
        ui.get_by_role_and_label(egui::Role::Button, "Next").rect(),
        footer
    );

    ui.state_mut().model.page = Some(ContactPage {
        rows: vec![],
        matching_total: 0,
        selected_total: 0,
        selected_outside_filter: 0,
        has_more: false,
    });
    ui.run();
    assert!(ui.query_by_label("Loading prior contacts…").is_none());
    assert!(ui.query_by_label("Worked Before · W7HEX").is_none());
    assert!(ui.get_by_label("No contacts in this view.").rect().top() >= header.bottom());
    assert_eq!(ui.get_by_label("Callsign").rect(), header);
    assert_eq!(
        ui.get_by_role_and_label(egui::Role::Button, "Next").rect(),
        footer
    );
}

#[tokio::test]
async fn status_changes_only_entry_and_drives_existing_observers() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = clacks_client_store::store::ClientStore::create(
        &directory.path().join("workspace.sqlite"),
        Default::default(),
        Default::default(),
    )
    .await?;
    let qso = fixtures::contact();
    clacks_client_store::save::contacts::save(&store, None, qso.clone(), None).await?;
    let context = egui::Context::default();
    let (sender, mut requests) = mpsc::channel(8);
    let mut entry = crate::ui::logging::LoggingUi::default();
    let mut model = WorkedBeforeModel::default();
    entry.incoming_call(qso.contacted_station().callsign.as_str());
    model.update(
        &context,
        entry.contact_call(),
        SelectedView::AllContacts,
        Default::default(),
        UtcSecond::from_unix_seconds(0),
        &sender,
    );
    let request = requests.try_recv()?;
    let SettingsAction::Contacts(action) = request.action else {
        panic!()
    };
    let response = crate::session::settings::contacts::execute(
        &store,
        action,
        None,
        tokio::sync::watch::channel(()).0,
    )
    .await?;
    request
        .reply
        .send(SettingsResponse::Contacts(response))
        .unwrap();
    model.update(
        &context,
        entry.contact_call(),
        SelectedView::AllContacts,
        Default::default(),
        UtcSecond::from_unix_seconds(0),
        &sender,
    );
    assert_eq!(
        model.page.as_ref().unwrap().rows[0].working.qso.id(),
        qso.id()
    );
    entry.incoming_call("");
    model.update(
        &context,
        entry.contact_call(),
        SelectedView::AllContacts,
        Default::default(),
        UtcSecond::from_unix_seconds(0),
        &sender,
    );
    assert!(model.page.is_none());
    assert!(requests.try_recv().is_err());
    store.close().await?;
    Ok(())
}

#[test]
fn worked_before_rejects_old_calls_and_refreshes_today() -> anyhow::Result<()> {
    let context = egui::Context::default();
    let (sender, mut requests) = mpsc::channel(8);
    let mut model = WorkedBeforeModel::default();
    let before_midnight = UtcSecond::from_unix_seconds(86_399);
    model.update(
        &context,
        "w7hex",
        SelectedView::AllContacts,
        Default::default(),
        before_midnight,
        &sender,
    );
    let old = requests.try_recv()?;
    let SettingsAction::Contacts(ContactAction::WorkedBefore { query }) = &old.action else {
        anyhow::bail!("not a Worked Before read")
    };
    assert_eq!(query.call_prefix, "W7HEX");
    model.update(
        &context,
        "W7HEX/P",
        SelectedView::AllContacts,
        Default::default(),
        before_midnight,
        &sender,
    );
    assert!(old.reply.is_closed());
    let current = requests.try_recv()?;
    let mut qso = fixtures::contact();
    crate::test_support::edit(&mut qso, |qso| {
        qso.start = before_midnight;
        Ok(())
    });
    let page = ContactPage {
        rows: vec![ContactRow {
            working: HistoryRow {
                qso,
                pending: true,
                review: None,
            },
            facts: Default::default(),
        }],
        matching_total: 1,
        selected_total: 0,
        selected_outside_filter: 0,
        has_more: false,
    };
    current
        .reply
        .send(SettingsResponse::Contacts(ContactResponse::WorkedBefore(
            page,
        )))
        .unwrap();
    model.update(
        &context,
        "W7HEX/P",
        SelectedView::AllContacts,
        Default::default(),
        before_midnight,
        &sender,
    );
    assert_eq!(model.day, 0);
    model.update(
        &context,
        "W7HEX/P",
        SelectedView::AllContacts,
        Default::default(),
        UtcSecond::from_unix_seconds(86_400),
        &sender,
    );
    assert_eq!(model.day, 1);
    assert_eq!(model.page.as_ref().unwrap().rows.len(), 1);
    assert!(requests.try_recv().is_err());
    model.update(
        &context,
        "",
        SelectedView::AllContacts,
        Default::default(),
        before_midnight,
        &sender,
    );
    assert!(model.page.is_none());
    assert!(requests.try_recv().is_err());
    Ok(())
}

#[test]
fn two_presentations_preserve_shared_column_widths_order_and_reset() {
    use super::{WorkedBeforeUi, show_table};
    use crate::ui::contact_table::preferences::{ContactTablePreferences, TableKind};
    use clacks_client_store::read::contacts::ContactColumn;
    use egui_kittest::Harness;
    let mut view = WorkedBeforeUi::default();
    view.model.query.call_prefix = "OK2CQR".into();
    view.model.page = Some(ContactPage {
        rows: vec![ContactRow {
            working: HistoryRow {
                qso: fixtures::contact(),
                pending: true,
                review: None,
            },
            facts: Default::default(),
        }],
        has_more: false,
        matching_total: 1,
        selected_total: 0,
        selected_outside_filter: 0,
    });
    let mut ui = Harness::builder()
        .with_size(egui::vec2(1800.0, 500.0))
        .build_ui_state(
            |ui, view: &mut WorkedBeforeUi| {
                ui.columns(2, |columns| {
                    for (index, (column, interaction)) in columns
                        .iter_mut()
                        .zip([&mut view.tab, &mut view.window])
                        .enumerate()
                    {
                        column.push_id(index, |ui| {
                            show_table(ui, &mut view.model, &mut view.preferences, interaction)
                        });
                    }
                });
            },
            view,
        );
    ui.run();
    // A resize in either presentation changes their shared canonical preference.
    ui.state_mut()
        .preferences
        .columns
        .iter_mut()
        .find(|c| c.column == ContactColumn::Callsign)
        .unwrap()
        .width = 160.0;
    ui.run_steps(3);
    assert!(
        (ui.state()
            .preferences
            .columns
            .iter()
            .find(|c| c.column == ContactColumn::Callsign)
            .unwrap()
            .width
            - 160.0)
            .abs()
            <= 1.0
    );
    ui.state_mut().preferences.columns.swap(0, 1);
    ui.run_steps(3);
    assert_eq!(
        ui.state().preferences.columns.first().unwrap().column,
        ContactColumn::Callsign
    );
    assert!((ui.state().preferences.columns.first().unwrap().width - 160.0).abs() <= 1.0);
    ui.state_mut().preferences = ContactTablePreferences::defaults(TableKind::WorkedBefore);
    ui.run_steps(3);
    assert_eq!(
        ui.state().preferences.columns.first().unwrap().column,
        ContactColumn::Start
    );
    assert!(
        (ui.state()
            .preferences
            .columns
            .iter()
            .find(|c| c.column == ContactColumn::Callsign)
            .unwrap()
            .width
            - 90.0)
            .abs()
            <= 1.0
    );
}

#[test]
fn pagination_stays_at_the_pane_edge_for_empty_and_scrolling_results() {
    use egui_kittest::{Harness, kittest::Queryable};
    for height in [220.0, 450.0] {
        for count in [0, 40] {
            let mut view = super::WorkedBeforeUi::default();
            view.model.query.call_prefix = "W7HEX".into();
            view.model.page = Some(ContactPage {
                rows: (0..count)
                    .map(|n| {
                        let mut qso = fixtures::contact();
                        crate::test_support::edit(&mut qso, |qso| {
                            qso.contacted_station.callsign =
                                clacks_model::scalar::RecordedCallsign::new(&format!("K1T{n:03}"))
                                    .unwrap();
                            Ok(())
                        });
                        ContactRow {
                            facts: Default::default(),
                            working: HistoryRow {
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
            });
            let mut ui = Harness::builder()
                .allow_missing_glyphs()
                .with_context_setup(|ctx| crate::ui::theme::install(ctx, &Default::default()))
                .with_size(egui::vec2(340.0, height))
                .build_ui_state(
                    |ui, view: &mut super::WorkedBeforeUi| {
                        view.show(ui);
                    },
                    view,
                );
            let footer = ui.get_by_role_and_label(egui::Role::Button, "Next").rect();
            assert!((footer.bottom() - (height - 8.0)).abs() < 1.0, "{footer:?}");
            if count > 0 {
                assert!(ui.query_by_label("K1T039").is_none());
                let last = ui
                    .state()
                    .model
                    .page
                    .as_ref()
                    .unwrap()
                    .rows
                    .last()
                    .unwrap()
                    .working
                    .qso
                    .id();
                ui.state_mut().tab.pending_focus = Some(last);
                ui.run();
                assert!(ui.query_by_label("K1T039").is_some());
                assert_eq!(
                    ui.get_by_role_and_label(egui::Role::Button, "Next").rect(),
                    footer
                );
            }
        }
    }
}

#[test]
fn detached_window_minimum_keeps_controls_and_a_row_visible() {
    use egui_kittest::{Harness, kittest::Queryable};
    for scale in [1.0, 1.5] {
        let setup = move |ctx: &egui::Context| {
            crate::ui::theme::install(ctx, &Default::default());
            ctx.all_styles_mut(|style| {
                for font in style.text_styles.values_mut() {
                    font.size *= scale;
                }
            });
        };
        let measured = Harness::builder().with_context_setup(setup).build_ui_state(
            |ui, minimum: &mut Option<egui::Vec2>| {
                *minimum = super::WorkedBeforeUi::default()
                    .window_builder(ui.ctx())
                    .min_inner_size;
            },
            None,
        );
        let minimum = measured
            .state()
            .expect("native resizing must have a minimum");
        let mut view = super::WorkedBeforeUi::default();
        view.model.page = Some(ContactPage {
            rows: vec![],
            matching_total: 0,
            selected_total: 0,
            selected_outside_filter: 0,
            has_more: false,
        });
        let ui = Harness::builder()
            .with_context_setup(setup)
            .allow_missing_glyphs()
            .with_size(minimum + egui::vec2(16.0, 16.0))
            .build_ui_state(
                |ui, view: &mut super::WorkedBeforeUi| {
                    view.show(ui);
                },
                view,
            );
        let footer = ui.get_by_role_and_label(egui::Role::Button, "Next").rect();
        assert!(footer.right() <= minimum.x + 8.0 && footer.bottom() <= minimum.y + 8.0);
        let message = ui
            .output()
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "No contacts in this view." => {
                    Some((
                        shape.clip_rect,
                        text.galley.rect.translate(text.pos.to_vec2()),
                    ))
                }
                _ => None,
            })
            .unwrap();
        assert!(
            message.0.expand(0.5).contains_rect(message.1),
            "{minimum:?}: {message:?}"
        );
    }
}
