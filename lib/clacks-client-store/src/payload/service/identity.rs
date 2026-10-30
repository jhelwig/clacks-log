//! Credential-free destination snapshots and exact provider record keys.

use crate::rows::service::{CollectionRow, DestinationRow};
use clacks_model::{
    ids::AccountId,
    service::{
        destination::{Destination, ExternalRecordKey, RemoteCollection, Service},
        evidence::ServiceRecordId,
    },
};
use clacks_sync::record::{RecordId, RecordKey};
use sqlx::SqliteConnection;

/// Encode a provider identity independently from its descriptive label.
pub(super) fn service(value: Service) -> &'static str {
    match value {
        Service::Lotw => "lotw",
        Service::Qrz => "qrz",
    }
}

/// Decode a provider discriminator without inferring it from field contents.
pub(super) fn read_service(value: &str) -> anyhow::Result<Service> {
    match value {
        "lotw" => Ok(Service::Lotw),
        "qrz" => Ok(Service::Qrz),
        _ => anyhow::bail!("unknown service discriminator"),
    }
}

/// Store a historical public descriptor as an owned copy, retaining unknown QRZ book identity.
pub(super) async fn descriptor(
    connection: &mut SqliteConnection,
    account: AccountId,
    payload: i64,
    value: &Destination,
) -> anyhow::Result<i64> {
    crate::payload::identity(
        connection,
        RecordKey {
            account,
            id: RecordId::Service(ServiceRecordId::Destination(value.id)),
        },
    )
    .await?;
    let account = account.to_string();
    let destination = value.id.to_string();
    Ok(match &value.collection {
        RemoteCollection::Lotw {
            account: lotw_account,
        } => {
            sqlx::query_file!(
                "sql/service/descriptor_insert.sql",
                payload,
                account,
                destination,
                "lotw",
                lotw_account,
                None::<String>,
                value.description
            )
            .fetch_one(connection)
            .await?
            .descriptor_id
        }
        RemoteCollection::Qrz { book_id } => {
            let book = book_id.as_ref().map(|id| id.as_str());
            sqlx::query_file!(
                "sql/service/descriptor_insert.sql",
                payload,
                account,
                destination,
                "qrz",
                None::<String>,
                book,
                value.description
            )
            .fetch_one(connection)
            .await?
            .descriptor_id
        }
    })
}

/// Restore a descriptor only within its owning payload and account.
pub(super) async fn load_descriptor(
    connection: &mut SqliteConnection,
    account: AccountId,
    payload: i64,
    descriptor: i64,
) -> anyhow::Result<Destination> {
    let account = account.to_string();
    let row = sqlx::query_file_as!(
        DestinationRow,
        "sql/service/descriptor_select.sql",
        descriptor,
        payload,
        account
    )
    .fetch_one(connection)
    .await?;
    let collection = match (CollectionRow {
        collection: row.collection.as_str(),
        lotw_account: row.lotw_account.as_deref(),
        qrz_book: row.qrz_book.as_deref(),
    }) {
        CollectionRow {
            collection: "lotw",
            lotw_account: Some(account),
            qrz_book: None,
        } => RemoteCollection::Lotw {
            account: account.to_owned(),
        },
        CollectionRow {
            collection: "qrz",
            lotw_account: None,
            qrz_book,
        } => RemoteCollection::Qrz {
            book_id: qrz_book.map(|book| book.parse()).transpose()?,
        },
        _ => anyhow::bail!("invalid destination collection fields"),
    };
    let value = Destination {
        id: row.destination_id.parse()?,
        collection,
        description: row.description,
    };
    clacks_model::service::destination::validate_destinations(std::slice::from_ref(&value))?;
    Ok(value)
}

/// Intern an exact account/destination-qualified remote identity using binary equality.
pub(super) async fn external(
    connection: &mut SqliteConnection,
    account: AccountId,
    value: &ExternalRecordKey,
) -> anyhow::Result<i64> {
    crate::payload::identity(
        connection,
        RecordKey {
            account,
            id: RecordId::Service(ServiceRecordId::Destination(value.destination)),
        },
    )
    .await?;
    let account = account.to_string();
    let destination = value.destination.to_string();
    let canonical = serde_json::to_string(&value.record)?;
    sqlx::query_file!("sql/external/insert.sql", account, destination, canonical)
        .execute(&mut *connection)
        .await?;
    Ok(
        sqlx::query_file!("sql/external/find.sql", account, destination, canonical)
            .fetch_one(connection)
            .await?
            .external_id,
    )
}

/// Restore an exact key without applying fuzzy matching or converting QRZ digits to integers.
pub(super) async fn load_external(
    connection: &mut SqliteConnection,
    account: AccountId,
    external: i64,
) -> anyhow::Result<ExternalRecordKey> {
    let account = account.to_string();
    let row = sqlx::query_file!("sql/external/select.sql", external, account)
        .fetch_one(connection)
        .await?;
    let record = serde_json::from_str(&row.canonical_key)?;
    Ok(ExternalRecordKey {
        destination: row.destination_id.parse()?,
        record,
    })
}
