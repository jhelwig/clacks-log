//! Two real desktop sessions exchanging actual account state over authenticated WebSockets.

use super::{NativeEntry, provider};
use crate::{
    enrollment::{self, credentials, flow},
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
use tokio_util::sync::CancellationToken;

/// Clean only the unique directory entry installed by this scenario.
struct DirectoryEntry(String);

impl Drop for DirectoryEntry {
    fn drop(&mut self) {
        let _ = crate::directory::credentials::remove(&self.0);
    }
}

/// Await actual native installation independently of unrelated record checkpoints.
async fn installed_directory(
    updates: &mut watch::Receiver<
        Option<clacks_sync::protocol::directory::ProvisionedConfiguration>,
    >,
    revision: i64,
) -> anyhow::Result<clacks_sync::protocol::directory::ProvisionedConfiguration> {
    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            let current = updates.borrow_and_update().clone();
            if let Some(current) = current
                && current.configuration.revision == revision
            {
                return Ok(current);
            }
            updates.changed().await?;
        }
    })
    .await?
}

/// Approve the second actual proof key through the same browser account and real issuer.
async fn second(
    origin: &str,
    browser: &reqwest::Client,
    provider: &provider::Provider,
    subject: &str,
) -> anyhow::Result<(EnrollmentBinding, NativeEntry)> {
    let reference = credentials::create(origin)?;
    let entry = NativeEntry(reference.clone());
    let (status, mut updates) = watch::channel(enrollment::EnrollmentStatus::default());
    let task = tokio::spawn(flow::authorize(
        enrollment::EnrollmentRequest {
            server: origin.into(),
            label: "Second synchronization desktop".into(),
            credential_reference: reference.clone(),
            expected: None,
        },
        status,
        CancellationToken::new(),
    ));
    let authorization = loop {
        updates.changed().await?;
        if let enrollment::EnrollmentStatus::Authorizing { authorization_url } =
            updates.borrow().clone()
        {
            break authorization_url;
        }
    };
    let url = url::Url::parse(&authorization)?;
    let state = url
        .query_pairs()
        .find(|(key, _)| key == "state")
        .context("missing native correlation")?
        .1
        .into_owned();
    browser
        .get(&authorization)
        .send()
        .await?
        .error_for_status()?;
    let login = browser
        .post(format!(
            "{origin}{}",
            clacks_web::actions::StartSignIn::PATH
        ))
        .header("origin", origin)
        .header("accept", "text/html")
        .form(&[("provider", "test"), ("enrollment", state.as_str())])
        .send()
        .await?
        .error_for_status()?;
    let request = url::Url::parse(
        login
            .headers()
            .get("location")
            .context("missing provider request")?
            .to_str()?,
    )?;
    let callback = provider.authorize(&request, subject)?;
    browser
        .get(format!("{origin}/auth/callback/test"))
        .query(&[("state", callback.state), ("code", callback.code)])
        .send()
        .await?
        .error_for_status()?;
    let approved = browser
        .post(format!(
            "{origin}{}",
            clacks_web::actions::clients::DecideApproval::PATH
        ))
        .header("origin", origin)
        .header("accept", "text/html")
        .form(&[("request", state.as_str()), ("decision", "approve")])
        .send()
        .await?
        .error_for_status()?;
    browser
        .get(
            approved
                .headers()
                .get("location")
                .context("missing callback")?
                .to_str()?,
        )
        .send()
        .await?
        .error_for_status()?;
    let authorized = task.await??;
    Ok((
        EnrollmentBinding {
            account: authorized.identity.account,
            actor: authorized.identity.actor,
            server: authorized.server,
            credential_reference: reference,
        },
        entry,
    ))
}

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
            if let SettingsResponse::Contacts(ContactResponse::Loaded(Some(row))) =
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
pub(super) async fn verify(
    origin: &str,
    first_binding: &EnrollmentBinding,
    browser: &reqwest::Client,
    provider: &provider::Provider,
    subject: &str,
) -> anyhow::Result<()> {
    crate::synchronization::tests::verify(first_binding).await?;
    let (second_binding, _entry) = second(origin, browser, provider, subject).await?;
    assert_eq!(first_binding.account, second_binding.account);
    assert_ne!(first_binding.actor, second_binding.actor);
    let mut first = session::tests::Fixture::new()?;
    let mut second = session::tests::Fixture::new()?;
    for (fixture, binding) in [(&first, first_binding), (&second, &second_binding)] {
        let store = ClientStore::create(
            &fixture.resources.paths.database,
            binding.account,
            Default::default(),
        )
        .await?;
        identity::bind(&store, None, binding.clone()).await?;
        store.close().await?;
    }
    let second_database = second.resources.paths.database.clone();
    let mut directory_entries = Vec::new();
    let mut directory_reference = String::new();
    for (fixture, binding) in [(&first, first_binding), (&second, &second_binding)] {
        let store = ClientStore::open(
            &fixture.resources.paths.database,
            binding.account,
            Default::default(),
        )
        .await?;
        let reference = clacks_client_store::directory::installation::reference(&store).await?;
        if directory_reference.is_empty() {
            directory_reference = reference.clone();
        }
        directory_entries.push(DirectoryEntry(reference));
        store.close().await?;
    }
    let mut first_directory = first.resources.directory.installed.subscribe();
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

    // Actual web updates reach the separate native entry even without QSO changes.
    for (expected, operation, password, revision) in [
        (0, "replace", "directory-first", 1),
        (1, "replace", "directory-replacement", 2),
        (2, "remove", "", 3),
    ] {
        browser
            .post(format!(
                "{origin}{}",
                clacks_web::actions::directory::SaveDirectory::PATH
            ))
            .header("origin", origin)
            .form(&[
                ("expected_revision", expected.to_string()),
                ("selected", "qrz".into()),
                ("operation", operation.into()),
                ("provider", "qrz".into()),
                ("username", "directory-user".into()),
                ("password", password.into()),
            ])
            .send()
            .await?
            .error_for_status()?;
        let configuration = installed_directory(&mut first_directory, revision).await?;
        if operation == "remove" {
            assert!(configuration.material.is_none());
        } else {
            assert_eq!(
                crate::directory::credentials::load(first_binding, &directory_reference)?
                    .context("missing protected directory configuration")?
                    .material
                    .context("missing selected directory material")?
                    .login
                    .password,
                password
            );
        }
        assert!(
            credentials::load(&first_binding.credential_reference)?
                .refresh
                .is_some()
        );
    }

    let mut form = crate::logging::form::ContactForm::empty();
    form.own.callsign = "W7HEX".into();
    form.dx.callsign = "OK2CQR".into();
    form.primary.band = "20m".into();
    form.mode = "SSB".into();
    form.start = crate::logging::time::ContactTimestamp::recorded(Some(
        clacks_model::scalar::UtcSecond::from_unix_seconds(1000),
    ));
    let qso = form
        .build(
            QsoId::new(),
            &clacks_model::group::DefinitionSet::new(vec![])?,
        )
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
    let id = qso.id();
    assert!(matches!(
        action(
            &first.settings,
            SettingsAction::Contacts(ContactAction::Save {
                expected: None,
                next: Box::new(qso.clone())
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
    let mut corrected = qso.into_fields();
    corrected.comment = Some("Second desktop correction".into());
    let corrected = clacks_model::contact::Qso::try_from(corrected)?;
    assert!(matches!(
        action(
            &second.settings,
            SettingsAction::Contacts(ContactAction::Save {
                expected: Some(Box::new(received)),
                next: Box::new(corrected.clone())
            })
        )
        .await?,
        SettingsResponse::Contacts(ContactResponse::Saved(SaveOutcome::Saved(_)))
    ));
    contact(&first.settings, &mut first.settings_view, id, |row| row.pending().is_none() && matches!(row.working_state(),Some(clacks_sync::record::RecordState::Live(clacks_sync::record::RecordValue::Qso(qso))) if qso.comment().as_deref() == Some("Second desktop correction"))).await?;
    contact(&second.settings, &mut second.settings_view, id, |row| {
        row.pending().is_none()
    })
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
        .unwrap();
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
    let second_task = tokio::spawn(session::run(second.resources, second.input, second.updates));
    session::tests::next_matching(&mut second.status, |state| {
        matches!(state, session::SessionStatus::Ready { .. })
    })
    .await?;
    contact(&second.settings, &mut second.settings_view, id, |row| row.pending().is_some() && matches!(row.latest(), clacks_sync::record::Observation::Accepted(shared) if matches!(shared.state(), clacks_sync::record::RecordState::Live(clacks_sync::record::RecordValue::Qso(qso)) if qso.comment().as_deref() == Some("Catch up after offline restart")))).await?;
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
    let shown = review.review.clone().expect("real concurrent correction");
    let Some(clacks_sync::record::RecordState::Live(clacks_sync::record::RecordValue::Qso(local))) =
        review.record.working_state()
    else {
        panic!("local contact")
    };
    let mut resolved = local.as_ref().clone().into_fields();
    resolved.comment = Some("Explicit combined resolution".into());
    assert!(
        matches!(action(&second.settings, SettingsAction::Review(ReviewAction::Resolve {
        expected: Box::new(review.record), shown: Box::new(shown),
        chosen: clacks_sync::record::RecordState::Live(clacks_sync::record::RecordValue::Qso(Box::new(resolved.try_into()?))),
    })).await?, SettingsResponse::Review(ReviewResponse::Resolved { outcome, .. }) if matches!(*outcome, SaveOutcome::Saved(_)))
    );
    contact(&second.settings, &mut second.settings_view, id, |row| row.pending().is_none() && matches!(row.working_state(),Some(clacks_sync::record::RecordState::Live(clacks_sync::record::RecordValue::Qso(qso))) if qso.comment().as_deref() == Some("Explicit combined resolution"))).await?;
    contact(&first.settings, &mut first.settings_view, id, |row| row.pending().is_none() && matches!(row.working_state(),Some(clacks_sync::record::RecordState::Live(clacks_sync::record::RecordValue::Qso(qso))) if qso.comment().as_deref() == Some("Explicit combined resolution"))).await?;

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
}
