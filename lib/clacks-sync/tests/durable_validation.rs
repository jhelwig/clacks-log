mod support;
use clacks_sync::{
    client::ClientRecord,
    error::UnknownReason,
    lineage::Lineage,
    record::{Observation, RecordId, RecordKey},
};
use support::id;

#[test]
fn durable_client_structure_and_lineage_root_are_available() -> anyhow::Result<()> {
    let lineage = Lineage::new(id(1), id(2));
    assert_eq!(lineage.initial_timeline(), id(2));
    let key = RecordKey {
        account: id(1),
        id: RecordId::Qso(id(10)),
    };
    let mut row = ClientRecord {
        key,
        latest: Observation::Unavailable {
            key,
            reason: UnknownReason::MissingOriginal,
        },
        retained: vec![],
        pending: None,
    };
    row.validate()?;
    row.latest = Observation::Unavailable {
        key: RecordKey {
            account: id(5),
            ..key
        },
        reason: UnknownReason::MissingOriginal,
    };
    assert!(row.validate().is_err());
    Ok(())
}
