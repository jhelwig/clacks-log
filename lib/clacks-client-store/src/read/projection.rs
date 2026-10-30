//! Native read projections shared by history and Worked Before.

mod evidence;
mod station;

use crate::{payload, rows::contact::WorkingContactRow};
use clacks_model::{
    effective::Sourced,
    export::{ExportContext, OutputPurpose, ProfileSet},
    group::DefinitionSet,
    ids::{AccountId, OutputProfileId, QsoId},
    location::RecordedLocation,
    recognition::RecognitionCatalog,
    service::{adif::ProjectionContext, lotw::LotwConfiguration},
};
use clacks_sync::record::{RecordKey, RecordState, RecordValue};
use sqlx::SqliteConnection;
use std::collections::BTreeMap;

/// Derived families required by a query or its visible columns.
#[derive(Clone, Copy, Default)]
pub(crate) struct ProjectionNeeds {
    /// Effective name and whole locations are needed.
    pub(crate) station: bool,
    /// Current positive service flags are needed.
    pub(crate) services: bool,
}

/// Current positive service state, independent of scheduler or credential state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServiceStatus {
    /// Current service-relevant values have positive publication coverage.
    pub uploaded: bool,
    /// Retained positive confirmation applies to the current contact.
    pub confirmed: bool,
}

/// Read-only facts selected without changing the recorded QSO.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContactFacts {
    /// Current definition names for this QSO's recorded reference groups.
    pub group_names: BTreeMap<clacks_model::ids::GroupId, String>,
    /// Effective contacted name and its actual source.
    pub name: Option<Sourced<String>>,
    /// Whole own-station location, including authoritative unknown geography.
    pub own_location: Option<Sourced<Option<RecordedLocation>>>,
    /// Whole contacted-station location, including authoritative unknown geography.
    pub contact_location: Option<Sourced<Option<RecordedLocation>>>,
    /// Current LoTW coverage and confirmation.
    pub lotw: ServiceStatus,
    /// Current QRZ coverage and confirmation.
    pub qrz: ServiceStatus,
    /// Evidence inconsistencies or missing interpretation needed by these facts.
    pub diagnostics: Vec<String>,
}

/// One account-qualified QSO's requested native projection.
#[derive(Clone, Debug)]
pub(crate) struct ContactProjection {
    /// Stable contact identity.
    pub(crate) id: QsoId,
    /// Selected source facts and derived status.
    pub(crate) facts: ContactFacts,
}

/// Current account definitions and public compatibility inputs for one read.
struct ReadContext {
    /// Ordinary synchronized reference definitions, not fresh seed data.
    definitions: DefinitionSet,
    /// Saved output profiles, including ordinary interchange.
    profiles: ProfileSet,
    /// Chosen ordinary output context when this account has one.
    ordinary: Option<ExportContext>,
    /// No unrecorded per-contact export ambiguity choices are invented.
    resolutions: BTreeMap<QsoId, clacks_model::export::select::ContextResolution>,
    /// Shared current recognition catalog.
    recognition: &'static RecognitionCatalog,
    /// Public LoTW compatibility definitions, never signing secrets.
    lotw: LotwConfiguration,
}

impl ReadContext {
    /// Borrow the installed ordinary context for service normalization.
    fn projection(&self) -> anyhow::Result<ProjectionContext<'_>> {
        Ok(ProjectionContext {
            recognition: self.recognition,
            definitions: &self.definitions,
            profiles: &self.profiles,
            export: self
                .ordinary
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Ordinary output profile is unavailable"))?,
            resolutions: &self.resolutions,
        })
    }
}

/// Read account definitions from current working roles in this snapshot.
async fn context(
    connection: &mut SqliteConnection,
    account_id: AccountId,
) -> anyhow::Result<ReadContext> {
    let account = account_id.to_string();
    let rows = sqlx::query_file!("sql/read/projection/definitions.sql", account)
        .fetch_all(&mut *connection)
        .await?;
    let mut groups = Vec::new();
    let mut profiles = ProfileSet::new();
    for row in rows {
        let id = payload::record_id(&row.kind, &row.record_id)?;
        match payload::load(
            connection,
            RecordKey {
                account: account_id,
                id,
            },
            row.payload_id,
        )
        .await?
        {
            RecordState::Live(RecordValue::Group(g)) => groups.push(*g),
            RecordState::Live(RecordValue::OutputProfile(p)) => {
                profiles.insert(p.id, *p);
            }
            _ => anyhow::bail!("Definition read selected an incompatible payload"),
        }
    }
    let ordinary: Option<OutputProfileId> = profiles
        .values()
        .find(|p| p.purpose == OutputPurpose::OrdinaryInterchange)
        .map(|p| p.id);
    Ok(ReadContext {
        definitions: DefinitionSet::new(groups)?,
        profiles,
        ordinary: ordinary.map(|output_profile_id| ExportContext {
            output_profile_id,
            target_group_id: None,
        }),
        resolutions: BTreeMap::new(),
        recognition: RecognitionCatalog::adif_318()?,
        lotw: LotwConfiguration::bundled()?,
    })
}

/// Select requested facts for a bounded batch of current saved working contacts.
///
/// Call from the same snapshot as the enclosing query's rows and counts. Missing
/// evidence remains missing; this function performs no provider requests or saves.
pub(crate) async fn load(
    connection: &mut SqliteConnection,
    account_id: AccountId,
    ids: &[QsoId],
    needs: ProjectionNeeds,
) -> anyhow::Result<Vec<ContactProjection>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let account = account_id.to_string();
    let ids = serde_json::to_string(ids)?;
    let rows = sqlx::query_file_as!(
        WorkingContactRow,
        "sql/read/projection/contacts.sql",
        account,
        ids
    )
    .fetch_all(&mut *connection)
    .await?;
    let rows = super::history::working_contacts(connection, account_id, rows).await?;
    let context = context(connection, account_id).await?;
    let evidence = if needs.station || needs.services {
        evidence::load(connection, account_id, &ids).await?
    } else {
        Default::default()
    };
    let mut result = Vec::new();
    for row in rows {
        let mut facts = evidence::evaluate(&row.qso, &evidence, &context, needs)?;
        if needs.station {
            station::select(&row.qso, &context.definitions, &mut facts);
        }
        facts.facts.group_names = row
            .qso
            .own_station()
            .references
            .iter()
            .chain(&row.qso.contacted_station().references)
            .filter_map(|r| {
                context
                    .definitions
                    .get(r.group_id)
                    .map(|g| (g.id, g.name.clone()))
            })
            .collect();
        result.push(ContactProjection {
            id: row.qso.id(),
            facts: facts.facts,
        });
    }
    Ok(result)
}
