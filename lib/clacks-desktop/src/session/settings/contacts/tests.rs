use super::*;
use clacks_client_store::store::StoreSettings;
use clacks_model::{group::DefinitionSet, scalar::UtcSecond};

fn contact() -> anyhow::Result<Qso> {
    let mut form = crate::logging::form::ContactForm::empty();
    form.own.callsign = "W7HEX".into();
    form.dx.callsign = "OK2CQR".into();
    form.primary.band = "20m".into();
    form.mode = "SSB".into();
    form.start =
        crate::logging::time::ContactTimestamp::recorded(Some(UtcSecond::from_unix_seconds(1)));
    form.build(QsoId::new(), &DefinitionSet::new(vec![])?)
        .map_err(|e| anyhow::anyhow!("{e:?}"))
}

#[tokio::test]
async fn repeated_create_acknowledges_only_the_same_submitted_contact() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("store.sqlite"),
        Default::default(),
        StoreSettings::default(),
    )
    .await?;
    let qso = contact()?;
    let created = execute(
        &store,
        ContactAction::Save {
            expected: None,
            next: Box::new(qso.clone()),
        },
    )
    .await?;
    assert!(matches!(
        created,
        ContactResponse::Saved(SaveOutcome::Saved(_))
    ));
    let repeated = execute(
        &store,
        ContactAction::Save {
            expected: None,
            next: Box::new(qso.clone()),
        },
    )
    .await?;
    let ContactResponse::Saved(SaveOutcome::Unchanged(current)) = repeated else {
        anyhow::bail!("retry did not acknowledge saved facts")
    };
    let deleted = execute(
        &store,
        ContactAction::Delete {
            expected: Box::new(current),
        },
    )
    .await?;
    assert!(matches!(
        deleted,
        ContactResponse::Deleted(SaveOutcome::Saved(_))
    ));
    let retry = execute(
        &store,
        ContactAction::Save {
            expected: None,
            next: Box::new(qso),
        },
    )
    .await?;
    assert!(matches!(
        retry,
        ContactResponse::Saved(SaveOutcome::Stale { .. })
    ));
    store.close().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn retry_acknowledgment_keeps_correlation_without_logging_contact_facts() -> anyhow::Result<()>
{
    use std::io::Read;
    use tracing::instrument::WithSubscriber;
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create(
        &directory.path().join("telemetry.sqlite"),
        Default::default(),
        StoreSettings::default(),
    )
    .await?;
    let mut qso = contact()?;
    qso.comment = Some("PRIVATE-COMMENT-CANARY".into());
    qso.sent_report = Some("PRIVATE-REPORT-CANARY".into());
    qso.contacted_operator = Some(clacks_model::contact::OperatorInformation {
        callsign: None,
        recorded_name: Some("PRIVATE-NAME-CANARY".into()),
    });
    let account = store.account_id();
    let id = qso.id;
    let output = tempfile::NamedTempFile::new()?;
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .with_writer(std::sync::Mutex::new(output.reopen()?))
        .finish();
    async {
        execute(
            &store,
            ContactAction::Save {
                expected: None,
                next: Box::new(qso.clone()),
            },
        )
        .await?;
        execute(
            &store,
            ContactAction::Save {
                expected: None,
                next: Box::new(qso),
            },
        )
        .await?;
        store.close().await?;
        Ok::<_, anyhow::Error>(())
    }
    .with_subscriber(subscriber)
    .await?;
    let mut text = String::new();
    output.reopen()?.read_to_string(&mut text)?;
    let event = text
        .lines()
        .find(|line| line.contains("contact.create_retry_acknowledged"))
        .expect("retry reports recovery at info");
    assert!(event.contains(&account.to_string()));
    assert!(event.contains(&id.to_string()));
    for secret in [
        "PRIVATE-COMMENT-CANARY",
        "PRIVATE-REPORT-CANARY",
        "PRIVATE-NAME-CANARY",
        "OK2CQR",
        "W7HEX",
    ] {
        assert!(
            !text.contains(secret),
            "contact facts must not enter telemetry"
        );
    }
    Ok(())
}
