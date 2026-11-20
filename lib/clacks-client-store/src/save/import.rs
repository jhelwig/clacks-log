//! Apply a reviewed contact and its source evidence in one guarded save.

use super::{SaveOutcome, report_saved, save_in};
use crate::{metadata::client, store::ClientStore};
use clacks_model::{
    contact::{Qso, input::QsoFields},
    ids::{AccountId, AssertionId},
    service::evidence::{ImportedAssertion, ServicePayload, ServiceRecordId},
};
use clacks_sync::{
    client::ClientRecord,
    record::{RecordId, RecordKey, RecordState, RecordValue},
    service::ServiceRecord,
};

/// The final contact and source assertions for one reviewed import row.
pub struct ResolvedImportRow<'a> {
    /// The record read before review began. Use `None` to create a new QSO.
    pub expected: Option<&'a ClientRecord>,
    /// The final QSO, including the destination tag and any existing memberships.
    pub qso: Qso,
    /// Claims from the source row. Each must refer to `qso`.
    pub assertions: Vec<ImportedAssertion>,
}

/// The result of applying one import row to local storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportRowOutcome {
    /// The saved record differed from `expected`. The row made no changes.
    Stale,
    /// The transaction committed, including when all requested values were already saved.
    Applied {
        created: bool,
        /// A saved contact's recorded facts changed independently of its tags.
        qso_changed: bool,
        memberships_added: usize,
        assertions_added: usize,
    },
}

/// Apply the QSO, its memberships, and its source assertions in one local
/// transaction if the saved record still matches the one supplied in `row.expected`.
///
/// Reuse equivalent live assertions even when the caller supplies new assertion IDs.
/// Changed records enter ordinary per-record synchronization after commit.
/// An error from commit can leave the outcome uncertain.
pub async fn apply_import_row(
    store: &ClientStore,
    row: ResolvedImportRow<'_>,
) -> anyhow::Result<ImportRowOutcome> {
    let key = RecordKey {
        account: store.account,
        id: RecordId::Qso(row.qso.id()),
    };
    let previous = if let Some(expected) = row.expected {
        anyhow::ensure!(
            expected.key() == key,
            "Import basis does not identify this contact"
        );
        match expected.working_state() {
            Some(RecordState::Live(RecordValue::Qso(qso))) => Some(qso.as_ref()),
            _ => anyhow::bail!("Import basis has no live contact"),
        }
    } else {
        None
    };

    anyhow::ensure!(
        previous.is_none_or(|previous| previous.tags().is_subset(row.qso.tags())),
        "Import cannot remove existing tag memberships"
    );
    let memberships_added = match previous {
        Some(previous) => row.qso.tags().difference(previous.tags()).count(),
        None => row.qso.tags().len(),
    };
    let qso_changed = previous.is_some_and(|previous| {
        let mut next = QsoFields::from(row.qso.clone());
        next.tags.clone_from(previous.tags());
        QsoFields::from(previous.clone()) != next
    });
    let qso = row.qso.id();
    let mut transaction = store.write().await?;
    let result = save_in(
        &mut transaction,
        key,
        row.expected,
        RecordState::Live(RecordValue::Qso(Box::new(row.qso))),
    )
    .await?;
    if matches!(result, SaveOutcome::Stale { .. }) {
        return Ok(ImportRowOutcome::Stale);
    }

    let mut assertions_added = 0;
    if !row.assertions.is_empty() {
        let mut existing = Vec::new();
        for assertion in sqlx::query_file!(
            "sql/import/working_assertions.sql",
            store.account.to_string(),
            qso.to_string()
        )
        .fetch_all(&mut *transaction)
        .await?
        {
            let id = assertion.record_id.parse()?;
            let key = assertion_key(store.account, id);
            if let Some(row) = client::load(&mut transaction, key).await?
                && let Some(RecordState::Live(RecordValue::Service(service))) = row.working_state()
                && let ServicePayload::Imported(assertion) = &service.payload
            {
                existing.push(assertion.as_ref().clone());
            }
        }
        for assertion in row.assertions {
            anyhow::ensure!(
                assertion.qso == qso,
                "Import assertion belongs to another contact"
            );
            if existing.iter().any(|old| equivalent(old, &assertion)) {
                continue;
            }
            let id = assertion.id;
            let key = assertion_key(store.account, id);
            let state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
                payload: ServicePayload::Imported(Box::new(assertion.clone())),
                support: Default::default(),
            })));
            anyhow::ensure!(
                matches!(
                    save_in(&mut transaction, key, None, state).await?,
                    SaveOutcome::Saved(_)
                ),
                "Imported assertion identity already names a different saved record"
            );
            existing.push(assertion);
            assertions_added += 1;
        }
    }
    transaction.commit().await?;
    report_saved(key, row.expected, &result);
    Ok(ImportRowOutcome::Applied {
        created: previous.is_none(),
        qso_changed,
        memberships_added,
        assertions_added,
    })
}

fn assertion_key(account: AccountId, id: AssertionId) -> RecordKey {
    RecordKey {
        account,
        id: RecordId::Service(ServiceRecordId::Assertion(id)),
    }
}

/// Treat assertions with different IDs as equivalent when their content and provenance agree.
fn equivalent(left: &ImportedAssertion, right: &ImportedAssertion) -> bool {
    left.qso == right.qso
        && left.service == right.service
        && left.destination == right.destination
        && left.claim == right.claim
        && left.date == right.date
        && left.provenance == right.provenance
        && left.baseline == right.baseline
}
