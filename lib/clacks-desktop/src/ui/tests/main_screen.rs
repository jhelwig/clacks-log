//! Main-screen hierarchy keeps operating controls prominent and diagnostics actionable.

use super::*;

impl Window {
    fn ready_main_screen(&mut self) {
        let identity = clacks_client_store::identity::WorkspaceStatus {
            account: Default::default(),
            enrolled: false,
        };
        self.updates.send_replace(SessionStatus::Ready { identity });
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

    fn main_frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
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

fn labels(output: &egui::FullOutput) -> Vec<&str> {
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
fn main_screen_groups_existing_tools_and_discloses_workspace_details_on_request() {
    let mut window = Window::new();
    window.ready_main_screen();
    let output = window.main_frame(vec![]);
    let text = labels(&output);
    assert!(text.contains(&"Operating context"));
    assert!(text.contains(&"Saved settings"));
    assert!(text.contains(&"Station profiles"));
    assert!(text.contains(&"Equipment configurations"));
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
                    "Operating settings",
                    "Station profiles",
                    "Equipment configurations",
                    "Refresh settings",
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
