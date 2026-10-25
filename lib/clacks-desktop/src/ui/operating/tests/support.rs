//! Rendered workflows exercise real requests and committed SQLite state.

use super::super::*;
use crate::{
    session::{
        self, SessionCommand, SessionStatus,
        tests::{Fixture, next_matching},
    },
    workspace,
};
use eframe::egui::{Event, Modifiers, PointerButton, Pos2, Rect, Vec2};

pub(in crate::ui::operating) struct Window {
    now: f64,
    pub(in crate::ui::operating) view: SettingsUi,
    pub(in crate::ui::operating) context: egui::Context,
    pub(in crate::ui::operating) output: egui::FullOutput,
    pub(in crate::ui::operating) sender: mpsc::Sender<SettingsRequest>,
    pub(in crate::ui::operating) settings_view:
        tokio::sync::watch::Receiver<Option<Arc<SettingsSnapshot>>>,
    pub(in crate::ui::operating) task: tokio::task::JoinHandle<anyhow::Result<()>>,
    pub(in crate::ui::operating) commands: mpsc::Sender<SessionCommand>,
    pub(in crate::ui::operating) snapshot: Arc<SettingsSnapshot>,
    pub(in crate::ui::operating) database: std::path::PathBuf,
    pub(in crate::ui::operating) store: clacks_client_store::store::ClientStore,
    pub(in crate::ui::operating) _owner: crate::instance::InstanceOwner,
    pub(in crate::ui::operating) _directory: tempfile::TempDir,
}
impl Window {
    pub(in crate::ui::operating) async fn new() -> anyhow::Result<Self> {
        let mut fixture = Fixture::new()?;
        let paths = fixture.resources.paths.clone();
        let task = tokio::spawn(session::run(
            fixture.resources,
            fixture.input,
            fixture.updates,
        ));
        next_matching(&mut fixture.status, |s| {
            matches!(s, SessionStatus::Ready { .. })
        })
        .await?;
        let snapshot = fixture.settings_view.borrow().clone().unwrap();
        let opened = workspace::open(&paths).await?;
        Ok(Self {
            now: 0.0,
            view: SettingsUi::default(),
            context: Default::default(),
            output: Default::default(),
            settings_view: fixture.settings_view,
            task,
            commands: fixture.commands,
            _owner: fixture._owner,
            sender: fixture.settings,
            snapshot,
            database: paths.database,
            store: opened.store,
            _directory: fixture._directory,
        })
    }
    pub(in crate::ui::operating) fn frame(&mut self, events: Vec<Event>) {
        self.now += 1.0 / 60.0;
        let input = egui::RawInput {
            time: Some(self.now),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1300.0, 2200.0))),
            events,
            ..Default::default()
        };
        self.output = self
            .context
            .run_ui(input, |ui| self.view.show(ui, &self.snapshot, &self.sender));
        self.output.textures_delta.clear();
    }

    pub(in crate::ui::operating) fn advance(&mut self, seconds: f64) {
        self.now += seconds;
        self.frame(vec![]);
    }

    pub(in crate::ui::operating) fn submit(&mut self, text: &str) {
        self.advance(0.300);
        self.click(text);
    }
    pub(in crate::ui::operating) fn click(&mut self, text: &str) {
        self.frame(vec![]);
        self.frame(vec![]);
        let position = self
            .output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(t) if t.galley.text() == text => {
                    Some(t.visual_bounding_rect().center())
                }
                _ => None,
            })
            .next_back()
            .unwrap_or_else(|| {
                panic!(
                    "missing rendered control: {text}; shapes: {:?}",
                    self.output
                        .shapes
                        .iter()
                        .filter_map(|shape| match &shape.shape {
                            egui::epaint::Shape::Text(t) => Some(t.galley.text()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                )
            });
        self.frame(vec![
            Event::PointerMoved(position),
            Event::PointerButton {
                pos: position,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ]);
        self.frame(vec![Event::PointerButton {
            pos: position,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }]);
    }

    pub(in crate::ui::operating) fn click_below(&mut self, anchor: &str, text: &str) {
        self.frame(vec![]);
        self.frame(vec![]);
        let anchor_y = self
            .output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(t) if t.galley.text() == anchor => {
                    Some(t.visual_bounding_rect().center().y)
                }
                _ => None,
            })
            .expect("record must be rendered");
        let position = self
            .output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(t)
                    if t.galley.text() == text
                        && t.visual_bounding_rect().center().y > anchor_y =>
                {
                    Some(t.visual_bounding_rect().center())
                }
                _ => None,
            })
            .min_by(|a, b| a.y.total_cmp(&b.y))
            .expect("record action must be rendered");
        self.frame(vec![
            Event::PointerMoved(position),
            Event::PointerButton {
                pos: position,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ]);
        self.frame(vec![Event::PointerButton {
            pos: position,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }]);
    }
    pub(in crate::ui::operating) fn text(&mut self, id: &str, value: &str) {
        self.frame(vec![]);
        self.context
            .memory_mut(|m| m.request_focus(egui::Id::unique(id)));
        self.frame(vec![
            Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            },
            Event::Key {
                key: egui::Key::Backspace,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text(value.into()),
        ]);
    }
    pub(in crate::ui::operating) async fn settle(&mut self) {
        tokio::time::timeout(std::time::Duration::from_secs(15), async {
            loop {
                let snapshot = self.settings_view.borrow_and_update().clone();
                if let Some(snapshot) = snapshot {
                    self.snapshot = snapshot;
                }
                self.frame(vec![]);
                if self.view.stack.last().is_none_or(|f| f.response.is_none())
                    && self.view.dismissed.is_empty()
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    pub(in crate::ui::operating) async fn close(self) -> anyhow::Result<()> {
        self.store.close().await?;
        self.commands.send(SessionCommand::BeginShutdown).await?;
        tokio::time::timeout(std::time::Duration::from_secs(15), self.task).await???;
        Ok(())
    }
}

pub(in crate::ui::operating) struct Sources {
    pub(in crate::ui::operating) home: SavedLocationProfile,
    pub(in crate::ui::operating) rig_a: EquipmentConfiguration,
    pub(in crate::ui::operating) rig_b: EquipmentConfiguration,
}
impl Window {
    pub(in crate::ui::operating) async fn seed(&mut self) -> anyhow::Result<Sources> {
        use clacks_model::{
            contact::StationSnapshot, equipment::EquipmentValues, scalar::RecordedCallsign,
        };
        let rig_a = EquipmentConfiguration {
            id: Default::default(),
            name: "Rig A".into(),
            values: EquipmentValues {
                equipment_description: Some("IC-7300".into()),
                antenna_description: Some("Dipole".into()),
                notes: None,
            },
        };
        let rig_b = EquipmentConfiguration {
            id: Default::default(),
            name: "Rig B".into(),
            values: EquipmentValues {
                equipment_description: Some("KX3".into()),
                ..Default::default()
            },
        };
        let home = SavedLocationProfile {
            id: Default::default(),
            name: "Home".into(),
            station: StationSnapshot {
                callsign: RecordedCallsign::new("W7HEX")?,
                location: None,
                references: Default::default(),
            },
            default_equipment: Some(rig_a.id),
        };
        clacks_client_store::settings::save_equipment(&self.store, None, &rig_a).await?;
        clacks_client_store::settings::save_equipment(&self.store, None, &rig_b).await?;
        clacks_client_store::settings::save_profile(&self.store, None, &home).await?;
        self.click("Refresh settings");
        self.settle().await;
        self.click("Operating settings");
        self.click(&format!("Load {}", profile_label(&home)));
        self.click("Apply");
        self.settle().await;
        self.snapshot = self.settings_view.borrow().clone().unwrap();
        Ok(Sources { home, rig_a, rig_b })
    }
    pub(in crate::ui::operating) fn visible(&self, text: &str) -> bool {
        self.output.shapes.iter().any(|shape| matches!(&shape.shape, egui::epaint::Shape::Text(t) if t.galley.text().contains(text)))
    }
    pub(in crate::ui::operating) async fn refresh(&mut self) {
        self.click("Refresh settings");
        self.settle().await;
        self.snapshot = self.settings_view.borrow().clone().unwrap();
        self.frame(vec![]);
    }
}
