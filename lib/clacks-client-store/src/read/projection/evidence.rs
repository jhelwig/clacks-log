//! Scope retained service evidence before applying the shared native rules.

mod observations;
mod status;

use super::{ContactFacts, ProjectionNeeds, ReadContext};
use crate::{metadata, payload};
use clacks_model::{contact::Qso, effective::StationFacts, service::evidence::ServicePayload};
use clacks_sync::{
    record::{AcceptedRecord, RecordKey, RecordState, RecordValue},
    service::ServiceRecord,
};
use serde::Serialize;

/// Owned relevant native records; no account-wide mirror or refreshed provider data.
#[derive(Default)]
pub(super) struct Evidence {
    /// Current saved working service values.
    pub(super) working: Vec<ServiceRecord>,
    /// Retained accepted envelopes needed by observation ordering and baselines.
    pub(super) accepted: Vec<AcceptedRecord>,
    /// Actual installed timeline interpretation when this client has one.
    pub(super) interpretation: Option<metadata::lineage::Interpretation>,
}

/// A bound key for a batched relational selection, not a SQL identifier.
#[derive(Serialize)]
struct KeySelection {
    /// Fixed native record kind.
    kind: String,
    /// Native unique record identifier.
    record_id: String,
}

/// Effective counterpart facts plus independently derived current status.
#[derive(Default)]
pub(super) struct Evaluated {
    /// Eligible directory facts for this contact's current normalized key.
    pub(super) directory: Option<StationFacts>,
    /// Selected read values returned to the caller.
    pub(super) facts: ContactFacts,
    /// Eligible LoTW counterpart facts before whole-source selection.
    pub(super) lotw: Option<StationFacts>,
    /// Eligible QRZ counterpart facts before whole-source selection.
    pub(super) qrz: Option<StationFacts>,
}

/// Load this batch's roots and their real retained interpretation dependencies.
pub(super) async fn load(
    connection: &mut sqlx::SqliteConnection,
    account_id: clacks_model::ids::AccountId,
    ids: &str,
) -> anyhow::Result<Evidence> {
    let account = account_id.to_string();
    let rows = sqlx::query_file!("sql/read/projection/evidence.sql", account, ids)
        .fetch_all(&mut *connection)
        .await?;
    let mut result = Evidence::default();
    let mut keys = Vec::new();
    for row in rows {
        let kind = row.kind;
        let record_id = row.record_id;
        let id = payload::record_id(&kind, &record_id)?;
        let key = RecordKey {
            account: account_id,
            id,
        };
        if matches!(id, clacks_sync::record::RecordId::Service(_))
            && let Some(client) = metadata::client::load(connection, key).await?
            && let Some(RecordState::Live(RecordValue::Service(s))) = client.working_state()
        {
            result.working.push((**s).clone());
        }
        keys.push(KeySelection { kind, record_id });
    }
    let keys = serde_json::to_string(&keys)?;
    for row in sqlx::query_file!("sql/read/projection/accepted.sql", account, keys)
        .fetch_all(&mut *connection)
        .await?
    {
        let key = RecordKey {
            account: account_id,
            id: payload::record_id(&row.kind, &row.record_id)?,
        };
        result.accepted.push(
            metadata::client::observation::load_envelope(connection, key, row.envelope_id).await?,
        );
    }
    result.interpretation = metadata::lineage::interpretation(connection, account_id).await?;
    Ok(result)
}

/// Derive confirmation/fallback first, then publication against current values.
pub(super) fn evaluate(
    qso: &Qso,
    input: &Evidence,
    context: &ReadContext,
    needs: ProjectionNeeds,
    directory: Option<clacks_model::directory::values::DirectorySnapshot>,
) -> anyhow::Result<Evaluated> {
    let mut result = Evaluated {
        directory: directory.as_ref().map(|snapshot| StationFacts {
            name: snapshot.facts().name.clone(),
            location: snapshot.facts().location.clone(),
            references: snapshot.facts().references.clone(),
        }),
        facts: ContactFacts {
            directory,
            ..Default::default()
        },
        ..Default::default()
    };
    let related = input.working.iter().any(|r| match &r.payload {
        ServicePayload::Imported(v) => v.qso == qso.id(),
        ServicePayload::Attempt(v) => v.qso == qso.id(),
        ServicePayload::Association(v) => v.qso == qso.id(),
        _ => false,
    });
    if !related {
        return Ok(result);
    }
    let projection = match context.projection() {
        Ok(v) => v,
        Err(e) => {
            result.facts.diagnostics.push(e.to_string());
            return Ok(result);
        }
    };
    observations::evaluate(qso, input, context, &projection, &mut result)?;
    if needs.services {
        status::evaluate(qso, input, context, &projection, &mut result)?;
    }
    Ok(result)
}
