//! Account-locked native acceptance inputs loaded from the same PostgreSQL transaction.

use crate::store::{
    metadata::{self, account, lineage::Interpretation},
    payload,
};
use clacks_model::service::{
    destination::{Destination, RemoteCollection, validate_destination_snapshots},
    evidence::ServicePayload,
};
use clacks_sync::{
    acceptance::AcceptanceInput,
    account::AccountMetadata,
    membership::MembershipSnapshot,
    record::{AcceptedRecord, Observation, RecordId, RecordKey, RecordState, RecordValue},
    service::{
        BindingVersion, DestinationRegistryObservation, ExternalBindingObservation,
        VersionReference,
    },
};
use sqlx::PgConnection;
use std::collections::BTreeSet;

/// Owned rows backing the core's borrowed input for one transaction.
pub(super) struct AcceptanceContext {
    /// Current account head and complete represented coverage.
    pub account: AccountMetadata,
    /// Actual current client lifecycle and acknowledgments.
    pub membership: MembershipSnapshot,
    /// Current target, including certified absence.
    pub current: Observation,
    /// Required references and retained exact support.
    pub references: Vec<Observation>,
    /// Complete retained public namespace registry, when admitting service values.
    pub registry: Option<DestinationRegistryObservation>,
    /// Actual index occupants or explicit absence.
    pub bindings: Vec<ExternalBindingObservation>,
    /// Actual retained baseline and ancestry meanings.
    pub interpretation: Interpretation,
}

impl AcceptanceContext {
    /// Borrow the loaded rows without changing their native transition meaning.
    pub(super) fn input(&self) -> AcceptanceInput<'_> {
        AcceptanceInput {
            destination_registry: self.registry.as_ref(),
            bindings: &self.bindings,
            account: &self.account,
            membership: &self.membership,
            current: &self.current,
            references: &self.references,
        }
    }
}

/// Read the latest accepted head; a missing head is absent only in this actual server view.
pub(crate) async fn observation(
    connection: &mut PgConnection,
    account: &AccountMetadata,
    key: RecordKey,
) -> anyhow::Result<Observation> {
    anyhow::ensure!(
        account.account == key.account,
        "observation crosses account"
    );
    let owner = key.account.to_string();
    let kind = payload::kind(key.id);
    let id = key.id.to_string();
    match sqlx::query_file!("sql/synchronization/head_select.sql", owner, kind, id)
        .fetch_optional(&mut *connection)
        .await?
    {
        Some(row) => Ok(Observation::Accepted(
            metadata::accepted::load_envelope(connection, key, row.envelope_id).await?,
        )),
        None => Ok(Observation::Absent {
            key,
            checkpoint: super::checkpoint(account),
        }),
    }
}

/// Read one exact accepted support version without substituting an unrelated newer value.
pub(crate) async fn exact(
    connection: &mut PgConnection,
    reference: &VersionReference,
) -> anyhow::Result<Option<AcceptedRecord>> {
    let key = reference.key;
    let owner = key.account.to_string();
    let kind = payload::kind(key.id);
    let id = key.id.to_string();
    let timeline = reference.stamp.accepted_at().timeline.to_string();
    let revision = i64::try_from(reference.stamp.accepted_at().revision)?;
    let baseline = reference.stamp.baseline().to_string();
    for row in sqlx::query_file!(
        "sql/persistence/client/envelope_candidates.sql",
        owner,
        kind,
        id,
        timeline,
        revision,
        baseline
    )
    .fetch_all(&mut *connection)
    .await?
    {
        let value = metadata::accepted::load_envelope(connection, key, row.envelope_id).await?;
        if value.stamp == reference.stamp
            && matches!(value.state(), RecordState::Deleted) == reference.deleted
        {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

/// Load target, prerequisites, namespace and binding observations under the caller's account lock.
pub(super) async fn load(
    connection: &mut PgConnection,
    key: RecordKey,
    state: &RecordState,
    dependencies: &[clacks_sync::client::VersionCondition],
) -> anyhow::Result<AcceptanceContext> {
    let account = account::load(connection, key.account).await?;
    let membership = crate::enrollment::membership::snapshot(connection, &account).await?;
    let interpretation = account::interpretation(connection, key.account).await?;
    let current = observation(connection, &account, key).await?;
    let mut keys: BTreeSet<_> = clacks_sync::acceptance::required_references(key, state)
        .into_iter()
        .chain(dependencies.iter().map(|value| value.key()))
        .collect();
    let service = match state {
        RecordState::Live(RecordValue::Service(value)) => Some(value.as_ref()),
        _ => None,
    };
    if let Some(service) = service {
        keys.extend(clacks_sync::service::dependency::required_keys(
            key.account,
            service,
        ));
        keys.extend(
            service
                .support
                .required
                .iter()
                .map(|value| value.reference().key),
        );
    }
    let mut references = Vec::new();
    for reference in keys {
        anyhow::ensure!(
            reference.account == key.account,
            "reference crosses account"
        );
        references.push(observation(connection, &account, reference).await?);
    }
    let mut registry = None;
    let mut bindings = Vec::new();
    if let Some(service) = service {
        for required in &service.support.required {
            if let Some(value) = exact(connection, required.reference()).await? {
                let observed = Observation::Accepted(value);
                if !references.contains(&observed) {
                    references.push(observed);
                }
            }
        }
        let owner = key.account.to_string();
        let mut destinations = Vec::new();
        for row in sqlx::query_file!("sql/synchronization/namespaces.sql", owner)
            .fetch_all(&mut *connection)
            .await?
        {
            let collection = match (row.collection.as_str(), row.lotw_account, row.qrz_book) {
                ("lotw", Some(account), None) => RemoteCollection::Lotw { account },
                ("qrz", None, book) => RemoteCollection::Qrz {
                    book_id: book.map(|value| value.parse()).transpose()?,
                },
                _ => anyhow::bail!("invalid retained destination namespace"),
            };
            destinations.push(Destination {
                id: row.destination_id.parse()?,
                collection,
                description: row.description,
            });
        }
        registry = Some(DestinationRegistryObservation {
            account: key.account,
            head: account.head,
            destinations: validate_destination_snapshots(&destinations)?,
        });
        let external = match &service.payload {
            ServicePayload::Association(value) => Some(&value.external),
            ServicePayload::Suppression(value) => Some(&value.external),
            _ => None,
        };
        if let Some(external) = external {
            let destination = external.destination.to_string();
            let encoded = serde_json::to_string(external)?;
            let row = sqlx::query_file!(
                "sql/synchronization/binding_select.sql",
                owner,
                destination,
                encoded
            )
            .fetch_optional(&mut *connection)
            .await?;
            let binding = match row {
                Some(row) if row.association_id.is_some() => {
                    let association = row
                        .association_id
                        .ok_or_else(|| anyhow::anyhow!("binding identity missing"))?
                        .parse()?;
                    let envelope = row
                        .envelope_id
                        .ok_or_else(|| anyhow::anyhow!("binding envelope missing"))?;
                    let binding_key = RecordKey {
                        account: key.account,
                        id: RecordId::Service(
                            clacks_model::service::evidence::ServiceRecordId::Association(
                                association,
                            ),
                        ),
                    };
                    let accepted =
                        metadata::accepted::load_envelope(connection, binding_key, envelope)
                            .await?;
                    let RecordState::Live(RecordValue::Service(value)) = accepted.state() else {
                        anyhow::bail!("binding has no service value");
                    };
                    let ServicePayload::Association(value) = &value.payload else {
                        anyhow::bail!("binding has no association value");
                    };
                    Some(BindingVersion {
                        association,
                        qso: value.qso,
                        stamp: accepted.stamp.clone(),
                        state: value.state,
                    })
                }
                _ => None,
            };
            bindings.push(ExternalBindingObservation {
                account: key.account,
                external: external.clone(),
                binding,
            });
        }
    }
    Ok(AcceptanceContext {
        account,
        membership,
        current,
        references,
        registry,
        bindings,
        interpretation,
    })
}
