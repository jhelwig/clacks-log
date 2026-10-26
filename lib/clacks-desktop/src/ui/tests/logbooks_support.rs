//! Actual desktop App input against the production session and SQLite workspace.

use super::*;
use crate::session::tests::{Fixture, next_matching};
use clacks_client_store::{settings, store::ClientStore};

pub(in crate::ui) struct LiveWindow {
    pub window: Window,
    pub output: egui::FullOutput,
    pub store: ClientStore,
    pub paths: WorkspacePaths,
    pub now: f64,
    pub size: egui::Vec2,
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
    commands: mpsc::Sender<SessionCommand>,
    _directory: tempfile::TempDir,
    _owner: crate::instance::InstanceOwner,
}

impl LiveWindow {
    pub async fn new() -> anyhow::Result<Self> {
        let mut fixture = Fixture::new()?;
        let paths = fixture.resources.paths.clone();
        let mut window = Window::new();
        window.app.status = fixture.status.clone();
        window.app.settings_view = fixture.settings_view.clone();
        window.app.settings_requests = fixture.settings.clone();
        window.app.commands = fixture.commands.clone();
        window.app.paths = paths.clone();
        let task = tokio::spawn(crate::session::run(
            fixture.resources,
            fixture.input,
            fixture.updates,
        ));
        next_matching(&mut fixture.status, |state| {
            matches!(state, SessionStatus::Ready { .. })
        })
        .await?;
        let store = ClientStore::open_discovered(&paths.database, Default::default()).await?;
        Ok(Self {
            window,
            output: Default::default(),
            store,
            paths,
            now: 0.0,
            size: egui::vec2(1100.0, 900.0),
            task,
            commands: fixture.commands,
            _directory: fixture._directory,
            _owner: fixture._owner,
        })
    }

    pub fn frame(&mut self, events: Vec<egui::Event>) {
        use eframe::App;
        self.now += 1.0 / 60.0;
        let input = egui::RawInput {
            time: Some(self.now),
            events,
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
            ..Default::default()
        };
        let mut frame = eframe::Frame::_new_kittest();
        self.output = self
            .window
            .context
            .run_ui(input, |ui| self.window.app.ui(ui, &mut frame));
        self.output.textures_delta.clear();
    }

    pub fn click(&mut self, label: &str) {
        self.frame(vec![]);
        self.frame(vec![]);
        let position = self
            .output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.visual_bounding_rect().center())
                }
                _ => None,
            })
            .next_back()
            .unwrap_or_else(|| {
                panic!(
                    "missing {label}; visible {:?}",
                    main_screen::labels(&self.output)
                )
            });
        for pressed in [true, false] {
            self.frame(vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
    }

    pub fn text(&mut self, value: &str) {
        self.window
            .context
            .memory_mut(|memory| memory.request_focus(egui::Id::unique("logbook.name")));
        self.frame(vec![
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            },
            egui::Event::Text(value.into()),
        ]);
        self.now += 0.4;
        self.frame(vec![]);
    }

    pub async fn settle(&mut self) -> anyhow::Result<()> {
        let (reply, result) = tokio::sync::oneshot::channel();
        self.window
            .app
            .settings_requests
            .send(SettingsRequest {
                action: crate::session::settings::SettingsAction::Refresh,
                reply,
            })
            .await?;
        tokio::time::timeout(Duration::from_secs(15), result).await??;
        self.frame(vec![]);
        self.frame(vec![]);
        Ok(())
    }

    pub async fn snapshot(&self) -> anyhow::Result<SettingsSnapshot> {
        settings::snapshot(&self.store).await
    }

    pub async fn close(self) -> anyhow::Result<()> {
        self.store.close().await?;
        self.commands.send(SessionCommand::BeginShutdown).await?;
        tokio::time::timeout(Duration::from_secs(15), self.task).await???;
        Ok(())
    }
}

/// Populate actual saved contacts for the native deletion workflow.
pub(in crate::ui) async fn seed_contacts(
    app: &mut LiveWindow,
) -> anyhow::Result<clacks_model::ids::TagId> {
    use clacks_client_store::{logbooks::tags, save};
    use clacks_model::{
        contact::{BandFrequency, OwnOperatingContext, Qso, StationSnapshot},
        ids::{QsoId, TagId},
        recognition::mode::{Mode, ModeIdentity},
        scalar::{Hertz, RecordedCallsign, UtcSecond},
        tag::LogbookTag,
    };
    use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};
    let tag = LogbookTag::new(TagId::new(), "Main".into());
    tags::save(&app.store, None, &tag).await?;
    for shared in [false, true] {
        let qso = Qso {
            id: QsoId::new(),
            start: UtcSecond::from_unix_seconds(1000),
            end: None,
            own_station: StationSnapshot {
                callsign: RecordedCallsign::new("W7HEX")?,
                location: None,
                references: Default::default(),
            },
            contacted_station: StationSnapshot {
                callsign: RecordedCallsign::new("K1ABC")?,
                location: None,
                references: Default::default(),
            },
            own_context: OwnOperatingContext::default(),
            contacted_operator: None,
            primary: BandFrequency::from_recorded_pair(Some(Hertz::new(14_074_000)?), None)?,
            receive_override: None,
            mode: Mode::from_recorded_identity(ModeIdentity::Parent { label: "CW".into() })?,
            sent_report: None,
            received_report: None,
            comment: None,
            tags: if shared {
                [tag.id, TagId::new()].into()
            } else {
                [tag.id].into()
            },
        };
        save::save(
            &app.store,
            RecordKey {
                account: app.store.account_id(),
                id: RecordId::Qso(qso.id),
            },
            None,
            RecordState::Live(RecordValue::Qso(Box::new(qso))),
        )
        .await?;
    }
    app.settle().await?;
    Ok(tag.id)
}
