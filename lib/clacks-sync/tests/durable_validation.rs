use crate::support;
use clacks_sync::{
    client::ClientRecord,
    error::UnknownReason,
    record::{Observation, RecordId, RecordKey},
};
use support::id;

#[test]
fn client_record_validation_rejects_cross_account_observations() -> anyhow::Result<()> {
    let key = RecordKey {
        account: id(1),
        id: RecordId::Qso(id(10)),
    };
    let mut fields = clacks_sync::client::ClientRecordFields {
        key,
        latest: Observation::Unavailable {
            key,
            reason: UnknownReason::MissingOriginal,
        },
        retained: vec![],
        pending: None,
    };
    let _: ClientRecord = fields.clone().try_into()?;
    fields.latest = Observation::Unavailable {
        key: RecordKey {
            account: id(5),
            ..key
        },
        reason: UnknownReason::MissingOriginal,
    };
    assert!(ClientRecord::try_from(fields).is_err());
    Ok(())
}
