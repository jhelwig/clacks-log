//! Web configuration reaches protected native storage independently of contact changes.

use super::DirectoryEntry;
use crate::enrollment::{credentials, tests::support::Server};
use crate::session;
use anyhow::Context;
use clacks_client_store::{
    identity::{self, EnrollmentBinding},
    store::ClientStore,
};
use leptos::server_fn::ServerFn;
use tokio::sync::watch;

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

#[tokio::test]
async fn directory_updates_replace_only_the_protected_directory_entry() -> anyhow::Result<()> {
    tokio::time::timeout(std::time::Duration::from_secs(90), async {
        let server = Server::start().await?;
        let authorized = server.authorize(&server.entry, "Directory desktop").await?;
        let origin = server.origin.as_str();

        let first_binding = &EnrollmentBinding {
            account: authorized.identity.account,
            actor: authorized.identity.actor,
            server: authorized.server,
            credential_reference: authorized.credential_reference,
        };

        let mut first = session::tests::Fixture::new()?;
        let store = ClientStore::create(
            &first.resources.paths.database,
            first_binding.account,
            Default::default(),
        )
        .await?;
        identity::bind(&store, None, first_binding.clone()).await?;
        let directory_reference =
            clacks_client_store::directory::installation::reference(&store).await?;
        let _entry = DirectoryEntry(directory_reference.clone());
        store.close().await?;

        let mut first_directory = first.resources.directory.installed.subscribe();
        let first_task = tokio::spawn(session::run(first.resources, first.input, first.updates));

        session::tests::next_matching(&mut first.status, |state| {
            matches!(state, session::SessionStatus::Ready { .. })
        })
        .await?;

        // Actual web updates reach the separate native entry even without QSO changes.
        for (expected, operation, password, revision) in [
            (0, "keep", "", 1),
            (1, "save", "directory-first", 2),
            (2, "save", "directory-replacement", 3),
            (3, "remove", "", 4),
        ] {
            server
                .browser
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
            if operation != "save" {
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

        first
            .commands
            .send(session::SessionCommand::BeginShutdown)
            .await?;
        first_task.await??;

        Ok(())
    })
    .await
    .context("the focused integration scenario exceeded its unattended time limit")?
}
