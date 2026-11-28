//! Native credential persistence and interrupted local enrollment completion.

#[path = "../../../clacks-web/tests/support/provider.rs"]
mod provider;
mod recovery;
pub(crate) mod support;

use crate::enrollment::{credentials, flow};
use anyhow::Context;
use support::Server;

#[tokio::test]
async fn saved_native_credential_resumes_the_actual_client() -> anyhow::Result<()> {
    tokio::time::timeout(std::time::Duration::from_secs(90), async {
        let telemetry = std::env::var_os("CLACKS_TELEMETRY_ACCEPTANCE")
            .map(|_| clacks_server::telemetry::init(true))
            .transpose()?;

        let server = Server::start().await?;
        let origin = server.origin.clone();
        let reference = server.entry.0.clone();
        let first = server
            .authorize(&server.entry, "Native persistence desktop")
            .await?;

        assert_eq!(
            server
                .saved_before_identity
                .load(std::sync::atomic::Ordering::SeqCst),
            2,
            "usable refresh credential was not saved before requesting identity"
        );

        let before = credentials::load(&reference)?;
        let refresh = before
            .refresh
            .clone()
            .context("native entry omitted refresh")?;
        let key = huskarl_crypto_native::asymmetric::signer::PrivateKey::from_jwk(
            before
                .key
                .private_jwk()
                .context("missing saved private key")?
                .try_into()?,
        )?;

        let lost_grant = huskarl::grant::refresh::RefreshGrant::builder()
            .token_endpoint(format!("{origin}/oauth/token").parse()?)
            .client_id("clacks-desktop")
            .http_client(huskarl_reqwest::ReqwestClient::from(
                reqwest_oauth::Client::new(),
            ))
            .client_auth(huskarl::core::client_auth::NoAuth)
            .dpop(huskarl::core::dpop::DPoP::builder().signer(key).build())
            .build();

        use huskarl::grant::core::OAuth2ExchangeGrant;

        let lost_response = lost_grant
            .exchange(
                huskarl::grant::refresh::RefreshGrantParameters::refresh_token(refresh.clone()),
            )
            .await?;
        drop(lost_response);
        drop(before);

        let expected = first.identity.clone();
        let mut desktop = crate::session::tests::Fixture::new()?;
        let paths = desktop.resources.paths.clone();
        let source = paths.database.clone();
        let initiating = crate::workspace::open(&paths).await?.store;

        // Native credential persistence can precede the SQLite phase update.
        let attempt = clacks_client_store::enrollment::EnrollmentAttempt::new(
            reference.clone(),
            origin.clone(),
            Some(paths.directory.clone()),
            None,
        )?;
        clacks_client_store::enrollment::save_attempt(&initiating, &attempt).await?;
        initiating.close().await?;
        drop(first);

        let resumed = flow::resume(reference.clone()).await?;

        assert_eq!(resumed.identity, expected);
        assert_eq!(resumed.server.trim_end_matches('/'), origin);
        assert_eq!(resumed.credential_reference, reference);
        assert_eq!(credentials::load(&reference)?.refresh, Some(refresh));

        let opening = crate::session::SettingsSessionId(1);
        desktop.settings_intent.send_replace(Some(opening));
        let session = tokio::spawn(crate::session::run(
            desktop.resources,
            desktop.input,
            desktop.updates,
        ));
        crate::session::tests::next_matching(&mut desktop.status, |status| {
            matches!(status, crate::session::SessionStatus::Ready { .. })
        })
        .await?;

        desktop
            .commands
            .send(crate::session::SessionCommand::Enrollment(
                super::EnrollmentCommand::Connect {
                    server: origin.clone(),
                    label: "Native enrollment desktop".into(),
                },
            ))
            .await?;

        crate::session::tests::next_matching(&mut desktop.status, |status| {
            matches!(
                status,
                crate::session::SessionStatus::Ready { identity, .. }
                    if identity.account == expected.account && identity.enrolled
            )
        })
        .await?;
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            desktop.settings_basis.wait_for(|basis| {
                basis.as_ref().is_some_and(|b| {
                    b.id == opening && b.identity.account == expected.account && b.identity.enrolled
                })
            }),
        )
        .await??;

        assert!(matches!(
            *desktop.status.borrow(),
            crate::session::SessionStatus::Ready {
                synchronization: crate::synchronization::Status::Disconnected,
                ..
            }
        ));

        desktop
            .commands
            .send(crate::session::SessionCommand::BeginShutdown)
            .await?;
        session.await??;

        let opened = clacks_client_store::store::ClientStore::open(
            &source,
            expected.account,
            Default::default(),
        )
        .await?;
        let binding = clacks_client_store::identity::binding(&opened)
            .await?
            .context("Connect did not install enrollment in the initiating workspace")?;

        assert_eq!(binding.actor, expected.actor);
        assert_eq!(binding.credential_reference, reference);
        assert!(
            clacks_client_store::enrollment::attempt(&opened)
                .await?
                .is_none(),
            "Connect must finish durable bookkeeping before reopening its session"
        );

        opened.close().await?;

        if let Some(telemetry) = telemetry {
            tokio::task::spawn_blocking(move || telemetry.shutdown()).await??;
        }

        Ok(())
    })
    .await
    .context("the focused integration scenario exceeded its unattended time limit")?
}
