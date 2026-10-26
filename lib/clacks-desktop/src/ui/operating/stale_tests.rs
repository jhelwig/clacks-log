//! A stale-save review belongs to its saved target, not to copied editor text.

use super::{tests::support::Window, *};

#[tokio::test]
async fn changing_equipment_target_discards_the_prior_stale_review() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    let sources = window.seed().await?;
    window.saved_settings("Equipment configurations");
    window.click_below(&equipment_label(&sources.rig_a), "Edit configuration");
    window.text("equipment.antenna", "Loop");
    let old = window
        .snapshot
        .equipment
        .iter()
        .find(|r| configuration(r).is_some_and(|e| e.id == sources.rig_a.id))
        .unwrap();
    let mut changed = sources.rig_a.clone();
    changed.values.notes = Some("Saved elsewhere".into());
    clacks_client_store::settings::save_equipment(&window.store, Some(old), &changed).await?;
    window.submit("Save configuration");
    window.settle().await;
    assert!(window.visible("Keep my values against this saved version"));
    window.click(&format!("Update {}", sources.rig_a.id));
    window.click(&format!("Update {}", equipment_label(&sources.rig_b)));
    window.frame(vec![]);
    assert!(!window.visible("Keep my values against this saved version"));
    window.submit("Save configuration");
    window.settle().await;
    let stored = clacks_client_store::settings::snapshot(&window.store).await?;
    assert!(stored.equipment.iter().filter_map(configuration).any(
        |e| e.id == sources.rig_b.id && e.values.antenna_description.as_deref() == Some("Loop")
    ));
    assert!(
        stored
            .equipment
            .iter()
            .filter_map(configuration)
            .any(|e| e.id == sources.rig_a.id
                && e.values.antenna_description.as_deref() == Some("Dipole"))
    );
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn partial_coordinates_survive_view_changes_and_apply_at_operating_precision()
-> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.click("Operating settings…");
    window.text("operating.station.call", "W7HEX");
    window.text("operating.station.grid", "CN");
    window.text("operating.station.longitude", "");
    window.text("operating.station.latitude", "47.");
    window.click("Equipment");
    window.click("Station");
    window.frame(vec![]);
    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::Operating(e) if e.station.position.latitude == "47." && e.station.position.grid == "CN")
    );
    window.text("operating.station.latitude", "47.123456");
    window.text("operating.station.longitude", "-122.654321");
    window.submit("Apply");
    window.settle().await;
    let active = clacks_client_store::operating::load(&window.store).await?;
    assert!(
        matches!(active.current.as_ref().unwrap().station.location.as_ref().unwrap().position, Some(clacks_model::position::Position::ExplicitPoint { grid_length, .. }) if grid_length == clacks_model::position::GridLength::default())
    );
    window.close().await?;
    Ok(())
}
