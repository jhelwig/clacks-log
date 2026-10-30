//! Pane allocation and reset are independent of database or session startup.

use super::*;
use crate::ui::tests::fixtures;
use clacks_client_store::{
    read::{contacts::ContactPage, history::HistoryRow},
    settings::SettingsSnapshot,
};
use egui_kittest::{Harness, kittest::Queryable};

struct Host {
    workbench: Workbench,
    snapshot: SettingsSnapshot,
    sender: mpsc::Sender<SettingsRequest>,
    requests: mpsc::Receiver<SettingsRequest>,
    available: egui::Rect,
}

#[test]
fn history_fills_remaining_bounds_with_empty_or_populated_rows() {
    for populated in [false, true] {
        let (sender, requests) = mpsc::channel(8);
        let mut workbench = Workbench::default();
        let snapshot = fixtures::snapshot();
        workbench.previous = Some(snapshot.clone());
        let mut ui = Harness::builder()
            .allow_missing_glyphs()
            .with_size(egui::vec2(1400.0, 950.0))
            .build_ui_state(
                |ui, host: &mut Host| {
                    host.available = ui.available_rect_before_wrap();
                    host.workbench.show(ui, &host.snapshot, &host.sender);
                },
                Host {
                    workbench,
                    snapshot,
                    sender,
                    requests,
                    available: egui::Rect::NOTHING,
                },
            );
        let restore = ui.state_mut().requests.try_recv().unwrap();
        assert!(matches!(
            restore.action,
            crate::session::settings::SettingsAction::Contacts(
                crate::session::settings::contacts::ContactAction::LoadQueryState
            )
        ));
        restore
            .reply
            .send(crate::session::settings::SettingsResponse::Contacts(
                crate::session::settings::contacts::ContactResponse::QueryState(Default::default()),
            ))
            .unwrap();
        ui.step();
        let request = ui.state_mut().requests.try_recv().unwrap();
        assert!(matches!(
            request.action,
            crate::session::settings::SettingsAction::Contacts(
                crate::session::settings::contacts::ContactAction::Page { .. }
            )
        ));
        request
            .reply
            .send(crate::session::settings::SettingsResponse::Contacts(
                crate::session::settings::contacts::ContactResponse::Page(ContactPage {
                    rows: if populated {
                        vec![clacks_client_store::read::contacts::ContactRow {
                            facts: Default::default(),
                            working: HistoryRow {
                                qso: fixtures::contact(),
                                pending: true,
                                review: None,
                            },
                        }]
                    } else {
                        vec![]
                    },
                    has_more: false,
                    matching_total: u64::from(populated),
                    selected_total: 0,
                    selected_outside_filter: 0,
                }),
            ))
            .unwrap();
        ui.run();

        let section = |heading: &str| {
            let label = ui.get_by_label(heading).rect();
            ui.output()
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Rect(rect)
                        if rect.stroke.width > 0.0 && rect.rect.contains_rect(label) =>
                    {
                        Some(rect.rect)
                    }
                    _ => None,
                })
                .min_by(|a, b| a.area().total_cmp(&b.area()))
                .unwrap()
        };
        let history = section("Contact history");
        let entry = section("QSO entry");
        let details = section("Contact details");
        let available = ui.state().available;
        assert!((history.left() - available.left()).abs() <= 2.0);
        assert!((history.right() - available.right()).abs() <= 2.0);
        assert!((history.bottom() - available.bottom()).abs() <= 2.0);
        assert!((entry.top() - details.top()).abs() <= 2.0);
        assert!((entry.left() - history.left()).abs() <= 2.0);
        assert!((details.right() - history.right()).abs() <= 2.0);
        assert!((entry.bottom() - history.top()).abs() <= 2.0);
        assert!((details.bottom() - history.top()).abs() <= 2.0);
        let expected = if populated {
            "OK2CQR"
        } else {
            "No contacts in this view."
        };
        assert!(ui.query_by_label(expected).is_some());
    }
}

#[test]
fn layout_reset_restores_dimensions_from_the_view_menu() {
    let workbench = Workbench {
        sizes: PaneSizes {
            height: 500.0,
            width: 800.0,
        },
        ..Default::default()
    };
    let mut ui = Harness::builder().build_ui_state(
        |ui, workbench: &mut Workbench| workbench.menu(ui),
        workbench,
    );
    assert!(ui.query_by_label("Reset workbench layout").is_none());
    ui.get_by_label("View").click();
    ui.run();
    ui.get_by_label("Layout").click();
    ui.run();
    ui.get_by_label("Reset workbench layout").click();
    ui.run();
    assert_eq!(ui.state().sizes.height, PaneSizes::default().height);
    assert_eq!(ui.state().sizes.width, PaneSizes::default().width);
    assert!(ui.state().reset);
}

#[test]
fn worked_before_shares_entry_and_activates_the_existing_editor() -> anyhow::Result<()> {
    use crate::session::settings::{
        SettingsAction, SettingsResponse,
        contacts::{ContactAction, ContactResponse},
    };
    use anyhow::Context;
    let (sender, requests) = mpsc::channel(16);
    let snapshot = fixtures::snapshot();
    let workbench = Workbench {
        previous: Some(snapshot.clone()),
        ..Default::default()
    };
    let mut ui = Harness::builder()
        .with_context_setup(|context| theme::install(context, &Default::default()))
        .allow_missing_glyphs()
        .with_size(egui::vec2(1450.0, 1000.0))
        .build_ui_state(
            |ui, host: &mut Host| {
                host.workbench.show(ui, &host.snapshot, &host.sender);
            },
            Host {
                workbench,
                snapshot,
                sender,
                requests,
                available: egui::Rect::NOTHING,
            },
        );
    let restore = ui.state_mut().requests.try_recv().context("restore")?;
    restore
        .reply
        .send(SettingsResponse::Contacts(ContactResponse::QueryState(
            Default::default(),
        )))
        .unwrap();
    ui.step();
    let history = ui.state_mut().requests.try_recv().context("history")?;
    assert!(matches!(
        history.action,
        SettingsAction::Contacts(ContactAction::Page { .. })
    ));
    history
        .reply
        .send(SettingsResponse::Contacts(ContactResponse::Page(
            ContactPage {
                rows: vec![],
                has_more: false,
                matching_total: 0,
                selected_total: 0,
                selected_outside_filter: 0,
            },
        )))
        .unwrap();
    ui.step();
    ui.get_by_role_and_label(egui::Role::TextInput, "Callsign")
        .focus();
    ui.get_by_role_and_label(egui::Role::TextInput, "Callsign")
        .type_text("OK2CQR");
    ui.step();
    let prior = ui.state_mut().requests.try_recv().context("prefix")?;
    let SettingsAction::Contacts(ContactAction::WorkedBefore { query }) = prior.action else {
        anyhow::bail!("not the actual prefix request")
    };
    assert_eq!(query.call_prefix, "OK2CQR");
    let contact = fixtures::contact();
    let id = contact.id();
    prior
        .reply
        .send(SettingsResponse::Contacts(ContactResponse::WorkedBefore(
            ContactPage {
                rows: (0..30)
                    .map(|n| {
                        let mut qso = if n == 0 {
                            contact.clone()
                        } else {
                            fixtures::contact()
                        };
                        if n > 0 {
                            crate::test_support::edit(&mut qso, |qso| {
                                qso.contacted_station.callsign =
                                    clacks_model::scalar::RecordedCallsign::new(&format!(
                                        "OK2CQR/{n}"
                                    ))
                                    .unwrap();
                                Ok(())
                            });
                        }
                        clacks_client_store::read::contacts::ContactRow {
                            working: HistoryRow {
                                qso,
                                pending: true,
                                review: None,
                            },
                            facts: Default::default(),
                        }
                    })
                    .collect(),
                has_more: false,
                matching_total: 30,
                selected_total: 0,
                selected_outside_filter: 0,
            },
        )))
        .unwrap();
    let height = ui.state().workbench.sizes.height;
    ui.run_steps(8);
    assert!(
        (ui.state().workbench.sizes.height - height).abs() <= 2.0,
        "prior-contact rows must not resize the pane: {height} -> {}",
        ui.state().workbench.sizes.height
    );
    ui.get_by_label("Contact details").click();
    ui.step();
    assert_eq!(ui.state().workbench.entry.contact_call(), "OK2CQR");
    ui.get_by_label("Worked Before").click();
    ui.step();
    ui.get_by_label("OK2CQR").click();
    ui.step();
    ui.key_press(egui::Key::Enter);
    ui.step();
    let open = ui.state_mut().requests.try_recv().context("activation")?;
    assert!(
        matches!(open.action, SettingsAction::Contacts(ContactAction::Load { id: requested }) if requested == id)
    );
    assert_eq!(ui.state().workbench.entry.contact_call(), "OK2CQR");
    Ok(())
}
