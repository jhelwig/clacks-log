use super::*;
use crate::session::settings::{SettingsAction, SettingsRequest, SettingsResponse};
use clacks_client_store::{
    operating::{OperatingSaveOutcome, OperatingState},
    save::SaveOutcome,
};
use clacks_model::{
    contact::StationSnapshot,
    profile::{CurrentOperatingValues, SavedLocationProfile},
    scalar::RecordedCallsign,
};
use tokio::sync::oneshot;

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
    let home = SavedLocationProfile {
        id: Default::default(),
        name: "Home".into(),
        station: StationSnapshot {
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
                next: home.clone(),
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
    let desired = OperatingState {
        current: Some(CurrentOperatingValues {
            station: home.station.clone(),
            operator: None,
            transmit_power: None,
            equipment: None,
        }),
        selected_profile: Some(home),
        selected_equipment: None,
    };
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
    let mut invalid = desired.clone();
    invalid.current.as_mut().unwrap().station.callsign = RecordedCallsign::new("K1ABC")?;
    let (reply, response) = oneshot::channel();
    requests
        .send(SettingsRequest {
            action: SettingsAction::Apply {
                expected: desired.clone(),
                next: invalid,
            },
            reply,
        })
        .await?;
    assert!(matches!(response.await?, SettingsResponse::Failed(_)));
    assert_eq!(settings.borrow().as_ref().unwrap().operating, desired);
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    tokio::time::timeout(Duration::from_secs(15), task).await???;
    let reopened = crate::workspace::open(&paths).await?;
    assert_eq!(reopened.settings.operating, desired);
    reopened.store.close().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn shutdown_drains_admitted_settings_write_and_rejects_later_requests() -> anyhow::Result<()>
{
    use sqlx::Connection;
    let mut fixture = Fixture::new()?;
    let paths = fixture.resources.paths.clone();
    let requests = fixture.settings.clone();
    let task = tokio::spawn(run(fixture.resources, fixture.input, fixture.updates));
    next_matching(&mut fixture.status, |s| {
        matches!(s, SessionStatus::Ready { .. })
    })
    .await?;
    let mut blocker = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&paths.database),
    )
    .await?;
    let held = blocker.begin_with("BEGIN IMMEDIATE").await?;
    let home = SavedLocationProfile {
        id: Default::default(),
        name: "Home".into(),
        station: StationSnapshot {
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
                next: home.clone(),
            },
            reply,
        })
        .await?;
    // Reserving the entire bounded channel proves the session consumed the request.
    // With Ready already published, that branch admits it into the drainable work set.
    let permits = tokio::time::timeout(Duration::from_secs(15), requests.reserve_many(8)).await??;
    drop(permits);
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    next_matching(&mut fixture.status, |s| {
        matches!(s, SessionStatus::Closing { .. })
    })
    .await?;
    assert!(!task.is_finished());
    let (reply, _) = oneshot::channel();
    assert!(
        requests
            .send(SettingsRequest {
                action: SettingsAction::Refresh,
                reply
            })
            .await
            .is_err()
    );
    held.rollback().await?;
    blocker.close().await?;
    assert!(
        matches!(tokio::time::timeout(Duration::from_secs(15), response).await??, SettingsResponse::SaveProfile(ref result) if matches!(result.outcome, SaveOutcome::Saved(_)))
    );
    task.await??;
    let reopened = crate::workspace::open(&paths).await?;
    assert!(
        reopened
            .settings
            .profiles
            .iter()
            .any(|row| row.key.id == clacks_sync::record::RecordId::Profile(home.id))
    );
    reopened.store.close().await?;
    Ok(())
}
