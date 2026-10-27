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
    let qso = contact()?;
    let expected = qso.clone();
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
    let reopened = clacks_client_store::store::ClientStore::open_discovered(
        &paths.database,
        Default::default(),
    )
    .await?;
    let saved = clacks_client_store::save::load(&reopened, key)
        .await?
        .unwrap();
    assert_eq!(
        saved.working_state(),
        Some(&RecordState::Live(RecordValue::Qso(Box::new(expected))))
    );
    reopened.close().await?;
    Ok(account)
}

/// Native recorded facts shared by actual composition operations.
fn contact() -> anyhow::Result<Qso> {
    Ok(Qso {
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
    })
}

/// Create and acknowledge the same submitted contact through the desktop adapter.
pub(super) async fn retry_contact(paths: &WorkspacePaths) -> anyhow::Result<RecordKey> {
    use crate::session::settings::contacts::{self, ContactAction, ContactResponse};
    use clacks_client_store::{save::SaveOutcome, store::ClientStore};

    let store = ClientStore::create(&paths.database, AccountId::new(), Default::default()).await?;
    let mut qso = contact()?;
    qso.sent_report = Some("SECRET_SENTINEL".into());
    qso.contacted_operator = Some(clacks_model::contact::OperatorInformation {
        callsign: None,
        recorded_name: Some("SECRET_SENTINEL".into()),
    });
    let key = RecordKey {
        account: store.account_id(),
        id: clacks_sync::record::RecordId::Qso(qso.id),
    };
    assert!(matches!(
        contacts::execute(
            &store,
            ContactAction::Save {
                expected: None,
                next: Box::new(qso.clone())
            }
        )
        .await?,
        ContactResponse::Saved(SaveOutcome::Saved(_))
    ));
    assert!(matches!(
        contacts::execute(
            &store,
            ContactAction::Save {
                expected: None,
                next: Box::new(qso)
            }
        )
        .await?,
        ContactResponse::Saved(SaveOutcome::Unchanged(_))
    ));
    store.close().await?;
    Ok(key)
}
