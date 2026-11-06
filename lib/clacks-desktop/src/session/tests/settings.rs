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
