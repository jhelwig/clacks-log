//! Two real desktop sessions exchanging actual account state over authenticated WebSockets.

use super::DirectoryEntry;
use crate::enrollment::tests::support::{NativeEntry, Server};
use crate::{
    enrollment::credentials,
    session::{
        self, SessionCommand,
        settings::{
            SettingsAction, SettingsRequest, SettingsResponse,
            contacts::{ContactAction, ContactResponse},
            review::{ReviewAction, ReviewResponse},
        },
    },
};
use anyhow::Context;
use clacks_client_store::{
    identity::{self, EnrollmentBinding},
    save::SaveOutcome,
    store::ClientStore,
};
use clacks_model::ids::QsoId;
use clacks_sync::client::ClientRecord;
use leptos::server_fn::ServerFn;
use tokio::sync::{oneshot, watch};

/// Admit a real settings request and await its authoritative application result.
async fn action(
    sender: &tokio::sync::mpsc::Sender<SettingsRequest>,
    action: SettingsAction,
) -> anyhow::Result<SettingsResponse> {
    let (reply, result) = oneshot::channel();
    sender
        .send(SettingsRequest { action, reply })
        .await
        .map_err(|_| anyhow::anyhow!("session request channel closed"))?;

    Ok(result.await?)
}

/// Read actual record roles after committed-view notifications rather than sleeping or inspecting widgets.
async fn contact(
    sender: &tokio::sync::mpsc::Sender<SettingsRequest>,
    updates: &mut watch::Receiver<
        Option<std::sync::Arc<clacks_client_store::settings::SettingsSnapshot>>,
    >,
    id: QsoId,
    predicate: impl Fn(&ClientRecord) -> bool,
) -> anyhow::Result<ClientRecord> {
    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            updates.borrow_and_update();
            if let SettingsResponse::Contacts(ContactResponse::Loaded(Some(row), _)) =
                action(sender, SettingsAction::Contacts(ContactAction::Load { id })).await?
                && predicate(&row)
            {
                return Ok(row);
            }

            updates.changed().await?;
        }
    })
    .await?
}

/// Establish two independently authorized workspaces, publish through their real session owners,
/// then reopen one offline workspace and observe ordinary journal catch-up.
#[tokio::test]
async fn sessions_pause_catch_up_and_resolve_offline_work() -> anyhow::Result<()> {
    tokio::time::timeout(std::time::Duration::from_secs(90), async {
        let server = Server::start().await?;
        let first = server
            .authorize(&server.entry, "First synchronization desktop")
            .await?;

        let second_entry = NativeEntry(credentials::create(&server.origin)?);
        let second = server
            .authorize(&second_entry, "Second synchronization desktop")
            .await?;

        let first_binding = &EnrollmentBinding {
            account: first.identity.account,
            actor: first.identity.actor,
            server: first.server,
            credential_reference: first.credential_reference,
        };
        let second_binding = EnrollmentBinding {
            account: second.identity.account,
            actor: second.identity.actor,
            server: second.server,
            credential_reference: second.credential_reference,
        };

        let origin = server.origin.as_str();
        let browser = &server.browser;

        assert_eq!(first_binding.account, second_binding.account);
        assert_ne!(first_binding.actor, second_binding.actor);

        let mut first = session::tests::Fixture::new()?;
        let mut second = session::tests::Fixture::new()?;
        let mut directory_entries = Vec::new();
        for (fixture, binding) in [(&first, first_binding), (&second, &second_binding)] {
            let store = ClientStore::create(
                &fixture.resources.paths.database,
                binding.account,
                Default::default(),
            )
            .await?;
            identity::bind(&store, None, binding.clone()).await?;
            directory_entries.push(DirectoryEntry(
                clacks_client_store::directory::installation::reference(&store).await?,
            ));
            store.close().await?;
        }

        let second_database = second.resources.paths.database.clone();
        let first_task = tokio::spawn(session::run(first.resources, first.input, first.updates));
        let second_task = tokio::spawn(session::run(second.resources, second.input, second.updates));

        session::tests::next_matching(&mut first.status, |state| {
            matches!(state, session::SessionStatus::Ready { .. })
        })
        .await?;

        session::tests::next_matching(&mut second.status, |state| {
            matches!(state, session::SessionStatus::Ready { .. })
        })
        .await?;

        let mut form = crate::logging::form::ContactForm::empty();
        form.own.callsign = "W7HEX".into();
        form.dx.callsign = "OK2CQR".into();
        form.primary.band = "20m".into();
        form.mode = "SSB".into();
        form.start = crate::logging::time::ContactTimestamp::recorded(Some(
            clacks_model::scalar::UtcSecond::from_unix_seconds(1),
        ));
        let qso = form
            .build(QsoId::new(), &Default::default())
            .map_err(|errors| {
                anyhow::anyhow!("the supplied SSB contact should be valid: {errors:?}")
            })?;

        let id = qso.id();

        assert!(matches!(
            action(
                &first.settings,
                SettingsAction::Contacts(ContactAction::Save {
                    directory: None,
                    expected: None,
                    next: Box::new(qso.clone()),
                })
            )
            .await?,
            SettingsResponse::Contacts(ContactResponse::Saved(SaveOutcome::Saved(_)))
        ));

        let received = contact(&second.settings, &mut second.settings_view, id, |row| {
            row.pending().is_none()
        })
        .await?;

        assert_eq!(
            received.working_state(),
            Some(&clacks_sync::record::RecordState::Live(
                clacks_sync::record::RecordValue::Qso(Box::new(qso.clone()))
            ))
        );

        // Pause an established connection and await the post-join editing basis.
        let opening = session::SettingsSessionId(1);
        first.settings_intent.send_replace(Some(opening));
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            first
                .settings_basis
                .wait_for(|basis| basis.as_ref().is_some_and(|b| b.id == opening)),
        )
        .await??;

        let mut local = qso.clone().into_fields();
        local.id = QsoId::new();
        let local_id = local.id;

        assert!(matches!(
            action(
                &first.settings,
                SettingsAction::Contacts(ContactAction::Save {
                    directory: None,
                    expected: None,
                    next: Box::new(local.try_into()?)
                })
            )
            .await?,
            SettingsResponse::Contacts(ContactResponse::Saved(SaveOutcome::Saved(_)))
        ));

        let mut corrected = qso.into_fields();
        corrected.comment = Some("Second desktop correction".into());
        let corrected = clacks_model::contact::Qso::try_from(corrected)?;

        assert!(matches!(
            action(
                &second.settings,
                SettingsAction::Contacts(ContactAction::Save {
                    directory: None,
                    expected: Some(Box::new(received)),
                    next: Box::new(corrected.clone())
                })
            )
            .await?,
            SettingsResponse::Contacts(ContactResponse::Saved(SaveOutcome::Saved(_)))
        ));

        // The peer's durable acceptance is the barrier for checking isolation in both directions.
        contact(&second.settings, &mut second.settings_view, id, |row| {
            row.pending().is_none()
        })
        .await?;

        assert!(matches!(
            action(
                &second.settings,
                SettingsAction::Contacts(ContactAction::Load { id: local_id })
            )
            .await?,
            SettingsResponse::Contacts(ContactResponse::Loaded(None, _))
        ));

        assert!(matches!(
            action(
                &first.settings,
                SettingsAction::Contacts(ContactAction::Load { id })
            )
            .await?,
            SettingsResponse::Contacts(ContactResponse::Loaded(Some(row), _))
                if matches!(
                    row.working_state(),
                    Some(clacks_sync::record::RecordState::Live(
                        clacks_sync::record::RecordValue::Qso(local)
                    )) if local.comment().as_deref() != Some("Second desktop correction")
                )
        ));

        first.settings_intent.send_replace(None);
        contact(&first.settings, &mut first.settings_view, id, |row|
            row.pending().is_none()
                && matches!(
                    row.working_state(),
                    Some(clacks_sync::record::RecordState::Live(
                        clacks_sync::record::RecordValue::Qso(qso)
                    )) if qso.comment().as_deref() == Some("Second desktop correction")
                )
        )
        .await?;
        contact(
            &second.settings,
            &mut second.settings_view,
            local_id,
            |row| row.pending().is_none(),
        )
        .await?;

        second.commands.send(SessionCommand::BeginShutdown).await?;
        second_task.await??;
        drop(second._owner);

        // Real offline edits remain local until this independently authorized client resumes.
        let offline_store = ClientStore::open_discovered(&second_database, Default::default()).await?;
        let key = clacks_sync::record::RecordKey {
            account: second_binding.account,
            id: clacks_sync::record::RecordId::Qso(id),
        };
        let offline_basis = clacks_client_store::save::load(&offline_store, key)
            .await?
            .context("the synchronized contact should remain available while offline")?;

        let mut offline_values = corrected.clone().into_fields();
        offline_values.comment = Some("Offline correction awaiting review".into());
        clacks_client_store::save::save(
            &offline_store,
            key,
            Some(&offline_basis),
            clacks_sync::record::RecordState::Live(clacks_sync::record::RecordValue::Qso(Box::new(
                offline_values.try_into()?,
            ))),
        )
        .await?;

        let mut unrelated = corrected.clone().into_fields();
        unrelated.id = QsoId::new();
        unrelated.comment = Some("Unrelated offline contact".into());
        let unrelated = clacks_model::contact::Qso::try_from(unrelated)?;
        let unrelated_id = unrelated.id();
        let unrelated_key = clacks_sync::record::RecordKey {
            account: second_binding.account,
            id: clacks_sync::record::RecordId::Qso(unrelated_id),
        };
        clacks_client_store::save::save(
            &offline_store,
            unrelated_key,
            None,
            clacks_sync::record::RecordState::Live(clacks_sync::record::RecordValue::Qso(Box::new(
                unrelated,
            ))),
        )
        .await?;
        offline_store.close().await?;

        let expected = contact(&first.settings, &mut first.settings_view, id, |row| {
            row.pending().is_none()
        })
        .await?;

        let mut fields = corrected.into_fields();
        fields.comment = Some("Catch up after offline restart".into());

        assert!(matches!(
            action(
                &first.settings,
                SettingsAction::Contacts(ContactAction::Save {
                    directory: None,
                    expected: Some(Box::new(expected)),
                    next: Box::new(fields.try_into()?)
                })
            )
            .await?,
            SettingsResponse::Contacts(ContactResponse::Saved(SaveOutcome::Saved(_)))
        ));
        contact(&first.settings, &mut first.settings_view, id, |row| {
            row.pending().is_none()
        })
        .await?;

        let mut second = session::tests::Fixture::with_directory(second._directory)?;
        second.settings_intent.send_replace(Some(opening));
        let second_task = tokio::spawn(session::run(second.resources, second.input, second.updates));

        session::tests::next_matching(&mut second.status, |state| {
            matches!(state, session::SessionStatus::Ready { .. })
        })
        .await?;
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            second
                .settings_basis
                .wait_for(|basis| basis.as_ref().is_some_and(|b| b.id == opening)),
        )
        .await??;

        second.settings_intent.send_replace(None);
        contact(&second.settings, &mut second.settings_view, id, |row|
            row.pending().is_some()
                && matches!(
                    row.latest(),
                    clacks_sync::record::Observation::Accepted(shared)
                        if matches!(
                            shared.state(),
                            clacks_sync::record::RecordState::Live(
                                clacks_sync::record::RecordValue::Qso(qso)
                            ) if qso.comment().as_deref() == Some("Catch up after offline restart")
                        )
                )
        )
        .await?;

        // Unrelated pending work publishes while the first contact still requires review.
        contact(
            &first.settings,
            &mut first.settings_view,
            unrelated_id,
            |row| row.pending().is_none(),
        )
        .await?;

        let SettingsResponse::Review(ReviewResponse::Record {
            row: Some(review), ..
        }) = action(
            &second.settings,
            SettingsAction::Review(ReviewAction::ReadRecord { key }),
        )
        .await?
        else {
            panic!("review record")
        };
        let shown = review
            .review
            .clone()
            .context("offline and remote corrections should require review")?;
        let Some(clacks_sync::record::RecordState::Live(clacks_sync::record::RecordValue::Qso(local))) =
            review.record.working_state()
        else {
            panic!("local contact")
        };

        let mut resolved = local.as_ref().clone().into_fields();
        resolved.comment = Some("Explicit combined resolution".into());

        assert!(
            matches!(
                action(
                    &second.settings,
                    SettingsAction::Review(ReviewAction::Resolve {
                        expected: Box::new(review.record),
                        shown: Box::new(shown),
                        chosen: clacks_sync::record::RecordState::Live(
                            clacks_sync::record::RecordValue::Qso(Box::new(resolved.try_into()?))
                        )
                    })
                )
                .await?,
                SettingsResponse::Review(ReviewResponse::Resolved { outcome, .. })
                    if matches!(*outcome, SaveOutcome::Saved(_))
            )
        );

        contact(&second.settings, &mut second.settings_view, id, |row|
            row.pending().is_none()
                && matches!(
                    row.working_state(),
                    Some(clacks_sync::record::RecordState::Live(
                        clacks_sync::record::RecordValue::Qso(qso)
                    )) if qso.comment().as_deref() == Some("Explicit combined resolution")
                )
        )
        .await?;

        contact(&first.settings, &mut first.settings_view, id, |row|
            row.pending().is_none()
                && matches!(
                    row.working_state(),
                    Some(clacks_sync::record::RecordState::Live(
                        clacks_sync::record::RecordValue::Qso(qso)
                    )) if qso.comment().as_deref() == Some("Explicit combined resolution")
                )
        )
        .await?;

        browser
            .post(format!(
                "{origin}{}",
                clacks_web::actions::clients::RevokeClient::PATH
            ))
            .header("origin", origin)
            .form(&[("actor", second_binding.actor.to_string())])
            .send()
            .await?
            .error_for_status()?;

        session::tests::next_matching(&mut second.status, |state| {
            matches!(
                state,
                session::SessionStatus::Ready {
                    synchronization: crate::synchronization::Status::Failed(message),
                    ..
                } if message == "Client authorization is no longer valid. Reauthorize this client if needed."
            )
        })
        .await?;

        let expected = contact(&first.settings, &mut first.settings_view, id, |row| {
            row.pending().is_none()
        })
        .await?;
        let Some(clacks_sync::record::RecordState::Live(clacks_sync::record::RecordValue::Qso(qso))) =
            expected.working_state()
        else {
            anyhow::bail!("missing contact after unrelated revocation");
        };

        let mut fields = qso.as_ref().clone().into_fields();
        fields.comment = Some("Other client remains authorized".into());

        assert!(matches!(
            action(
                &first.settings,
                SettingsAction::Contacts(ContactAction::Save {
                    directory: None,
                    expected: Some(Box::new(expected)),
                    next: Box::new(fields.try_into()?)
                })
            )
            .await?,
            SettingsResponse::Contacts(ContactResponse::Saved(SaveOutcome::Saved(_)))
        ));
        contact(&first.settings, &mut first.settings_view, id, |row| {
            row.pending().is_none()
        })
        .await?;

        first.commands.send(SessionCommand::BeginShutdown).await?;
        second.commands.send(SessionCommand::BeginShutdown).await?;
        first_task.await??;
        second_task.await??;

        Ok(())
    })
    .await
    .context("the focused integration scenario exceeded its unattended time limit")?
}
