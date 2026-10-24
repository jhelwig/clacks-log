use clacks_model::service::destination::RemoteRecordId;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn qrz_identity_preserves_decimal_values_beyond_machine_integers() -> TestResult {
    let encoded = r#"{"Qrz":"184467440737095516160000000001"}"#;
    let identity: RemoteRecordId = serde_json::from_str(encoded)?;
    assert_eq!(serde_json::to_string(&identity)?, encoded);
    Ok(())
}

#[test]
fn decimal_ids_use_numeric_order_and_unbounded_carry() -> TestResult {
    use clacks_model::service::destination::{QrzBookId, QrzRecordId};
    assert_eq!(QrzRecordId::new("00010")?, QrzRecordId::new("10")?);
    assert!(QrzRecordId::new("9")? < QrzRecordId::new("10")?);
    assert_eq!(QrzRecordId::new("0")?.next_for_pagination().as_str(), "1");
    assert_eq!(
        QrzRecordId::new("18446744073709551615")?
            .next_for_pagination()
            .as_str(),
        "18446744073709551616"
    );
    assert_eq!(
        QrzRecordId::new("999999999999999999999999999999")?
            .next_for_pagination()
            .as_str(),
        "1000000000000000000000000000000"
    );
    assert!(QrzBookId::new("000").is_err());
    for invalid in ["", " 1", "1 ", "+1", "-1", "1.0", "１", "1a"] {
        assert!(QrzRecordId::new(invalid).is_err());
        assert!(serde_json::from_str::<QrzRecordId>(&serde_json::to_string(invalid)?).is_err());
    }
    Ok(())
}

#[test]
fn destinations_and_associations_keep_canonical_decimal_ids() -> TestResult {
    use clacks_model::ids::{AssociationId, DestinationId, QsoId};
    use clacks_model::service::destination::{
        Destination, ExternalRecordKey, QrzBookId, QrzRecordId, RemoteCollection,
        validate_destinations,
    };
    use clacks_model::service::evidence::processing::{AssociationState, ExternalAssociation};
    let destination = Destination {
        id: DestinationId::new(),
        collection: RemoteCollection::Qrz {
            book_id: Some(QrzBookId::new("001844674407370955161600")?),
        },
        description: "Historical book".into(),
    };
    validate_destinations(std::slice::from_ref(&destination))?;
    assert_eq!(
        serde_json::from_str::<Destination>(&serde_json::to_string(&destination)?)?,
        destination
    );
    let association = ExternalAssociation {
        id: AssociationId::new(),
        external: ExternalRecordKey {
            destination: destination.id,
            record: RemoteRecordId::Qrz(QrzRecordId::new("001844674407370955161611")?),
        },
        qso: QsoId::new(),
        state: AssociationState::Active,
    };
    assert_eq!(
        serde_json::from_str::<ExternalAssociation>(&serde_json::to_string(&association)?)?,
        association
    );
    Ok(())
}
