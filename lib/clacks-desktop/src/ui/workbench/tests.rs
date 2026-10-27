//! The workbench keeps history allocated even when no table rows are present.

use super::*;
use crate::{
    logging::{form::ContactForm, time::ContactTimestamp},
    session::settings::{SettingsAction, SettingsResponse, contacts},
    ui::{
        preferences::{Appearance, AppearancePreferences},
        theme,
    },
};
use clacks_client_store::{
    settings,
    store::{ClientStore, StoreSettings},
};
use clacks_model::{group::DefinitionSet, ids::QsoId, scalar::UtcSecond};

#[tokio::test]
async fn history_fills_remaining_workbench_bounds_when_empty_or_populated() -> anyhow::Result<()> {
    for populated in [false, true] {
        let directory = tempfile::tempdir()?;
        let store = ClientStore::create(
            &directory.path().join("bounds.sqlite"),
            Default::default(),
            StoreSettings::default(),
        )
        .await?;
        if populated {
            let mut form = ContactForm::empty();
            form.own.callsign = "W7HEX".into();
            form.dx.callsign = "K1BOUNDS".into();
            form.primary.band = "20m".into();
            form.mode = "SSB".into();
            form.start = ContactTimestamp::recorded(Some(UtcSecond::from_unix_seconds(1)));
            let qso = form
                .build(QsoId::new(), &DefinitionSet::default())
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            contacts::execute(
                &store,
                contacts::ContactAction::Save {
                    expected: None,
                    next: Box::new(qso),
                },
            )
            .await?;
        }
        let snapshot = settings::snapshot(&store).await?;
        for appearance in [Appearance::Light, Appearance::Dark] {
            for size in [egui::vec2(1100.0, 800.0), egui::vec2(1500.0, 1050.0)] {
                let context = egui::Context::default();
                theme::install(
                    &context,
                    &AppearancePreferences {
                        appearance,
                        ..Default::default()
                    },
                );
                let mut workbench = Workbench::default();
                let (sender, mut requests) = mpsc::channel(8);
                let mut available = egui::Rect::NOTHING;
                let mut output = None;
                for _ in 0..3 {
                    let mut next = context.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                            ..Default::default()
                        },
                        |ui| {
                            available = ui.available_rect_before_wrap();
                            workbench.show(ui, &snapshot, &sender);
                        },
                    );
                    next.textures_delta.clear();
                    output = Some(next);
                    while let Ok(request) = requests.try_recv() {
                        let SettingsAction::Contacts(action) = request.action else {
                            anyhow::bail!("unexpected operation")
                        };
                        request
                            .reply
                            .send(SettingsResponse::Contacts(
                                contacts::execute(&store, action).await?,
                            ))
                            .map_err(|_| anyhow::anyhow!("view response dropped"))?;
                    }
                }
                let output = output.unwrap();
                let colors = theme::palette(context.theme());
                let section = |heading: &str| {
                    let label = output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::epaint::Shape::Text(text) if text.galley.text() == heading => {
                                Some(text.visual_bounding_rect())
                            }
                            _ => None,
                        })
                        .unwrap_or_else(|| panic!("missing {heading}"));
                    output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::epaint::Shape::Rect(rect)
                                if rect.fill == colors.surface
                                    && rect.stroke.color == colors.border
                                    && rect.rect.contains_rect(label) =>
                            {
                                Some(rect.rect)
                            }
                            _ => None,
                        })
                        .unwrap_or_else(|| panic!("missing section frame for {heading}"))
                };
                let frame = section("Contact history");
                let quick = section("QSO entry");
                let details = section("Contact details");
                for pane in [quick, details] {
                    assert!(
                        (pane.top() - available.top()).abs() <= 2.0,
                        "upper pane must start at the workbench top: {pane:?}, {available:?}"
                    );
                    assert!(
                        (pane.bottom() - frame.top()).abs() <= 2.0,
                        "upper pane must extend to history: {pane:?}, {frame:?}"
                    );
                }
                assert!((quick.left() - frame.left()).abs() <= 2.0);
                assert!((details.right() - frame.right()).abs() <= 2.0);
                assert!((quick.right() - details.left()).abs() <= 2.0);
                assert!(
                    (frame.left() - available.left()).abs() <= 2.0,
                    "history left: {frame:?}, available {available:?}"
                );
                assert!(
                    (frame.right() - available.right()).abs() <= 2.0,
                    "history must span remaining width: {frame:?}, available {available:?}"
                );
                assert!(
                    (frame.bottom() - available.bottom()).abs() <= 2.0,
                    "history must fill remaining height: {frame:?}, available {available:?}"
                );
                assert!(
                    (frame.top() - (available.top() + workbench.sizes.height)).abs() <= 2.0,
                    "history starts immediately below upper panes"
                );
                let expected = if populated {
                    "K1BOUNDS"
                } else {
                    "No contacts in this view."
                };
                assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::epaint::Shape::Text(text) if text.galley.text() == expected)));
            }
        }
        store.close().await?;
    }
    Ok(())
}
