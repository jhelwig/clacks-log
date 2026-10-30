//! Save logbook names and upload policies as account-owned tag records.

use crate::{
    save::{self, SaveOutcome},
    store::ClientStore,
};
use clacks_model::tag::LogbookTag;
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordKey, RecordState, RecordValue},
};

/// Save a logbook's name and upload policies against its original client record.
///
/// Names must contain non-whitespace text; their spelling and duplicate names
/// are preserved. An expected record must identify this tag in this account.
/// If that record changed, reload and review it before submitting again.
#[tracing::instrument(skip_all, name = "client_store.logbook.save", fields(account_id = %store.account, tag_id = %next.id))]
pub async fn save(
    store: &ClientStore,
    expected: Option<&ClientRecord>,
    next: &LogbookTag,
) -> anyhow::Result<SaveOutcome> {
    anyhow::ensure!(!next.name.trim().is_empty(), "logbook name is required");
    let key = RecordKey {
        account: store.account,
        id: RecordId::Tag(next.id),
    };
    anyhow::ensure!(
        expected.is_none_or(|record| record.key() == key),
        "logbook editor basis identifies a different record"
    );

    save::save(
        store,
        key,
        expected,
        RecordState::Live(RecordValue::Tag(Box::new(next.clone()))),
    )
    .await
}
