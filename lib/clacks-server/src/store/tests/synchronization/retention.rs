//! Retention obligations across real account storage and discovery.

use super::super::support::synchronization::{NativeAccount, qso};
use crate::store::{discovery::Source, metadata::account, synchronization};
use clacks_model::ids::QsoId;
use clacks_sync::{
    acceptance::AcceptanceDecision,
    record::{RecordId, RecordKey, RecordState, RecordValue},
};

#[tokio::test]
async fn retention_journal_pages_shared_installation_revision() -> anyhow::Result<()> {
    let native = NativeAccount::create().await?;
    let mut connection = native.store.pool.acquire().await?;
    let before = account::load(&mut connection, native.first.account).await?;
    drop(connection);
    let checkpoint = synchronization::checkpoint(&before);
    let mut keys = Vec::new();
    for _ in 0..3 {
        let contact = qso(QsoId::new(), Default::default());
        let key = RecordKey {
            account: native.first.account,
            id: RecordId::Qso(contact.id()),
        };
        let change = native
            .prepare(
                &native.first,
                key,
                RecordState::Live(RecordValue::Qso(Box::new(contact))),
            )
            .await?;
        assert!(matches!(
            synchronization::accept_pending(&native.store, &native.first, &change).await?,
            AcceptanceDecision::Commit(_)
        ));
        keys.push(key);
    }

    // A baseline installation can journal several existing heads at one revision.
    // Author that valid stored input using the accepted records' real stamps.
    let fixtures = sqlx::PgPool::connect(&std::env::var("CLACKS_TEST_DATABASE_URL")?).await?;
    sqlx::query_file!(
        "tests/sql/shared_installation_revision.sql",
        native.first.account.to_string(),
        i64::try_from(before.head.revision)?
    )
    .execute(&fixtures)
    .await?;
    fixtures.close().await;
    let mut source = Source::start(&native.store, &native.first, Some(&checkpoint)).await?;
    assert!(source.end().is_err());
    let mut received = Vec::new();
    while let Some(page) = source.page(&native.store, &native.first, 1).await? {
        received.extend(page.requirements.into_iter().map(|item| item.key));
    }
    received.sort();
    keys.sort();
    assert_eq!(
        received, keys,
        "one installation must not lose entries at a page boundary"
    );
    assert_eq!(source.end()?.pages, 3);
    Ok(())
}
