//! Selection responses belong to their original applied view and selection intent.
use super::*;

#[test]
fn select_all_history_focus_is_owned_by_the_actual_table() -> anyhow::Result<()> {
    use egui_kittest::kittest::Queryable;
    let (sender, mut requests) = mpsc::channel(8);
    let mut ui = Harness::builder()
        .with_size(egui::vec2(1100.0, 500.0))
        .build_ui_state(
            move |ui, view: &mut ContactsUi| {
                view.show(ui, &fixtures::snapshot(), &sender, &Default::default());
            },
            ContactsUi::default(),
        );
    requests
        .try_recv()?
        .reply
        .send(SettingsResponse::Contacts(ContactResponse::QueryState(
            Default::default(),
        )))
        .unwrap();
    ui.run_steps(2);
    requests
        .try_recv()?
        .reply
        .send(SettingsResponse::Contacts(ContactResponse::Page(
            fixtures::contact_page(0, 1, 1),
        )))
        .unwrap();
    ui.run_steps(2);
    ui.get_by_label("K1T000").click();
    ui.run_steps(2);
    assert!(ui.state().history_has_focus(&ui.ctx));
    ui.get_by_label("Filter contacts").click();
    ui.run_steps(2);
    assert!(!ui.state().history_has_focus(&ui.ctx));
    Ok(())
}

#[test]
fn select_all_uses_applied_filter_and_preserves_hidden_selection() {
    let (sender, mut requests) = mpsc::channel(8);
    let hidden = QsoId::new();
    let another_hidden = QsoId::new();
    let mut view = ContactsUi {
        selected: [hidden, another_hidden].into(),
        ..Default::default()
    };
    view.filter.state.applied = "mode:SSB".into();
    view.filter.state.edited = "mode:CW OR".into();
    view.filter.desired_text = "mode:CW".into();
    view.query.show_selected = true;
    view.select_all(&sender);
    assert!(view.selection_pending());
    assert_eq!(view.selected, [hidden, another_hidden].into());
    let request = requests.try_recv().unwrap();
    let SettingsAction::Contacts(ContactAction::SelectAll { query }) = request.action else {
        panic!("selection");
    };
    assert!(!query.show_selected);
    assert_eq!(query.offset, 0);
    assert_eq!(
        query.filter,
        clacks_model::query::parse(
            "mode:SSB",
            clacks_model::recognition::RecognitionCatalog::adif_318().unwrap()
        )
        .unwrap()
    );
    let matches: BTreeSet<_> = (0..205).map(|_| QsoId::new()).collect();
    request
        .reply
        .send(SettingsResponse::Contacts(ContactResponse::SelectedAll(
            matches.clone(),
        )))
        .unwrap();
    view.poll_selection();
    assert_eq!(view.selected.len(), 207);
    assert!(view.selected.contains(&hidden));
    assert!(matches.is_subset(&view.selected));
    assert!(!view.selection_pending());
}

#[test]
fn select_all_rejects_obsolete_results_and_preserves_selection_on_failure() {
    for changed in ["view", "filter", "selection", "failure"] {
        let (sender, mut requests) = mpsc::channel(8);
        let old = QsoId::new();
        let mut view = ContactsUi {
            selected: [old].into(),
            ..Default::default()
        };
        view.select_all(&sender);
        let request = requests.try_recv().unwrap();
        match changed {
            "view" => view.refresh(SelectedView::Untagged),
            "filter" => view.filter.state.applied = "mode:CW".into(),
            "selection" => view.selected.clear(),
            _ => {}
        }
        let expected = view.selected.clone();
        let result = if changed == "failure" {
            SettingsResponse::Failed("Cannot read contacts".into())
        } else {
            SettingsResponse::Contacts(ContactResponse::SelectedAll([QsoId::new()].into()))
        };
        let _ = request.reply.send(result);
        view.poll_selection();
        assert_eq!(view.selected, expected, "{changed}");
        assert!(!view.selection_pending());
    }
}
