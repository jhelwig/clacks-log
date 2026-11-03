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
fn narrow_details_pane_keeps_every_tab_inside_the_workbench() {
    for font_scale in [1.0, 1.5] {
        let (sender, requests) = mpsc::channel(16);
        let snapshot = fixtures::snapshot();
        let mut workbench = Workbench::default();
        workbench.sizes.width = 2000.0;
        let ui = Harness::builder()
            .with_context_setup(move |ctx| {
                theme::install(ctx, &Default::default());
                ctx.all_styles_mut(|style| {
                    for font in style.text_styles.values_mut() {
                        font.size *= font_scale;
                    }
                });
            })
            .allow_missing_glyphs()
            .with_size(egui::vec2(1000.0, 800.0))
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
        for label in ["Worked Before", "Contact details", "Our settings"] {
            let rect = ui.get_by_role_and_label(egui::Role::Tab, label).rect();
            assert!(
                ui.state().available.contains_rect(rect),
                "{label}: {rect:?}"
            );
        }
    }
}

#[test]
fn worked_before_tab_icon_requests_a_window_without_switching_tabs() {
    struct Tabs {
        workbench: Workbench,
        open_window: bool,
    }
    let mut ui = Harness::builder()
        .with_context_setup(|ctx| theme::install(ctx, &Default::default()))
        .build_ui_state(
            |ui, state: &mut Tabs| state.open_window |= state.workbench.tabs(ui),
            Tabs {
                workbench: Workbench {
                    tab: DetailTab::Contact,
                    ..Default::default()
                },
                open_window: false,
            },
        );
    let icon = ui.get_by_role_and_label(egui::Role::Button, "Open Worked Before in new window");
    let tab = ui.get_by_role_and_label(egui::Role::Tab, "Worked Before");
    assert!(tab.rect().contains_rect(icon.rect()));
    for label in ["Contact details", "Our settings"] {
        let other = ui.get_by_role_and_label(egui::Role::Tab, label).rect();
        assert!((tab.rect().top() - other.top()).abs() < 0.5);
        assert!((tab.rect().bottom() - other.bottom()).abs() < 0.5);
    }
    assert!((tab.rect().center().y - icon.rect().center().y).abs() < 0.5);
    let text_center = ui
        .output()
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == "Worked Before" => {
                Some(text.pos.y + text.galley.rect.center().y)
            }
            _ => None,
        })
        .unwrap();
    let icon_bounds = ui
        .output()
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Path(path)
                if path.points.iter().all(|point| icon.rect().contains(*point)) =>
            {
                Some(egui::Rect::from_points(&path.points))
            }
            _ => None,
        })
        .fold(egui::Rect::NOTHING, |bounds, rect| bounds.union(rect));
    assert!(icon_bounds.is_finite());
    assert!(
        (text_center - icon_bounds.center().y).abs() < 1.0,
        "text center {text_center}, icon {icon_bounds:?}"
    );
    icon.click();
    ui.run();
    assert!(ui.state().open_window);
    assert!(ui.state().workbench.tab == DetailTab::Contact);
    ui.state_mut().open_window = false;
    ui.get_by_role_and_label(egui::Role::Button, "Open Worked Before in new window")
        .focus();
    ui.key_press(egui::Key::Enter);
    ui.run();
    assert!(ui.state().open_window);
    assert!(ui.state().workbench.tab == DetailTab::Contact);
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
                        if rect.fill == theme::palette(ui.ctx.theme()).surface
                            && rect.stroke.width == 0.0
                            && rect.rect.contains_rect(label) =>
                    {
                        assert_eq!(rect.corner_radius, egui::CornerRadius::ZERO);
                        assert_eq!(rect.stroke.width, 0.0);
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
        let first_tab = ui
            .get_by_role_and_label(egui::Role::Tab, "Worked Before")
            .rect();
        assert!((first_tab.left() - details.left()).abs() < 1.0);
        assert!((first_tab.top() - details.top()).abs() < 1.0);
        let available = ui.state().available;
        assert!((history.left() - available.left()).abs() <= 2.0);
        assert!((history.right() - available.right()).abs() <= 2.0);
        assert!((history.bottom() - available.bottom()).abs() <= 2.0);
        assert!((entry.top() - details.top()).abs() <= 2.0);
        assert!((entry.right() - details.left()).abs() <= 1.0);
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
        ui.state_mut().workbench.sizes.height = 2000.0;
        ui.state_mut().workbench.reset = true;
        ui.run();
        let (clip, text_bounds) = ui
            .output()
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == expected => Some((
                    shape.clip_rect,
                    text.galley.rect.translate(text.pos.to_vec2()),
                )),
                _ => None,
            })
            .expect("history must render its row or empty-state message");
        assert!(
            clip.expand(0.5).contains_rect(text_bounds),
            "minimum pane must show the complete row/message: clip={clip:?}, text={text_bounds:?}"
        );
    }
}

#[test]
fn layout_reset_restores_dimensions() {
    let mut workbench = Workbench {
        sizes: PaneSizes {
            height: 500.0,
            width: 800.0,
        },
        ..Default::default()
    };
    workbench.reset_layout();
    assert_eq!(workbench.sizes.height, PaneSizes::default().height);
    assert_eq!(workbench.sizes.width, PaneSizes::default().width);
    assert!(workbench.reset);
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
    for label in ["Worked Before", "Contact details", "Our settings"] {
        let tab = ui.get_by_role_and_label(egui::Role::Tab, label).rect();
        assert!(
            ui.output().shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::Shape::Rect(rect)
                    if rect.rect.contains_rect(tab) && rect.stroke.width > 0.0
                        && rect.fill != egui::Color32::TRANSPARENT
            )),
            "{label} must have a visible tab boundary before hover"
        );
    }
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
    let lookup = ui.state_mut().requests.try_recv().context("directory")?;
    assert!(matches!(
        lookup.action,
        SettingsAction::Directory(crate::directory::lookup::LookupCommand::CallChanged { .. })
    ));
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

#[test]
fn compressed_history_and_detail_tabs_respect_their_pane_bounds() {
    let (sender, requests) = mpsc::channel(16);
    let snapshot = fixtures::snapshot();
    let mut workbench = Workbench {
        tab: DetailTab::OurSettings,
        ..Default::default()
    };
    workbench.sizes.height = 2000.0;
    let ui = Harness::builder()
        .allow_missing_glyphs()
        .with_context_setup(|ctx| theme::install(ctx, &Default::default()))
        .with_size(egui::vec2(1000.0, 500.0))
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
    let next = ui.get_by_role_and_label(egui::Role::Button, "Next").rect();
    assert!(next.bottom() <= ui.state().available.bottom());
    assert!((next.bottom() - ui.state().available.bottom()).abs() < 1.0);
    let tab = ui
        .get_by_role_and_label(egui::Role::Tab, "Worked Before")
        .rect();
    let heading = ui.get_by_label("Station identity").rect();
    assert!(((heading.top() - tab.bottom()) - (heading.left() - tab.left())).abs() < 0.5);
}
