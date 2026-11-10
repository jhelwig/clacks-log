use super::*;
use clacks_geography::{artifact::ParentScope, lookup::Candidate};
use egui_kittest::{Harness, kittest::Queryable};

/// Candidate interaction state belonging to a real component host.
struct CandidateHost {
    /// Current operation's geographic presentation.
    view: GeographyView,
    /// Pending station values changed by the control.
    form: StationForm,
}

#[test]
fn candidate_selection_records_value_and_distinguishes_failure_from_unknown() {
    let mut form = StationForm::empty();
    form.edit_grid("JJ00".into());
    let unknown = LookupResult {
        release_id: "authored".into(),
        dxcc: FieldLookup {
            coverage: Coverage::None,
            candidates: vec![],
        },
        primary_subdivision: FieldLookup {
            coverage: Coverage::None,
            candidates: vec![],
        },
        secondary_subdivision: FieldLookup {
            coverage: Coverage::None,
            candidates: vec![],
        },
        cq_zone: FieldLookup {
            coverage: Coverage::None,
            candidates: vec![],
        },
        itu_zone: FieldLookup {
            coverage: Coverage::None,
            candidates: vec![],
        },
    };
    let mut result = unknown.clone();
    result.primary_subdivision = FieldLookup {
        coverage: Coverage::Complete,
        candidates: [("WA", "Washington"), ("OR", "Oregon")]
            .into_iter()
            .map(|(value, label)| Candidate {
                value: value.into(),
                display_label: label.into(),
                parent_scope: ParentScope::Entity("291".into()),
                source_identities: vec![],
                derivation_keys: vec![],
            })
            .collect(),
    };
    let _ = form.take_geographic_intent();
    let view = GeographyView {
        basis: form.position.build_recorded().unwrap(),
        result: Some(result),
        ..Default::default()
    };
    let mut ui = Harness::builder()
        .with_size(egui::vec2(600., 500.))
        .build_ui_state(
            |ui, state: &mut CandidateHost| {
                state.view.show(ui, &mut state.form);
            },
            CandidateHost { view, form },
        );
    ui.get_by_label("Choose State / province").click();
    ui.run();
    ui.get_by_label("Oregon · DXCC 291").click();
    ui.run();
    assert_eq!(ui.state().form.primary_subdivision, "OR");
    assert!(
        ui.state()
            .form
            .geographic_protection
            .contains(&Field::PrimarySubdivision)
    );
    ui.state_mut().form.edit_grid("JJ01".into());
    ui.run();
    assert!(ui.query_by_label("Choose State / province").is_none());
    let _ = ui.state_mut().form.take_geographic_intent();
    let basis = ui.state().form.position.build_recorded().unwrap();
    ui.state_mut().view.basis = basis;
    ui.state_mut().view.result = Some(unknown);
    ui.run();
    assert!(
        ui.query_by_label("State / province: No geographic coverage")
            .is_some()
    );

    let (commands, _requests) = mpsc::channel(1);
    let (reply, receiver) = oneshot::channel();
    {
        let state = ui.state_mut();
        state.view = GeographyView {
            basis: state.form.position.build_recorded().unwrap(),
            pending: Some(receiver),
            ..Default::default()
        };
    }
    reply
        .send(Err(GeographyFailure::QueryFailed(std::sync::Arc::new(
            anyhow::anyhow!("reader failure"),
        ))))
        .unwrap();
    {
        let state = ui.state_mut();
        state.view.advance(
            &mut state.form,
            &commands,
            &GeographyStatus::Opening,
            false,
            "component_test",
        );
    }
    ui.run();
    assert!(
        ui.query_by_label("Geographic lookup failed. Fields can still be entered manually.")
            .is_some()
    );
    assert!(
        ui.query_by_label("State / province: No geographic coverage")
            .is_none()
    );
}
