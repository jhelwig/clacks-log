//! Real persisted operations used to observe application composition decisions.

use crate::workspace::WorkspacePaths;
use clacks_model::{
    contact::{BandFrequency, OwnOperatingContext, Qso, StationSnapshot},
    ids::{AccountId, QsoId},
    recognition::{
        RecognitionCatalog,
        mode::{Mode, ModeIdentity},
    },
    scalar::{Hertz, RecordedCallsign, UtcSecond},
};
use clacks_sync::record::{RecordKey, RecordState, RecordValue};

/// Persist a real local QSO and return its account identity for correlation assertions.
pub(super) async fn save_contact(paths: &WorkspacePaths) -> anyhow::Result<AccountId> {
    let account = AccountId::new();

    let store = clacks_client_store::store::ClientStore::create_with_definitions(
        &paths.database,
        account,
        &clacks_model::defaults::account_definitions()?,
        Default::default(),
    )
    .await?;
    let qso = Qso {
        id: QsoId::new(),
        start: UtcSecond::from_unix_seconds(1000),
        end: None,
        own_station: StationSnapshot {
            callsign: RecordedCallsign::new("W7HEX")?,
            location: None,
            references: Default::default(),
        },
        contacted_station: StationSnapshot {
            callsign: RecordedCallsign::new("K1ABC")?,
            location: None,
            references: Default::default(),
        },
        own_context: OwnOperatingContext::default(),
        contacted_operator: None,
        primary: BandFrequency::new(
            Some(Hertz::new(14_074_000)?),
            None,
            RecognitionCatalog::adif_318()?,
        )?,
        receive_override: None,
        mode: Mode::from_recorded_identity(ModeIdentity::Specialization {
            label: "FT4".into(),
            parent: Some("MFSK".into()),
        })?,
        sent_report: None,
        received_report: None,
        comment: Some("SECRET_SENTINEL".into()),
        tags: Default::default(),
    };
    let value = RecordValue::Qso(Box::new(qso));
    let key = RecordKey {
        account,
        id: value.id(),
    };

    assert!(matches!(
        clacks_client_store::save::save(&store, key, None, RecordState::Live(value)).await?,
        clacks_client_store::save::SaveOutcome::Saved(_)
    ));

    store.close().await?;
    Ok(account)
}
