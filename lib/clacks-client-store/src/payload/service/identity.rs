//! Credential-free destination snapshots and exact provider record keys.

use crate::rows::service::{CollectionFields, DestinationRow, ExternalKeyFields};
use anyhow::Context;
use clacks_model::{
    ids::AccountId,
    scalar::{RecordedCallsign, UtcSecond},
    service::{
        destination::{
            Destination, ExternalRecordKey, LotwRecordKey, RemoteCollection, RemoteRecordId,
            Service,
        },
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
    let fields = match &value.collection {
        RemoteCollection::Lotw { account } => CollectionFields {
            collection: "lotw",
            lotw_account: Some(account.as_str()),
            qrz_book: None,
        },
        RemoteCollection::Qrz { book_id } => CollectionFields {
            collection: "qrz",
            lotw_account: None,
            qrz_book: book_id.as_ref().map(|id| id.as_str()),
        },
    };
    Ok(sqlx::query_file!(
        "sql/service/descriptor_insert.sql",
        payload,
        account,
        destination,
        fields.collection,
        fields.lotw_account,
        fields.qrz_book,
        value.description
    )
    .fetch_one(connection)
    .await?
    .descriptor_id)
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
    let collection = match (CollectionFields {
        collection: row.collection.as_str(),
        lotw_account: row.lotw_account.as_deref(),
        qrz_book: row.qrz_book.as_deref(),
    }) {
        CollectionFields {
            collection: "lotw",
            lotw_account: Some(account),
            qrz_book: None,
        } => RemoteCollection::Lotw {
            account: account.to_owned(),
        },
        CollectionFields {
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
    let fields = match &value.record {
        RemoteRecordId::Qrz(id) => ExternalKeyFields {
            service: "qrz",
            qrz_id: Some(id.as_str()),
            own_call: None,
            worked_call: None,
            start: None,
            band: None,
            receive_band: None,
            mode: None,
        },
        RemoteRecordId::Lotw(key) => ExternalKeyFields {
            service: "lotw",
            qrz_id: None,
            own_call: Some(key.own_call.as_str()),
            worked_call: Some(key.worked_call.as_str()),
            start: Some(key.start.unix_seconds()),
            band: Some(key.band.as_str()),
            receive_band: Some(key.receive_band.as_str()),
            mode: Some(key.mode.as_str()),
        },
    };
    sqlx::query_file!(
        "sql/external/insert.sql",
        account,
        destination,
        fields.service,
        fields.qrz_id,
        fields.own_call,
        fields.worked_call,
        fields.start,
        fields.band,
        fields.receive_band,
        fields.mode
    )
    .execute(&mut *connection)
    .await?;
    Ok(sqlx::query_file!(
        "sql/external/find.sql",
        account,
        destination,
        fields.service,
        fields.qrz_id,
        fields.own_call,
        fields.worked_call,
        fields.start,
        fields.band,
        fields.receive_band,
        fields.mode
    )
    .fetch_one(connection)
    .await?
    .external_id)
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
    let record = match row.service.as_str() {
        "qrz"
            if row.own_call.is_none()
                && row.worked_call.is_none()
                && row.start.is_none()
                && row.band.is_none()
                && row.receive_band.is_none()
                && row.mode.is_none() =>
        {
            RemoteRecordId::Qrz(row.qrz_id.context("QRZ key lacks record ID")?.parse()?)
        }
        "lotw" if row.qrz_id.is_none() => RemoteRecordId::Lotw(LotwRecordKey {
            own_call: RecordedCallsign::new(&row.own_call.context("LoTW key lacks own callsign")?)?,
            worked_call: RecordedCallsign::new(
                &row.worked_call.context("LoTW key lacks worked callsign")?,
            )?,
            start: UtcSecond::from_unix_seconds(row.start.context("LoTW key lacks start")?),
            band: row.band.context("LoTW key lacks band")?,
            receive_band: row.receive_band.context("LoTW key lacks receive band")?,
            mode: row.mode.context("LoTW key lacks mode")?,
        }),
        _ => anyhow::bail!("invalid external key shape"),
    };
    Ok(ExternalRecordKey {
        destination: row.destination_id.parse()?,
        record,
    })
}
