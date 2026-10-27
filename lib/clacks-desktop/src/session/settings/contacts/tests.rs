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
