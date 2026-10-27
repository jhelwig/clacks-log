use crate::support;

use clacks_lotw::{
    certificate::{eligibility::CredentialState, import_pkcs12},
    signing::{BatchItem, ProducerInformation, definition::SignatureDefinitions, write_batch},
};
use clacks_model::{
    contact::{BandFrequency, OwnOperatingContext, Qso, StationSnapshot},
    group::DefinitionSet,
    ids::{DestinationId, QsoId},
    recognition::{
        RecognitionCatalog,
        band::Band,
        mode::{Mode, ModeIdentity},
    },
    scalar::{RecordedCallsign, UtcSecond},
    service::{
        destination::{Destination, RemoteCollection},
        error::ProjectionDecision,
        lotw::{self, LotwConfiguration, LotwProjectionInput, LotwSubmission},
    },
};

#[test]
fn native_projection_survives_storage_and_signing_without_remapping() -> anyhow::Result<()> {
    let credential = import_pkcs12(&support::fixture("1024/normal.p12"), "fixture-password")?;
    let catalog = RecognitionCatalog::adif_318()?;
    let group: clacks_model::ids::GroupId = "00000000000000000000000001".parse()?;
    let definitions = DefinitionSet::new(serde_json::from_value(serde_json::json!([{
        "id": group, "name": "Island references", "identifier": { "required": true, "normalization": [], "syntax": null }, "bears_location_context": true, "catalog": null,
        "export_mappings": {"ordinary_interchange": [{"field":"MY_IOTA", "value":{"Join":{"values":{"Identifiers":"Own"},"separator":","}},"when":"Always"}], "targeted":{}}
    }]))?)?;
    let mut qso = Qso {
        id: QsoId::new(),
        start: UtcSecond::from_unix_seconds(support::now().unix_timestamp()),
        end: None,
        own_station: StationSnapshot {
            callsign: RecordedCallsign::new("N0CALL")?,
            location: None,
            references: [clacks_model::reference::ReferenceAssociation {
                group_id: group,
                identifier: Some("NA-001".into()),
            }]
            .into(),
        },
        contacted_station: StationSnapshot {
            callsign: RecordedCallsign::new("K1ABC")?,
            location: None,
            references: Default::default(),
        },
        own_context: OwnOperatingContext::default(),
        contacted_operator: None,
        primary: BandFrequency::new(None, Some(Band::from_recorded_label("20m")?), catalog)?,
        receive_override: None,
        mode: Mode::from_recorded_identity(ModeIdentity::Specialization {
            label: "USB".into(),
            parent: Some("SSB".into()),
        })?,
        sent_report: None,
        received_report: None,
        comment: None,
        tags: Default::default(),
    };
    let destination = Destination {
        id: DestinationId::new(),
        collection: RemoteCollection::Lotw {
            account: "test".into(),
        },
        description: "test".into(),
    };
    let config = LotwConfiguration::bundled()?;
    for receive in [false, true] {
        if receive {
            qso.receive_override = Some(BandFrequency::new(
                None,
                Some(Band::from_recorded_label("40m")?),
                catalog,
            )?);
        }
        let ProjectionDecision::Ready(projected) = lotw::project(&LotwProjectionInput {
            qso: &qso,
            destination: &destination,
            signing: &credential.metadata().identity,
            configuration: &config,
            definitions: &definitions,
        })?
        else {
            panic!("valid native fixture must be ready")
        };
        let original = projected.value;
        let stored: LotwSubmission = serde_json::from_slice(&serde_json::to_vec(&original)?)?;
        for submission in [&original, &stored] {
            let mut bytes = Vec::new();
            write_batch(
                &mut bytes,
                &[BatchItem {
                    submission,
                    credential: &credential,
                    state: CredentialState::Available,
                }],
                &SignatureDefinitions::bundled()?,
                &ProducerInformation {
                    application: "Clacks Log",
                    version: "test",
                },
                support::now(),
            )?;
            let fields = support::fields(&bytes);
            for (name, value) in [
                ("MODE", "SSB"),
                ("QSO_DATE", "2026-10-22"),
                ("QSO_TIME", "12:34:56Z"),
                ("IOTA", "NA-001"),
            ] {
                assert!(
                    fields
                        .iter()
                        .any(|f| f.name == name && f.bytes == value.as_bytes())
                );
            }
            assert!(
                !fields
                    .iter()
                    .any(|f| f.name == "FREQ" || f.name == "FREQ_RX")
            );
            assert_eq!(
                fields
                    .iter()
                    .find(|f| f.name == "BAND_RX")
                    .map(|f| f.bytes.as_slice()),
                receive.then_some(b"40M".as_slice())
            );
        }
        assert_eq!(original, stored);
        assert_eq!(original.external_key(), stored.external_key());
        let mut mismatched = stored.clone();
        mismatched.signing_identity.dxcc = "1".into();
        assert!(
            write_batch(
                &mut Vec::new(),
                &[BatchItem {
                    submission: &mismatched,
                    credential: &credential,
                    state: CredentialState::Available
                }],
                &SignatureDefinitions::bundled()?,
                &ProducerInformation {
                    application: "Clacks",
                    version: "test"
                },
                support::now()
            )
            .is_err()
        );
    }
    Ok(())
}
