//! Connect must bind the current workspace and preserve populated standalone logs.

use super::{AuthorizedEnrollment, completion};
use crate::workspace::{self, WorkspacePaths};
use clacks_client_store::{
    enrollment::{self, EnrollmentAttempt},
    identity, save,
    store::ClientStore,
};
use clacks_model::ids::{AccountId, ClientActorId};
use clacks_sync::protocol::ClientIdentity;
use tokio_util::sync::CancellationToken;

#[tokio::test(flavor = "multi_thread")]
async fn connect_enrolls_existing_empty_workspace() -> anyhow::Result<()> {
    use crate::session::{
        self, SessionCommand, SessionStatus,
        tests::{Fixture, next_matching},
    };
    let mut fixture = Fixture::new()?;
    let paths = fixture.resources.paths.clone();
    let enrollment_updates = fixture.resources.enrollment_updates.clone();
    let task = tokio::spawn(session::run(
        fixture.resources,
        fixture.input,
        fixture.updates,
    ));
    next_matching(&mut fixture.status, |state| {
        matches!(state, SessionStatus::Ready { .. })
    })
    .await?;
    let opened = workspace::open(&paths).await?;
    let operating = clacks_client_store::operating::OperatingState::new(
        Some(clacks_model::profile::CurrentOperatingValues {
            station: clacks_model::contact::StationSnapshot {
                callsign: clacks_model::scalar::RecordedCallsign::new("W7HEX")?,
                location: None,
                references: Default::default(),
            },
            operator: None,
            transmit_power: None,
            equipment: Some(
                clacks_model::equipment::EquipmentValuesFields {
                    equipment_description: Some("Local rig".into()),
                    antenna_description: None,
                    notes: None,
                }
                .try_into()?,
            ),
        }),
        None,
        None,
    )?;
    clacks_client_store::operating::save(&opened.store, &Default::default(), &operating).await?;
    let verified = AuthorizedEnrollment {
        identity: ClientIdentity {
            account: AccountId::new(),
            actor: ClientActorId::new(),
        },
        server: "https://clacks.example".into(),
        credential_reference: "protected-connect-test".into(),
    };
    let mut attempt = EnrollmentAttempt::new(
        verified.credential_reference.clone(),
        verified.server.clone(),
        Some(paths.directory.clone()),
        None,
    )?;
    attempt.credential_saved();
    enrollment::save_attempt(&opened.store, &attempt).await?;
    let completed = completion::finish(&opened.store, &verified, CancellationToken::new()).await?;
    assert_eq!(completed.destination, paths.directory);
    // The session consumes the actual completion notification, as after joining its worker.
    enrollment_updates.send_replace(super::EnrollmentStatus::Ready {
        destination: completed.destination.clone(),
    });
    next_matching(&mut fixture.status, |state| matches!(state, SessionStatus::Ready { identity, .. } if identity.account == verified.identity.account && identity.enrolled)).await?;
    assert_eq!(
        fixture.settings_view.borrow().as_ref().unwrap().operating,
        operating
    );
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    task.await??;
    opened.store.close().await?;
    let reopened = ClientStore::open_discovered(&paths.database, Default::default()).await?;
    assert_eq!(reopened.account_id(), verified.identity.account);
    assert_eq!(identity::binding(&reopened).await?, Some(completed.binding));
    assert!(
        save::load(
            &reopened,
            clacks_sync::record::RecordKey {
                account: reopened.account_id(),
                id: clacks_sync::record::RecordId::Group(
                    clacks_model::defaults::account_definitions()?
                        .groups
                        .first()
                        .unwrap()
                        .id()
                )
            }
        )
        .await?
        .is_none(),
        "server definitions must replace local initialization"
    );
    reopened.close().await?;
    Ok(())
}

#[tokio::test]
async fn connect_preserves_populated_standalone_log() -> anyhow::Result<()> {
    use clacks_model::{group::DefinitionSet, ids::QsoId, scalar::UtcSecond};
    use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};
    let directory = tempfile::tempdir()?;
    let paths = WorkspacePaths::at(directory.path())?;
    let opened = workspace::open(&paths).await?;
    let original_account = opened.store.account_id();
    let mut form = crate::logging::form::ContactForm::empty();
    form.own.callsign = "W7HEX".into();
    form.dx.callsign = "W1AW".into();
    form.primary.band = "20m".into();
    form.mode = "SSB".into();
    form.start =
        crate::logging::time::ContactTimestamp::recorded(Some(UtcSecond::from_unix_seconds(1)));
    let qso = form
        .build(QsoId::new(), &DefinitionSet::new(vec![])?)
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
    let key = RecordKey {
        account: original_account,
        id: RecordId::Qso(qso.id()),
    };
    save::save(
        &opened.store,
        key,
        None,
        RecordState::Live(RecordValue::Qso(Box::new(qso))),
    )
    .await?;
    let before = save::load(&opened.store, key).await?;
    let verified = AuthorizedEnrollment {
        identity: ClientIdentity {
            account: AccountId::new(),
            actor: ClientActorId::new(),
        },
        server: "https://clacks.example".into(),
        credential_reference: "protected-connect-test".into(),
    };
    let mut attempt = EnrollmentAttempt::new(
        verified.credential_reference.clone(),
        verified.server.clone(),
        Some(paths.directory.clone()),
        None,
    )?;
    attempt.credential_saved();
    enrollment::save_attempt(&opened.store, &attempt).await?;
    let result = completion::finish(&opened.store, &verified, CancellationToken::new()).await;
    let error = result.err().expect("populated logs must not be merged");
    assert!(matches!(
        error.downcast_ref::<clacks_client_store::error::StoreError>(),
        Some(clacks_client_store::error::StoreError::PopulatedWorkspace)
    ));
    opened.store.close().await?;
    let reopened = ClientStore::open_discovered(&paths.database, Default::default()).await?;
    assert_eq!(reopened.account_id(), original_account);
    assert!(identity::binding(&reopened).await?.is_none());
    assert_eq!(save::load(&reopened, key).await?, before);
    assert!(
        enrollment::attempt(&reopened).await?.is_some(),
        "saved authorization remains resumable after export"
    );
    reopened.close().await?;
    Ok(())
}
