use clacks_model::ids::DestinationId;
use clacks_model::service::destination::{
    Destination, ExternalRecordKey, RemoteCollection, RemoteRecordId, validate_destinations,
};

fn book(id: DestinationId, book_id: Option<u64>) -> Destination {
    Destination {
        id,
        collection: RemoteCollection::Qrz {
            book_id: book_id.map(|value| value.to_string().parse().unwrap()),
        },
        description: "Main".into(),
    }
}

#[test]
fn duplicate_remote_collection_is_rejected() {
    assert!(
        validate_destinations(&[
            book(DestinationId::new(), Some(42)),
            book(DestinationId::new(), Some(42))
        ])
        .is_err()
    );
}

#[test]
fn unknown_book_id_is_not_inferred() {
    assert!(
        validate_destinations(&[
            book(DestinationId::new(), None),
            book(DestinationId::new(), Some(42))
        ])
        .is_err()
    );
}

#[test]
fn distinct_credentials_do_not_create_destinations() {
    let mut destination = book(DestinationId::new(), Some(42));
    let key = ExternalRecordKey {
        destination: destination.id,
        record: RemoteRecordId::Qrz("7".parse().unwrap()),
    };
    destination.description = "Renewed credentials".into();
    assert_eq!(key.destination, destination.id);
    assert!(validate_destinations(&[destination]).is_ok());
}

#[test]
fn remote_ids_are_destination_scoped() {
    assert_ne!(
        ExternalRecordKey {
            destination: DestinationId::new(),
            record: RemoteRecordId::Qrz("7".parse().unwrap())
        },
        ExternalRecordKey {
            destination: DestinationId::new(),
            record: RemoteRecordId::Qrz("7".parse().unwrap())
        }
    );
}

#[test]
fn service_ids_round_trip() -> anyhow::Result<()> {
    let d = book(DestinationId::new(), Some(42));
    let decoded: Destination = serde_json::from_str(&serde_json::to_string(&d)?)?;
    assert_eq!(decoded, d);
    assert!(
        validate_destinations(&[
            d.clone(),
            Destination {
                description: "different".into(),
                ..d
            }
        ])
        .is_err()
    );
    Ok(())
}

#[test]
fn retained_snapshots_refine_unknowns_but_reject_changed_established_collections()
-> anyhow::Result<()> {
    use clacks_model::service::destination::validate_destination_snapshots;
    let id = DestinationId::new();
    let old = book(id, None);
    let known = book(id, Some(42));
    let renamed = Destination {
        description: "Renamed".into(),
        ..known.clone()
    };
    for snapshots in [
        vec![old.clone(), known.clone(), renamed.clone()],
        vec![renamed, known.clone(), old],
    ] {
        validate_destination_snapshots(&snapshots)?;
    }
    assert!(validate_destination_snapshots(&[known.clone(), book(id, Some(43))]).is_err());
    assert!(
        validate_destination_snapshots(&[
            known,
            Destination {
                id,
                collection: RemoteCollection::Lotw {
                    account: "other-service".into()
                },
                description: "Other".into()
            }
        ])
        .is_err()
    );
    Ok(())
}
