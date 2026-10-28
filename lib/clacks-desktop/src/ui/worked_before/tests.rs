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
    assert!(model.today(before_midnight));
    model.update(
        &context,
        "W7HEX/P",
        SelectedView::AllContacts,
        Default::default(),
        UtcSecond::from_unix_seconds(86_400),
        &sender,
    );
    assert!(!model.today(before_midnight));
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
