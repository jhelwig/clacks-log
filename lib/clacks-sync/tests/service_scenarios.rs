use crate::support;
#[test]
fn collection_never_uses_acknowledgment_as_permission_to_drop_semantic_facts() -> anyhow::Result<()>
{
    use clacks_model::service::{
        destination::{Destination, RemoteCollection},
        evidence::{ServicePayload, processing::DeletionSuppression},
    };
    use clacks_sync::{
        record::{RecordId, RecordState, RecordValue},
        service::{
            ServiceRecord, ServiceSupport,
            retention::{requirements, validate_collection},
        },
    };
    let destination = Destination {
        id: support::id(42),
        collection: RemoteCollection::Qrz {
            book_id: Some("7".parse().unwrap()),
        },
        description: "book".into(),
    };
    let payload = ServicePayload::Suppression(DeletionSuppression {
        id: support::id(101),
        qso: support::id(10),
        external: clacks_model::service::destination::ExternalRecordKey {
            destination: destination.id,
            record: clacks_model::service::destination::RemoteRecordId::Qrz("7".parse().unwrap()),
        },
    });
    let mut row = support::accepted(200, "fixture");
    row.key.id = RecordId::Service(payload.id());
    row.state = RecordState::Live(RecordValue::Service(Box::new(ServiceRecord {
        payload,
        support: ServiceSupport {
            destination: Some(destination),
            ..Default::default()
        },
    })));
    let r = requirements(&[row.clone()], &[])?;
    assert!(validate_collection(&r, &[]).is_err());
    validate_collection(&r, &[row.key])?;
    Ok(())
}
