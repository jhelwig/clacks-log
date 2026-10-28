//! Prefix correlation and UTC Today are local presentation decisions.

use super::model::WorkedBeforeModel;
use crate::{
    session::settings::{
        SettingsAction, SettingsResponse,
        contacts::{ContactAction, ContactResponse},
    },
    ui::tests::fixtures,
};
use clacks_client_store::{
    logbooks::selection::SelectedView,
    read::{
        contacts::{ContactPage, ContactRow},
        history::HistoryRow,
    },
};
use clacks_model::scalar::UtcSecond;
use eframe::egui;
use tokio::sync::mpsc;

#[test]
fn worked_before_rejects_old_calls_and_refreshes_today() -> anyhow::Result<()> {
    let context = egui::Context::default();
    let (sender, mut requests) = mpsc::channel(8);
    let mut model = WorkedBeforeModel::default();
    let before_midnight = UtcSecond::from_unix_seconds(86_399);
    model.update(
        &context,
        "w7hex",
        SelectedView::AllContacts,
        Default::default(),
        before_midnight,
        &sender,
    );
    let old = requests.try_recv()?;
    let SettingsAction::Contacts(ContactAction::WorkedBefore { query }) = &old.action else {
        anyhow::bail!("not a Worked Before read")
    };
    assert_eq!(query.call_prefix, "W7HEX");
    model.update(
        &context,
        "W7HEX/P",
        SelectedView::AllContacts,
        Default::default(),
        before_midnight,
        &sender,
    );
    assert!(old.reply.is_closed());
    let current = requests.try_recv()?;
    let mut qso = fixtures::contact();
    qso.start = before_midnight;
    let page = ContactPage {
        rows: vec![ContactRow {
            working: HistoryRow {
                qso,
                pending: true,
                review: None,
            },
            facts: Default::default(),
        }],
        matching_total: 1,
        selected_total: 0,
        selected_outside_filter: 0,
        has_more: false,
    };
    current
        .reply
        .send(SettingsResponse::Contacts(ContactResponse::WorkedBefore(
            page,
        )))
        .unwrap();
    model.update(
        &context,
        "W7HEX/P",
        SelectedView::AllContacts,
        Default::default(),
        before_midnight,
        &sender,
    );
    assert_eq!(model.day, 0);
    model.update(
        &context,
        "W7HEX/P",
        SelectedView::AllContacts,
        Default::default(),
        UtcSecond::from_unix_seconds(86_400),
        &sender,
    );
    assert_eq!(model.day, 1);
    assert_eq!(model.page.as_ref().unwrap().rows.len(), 1);
    assert!(requests.try_recv().is_err());
    model.update(
        &context,
        "",
        SelectedView::AllContacts,
        Default::default(),
        before_midnight,
        &sender,
    );
    assert!(model.page.is_none());
    assert!(requests.try_recv().is_err());
    Ok(())
}

#[test]
fn two_presentations_preserve_shared_column_widths_order_and_reset() {
    use super::{WorkedBeforeUi, show_table};
    use crate::ui::contact_table::preferences::{ContactTablePreferences, TableKind};
    use clacks_client_store::read::contacts::ContactColumn;
    use egui_kittest::Harness;
    let mut view = WorkedBeforeUi::default();
    view.model.query.call_prefix = "OK2CQR".into();
    view.model.page = Some(ContactPage {
        rows: vec![ContactRow {
            working: HistoryRow {
                qso: fixtures::contact(),
                pending: true,
                review: None,
            },
            facts: Default::default(),
        }],
        has_more: false,
        matching_total: 1,
        selected_total: 0,
        selected_outside_filter: 0,
    });
    let mut ui = Harness::builder()
        .with_size(egui::vec2(1800.0, 500.0))
        .build_ui_state(
            |ui, view: &mut WorkedBeforeUi| {
                ui.columns(2, |columns| {
                    for (index, (column, interaction)) in columns
                        .iter_mut()
                        .zip([&mut view.tab, &mut view.window])
                        .enumerate()
                    {
                        column.push_id(index, |ui| {
                            show_table(ui, &mut view.model, &mut view.preferences, interaction)
                        });
                    }
                });
            },
            view,
        );
    ui.run();
    // A resize in either presentation changes their shared canonical preference.
    ui.state_mut()
        .preferences
        .columns
        .iter_mut()
        .find(|c| c.column == ContactColumn::Callsign)
        .unwrap()
        .width = 160.0;
    ui.run_steps(3);
    assert!(
        (ui.state()
            .preferences
            .columns
            .iter()
            .find(|c| c.column == ContactColumn::Callsign)
            .unwrap()
            .width
            - 160.0)
            .abs()
            <= 1.0
    );
    ui.state_mut().preferences.columns.swap(0, 1);
    ui.run_steps(3);
    assert_eq!(
        ui.state().preferences.columns.first().unwrap().column,
        ContactColumn::Callsign
    );
    assert!((ui.state().preferences.columns.first().unwrap().width - 160.0).abs() <= 1.0);
    ui.state_mut().preferences = ContactTablePreferences::defaults(TableKind::WorkedBefore);
    ui.run_steps(3);
    assert_eq!(
        ui.state().preferences.columns.first().unwrap().column,
        ContactColumn::Start
    );
    assert!(
        (ui.state()
            .preferences
            .columns
            .iter()
            .find(|c| c.column == ContactColumn::Callsign)
            .unwrap()
            .width
            - 90.0)
            .abs()
            <= 1.0
    );
}
