//! Main-screen hierarchy keeps operating controls prominent and diagnostics actionable.

use super::*;

impl Window {
    fn click_main(&mut self, label: &str) {
        self.main_frame(vec![]).drop_without_applying_deltas();
        let output = self.main_frame(vec![]);
        let position = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.visual_bounding_rect().center())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing {label}"));
        for pressed in [true, false] {
            self.main_frame(vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ])
            .drop_without_applying_deltas();
        }
    }

    pub(super) fn ready_main_screen(&mut self) {
        let identity = clacks_client_store::identity::WorkspaceStatus {
            account: Default::default(),
            enrolled: false,
        };
        self.updates.send_replace(SessionStatus::Ready {
            identity,
            restored_view_from: None,
        });
        let (_, view) = watch::channel(Some(Arc::new(SettingsSnapshot {
            logbooks: clacks_client_store::logbooks::LogbookSnapshot {
                selected_view: clacks_client_store::logbooks::selection::SelectedView::AllContacts,
                tags: vec![],
            },
            operating: Default::default(),
            profiles: vec![],
            equipment: vec![],
            groups: vec![],
        })));
        self.app.settings_view = view;
    }

    pub(super) fn main_frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
        self.main_frame_at_size(events, egui::vec2(1024.0, 768.0))
    }

    fn main_frame_at_size(
        &mut self,
        events: Vec<egui::Event>,
        size: egui::Vec2,
    ) -> egui::FullOutput {
        use eframe::App;
        let mut frame = eframe::Frame::_new_kittest();
        let input = egui::RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        };
        let mut output = self.context.run_ui(input, |ui| self.app.ui(ui, &mut frame));
        output.textures_delta.clear();
        output
    }
}

pub(super) fn labels(output: &egui::FullOutput) -> Vec<&str> {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) => Some(text.galley.text()),
            _ => None,
        })
        .collect()
}

#[test]
fn main_screen_keeps_management_in_chrome_and_discloses_workspace_details_on_request() {
    let mut window = Window::new();
    window.ready_main_screen();
    let output = window.main_frame(vec![]);
    let text = labels(&output);
    assert!(!text.contains(&"Operating context"));
    assert!(text.contains(&"Saved settings"));
    assert!(!text.contains(&"Station profiles"));
    assert!(!text.contains(&"Equipment configurations"));
    assert!(text.contains(&"Workspace & diagnostics"));
    assert!(!text.iter().any(|label| label.contains("Diagnostic logs:")));
    assert!(!text.iter().any(|label| label.starts_with("Account:")));
    let position = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) if text.galley.text() == "Workspace & diagnostics" => {
                Some(text.visual_bounding_rect().center())
            }
            _ => None,
        })
        .unwrap();
    output.drop_without_applying_deltas();
    for pressed in [true, false] {
        window
            .main_frame(vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ])
            .drop_without_applying_deltas();
    }
    for _ in 0..20 {
        window.main_frame(vec![]).drop_without_applying_deltas();
    }
    let output = window.main_frame(vec![]);
    let text = labels(&output);
    assert!(text.iter().any(|label| label.contains("Diagnostic logs:")));
    assert!(text.iter().any(|label| label.starts_with("Account:")));
    output.drop_without_applying_deltas();
}

#[test]
fn collapsed_workspace_details_do_not_hide_failures_or_degraded_diagnostics() {
    let mut window = Window::new();
    window.updates.send_replace(SessionStatus::Failed {
        message: "Database could not be opened".into(),
    });
    window
        .app
        .file_failure
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let output = window.main_frame(vec![]);
    let text = labels(&output);
    assert!(text.contains(&"Database could not be opened"));
    assert!(text.contains(&"Retry"));
    assert!(
        text.iter()
            .any(|label| label.contains("Local log file writes failed"))
    );
    assert!(!text.iter().any(|label| label.contains("Diagnostic logs:")));
    output.drop_without_applying_deltas();
}

#[test]
fn main_screen_actions_and_radio_values_remain_reachable_across_scale_and_size() {
    use crate::ui::preferences::{Appearance, AppearancePreferences};
    use clacks_model::{
        contact::StationSnapshot,
        profile::CurrentOperatingValues,
        scalar::{Milliwatts, RecordedCallsign},
    };
    let mut window = Window::new();
    window.ready_main_screen();
    let (_, view) = watch::channel(Some(Arc::new(SettingsSnapshot {
        logbooks: clacks_client_store::logbooks::LogbookSnapshot {
            selected_view: clacks_client_store::logbooks::selection::SelectedView::AllContacts,
            tags: vec![],
        },
        operating: clacks_client_store::operating::OperatingState {
            current: Some(CurrentOperatingValues {
                station: StationSnapshot {
                    callsign: RecordedCallsign::new("W7LAYOUT").unwrap(),
                    location: None,
                    references: Default::default(),
                },
                equipment: None,
                operator: None,
                transmit_power: Some(Milliwatts::new(500).unwrap()),
            }),
            ..Default::default()
        },
        profiles: vec![],
        equipment: vec![],
        groups: vec![],
    })));
    window.app.settings_view = view;
    for appearance in [Appearance::Light, Appearance::Dark] {
        for zoom in [1.0, 1.25, 1.5, 2.0] {
            for size in [
                egui::vec2(680.0, 440.0),
                egui::vec2(1024.0, 768.0),
                egui::vec2(1440.0, 900.0),
            ] {
                theme::install(
                    &window.context,
                    &AppearancePreferences {
                        appearance,
                        zoom,
                        ..Default::default()
                    },
                );
                let mut found = std::collections::BTreeSet::new();
                for delta in std::iter::repeat_n(1000.0, 8).chain(std::iter::repeat_n(-35.0, 40)) {
                    let center = window.context.content_rect().center();
                    let output = window.main_frame_at_size(
                        vec![
                            egui::Event::PointerMoved(center),
                            egui::Event::MouseWheel {
                                unit: egui::MouseWheelUnit::Point,
                                phase: egui::TouchPhase::Move,
                                source: egui::MouseWheelSource::Unknown,
                                delta: egui::vec2(0.0, delta),
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                        size,
                    );
                    for shape in &output.shapes {
                        if let egui::epaint::Shape::Text(text) = &shape.shape
                            && shape
                                .clip_rect
                                .contains(text.visual_bounding_rect().center())
                            && window
                                .context
                                .content_rect()
                                .contains(text.visual_bounding_rect().center())
                        {
                            found.insert(text.galley.text().to_owned());
                        }
                    }
                    output.drop_without_applying_deltas();
                }
                for required in [
                    "Appearance…",
                    "Close",
                    "Operating settings…",
                    "Saved settings",
                    "Logbook",
                    "Manage logbooks…",
                    "Workspace & diagnostics",
                    "W7LAYOUT",
                    "0.5",
                ] {
                    assert!(
                        found.contains(required),
                        "{required} unreachable at {appearance:?}, {zoom}, {size:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn application_bar_and_operating_strip_follow_the_accepted_workbench_layout() {
    use clacks_client_store::operating::OperatingState;
    use clacks_model::{
        contact::{OperatorInformation, StationSnapshot},
        profile::{CurrentOperatingValues, EquipmentConfiguration, SavedLocationProfile},
        scalar::{Milliwatts, RecordedCallsign},
    };
    let mut window = Window::new();
    window.ready_main_screen();
    theme::install(&window.context, &Default::default());
    let station = StationSnapshot {
        callsign: RecordedCallsign::new("W7LAYOUT").unwrap(),
        location: None,
        references: Default::default(),
    };
    let profile = SavedLocationProfile {
        id: Default::default(),
        name: "Home".into(),
        station: station.clone(),
        default_equipment: None,
    };
    let equipment = EquipmentConfiguration {
        id: Default::default(),
        name: "Rig A".into(),
        values: Default::default(),
    };
    let mut snapshot = (**window.app.settings_view.borrow().as_ref().unwrap()).clone();
    snapshot.operating = OperatingState {
        current: Some(CurrentOperatingValues {
            station,
            operator: Some(OperatorInformation {
                callsign: Some(RecordedCallsign::new("W7ABC").unwrap()),
                recorded_name: None,
            }),
            transmit_power: Some(Milliwatts::new(50_000).unwrap()),
            equipment: Some(equipment.values.clone()),
        }),
        selected_profile: Some(profile),
        selected_equipment: Some(equipment),
    };
    let (_, view) = watch::channel(Some(Arc::new(snapshot)));
    window.app.settings_view = view;
    window.main_frame(vec![]).drop_without_applying_deltas();
    let output = window.main_frame(vec![]);
    let position = |prefix: &str| {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.text().starts_with(prefix) => {
                    Some(text.visual_bounding_rect())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing {prefix}: {:?}", labels(&output)))
    };
    let header = position("Clacks Log");
    for label in [
        "Logbook",
        "Manage logbooks…",
        "Saved settings",
        "Appearance…",
    ] {
        assert!(
            (position(label).center().y - header.center().y).abs() < header.height(),
            "{label} is outside the application bar"
        );
    }
    assert!(labels(&output).contains(&"Station"));
    assert!(labels(&output).contains(&"Home ▾"));
    assert!(labels(&output).contains(&"Equipment"));
    assert!(labels(&output).contains(&"Rig A ▾"));
    let station = position("Station");
    assert!(station.top() > header.bottom());
    for label in [
        "W7LAYOUT",
        "Equipment",
        "W7ABC ▾",
        "Power",
        "Operating settings…",
    ] {
        assert!(
            (position(label).center().y - station.center().y).abs() < station.height(),
            "{label} is outside the operating strip"
        );
    }
    assert!(
        station.bottom() - header.top() < 4.0 * header.height(),
        "default chrome displaces the logging region"
    );
    assert!(!labels(&output).contains(&"Operating context"));
    assert!(!labels(&output).contains(&"Station profiles"));
    assert!(!labels(&output).contains(&"Equipment configurations"));
    assert!(
        !labels(&output)
            .iter()
            .any(|label| label.starts_with("Manage reusable station profiles"))
    );
}

#[test]
fn narrow_operating_strip_keeps_the_power_value_and_unit_together() {
    let mut window = Window::new();
    window.ready_main_screen();
    let mut snapshot = (**window.app.settings_view.borrow().as_ref().unwrap()).clone();
    snapshot.operating.current = Some(clacks_model::profile::CurrentOperatingValues {
        station: clacks_model::contact::StationSnapshot {
            callsign: clacks_model::scalar::RecordedCallsign::new("W7POWER").unwrap(),
            location: None,
            references: Default::default(),
        },
        equipment: None,
        operator: None,
        transmit_power: Some(clacks_model::scalar::Milliwatts::new(12_500).unwrap()),
    });
    let (_, view) = watch::channel(Some(Arc::new(snapshot)));
    window.app.settings_view = view;
    theme::install(&window.context, &Default::default());
    window
        .main_frame_at_size(vec![], egui::vec2(680.0, 440.0))
        .drop_without_applying_deltas();
    let output = window.main_frame_at_size(vec![], egui::vec2(680.0, 440.0));
    let mut label = None;
    let mut unit = None;
    let mut value = None;
    for shape in &output.shapes {
        if let egui::epaint::Shape::Text(text) = &shape.shape
            && matches!(text.galley.text(), "Power" | "W" | "12.5")
        {
            let bounds = text.visual_bounding_rect();
            assert!(
                shape.clip_rect.contains_rect(bounds),
                "{} is clipped in the operating strip",
                text.galley.text()
            );
            if text.galley.text() == "Power" {
                label = Some(bounds);
            } else if text.galley.text() == "W" {
                unit = Some(bounds);
            } else {
                value = Some(bounds);
            }
        }
    }
    let label = label.unwrap();
    let unit = unit.unwrap();
    let value = value.unwrap();
    assert!(
        value.top() < unit.bottom() && unit.top() < value.bottom(),
        "power value and unit must share a row: {value:?} {unit:?}"
    );
    assert!(value.right() <= unit.left());
    assert!(
        unit.left() - value.right() < 80.0,
        "unit must follow the compact value field"
    );
    assert!((label.center().y - unit.center().y).abs() < label.height());
}

#[test]
fn manual_logging_uses_the_agreed_rows_and_two_upper_panes() {
    for appearance in [
        preferences::Appearance::Light,
        preferences::Appearance::Dark,
    ] {
        let mut window = Window::new();
        window.ready_main_screen();
        theme::install(
            &window.context,
            &preferences::AppearancePreferences {
                appearance,
                ..Default::default()
            },
        );
        window.main_frame(vec![]).drop_without_applying_deltas();
        let output = window.main_frame(vec![]);
        let bounds = |label: &str| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) if text.galley.text() == label => {
                        Some(text.visual_bounding_rect())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing {label}"))
        };
        assert!((bounds("Callsign").center().y - bounds("Name").center().y).abs() < 8.0);
        assert!(bounds("Name").left() > bounds("Callsign").right());
        for label in ["MHz", "Band", "Mode", "DX grid"] {
            assert!((bounds(label).center().y - bounds("MHz").center().y).abs() < 8.0);
        }
        for label in ["Sent", "Received", "Start · UTC", "End · UTC"] {
            assert!(
                (bounds(label).center().y - bounds("Sent").center().y).abs() < 10.0,
                "{label} belongs with the reports/timestamps row"
            );
        }
        assert!(bounds("Contact details").left() > bounds("QSO entry").right());
        assert!(bounds("Contact history").top() > bounds("Save QSO").bottom());
        assert!(!labels(&output).contains(&"Worked Before"));
        assert_eq!(
            labels(&output)
                .iter()
                .filter(|label| **label == "Start · UTC")
                .count(),
            1
        );
    }
}

#[test]
fn workbench_reset_is_available_only_through_view_layout_menu() {
    let mut window = Window::new();
    window.ready_main_screen();
    window.main_frame(vec![]).drop_without_applying_deltas();
    let output = window.main_frame(vec![]);
    assert!(labels(&output).contains(&"View"));
    assert!(!labels(&output).contains(&"Reset workbench layout"));
    window.click_main("View");
    window.click_main("Layout");
    let output = window.main_frame(vec![]);
    assert!(labels(&output).contains(&"Reset workbench layout"));
    window.click_main("Reset workbench layout");
    let output = window.main_frame(vec![]);
    assert!(!labels(&output).contains(&"Reset workbench layout"));
}
