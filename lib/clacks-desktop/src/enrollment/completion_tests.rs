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
    let enrollment_view = enrollment_updates.subscribe();
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
    // Installation can commit before completion bookkeeping finishes. The binding
    // is enough to finish cleanup even with no native entry or reachable server.
    let (updates, _) = tokio::sync::watch::channel(super::EnrollmentStatus::default());
    let outcome = super::worker::execute(
        completed.store.clone(),
        paths.clone(),
        super::EnrollmentCommand::Connect {
            server: verified.server.clone(),
            label: "Desktop".into(),
        },
        updates,
        CancellationToken::new(),
    )
    .await;
    assert!(
        matches!(outcome, super::EnrollmentStatus::Ready),
        "committed completion must not restart authorization: {outcome:?}"
    );
    assert!(enrollment::attempt(&completed.store).await?.is_none());

    // The session consumes the actual completion notification, as after joining its worker.
    enrollment_updates.send_replace(super::EnrollmentStatus::Ready);
    next_matching(&mut fixture.status, |state| matches!(state, SessionStatus::Ready { identity, .. } if identity.account == verified.identity.account && identity.enrolled)).await?;
    assert!(
        matches!(*enrollment_view.borrow(), super::EnrollmentStatus::Ready),
        "session reopening must preserve terminal completion for the dialog"
    );
    assert_eq!(
        fixture.settings_view.borrow().as_ref().unwrap().operating,
        operating
    );
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    task.await??;
    opened.store.close().await?;
    let reopened = ClientStore::open_discovered(&paths.database, Default::default()).await?;
    assert_eq!(reopened.account_id(), verified.identity.account);
    let binding = identity::binding(&reopened).await?.unwrap();
    assert_eq!(binding.account, verified.identity.account);
    assert_eq!(binding.actor, verified.identity.actor);
    assert_eq!(binding.credential_reference, verified.credential_reference);
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
    enrollment::first::check_eligible(&opened.store).await?;
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
    let error = enrollment::first::check_eligible(&opened.store)
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<clacks_client_store::error::StoreError>(),
        Some(clacks_client_store::error::StoreError::PopulatedWorkspace)
    ));
    let (updates, _) = tokio::sync::watch::channel(super::EnrollmentStatus::default());
    let failed = super::worker::execute(
        opened.store.clone(),
        paths.clone(),
        super::EnrollmentCommand::Connect {
            server: "https://clacks.example".into(),
            label: "Desktop".into(),
        },
        updates,
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(failed, super::EnrollmentStatus::Failed { .. }));
    assert!(
        enrollment::attempt(&opened.store).await?.is_none(),
        "ineligible logs must not begin authorization"
    );
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

#[tokio::test]
async fn preflight_preserves_customized_bundled_definitions() -> anyhow::Result<()> {
    use clacks_sync::record::{RecordId, RecordKey, RecordState, RecordValue};
    let directory = tempfile::tempdir()?;
    let opened = workspace::open(&WorkspacePaths::at(directory.path())?).await?;
    enrollment::first::check_eligible(&opened.store).await?;
    let definition = clacks_model::defaults::account_definitions()?
        .groups
        .remove(0);
    let key = RecordKey {
        account: opened.store.account_id(),
        id: RecordId::Group(definition.id()),
    };
    let before = save::load(&opened.store, key).await?.unwrap();
    let mut fields = definition.into_fields();
    fields.name = "My customized definition".into();
    let next = RecordState::Live(RecordValue::Group(Box::new(fields.try_into()?)));
    save::save(&opened.store, key, Some(&before), next.clone()).await?;
    let error = enrollment::first::check_eligible(&opened.store)
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<clacks_client_store::error::StoreError>(),
        Some(clacks_client_store::error::StoreError::PopulatedWorkspace)
    ));
    assert_eq!(
        save::load(&opened.store, key)
            .await?
            .unwrap()
            .working_state(),
        Some(&next)
    );
    opened.store.close().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn committed_reauthorization_reloads_services_even_when_cleanup_failed() -> anyhow::Result<()>
{
    use crate::session::{
        self, SessionCommand, SessionStatus,
        tests::{Fixture, next_matching},
    };
    use clacks_client_store::logbooks::selection::{self, SelectedView};
    let mut fixture = Fixture::new()?;
    let paths = fixture.resources.paths.clone();
    let updates = fixture.resources.enrollment_updates.clone();
    let view = updates.subscribe();
    let initial = workspace::open(&paths).await?;
    let binding = identity::EnrollmentBinding {
        account: AccountId::new(),
        actor: ClientActorId::new(),
        server: "https://clacks.example".into(),
        credential_reference: "prior-test-credential".into(),
    };
    enrollment::first::install(&initial.store, &binding)
        .await?
        .close()
        .await?;
    let task = tokio::spawn(session::run(
        fixture.resources,
        fixture.input,
        fixture.updates,
    ));
    next_matching(&mut fixture.status, |state| {
        matches!(state, SessionStatus::Ready { .. })
    })
    .await?;
    assert_eq!(
        fixture
            .settings_view
            .borrow()
            .as_ref()
            .unwrap()
            .logbooks
            .selected_view,
        SelectedView::AllContacts
    );
    let current = ClientStore::open_discovered(&paths.database, Default::default()).await?;
    selection::select(&current, SelectedView::Untagged).await?;
    let replacement = identity::EnrollmentBinding {
        actor: ClientActorId::new(),
        credential_reference: "replacement-test-credential".into(),
        ..binding.clone()
    };
    identity::bind(&current, Some(&binding), replacement).await?;
    // Deliver the real terminal category after a binding commit. No database or
    // native-store failure is fabricated; the session must reload on this input.
    updates.send_replace(super::EnrollmentStatus::Failed {
        enrolled: true,
        message: "Connected, but cleanup remains".into(),
        details: None,
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if fixture
                .settings_view
                .borrow_and_update()
                .as_ref()
                .is_some_and(|settings| settings.logbooks.selected_view == SelectedView::Untagged)
            {
                break;
            }
            fixture.settings_view.changed().await?;
        }
        anyhow::Ok(())
    })
    .await??;
    assert!(matches!(
        *view.borrow(),
        super::EnrollmentStatus::Failed { enrolled: true, .. }
    ));
    fixture.commands.send(SessionCommand::BeginShutdown).await?;
    task.await??;
    current.close().await?;
    Ok(())
}
