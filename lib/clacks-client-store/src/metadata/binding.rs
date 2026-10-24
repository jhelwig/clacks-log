//! Current accepted external ownership, independent of local or retained alternatives.

use crate::rows::service::DestinationRow;
use clacks_model::service::evidence::ServiceRecordId;
use clacks_sync::record::{RecordId, RecordKey};
use sqlx::SqliteConnection;

/// Remove replaced index entries before installing all new accepted bindings in a transaction.
pub(crate) async fn remove(
    connection: &mut SqliteConnection,
    key: RecordKey,
) -> anyhow::Result<()> {
    if matches!(key.id, RecordId::Service(ServiceRecordId::Association(_))) {
        let account = key.account.to_string();
        let id = key.id.to_string();
        sqlx::query_file!("sql/external/current_remove.sql", account, id)
            .execute(connection)
            .await?;
    }
    Ok(())
}

/// Project the actual latest accepted association; uniqueness rejects competing ownership atomically.
pub(crate) async fn insert(
    connection: &mut SqliteConnection,
    key: RecordKey,
) -> anyhow::Result<()> {
    if matches!(key.id, RecordId::Service(ServiceRecordId::Association(_))) {
        let account = key.account.to_string();
        let id = key.id.to_string();
        sqlx::query_file!("sql/external/current_insert.sql", account, id)
            .execute(connection)
            .await?;
    }
    Ok(())
}

/// Check the complete accepted descriptor view while the caller's installation transaction is held.
pub(crate) async fn validate_namespaces(
    connection: &mut SqliteConnection,
    account: clacks_model::ids::AccountId,
) -> anyhow::Result<()> {
    let text = account.to_string();
    use clacks_model::service::destination::{
        Destination, RemoteCollection, validate_destination_snapshots,
    };
    let mut destinations = vec![];
    for row in sqlx::query_file_as!(DestinationRow, "sql/service/current_namespaces.sql", text)
        .fetch_all(&mut *connection)
        .await?
    {
        let collection = match &row {
            DestinationRow {
                collection,
                lotw_account: Some(account),
                qrz_book: None,
                ..
            } if collection == "lotw" => RemoteCollection::Lotw {
                account: account.clone(),
            },
            DestinationRow {
                collection,
                lotw_account: None,
                qrz_book,
                ..
            } if collection == "qrz" => RemoteCollection::Qrz {
                book_id: qrz_book.as_ref().map(|value| value.parse()).transpose()?,
            },
            _ => anyhow::bail!("invalid retained destination namespace"),
        };
        destinations.push(Destination {
            id: row.destination_id.parse()?,
            collection,
            description: row.description,
        });
    }
    validate_destination_snapshots(&destinations)
}
