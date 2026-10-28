//! Query editing decisions, not parser or egui internals.

use super::*;
use crate::ui::tests::fixtures;
use egui_kittest::{Harness, kittest::Queryable};

struct Host {
    query: HistoryQueryUi,
    snapshot: SettingsSnapshot,
    definitions: DefinitionSet,
    requests: mpsc::Sender<SettingsRequest>,
    received: mpsc::Receiver<SettingsRequest>,
}

#[test]
fn invalid_query_retains_applied_results_and_completion_focus() -> anyhow::Result<()> {
    let (requests, received) = mpsc::channel(16);
    let mut host = Host {
        query: HistoryQueryUi {
            loaded: true,
            desired: Some(Query::All),
            saved: Some(QueryState::default()),
            ..Default::default()
        },
        snapshot: fixtures::snapshot(),
        definitions: DefinitionSet::new(vec![])?,
        requests,
        received,
    };
    let mut ui = Harness::builder()
        .with_context_setup(|context| crate::ui::theme::install(context, &Default::default()))
        .with_size(egui::vec2(1000.0, 250.0))
        .build_ui_state(
            |ui, host: &mut Host| {
                host.query.advance(ui.ctx(), &host.requests, false);
                host.query.show(ui, &host.snapshot, &host.definitions);
                host.query.advance(ui.ctx(), &host.requests, false);
            },
            host,
        );
    ui.get_by_label("Filter contacts").click();
    ui.get_by_label("Filter contacts").type_text("mode:SSB");
    ui.step();
    ui.input_mut().time = Some(5.0);
    ui.step();
    assert_eq!(ui.state().query.desired_text, "mode:SSB");
    assert!(matches!(
        ui.state().query.desired,
        Some(Query::Predicate(_))
    ));
    let request = ui.state_mut().received.try_recv()?;
    let SettingsAction::Contacts(ContactAction::SaveQueryState { state }) = request.action else {
        anyhow::bail!("expected real preference request")
    };
    request
        .reply
        .send(SettingsResponse::Contacts(
            ContactResponse::QueryStateSaved(state),
        ))
        .unwrap();
    ui.state_mut().query.applied("mode:SSB".into());
    ui.get_by_label("Filter contacts").type_text(" OR");
    ui.step();
    ui.input_mut().time = Some(6.0);
    ui.step();
    assert_eq!(ui.state().query.state.applied, "mode:SSB");
    assert_eq!(ui.state().query.desired_text, "mode:SSB");
    assert!(ui.state().query.diagnostic.is_some());
    assert!(ui.query_by_label("Not applied").is_some());

    // Completing in a token after Unicode text preserves both surrounding tokens.
    ui.get_by_label("Filter contacts").focus();
    ui.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    ui.get_by_label("Filter contacts")
        .type_text("comment:élève mo");
    ui.step();
    ui.key_press(egui::Key::Enter);
    ui.step();
    assert_eq!(ui.state().query.state.edited, "comment:élève mode:");
    assert!(ui.get_by_label("Filter contacts").is_focused());
    ui.key_press(egui::Key::Escape);
    ui.step();
    assert!(ui.state().query.dismissed);
    assert!(ui.output().shapes.iter().any(|shape| matches!(&shape.shape,
        egui::epaint::Shape::Text(text) if text.galley.text() == "comment:élève mode:"
            && text.galley.job.sections.iter().all(|section| section.format.font_id.family == egui::FontFamily::Monospace)
    )), "query input uses the selected radio-data font role");
    let clear = ui.get_by_label("Clear query").rect();
    let input = ui.get_by_label("Filter contacts").rect();
    assert!(clear.right() <= 1000.0 && clear.left() >= input.right());
    assert!((clear.center().y - input.center().y).abs() <= 2.0);
    ui.get_by_label("Clear query").click();
    ui.step();
    assert!(ui.state().query.state.edited.is_empty());
    ui.input_mut().time = Some(10.0);
    ui.step();
    assert_eq!(ui.state().query.desired, Some(Query::All));
    host = ui.into_state();
    assert!(host.query.error.is_none());
    Ok(())
}

#[tokio::test]
async fn shutdown_flushes_unfinished_query_without_waiting_for_debounce() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = clacks_client_store::store::ClientStore::create(
        &directory.path().join("query.sqlite"),
        Default::default(),
        Default::default(),
    )
    .await?;
    let state = QueryState {
        edited: "mode:CW OR".into(),
        applied: "mode:CW".into(),
    };
    let mut query = HistoryQueryUi {
        loaded: true,
        state: state.clone(),
        saved: Some(QueryState::default()),
        deadline: Some(100.0),
        ..Default::default()
    };
    let context = egui::Context::default();
    let (sender, mut received) = mpsc::channel(4);
    assert!(!query.flush(&context, &sender));
    let request = received.try_recv()?;
    let SettingsAction::Contacts(action) = request.action else {
        anyhow::bail!("not a contact preference")
    };
    let response = crate::session::settings::contacts::execute(&store, action).await?;
    request
        .reply
        .send(SettingsResponse::Contacts(response))
        .unwrap();
    assert!(query.flush(&context, &sender));
    let response =
        crate::session::settings::contacts::execute(&store, ContactAction::LoadQueryState).await?;
    assert!(matches!(response, ContactResponse::QueryState(saved) if saved == state));
    store.close().await?;
    Ok(())
}
