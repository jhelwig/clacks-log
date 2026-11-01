use super::*;
use crate::session::settings::{SettingsAction, SettingsRequest, SettingsResponse};
use clacks_client_store::{
    operating::{OperatingSaveOutcome, OperatingState},
    save::SaveOutcome,
};
use clacks_model::scalar::RecordedCallsign;
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
