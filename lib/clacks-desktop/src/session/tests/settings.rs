use super::*;
use crate::session::settings::{SettingsAction, SettingsRequest, SettingsResponse};
use clacks_client_store::{
    operating::{OperatingSaveOutcome, OperatingState},
    save::SaveOutcome,
};
use clacks_model::scalar::RecordedCallsign;
use tokio::sync::oneshot;

/// Submit through the production session worker and await its own acknowledgment.
async fn contact_request(
    sender: &mpsc::Sender<SettingsRequest>,
    action: super::super::settings::contacts::ContactAction,
) -> anyhow::Result<super::super::settings::contacts::ContactResponse> {
    let (reply, response) = oneshot::channel();
    sender
        .send(SettingsRequest {
            action: SettingsAction::Contacts(action),
            reply,
        })
        .await?;
    match tokio::time::timeout(Duration::from_secs(5), response).await?? {
        SettingsResponse::Contacts(response) => Ok(response),
        response => anyhow::bail!("Unexpected contact outcome: {response:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn bulk_lookup_leaves_session_saves_responsive_and_drains_before_close() -> anyhow::Result<()>
{
    use crate::directory::{
        lookup::{
            bulk::report::BulkCounts,
            tests::{bulk::finish, support},
        },
        test_support,
    };
    use crate::session::settings::contacts::{ContactAction, ContactResponse};
    use clacks_model::ids::QsoId;
    let (held, release) = support::delayed();
    let provider = test_support::fixture(vec![support::login(), support::found(), held]).await?;
    let mut fixture = Fixture::new()?;
    let paths = fixture.resources.paths.clone();
    let requests = fixture.settings.clone();
    let mut lookup = fixture.lookup.clone();
    let task = tokio::spawn(run(fixture.resources, fixture.input, fixture.updates));
    next_matching(&mut fixture.status, |s| {
        matches!(s, SessionStatus::Ready { .. })
    })
    .await?;
    let client = tokio::time::timeout(
        Duration::from_secs(5),
        lookup.wait_for(|value| value.is_some()),
    )
    .await??
    .clone()
    .unwrap();
    support::configure_client(&client, provider.endpoint.clone())?;
    client.request(support::key("OK2CQR")).wait().await;
    let first: QsoId = ulid::Ulid::from(1_u128).to_string().parse()?;
    let second: QsoId = ulid::Ulid::from(2_u128).to_string().parse()?;
    for (id, call) in [(first, "OK2CQR"), (second, "W7WAIT")] {
        let outcome = contact_request(
            &requests,
            ContactAction::Save {
                expected: None,
                next: Box::new(support::contact(id, call)),
                directory: None,
            },
        )
        .await?;
        assert!(matches!(
            outcome,
            ContactResponse::Saved(SaveOutcome::Saved(_))
        ));
    }
    let ContactResponse::DirectoryLookupStarted(mut bulk) = contact_request(
        &requests,
        ContactAction::StartDirectoryLookup {
            ids: [first, second].into(),
        },
    )
    .await?
    else {
        panic!("bulk admission");
    };
    support::requests(&provider, 3).await;
    let extra = support::contact(QsoId::new(), "W7EXTRA");
    assert!(matches!(
        contact_request(
            &requests,
            ContactAction::Save {
                expected: None,
                next: Box::new(extra.clone()),
                directory: None
            }
        )
        .await?,
        ContactResponse::Saved(SaveOutcome::Saved(_))
    ));
    let ContactResponse::Page(page) = contact_request(
        &requests,
        ContactAction::Page {
            query: Default::default(),
        },
    )
    .await?
    else {
        panic!("page");
    };
    assert_eq!(page.matching_total, 3);
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    tokio::time::timeout(Duration::from_secs(15), task).await???;
    let report = finish(&mut bulk).await?;
    assert_eq!(
        report.counts,
        BulkCounts {
            new_updated: 1,
            unprocessed: 1,
            ..Default::default()
        }
    );
    let _ = release.send(include_str!("../../directory/providers/fixtures/qrz.xml"));
    let reopened = crate::workspace::open(&paths).await?;
    let mut snapshot = clacks_client_store::read::snapshot(&reopened.store).await?;
    let page = snapshot.contact_page(&Default::default()).await?;
    assert_eq!(page.matching_total, 3);
    let directory = snapshot
        .record(clacks_sync::record::RecordKey {
            account: reopened.store.account_id(),
            id: clacks_sync::record::RecordId::Directory(first),
        })
        .await?;
    assert!(directory.is_some());
    snapshot.close().await?;
    reopened.store.close().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn settings_requests_save_without_apply_then_publish_committed_operation()
-> anyhow::Result<()> {
    let mut fixture = Fixture::new()?;
    let paths = fixture.resources.paths.clone();
    let requests = fixture.settings.clone();
    let mut settings = fixture.settings_view.clone();
    let task = tokio::spawn(run(fixture.resources, fixture.input, fixture.updates));
    next_matching(&mut fixture.status, |s| {
        matches!(s, SessionStatus::Ready { .. })
    })
    .await?;
    let initial = settings.borrow_and_update().clone().unwrap();
    assert_eq!(initial.operating, OperatingState::default());
    let home = clacks_model::profile::SavedLocationProfileFields {
        id: Default::default(),
        name: "Home".into(),
        station: clacks_model::contact::StationSnapshot {
            callsign: RecordedCallsign::new("W7HEX")?,
            location: None,
            references: Default::default(),
        },
        default_equipment: None,
    };
    let (reply, response) = oneshot::channel();
    requests
        .send(SettingsRequest {
            action: SettingsAction::SaveProfile {
                expected: None,
                next: (home.clone()).try_into().expect("valid native fixture"),
            },
            reply,
        })
        .await?;
    let response = tokio::time::timeout(Duration::from_secs(15), response).await??;
    assert!(
        matches!(response, SettingsResponse::SaveProfile(ref result) if matches!(result.outcome, SaveOutcome::Saved(_)))
    );
    while settings
        .borrow_and_update()
        .as_ref()
        .unwrap()
        .profiles
        .is_empty()
    {
        settings.changed().await?;
    }
    assert_eq!(
        settings.borrow().as_ref().unwrap().operating,
        OperatingState::default()
    );
    let desired = OperatingState::new(
        Some(clacks_model::profile::CurrentOperatingValues {
            station: home.station.clone(),
            operator: None,
            transmit_power: None,
            equipment: None,
        }),
        Some((home).try_into().expect("valid native fixture")),
        None,
    )
    .expect("coherent operating fixture");
    let (reply, response) = oneshot::channel();
    requests
        .send(SettingsRequest {
            action: SettingsAction::Apply {
                expected: OperatingState::default(),
                next: desired.clone(),
            },
            reply,
        })
        .await?;
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(15), response).await??,
        SettingsResponse::Apply(OperatingSaveOutcome::Saved(_))
    ));
    assert_eq!(settings.borrow().as_ref().unwrap().operating, desired);
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    tokio::time::timeout(Duration::from_secs(15), task).await???;
    let reopened = crate::workspace::open(&paths).await?;
    assert_eq!(reopened.settings.operating, desired);
    reopened.store.close().await?;
    Ok(())
}

#[test]
fn export_session_publishes_reviewed_values_after_the_selected_contact_changes()
-> anyhow::Result<()> {
    crate::telemetry::tests::child_case("adi-export")
}

/// Exercise export and its local telemetry in an isolated subscriber process.
pub(crate) async fn export_session_isolated() -> anyhow::Result<()> {
    use crate::{
        directory::lookup::tests::support,
        export::{ExportFormat, publish::ExportDestination},
        session::{
            export::{Action, History, Reply, Request, Scope},
            settings::contacts::{ContactAction, ContactResponse},
        },
    };
    use clacks_model::{ids::QsoId, query::Query};
    use qso_io::adif::adi::parser::{
        options::ParseOpts,
        strict::{Section, StrictParser},
    };
    use std::collections::BTreeMap;

    let mut fixture = Fixture::with_log_filter(
        tempfile::tempdir()?,
        Some("warn,clacks_desktop::session::export=info"),
    )?;
    let destination = fixture._directory.path().join("reviewed.adi");
    let log_directory = fixture.resources.paths.log_directory.clone();
    let task = tokio::spawn(run(fixture.resources, fixture.input, fixture.updates));
    let SessionStatus::Ready { identity, .. } = next_matching(&mut fixture.status, |state| {
        matches!(state, SessionStatus::Ready { .. })
    })
    .await?
    else {
        anyhow::bail!("The disposable workspace must be ready for export");
    };
    let issue = |action| {
        let (reply, result) = oneshot::channel();
        (
            SessionCommand::Export(Request {
                account: identity.account,
                attempt: 1,
                action,
                stop: CancellationToken::new(),
                reply,
            }),
            result,
        )
    };
    let saved = QsoId::new();
    let ContactResponse::Saved(SaveOutcome::Saved(expected)) = contact_request(
        &fixture.settings,
        ContactAction::Save {
            expected: None,
            next: Box::new(support::contact(saved, "K1OLD")),
            directory: None,
        },
    )
    .await?
    else {
        anyhow::bail!("The selected contact must first be saved to the workspace");
    };
    assert!(matches!(
        contact_request(
            &fixture.settings,
            ContactAction::Save {
                expected: None,
                next: Box::new(support::contact(QsoId::new(), "K1OTHER")),
                directory: None,
            }
        )
        .await?,
        ContactResponse::Saved(SaveOutcome::Saved(_))
    ));
    let (command, result) = issue(Action::Prepare {
        history: Arc::new(History {
            view: Default::default(),
            filter: Query::All,
            selected: [saved].into(),
        }),
        scope: Scope::Selected,
        format: ExportFormat::Ordinary,
        capture: None,
        resolutions: BTreeMap::new(),
    });
    fixture.commands.send(command).await?;
    let Reply::Prepared { capture, plan } = tokio::time::timeout(Duration::from_secs(5), result)
        .await??
        .map_err(anyhow::Error::msg)?
    else {
        anyhow::bail!("Preparation must return the reviewed contact and plan");
    };
    assert!(matches!(
        contact_request(
            &fixture.settings,
            ContactAction::Save {
                expected: Some(Box::new(expected)),
                next: Box::new(support::contact(saved, "K1NEW")),
                directory: None,
            }
        )
        .await?,
        ContactResponse::Saved(SaveOutcome::Saved(_))
    ));

    let (command, result) = issue(Action::Publish {
        plan,
        captured_workspace_stop: capture.workspace_stop.clone(),
        destination: ExportDestination::Ordinary {
            path: destination.clone(),
            replace: false,
        },
    });
    fixture.commands.send(command).await?;
    let Reply::Published(report) = tokio::time::timeout(Duration::from_secs(5), result)
        .await??
        .map_err(anyhow::Error::msg)?
    else {
        anyhow::bail!("Publication must report the actual export outcome");
    };
    let report = report?;
    assert_eq!(report.paths, std::slice::from_ref(&destination));
    assert_eq!(report.entries_written, 1);
    assert_eq!(report.qsos_exported, 1);
    let adi = std::fs::read(destination)?;
    let mut parser = StrictParser::new(&adi, ParseOpts::default());
    let mut calls = Vec::new();
    for section in &mut parser {
        if let Section::Record(record) = section.map_err(|error| anyhow::anyhow!("{error}"))? {
            let call = record
                .field("CALL")?
                .ok_or_else(|| anyhow::anyhow!("The exported contact must have a callsign"))?;
            calls.push(call.text().to_owned());
        }
    }
    assert_eq!(calls, ["K1OLD"]);
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    tokio::time::timeout(Duration::from_secs(15), task).await???;

    let traces = std::fs::read_dir(log_directory)?
        .map(|entry| std::fs::read_to_string(entry?.path()))
        .collect::<Result<Vec<_>, _>>()?
        .join("\n");
    let events = traces
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|event| event["target"] == "clacks_desktop::session::export")
        .collect::<Vec<_>>();
    let prepared = events
        .iter()
        .find(|event| event["fields"]["requested"].is_number())
        .ok_or_else(|| anyhow::anyhow!("Preparation must record its reviewed scope"))?;
    let published = events
        .iter()
        .find(|event| {
            event["fields"]["outcome"].is_string() && event["fields"]["entries"].is_number()
        })
        .ok_or_else(|| anyhow::anyhow!("Publication must record its actual result"))?;
    assert_eq!(prepared["fields"]["requested"], 1);
    assert_eq!(published["fields"]["outcome"], "published");
    assert_eq!(published["fields"]["entries"], 1);
    for event in [prepared, published] {
        assert!(event["spans"].as_array().is_some_and(|spans| {
            spans
                .iter()
                .any(|span| span["name"] == "desktop.export" && span["attempt"] == 1)
        }));
    }
    for excluded in ["K1OLD", "K1NEW", "K1OTHER", "reviewed.adi"] {
        assert!(!traces.contains(excluded));
    }
    Ok(())
}

#[tokio::test]
async fn failed_follow_up_read_recovers_the_committed_logbook_selection() -> anyhow::Result<()> {
    use crate::session::{
        settings::{self, logbooks::LogbookResponse},
        settings_view::Recovery,
    };
    use clacks_client_store::logbooks::selection::{self, SelectedView};
    let fixture = Fixture::new()?;
    let workspace = crate::workspace::open(&fixture.resources.paths).await?;
    let (published, view) = watch::channel(Some(Arc::new(workspace.settings)));
    let selected = selection::select(&workspace.store, SelectedView::Untagged).await?;

    // Release this handle between the acknowledged write and its follow-up read.
    workspace.store.close().await?;
    let (reply, response) = oneshot::channel();
    let result = settings::complete(
        reply,
        SettingsResponse::Logbooks(LogbookResponse::Selected(selected)),
        &workspace.store,
        "select",
    )
    .await;
    let mut recovery = Recovery::default();
    settle_settings(
        Ok(Some(result)),
        &published,
        &fixture.resources.settings_basis,
        &fixture.resources.context,
        &mut recovery,
    );

    assert!(
        matches!(response.await?, SettingsResponse::RefreshFailed { outcome, .. } if matches!(*outcome, SettingsResponse::Logbooks(LogbookResponse::Selected(SelectedView::Untagged))))
    );
    assert!(recovery.scheduled());
    assert_eq!(
        view.borrow().as_ref().map(|s| s.logbooks.selected_view),
        Some(SelectedView::AllContacts)
    );
    let reopened = clacks_client_store::store::ClientStore::open_discovered(
        &fixture.resources.paths.database,
        Default::default(),
    )
    .await?;
    recovery.wait().await;
    recovery.read(&reopened, &published).await?;
    assert!(!recovery.scheduled());
    assert_eq!(
        view.borrow().as_ref().map(|s| s.logbooks.selected_view),
        Some(SelectedView::Untagged)
    );
    reopened.close().await?;
    Ok(())
}
