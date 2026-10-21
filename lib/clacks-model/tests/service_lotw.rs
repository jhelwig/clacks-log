mod support;
use clacks_model::{
    contact::BandFrequency,
    ids::{DestinationId, QsoId},
    recognition::RecognitionCatalog,
    recognition::band::Band,
    recognition::mode::{Mode, ModeIdentity},
    service::{
        destination::{Destination, RemoteCollection},
        error::ProjectionDecision,
        lotw::{self, LotwConfiguration, LotwProjectionInput, PublicSigningIdentity},
    },
};

#[test]
fn full_mapping_chain_and_unsupported_modes() -> anyhow::Result<()> {
    let c = LotwConfiguration::bundled()?;
    for (parent, sub, want) in [
        ("MFSK", Some("FT4"), "FT4"),
        ("FT4", None, "FT4"),
        ("FT8", None, "FT8"),
        ("SSB", Some("USB"), "SSB"),
        ("PSK", Some("QPSK31"), "PSK31"),
        ("MFSK", Some("JS8"), "DATA"),
        ("MFSK", None, "DATA"),
    ] {
        let m = Mode::from_recorded_identity(match sub {
            Some(s) => ModeIdentity::Specialization {
                label: s.into(),
                parent: Some(parent.into()),
            },
            None => ModeIdentity::Parent {
                label: parent.into(),
            },
        })?;
        assert_eq!(lotw::normalize_mode(&m, &c)?, want);
    }
    assert!(
        lotw::normalize_mode(
            &Mode::from_recorded_identity(ModeIdentity::Parent {
                label: "NOT-A-MODE".into()
            })?,
            &c
        )
        .is_err()
    );
    assert!(LotwConfiguration::from_json("{}").is_err());
    Ok(())
}

#[test]
fn projection_freezes_actual_values_and_whole_receive_override() -> anyhow::Result<()> {
    let c = LotwConfiguration::bundled()?;
    let mut q = support::qso(support::id::<QsoId>(1));
    let d = Destination {
        id: DestinationId::new(),
        collection: RemoteCollection::Lotw {
            account: "w7hex".into(),
        },
        description: "main".into(),
    };
    let signing = PublicSigningIdentity {
        callsign: q.own_station.callsign.clone(),
        dxcc: "291".into(),
    };
    let context = support::definitions();
    let get = |q: &clacks_model::contact::Qso| -> anyhow::Result<lotw::LotwSubmission> {
        match lotw::project(&LotwProjectionInput {
            qso: q,
            destination: &d,
            signing: &signing,
            configuration: &c,
            definitions: &context,
        })? {
            ProjectionDecision::Ready(r) => Ok(r.value),
            x => anyhow::bail!("{x:?}"),
        }
    };
    let original = get(&q)?;
    q.comment = Some("not sent".into());
    assert_eq!(original, get(&q)?);
    q.receive_override = Some(BandFrequency::new(
        None,
        Some(Band::from_recorded_label("40m")?),
        RecognitionCatalog::adif_318()?,
    )?);
    let rx = get(&q)?;
    assert_ne!(original.external_key(), rx.external_key());
    assert!(!rx.contact_fields.contains_key("FREQ_RX"));
    assert_eq!(rx.signing_identity.dxcc, "291");
    Ok(())
}
