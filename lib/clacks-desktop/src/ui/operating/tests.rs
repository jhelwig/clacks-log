//! Actual rendered workflows use the production session and SQLite.

pub(super) mod support;

use super::*;
use support::Window;

#[tokio::test]
async fn semantic_normalization_does_not_create_false_edit_markers() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.seed().await?;
    window.click("Operating settings…");

    window.text("operating.station.call", "  w7hex  ");
    window.frame(vec![]);

    assert!(!window.visible("Not applied"));
    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::Operating(editor) if editor.selected_profile.is_some())
    );
    window.text("operating.station.call", "W7OTHER");
    window.frame(vec![]);
    assert!(window.visible("Not applied"));
    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::Operating(editor) if editor.selected_profile.is_none())
    );
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn validation_live_power_reports_errors_without_submitting() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.context.enable_accesskit();
    window.click("Operating settings…");
    window.text("operating.station.call", "W7HEX");
    window.click("Operator / power");
    window.text("operating.power", "invalid");

    window.advance(0.3);

    assert!(
        window
            .output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Apply") && node.is_disabled())
    );

    assert!(
        window
            .view
            .stack
            .last()
            .unwrap()
            .errors
            .iter()
            .any(|error| error.field == "Transmit power")
    );
    assert!(
        clacks_client_store::settings::snapshot(&window.store)
            .await?
            .operating
            .current
            .is_none()
    );
    window.text("operating.power", ".5");
    window.advance(0.3);
    assert!(window.view.stack.last().unwrap().errors.is_empty());
    assert!(
        window
            .output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .any(|(_, node)| node.label() == Some("Apply") && !node.is_disabled())
    );
    assert!(
        clacks_client_store::settings::snapshot(&window.store)
            .await?
            .operating
            .current
            .is_none()
    );
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn font_and_scale_changes_preserve_pending_operating_text() -> anyhow::Result<()> {
    use crate::ui::{
        appearance::{AppearanceAction, AppearanceUi},
        preferences::Appearance,
    };

    let mut window = Window::new().await?;
    window.click("Operating settings…");
    window.text("operating.station.call", "VE7ABC/P");
    let mut appearance = AppearanceUi::default();
    appearance.open(egui::Id::unique("test.appearance.opener"));
    let pending = &mut appearance.editor.as_mut().unwrap().pending;
    pending.appearance = Appearance::Light;
    pending.zoom = 1.25;

    appearance.finish(&window.context, AppearanceAction::Apply);
    window.frame(vec![]);

    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::Operating(editor) if editor.station.callsign == "VE7ABC/P")
    );
    assert!(
        clacks_client_store::settings::snapshot(&window.store)
            .await?
            .operating
            .current
            .is_none()
    );
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn nested_equipment_save_survives_profile_cancel_without_applying() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    window.saved_settings("Station profiles");
    window.click("New profile");
    window.text("profile.name", "Home");
    window.text("profile.station.call", "W7HEX");
    window.click("Manage equipment");
    window.click("New configuration");
    window.text("equipment.name", "Rig C");
    window.text("equipment.description", "IC-7300");
    window.submit("Save configuration");
    window.settle().await;
    window.click("Back");
    window.click("Cancel profile");
    let stored = clacks_client_store::settings::snapshot(&window.store).await?;
    assert!(stored.profiles.is_empty());
    assert_eq!(stored.equipment.len(), 1);
    assert!(stored.operating.current.is_none());
    assert!(
        matches!(stored.equipment.first().unwrap().working_state(), Some(RecordState::Live(RecordValue::Equipment(e))) if e.name == "Rig C" && e.values.equipment_description.as_deref() == Some("IC-7300"))
    );
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn independent_equipment_selection_applies_and_survives_reopen() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    let sources = window.seed().await?;
    window.click("Operating settings…");
    window.click("Equipment");
    window.click(&format!("Load {}", equipment_label(&sources.rig_b)));
    assert_eq!(
        window
            .snapshot
            .operating
            .selected_equipment
            .as_ref()
            .unwrap()
            .id,
        sources.rig_a.id
    );
    window.submit("Apply");
    window.settle().await;
    let stored = clacks_client_store::settings::snapshot(&window.store).await?;
    assert_eq!(
        stored.operating.selected_profile.as_ref().unwrap(),
        &sources.home
    );
    assert_eq!(
        stored.operating.selected_equipment.as_ref().unwrap(),
        &sources.rig_b
    );
    assert_eq!(
        stored
            .profiles
            .iter()
            .filter_map(profile)
            .find(|p| p.id == sources.home.id)
            .unwrap()
            .default_equipment,
        Some(sources.rig_a.id)
    );
    // Use the actual pool path from the fixture's workspace discovery after closing.
    let database = window.database.clone();
    window.store.close().await?;
    let reopened =
        clacks_client_store::store::ClientStore::open_discovered(&database, Default::default())
            .await?;
    assert_eq!(
        clacks_client_store::operating::load(&reopened).await?,
        stored.operating
    );
    reopened.close().await?;
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn manual_detachment_cancel_and_separate_profile_save_use_pending_values()
-> anyhow::Result<()> {
    let mut window = Window::new().await?;
    let sources = window.seed().await?;
    window.click("Operating settings…");
    window.text("operating.station.grid", "CN88");
    window.frame(vec![]);
    assert!(window.visible("Selected profile: None"));
    assert!(!window.visible("Selected profile has changed"));
    window.click("Operator / power");
    window.text("operating.power", "invalid");
    window.click("Station");
    window.click("Save station values as profile…");
    window.text("profile.name", "Field");
    window.submit("Save profile");
    window.settle().await;
    window.submit("Apply");
    window.frame(vec![]);
    assert!(window.visible("Transmit power"));
    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::Operating(e) if e.power_watts == "invalid" && e.selected_profile.is_none())
    );
    window.click("Cancel operating changes");
    let stored = clacks_client_store::settings::snapshot(&window.store).await?;
    assert_eq!(stored.operating.selected_profile, Some(sources.home));
    assert!(stored.profiles.iter().filter_map(profile).any(|p| {
        p.name == "Field"
            && p.station
                .location
                .as_ref()
                .unwrap()
                .position
                .as_ref()
                .unwrap()
                .display_locator()
                .to_string()
                == "CN88"
    }));
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn nested_saved_equipment_edits_and_source_deletions_preserve_operating_text()
-> anyhow::Result<()> {
    let mut window = Window::new().await?;
    let sources = window.seed().await?;
    window.click("Operating settings…");
    window.click("Operator / power");
    window.text("operating.power", "25");
    window.click("Station");
    window.click("Manage station profiles");
    window.click("Edit profile");
    window.click("Manage equipment");
    window.click_below(&equipment_label(&sources.rig_a), "Edit configuration");
    window.text("equipment.antenna", "Loop");
    window.submit("Save configuration");
    window.settle().await;
    window.click("Back");
    window.click("Cancel profile");
    window.click("Back");
    window.click("Equipment");
    window.frame(vec![]);
    assert!(window.visible("Selected configuration has changed"));
    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::Operating(e) if e.power_watts == "25" && e.equipment.antenna == "Dipole")
    );
    window.click("Station");
    window.click("Manage station profiles");
    window.click("Edit profile");
    window.click("Delete profile");
    window.settle().await;
    window.click("Back");
    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::Operating(e) if e.selected_profile.is_none() && e.power_watts == "25")
    );
    window.click("Cancel operating changes");
    let stored = clacks_client_store::settings::snapshot(&window.store).await?;
    assert!(stored.operating.selected_profile.is_none());
    assert_eq!(
        stored.operating.current.as_ref().unwrap().station,
        sources.home.station
    );
    assert!(
        stored
            .operating
            .current
            .as_ref()
            .unwrap()
            .transmit_power
            .is_none()
    );
    assert!(stored.equipment.iter().filter_map(configuration).any(
        |e| e.id == sources.rig_a.id && e.values.antenna_description.as_deref() == Some("Loop")
    ));
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn stale_profile_save_and_deleted_default_have_distinct_repair_paths() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    let sources = window.seed().await?;
    window.saved_settings("Station profiles");
    window.click("Edit profile");
    window.text("profile.name", "My draft");
    let record = window
        .snapshot
        .profiles
        .iter()
        .find(|r| profile(r).is_some_and(|p| p.id == sources.home.id))
        .unwrap()
        .clone();
    let mut remote = sources.home.clone();
    remote.name = "Changed elsewhere".into();
    clacks_client_store::settings::save_profile(&window.store, Some(&record), &remote).await?;
    window.submit("Save profile");
    window.settle().await;
    assert!(window.visible("The saved target changed"));
    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::Profile(e) if e.name == "My draft")
    );
    window.click("Keep my values against this saved version");
    // Remove the default under a proper full reference repair while the editor remains open.
    let review =
        clacks_client_store::settings::preview_equipment_deletion(&window.store, sources.rig_a.id)
            .await?;
    clacks_client_store::settings::delete_equipment(
        &window.store,
        &review,
        &[clacks_client_store::settings::ProfileEquipmentReplacement {
            profile: sources.home.id,
            equipment: None,
        }],
    )
    .await?;
    // That repair also changed the profile, so stale protection takes precedence.
    window.submit("Save profile");
    window.settle().await;
    assert!(window.visible("The saved target changed"));
    window.click("Keep my values against this saved version");
    window.submit("Save profile");
    window.settle().await;
    assert!(window.visible("Profile saved locally with a missing equipment reference"));
    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::Profile(e) if e.name == "My draft")
    );
    window.click(&format!(
        "Unavailable {} — replace or choose None",
        sources.rig_a.id
    ));
    window.click("None");
    window.submit("Save profile");
    window.settle().await;
    let stored = clacks_client_store::settings::snapshot(&window.store).await?;
    assert!(
        stored
            .profiles
            .iter()
            .filter_map(profile)
            .any(|p| p.name == "My draft" && p.default_equipment.is_none())
    );
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn duplicate_names_remain_distinct_and_failed_request_keeps_form_and_focus()
-> anyhow::Result<()> {
    let mut window = Window::new().await?;
    let sources = window.seed().await?;
    let mut other = sources.home.clone();
    other.id = Default::default();
    other.station.callsign = clacks_model::scalar::RecordedCallsign::new("W7OTHER")?;
    clacks_client_store::settings::save_profile(&window.store, None, &other).await?;
    window.refresh().await;
    window.saved_settings("Station profiles");
    window.click_below(&profile_label(&other), "Edit profile");
    window.text("profile.name", "Other location");
    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::Profile(e) if e.id == other.id)
    );
    // A genuine storage failure must not discard or replace the editing form.
    use sqlx::{Connection, Executor};
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&window.database),
    )
    .await?;
    connection
        .execute(include_str!("fail_profile_save.sql"))
        .await?;
    connection.close().await?;
    window.submit("Save profile");
    window.settle().await;
    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::Profile(e) if e.name == "Other location")
    );
    window.text("profile.name", "Still editing");
    assert!(
        window
            .context
            .memory(|m| m.has_focus(egui::Id::unique("profile.name")))
    );
    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::Profile(e) if e.name == "Still editing")
    );
    window.close().await?;
    Ok(())
}

#[tokio::test]
async fn equipment_deletion_refreshes_phantoms_without_partial_repairs() -> anyhow::Result<()> {
    let mut window = Window::new().await?;
    let sources = window.seed().await?;
    window.saved_settings("Equipment configurations");
    window.click_below(&equipment_label(&sources.rig_a), "Review deletion");
    window.settle().await;
    window.click("Choose treatment");
    window.click("Remove default");
    let mut extra = sources.home.clone();
    extra.id = Default::default();
    extra.name = "New referrer".into();
    clacks_client_store::settings::save_profile(&window.store, None, &extra).await?;
    window.click("Delete configuration and repair profiles");
    window.settle().await;
    assert!(window.visible("nothing was deleted"));
    assert!(
        matches!(&window.view.stack.last().unwrap().screen, Screen::DeleteEquipment(r) if r.choices.get(&sources.home.id) == Some(&None) && !r.choices.contains_key(&extra.id))
    );
    let stored = clacks_client_store::settings::snapshot(&window.store).await?;
    assert_eq!(
        stored
            .profiles
            .iter()
            .filter_map(profile)
            .filter(|p| p.default_equipment == Some(sources.rig_a.id))
            .count(),
        2
    );
    window.click("Choose treatment");
    window.click("Remove default");
    window.click("Delete configuration and repair profiles");
    window.settle().await;
    let stored = clacks_client_store::settings::snapshot(&window.store).await?;
    assert!(stored.equipment.iter().any(|r| r.key.id
        == clacks_sync::record::RecordId::Equipment(sources.rig_a.id)
        && matches!(r.working_state(), Some(RecordState::Deleted))));
    assert!(
        stored
            .profiles
            .iter()
            .filter_map(profile)
            .all(|p| p.default_equipment.is_none())
    );
    assert!(stored.operating.selected_equipment.is_none());
    assert_eq!(
        stored
            .operating
            .current
            .as_ref()
            .unwrap()
            .equipment
            .as_ref()
            .unwrap()
            .antenna_description
            .as_deref(),
        Some("Dipole")
    );
    window.close().await?;
    Ok(())
}
