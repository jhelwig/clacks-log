//! Shared equipment deletion must respect current accepted profile references.

use super::super::support::synchronization::{NativeAccount, qso};
use crate::store::synchronization::accept_pending;
use clacks_model::{
    ids::{EquipmentConfigurationId, ProfileId, QsoId},
    profile::{EquipmentConfigurationFields, SavedLocationProfileFields},
};
use clacks_sync::{
    acceptance::{AcceptanceDecision, RejectionReason},
    record::{RecordId, RecordKey, RecordState, RecordValue},
};

#[tokio::test]
async fn equipment_deletion_waits_for_profile_reassignment() -> anyhow::Result<()> {
    let native = NativeAccount::create().await?;
    let equipment = EquipmentConfigurationFields {
        id: EquipmentConfigurationId::new(),
        name: "Portable rig".into(),
        values: Default::default(),
    };
    let key = RecordKey {
        account: native.first.account,
        id: RecordId::Equipment(equipment.id),
    };
    let create = native
        .prepare(
            &native.first,
            key,
            RecordState::Live(RecordValue::Equipment(Box::new(equipment.try_into()?))),
        )
        .await?;
    assert!(matches!(
        accept_pending(&native.store, &native.first, &create).await?,
        AcceptanceDecision::Commit(_)
    ));
    let profile = SavedLocationProfileFields {
        id: ProfileId::new(),
        name: "Portable station".into(),
        station: qso(QsoId::new(), Default::default()).own_station().clone(),
        default_equipment: match key.id {
            RecordId::Equipment(id) => Some(id),
            _ => unreachable!(),
        },
    };
    let profile_key = RecordKey {
        account: key.account,
        id: RecordId::Profile(profile.id),
    };
    let create = native
        .prepare(
            &native.second,
            profile_key,
            RecordState::Live(RecordValue::Profile(Box::new(profile.clone().try_into()?))),
        )
        .await?;
    assert!(matches!(
        accept_pending(&native.store, &native.second, &create).await?,
        AcceptanceDecision::Commit(_)
    ));
    let delete = native
        .prepare(&native.first, key, RecordState::Deleted)
        .await?;
    assert!(
        matches!(accept_pending(&native.store,&native.first,&delete).await?,AcceptanceDecision::Rejected(RejectionReason::Referenced {ref referrers,total:1,..}) if referrers == &[profile_key])
    );
    let mut cleared = profile;
    cleared.default_equipment = None;
    let update = native
        .prepare(
            &native.second,
            profile_key,
            RecordState::Live(RecordValue::Profile(Box::new(cleared.try_into()?))),
        )
        .await?;
    assert!(matches!(
        accept_pending(&native.store, &native.second, &update).await?,
        AcceptanceDecision::Commit(_)
    ));
    assert!(matches!(
        accept_pending(&native.store, &native.first, &delete).await?,
        AcceptanceDecision::Commit(_)
    ));
    Ok(())
}
