//! Exercise production discovery, admission and receipt through persisted stores.

use crate::{
    enrollment::AuthorizedClient,
    store::{AccountStore, discovery, synchronization},
};
use clacks_client_store::{identity::EnrollmentBinding, store::ClientStore};
use clacks_sync::{
    account::InstalledCheckpoint,
    record::{RecordKey, RecordState},
};

/// Install verified authority in an actual SQLite workspace.
pub(crate) async fn bind(
    store: &ClientStore,
    authority: &AuthorizedClient,
    previous: Option<&EnrollmentBinding>,
) -> anyhow::Result<EnrollmentBinding> {
    let binding = EnrollmentBinding {
        account: authority.account,
        actor: authority.actor,
        server: "https://clacks.example".into(),
        credential_reference: authority.actor.to_string(),
    };

    clacks_client_store::identity::bind(store, previous, binding.clone()).await?;

    Ok(binding)
}

/// Ingest an exhausted source, persist its checkpoint, then acknowledge that durable boundary.
pub(crate) async fn pull(
    server: &AccountStore,
    authority: &AuthorizedClient,
    client: &ClientStore,
) -> anyhow::Result<InstalledCheckpoint> {
    let previous = clacks_client_store::synchronization::state(client).await?;
    let mut source =
        discovery::Source::start(server, authority, previous.checkpoint.as_ref()).await?;
    let mut receipt = clacks_client_store::transfer::begin(client, &source.start).await?;

    while let Some(page) = source.page(server, authority, 3).await? {
        clacks_client_store::transfer::install_page(client, &mut receipt, &page).await?;
    }

    let installed =
        clacks_client_store::transfer::complete(client, &receipt, &source.end()?).await?;
    assert_eq!(
        clacks_client_store::synchronization::state(client)
            .await?
            .checkpoint
            .as_ref(),
        Some(&installed)
    );

    synchronization::acknowledge(server, authority, &installed).await?;

    Ok(installed)
}

/// Save through the normal stale-save guard, retaining actual comparison evidence.
pub(crate) async fn save(
    client: &ClientStore,
    key: RecordKey,
    state: RecordState,
) -> anyhow::Result<()> {
    let before = clacks_client_store::save::load(client, key).await?;
    assert!(matches!(
        clacks_client_store::save::save(client, key, before.as_ref(), state).await?,
        clacks_client_store::save::SaveOutcome::Saved(_)
    ));

    Ok(())
}

/// Publish the actual frozen client proposal and receive its accepted observation.
pub(crate) async fn publish(
    server: &AccountStore,
    authority: &AuthorizedClient,
    client: &ClientStore,
    key: RecordKey,
) -> anyhow::Result<clacks_sync::record::AcceptedRecord> {
    use clacks_client_store::synchronization::PrepareOutcome;
    use clacks_sync::{acceptance::AcceptanceDecision, client::Preparation};

    let PrepareOutcome::Core(preparation) =
        clacks_client_store::synchronization::prepare(client, key).await?
    else {
        anyhow::bail!("fixture proposal not enrolled");
    };
    let change = match *preparation {
        Preparation::Ready { change, .. } | Preparation::Retry { change, .. } => change,
        _ => anyhow::bail!("fixture proposal not ready"),
    };

    let AcceptanceDecision::Commit(commit) =
        synchronization::accept_pending(server, authority, &change).await?
    else {
        anyhow::bail!("fixture proposal not accepted");
    };

    clacks_client_store::synchronization::receive(client, &commit.accepted).await?;
    Ok(commit.accepted)
}

/// Return actual accepted roles, optionally disconnecting before source exhaustion.
pub(crate) async fn return_state(
    server: &AccountStore,
    authority: &AuthorizedClient,
    client: &ClientStore,
    interrupted: bool,
) -> anyhow::Result<bool> {
    use clacks_sync::{
        service::VersionReference,
        transfer::receipt::{ReceiptState, register_page},
    };

    let mut source = clacks_client_store::discovery::Source::start(client).await?;
    let mut receipt = ReceiptState::begin(
        source
            .start
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("client has no complete checkpoint"))?,
    )?;

    while let Some(page) = source.page(client, 3).await? {
        let mut next = register_page(&receipt, &page)?;
        let committed = synchronization::receipt::receive(server, authority, &page).await?;
        next.staged = committed
            .staged
            .iter()
            .map(VersionReference::from)
            .collect();
        next.completed_units.extend(committed.completed_units);

        for review in committed.reviews {
            clacks_client_store::synchronization::receive_review(client, &review).await?;
        }

        receipt = next;

        if interrupted {
            assert!(source.end().is_err());
            return Ok(false);
        }
    }

    synchronization::receipt::complete_return(
        server,
        authority,
        &receipt,
        &source
            .end()?
            .ok_or_else(|| anyhow::anyhow!("source has no end"))?,
    )
    .await
}
