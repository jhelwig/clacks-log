//! Native saved fixtures for the logbook management workflows.

use clacks_client_store::{
    save::{self, SaveOutcome},
    store::{ClientStore, StoreSettings},
};
use clacks_model::tag::LogbookTag;
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordKey, RecordState, RecordValue},
};
use std::path::PathBuf;

use crate::support::id;

/// Keep an actual account database alive for one scenario.
pub struct Workspace {
    /// Own the temporary files until the scenario completes.
    pub _directory: tempfile::TempDir,
    /// Actual database used for reopen and failure fixtures.
    pub path: PathBuf,
    /// Production account store under test.
    pub store: ClientStore,
}

impl Workspace {
    /// Create an independent account workspace for one scenario.
    pub async fn new() -> anyhow::Result<Self> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("account.sqlite");
        let store = ClientStore::create(&path, id(1), StoreSettings::default()).await?;
        Ok(Self {
            _directory: directory,
            path,
            store,
        })
    }
}

/// Insert a native tag through the actual guarded store.
#[allow(dead_code)]
pub async fn insert(store: &ClientStore, value: u128, name: &str) -> anyhow::Result<ClientRecord> {
    let tag = LogbookTag::new(id(value), name.into());
    let key = RecordKey {
        account: store.account_id(),
        id: RecordId::Tag(tag.id),
    };
    let SaveOutcome::Saved(record) = save::save(
        store,
        key,
        None,
        RecordState::Live(RecordValue::Tag(Box::new(tag))),
    )
    .await?
    else {
        panic!("expected a new saved tag");
    };
    Ok(record)
}
