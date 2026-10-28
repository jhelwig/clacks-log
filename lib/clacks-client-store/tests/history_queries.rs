use crate::support;
use clacks_client_store::{
    logbooks::selection::SelectedView,
    read::{self, contacts::ContactPageQuery},
    save,
    store::{ClientStore, StoreSettings},
};
use clacks_model::{
    contact::{BandFrequency, OperatorInformation},
    defaults::account_definitions,
    ids::QsoId,
    location::RecordedLocation,
    position::{Coordinates, GridLength, Locator, Position},
    query,
    recognition::{
        RecognitionCatalog,
        band::Band,
        mode::{Mode, ModeIdentity},
    },
    reference::ReferenceAssociation,
    scalar::{Hertz, RecordedCallsign, UtcSecond},
    tag::LogbookTag,
};
use clacks_sync::record::{RecordState, RecordValue};

/// One coherent saved account scenario reused across query assertions.
struct Fixture {
    /// Keep the workspace directory alive for the real store.
    _directory: tempfile::TempDir,
    /// Production store initialized with ordinary account definitions.
    store: ClientStore,
}

async fn fixture() -> anyhow::Result<Fixture> {
    let directory = tempfile::tempdir()?;
    let seed = account_definitions()?;
    let group = |name| seed.groups.iter().find(|g| g.name == name).unwrap().id;
    let store = ClientStore::create_with_definitions(
        &directory.path().join("history.sqlite"),
        support::id(1),
        &seed,
        StoreSettings::default(),
    )
    .await?;
    for (id, name) in [(20, "A"), (21, "A"), (22, "B")] {
        let value = RecordValue::Tag(Box::new(LogbookTag::new(support::id(id), name.into())));
        save::save(&store, support::key(&value), None, RecordState::Live(value)).await?;
    }
    let mut activation = support::qso(support::id(10));
    activation.start = UtcSecond::from_unix_seconds(1_793_059_200 + 18 * 3600);
    activation.contacted_station.callsign = RecordedCallsign::new("W7HEX")?;
    activation.comment = Some("activation rain 100% _ [x]".into());
    activation.contacted_operator = Some(OperatorInformation {
        callsign: None,
        recorded_name: Some("Élodie".into()),
    });
    activation.tags = [support::id(20), support::id(21), support::id(22)].into();
    activation.mode = Mode::from_recorded_identity(ModeIdentity::Specialization {
        label: "USB".into(),
        parent: Some("SSB".into()),
    })?;
    activation.primary = BandFrequency::from_recorded_pair(
        Some(Hertz::new(14_074_000)?),
        Some(Band::from_recorded_label("20M")?),
    )?;
    activation.receive_override = Some(BandFrequency::from_recorded_pair(
        None,
        Some(Band::from_recorded_label("40M")?),
    )?);
    activation.contacted_station.location = Some(RecordedLocation {
        position: Some(Position::GridArea(Locator::parse("CN85ab")?)),
        ..Default::default()
    });
    activation.contacted_station.references = [
        ReferenceAssociation {
            group_id: group("POTA"),
            identifier: Some("US-0001".into()),
        },
        ReferenceAssociation {
            group_id: group("IOTA"),
            identifier: Some("EU-001".into()),
        },
    ]
    .into();
    activation.own_station.references = [ReferenceAssociation {
        group_id: group("070-160M-SPRINT"),
        identifier: None,
    }]
    .into();
    let mut portable = support::qso(support::id(11));
    portable.start = UtcSecond::from_unix_seconds(1_793_059_200 + 23 * 3600);
    portable.end = Some(UtcSecond::from_unix_seconds(1_793_059_200 + 25 * 3600));
    portable.contacted_station.callsign = RecordedCallsign::new("W7HEX/P")?;
    portable.comment = Some("activation dry".into());
    portable.contacted_operator = Some(OperatorInformation {
        callsign: None,
        recorded_name: Some("Alice".into()),
    });
    portable.tags.insert(support::id(20));
    portable.primary = BandFrequency::from_recorded_pair(
        Some(Hertz::new(7_074_000)?),
        Some(Band::from_recorded_label("40M")?),
    )?;
    portable.mode = Mode::from_recorded_identity(ModeIdentity::Specialization {
        label: "LSB".into(),
        parent: Some("SSB".into()),
    })?;
    portable.contacted_station.location = Some(RecordedLocation {
        position: Some(Position::GridArea(Locator::parse("CN85")?)),
        ..Default::default()
    });
    portable
        .contacted_station
        .references
        .insert(ReferenceAssociation {
            group_id: group("070-160M-SPRINT"),
            identifier: None,
        });
    let mut coarse = support::qso(support::id(12));
    coarse.start = UtcSecond::from_unix_seconds(1_793_059_200 + 30 * 3600);
    coarse.comment = Some("rain".into());
    coarse.primary = BandFrequency::from_recorded_pair(
        Some(Hertz::new(14_000_001)?),
        Some(Band::from_recorded_label("20M")?),
    )?;
    coarse.contacted_station.location = Some(RecordedLocation {
        position: Some(Position::GridArea(Locator::parse("CN")?)),
        ..Default::default()
    });
    let mut precise = support::qso(support::id(13));
    precise.start = activation.start;
    precise.tags.insert(support::id(21));
    precise.primary = BandFrequency::from_recorded_pair(
        Some(Hertz::new(14_000_000)?),
        Some(Band::from_recorded_label("20M")?),
    )?;
    precise.mode = Mode::from_recorded_identity(ModeIdentity::Specialization {
        label: "future-mode".into(),
        parent: None,
    })?;
    precise.contacted_station.location = Some(RecordedLocation {
        position: Some(Position::ExplicitPoint {
            coordinates: Coordinates::from_microdegrees(45_062_500, -123_958_333)?,
            grid_length: GridLength::new(2)?,
        }),
        ..Default::default()
    });
    precise
        .contacted_station
        .references
        .insert(ReferenceAssociation {
            group_id: support::id(99),
            identifier: Some("unknown-1".into()),
        });
    for qso in [activation, portable, coarse, precise] {
        save::save(
            &store,
            support::qso_key(qso.id),
            None,
            support::contact(qso),
        )
        .await?;
    }
    Ok(Fixture {
        _directory: directory,
        store,
    })
}

async fn matching(store: &ClientStore, expression: &str) -> anyhow::Result<Vec<QsoId>> {
    let mut snapshot = read::snapshot(store).await?;
    let page = snapshot
        .contact_page(&ContactPageQuery {
            filter: query::parse(expression, RecognitionCatalog::adif_318()?)?,
            ..Default::default()
        })
        .await?;
    let mut ids: Vec<_> = page.rows.into_iter().map(|r| r.working.qso.id).collect();
    ids.sort();
    snapshot.close().await?;
    Ok(ids)
}

#[tokio::test]
async fn expressions_select_unique_working_contacts() -> anyhow::Result<()> {
    let f = fixture().await?;
    assert_eq!(
        matching(&f.store, "tag:A").await?,
        vec![support::id(10), support::id(11), support::id(13)]
    );
    assert_eq!(
        matching(&f.store, "tag:A tag:B").await?,
        vec![support::id(10)]
    );
    assert_eq!(
        matching(&f.store, "comment:activation comment:rain -name:alice").await?,
        vec![support::id(10)]
    );
    assert_eq!(
        matching(&f.store, "(comment:activation OR comment:rain) -name:alice").await?,
        vec![support::id(10), support::id(12)]
    );
    assert_eq!(
        matching(&f.store, r#"comment:"100% _ [x]" name:=éLODIE"#).await?,
        vec![support::id(10)]
    );
    assert!(
        matching(&f.store, "contact.reference:IOTA/US-0001")
            .await?
            .is_empty()
    );
    assert_eq!(
        matching(&f.store, "contact.reference:POTA/US-0001").await?,
        vec![support::id(10)]
    );
    assert_eq!(
        matching(
            &f.store,
            "contact.activity:070-160M-SPRINT -has:contact.reference"
        )
        .await?,
        vec![support::id(11)]
    );
    assert_eq!(
        matching(&f.store, "has:contact.reference").await?,
        vec![support::id(10), support::id(13)]
    );
    assert_eq!(
        matching(&f.store, "own.activity:070-160M-SPRINT").await?,
        vec![support::id(10)]
    );
    f.store.close().await?;
    Ok(())
}

#[tokio::test]
async fn field_bindings_preserve_missing_and_position_semantics() -> anyhow::Result<()> {
    let f = fixture().await?;
    assert_eq!(
        matching(&f.store, "contact.grid:CN85ab").await?,
        vec![support::id(10), support::id(13)]
    );
    assert_eq!(
        matching(&f.store, "contact.grid:=CN85ab").await?,
        vec![support::id(10), support::id(13)]
    );
    assert_eq!(
        matching(&f.store, "recorded.contact.grid:CN85").await?,
        vec![support::id(10), support::id(11), support::id(13)]
    );
    assert_eq!(
        matching(&f.store, "-has:receive.frequency").await?,
        vec![support::id(10)]
    );
    assert_eq!(
        matching(
            &f.store,
            "has:recorded.receive.band -has:recorded.receive.frequency"
        )
        .await?,
        vec![support::id(10)]
    );
    assert_eq!(
        matching(&f.store, "-name:alice").await?,
        vec![support::id(10), support::id(12), support::id(13)]
    );
    assert_eq!(
        matching(&f.store, "qso_start:2026-10-28").await?,
        vec![support::id(12)]
    );
    assert_eq!(
        matching(&f.store, "qso_end:2026-10-28").await?,
        vec![support::id(11)]
    );
    assert_eq!(
        matching(&f.store, "has:qso_end").await?,
        vec![support::id(11)]
    );
    let fine =
        Coordinates::from_microdegrees(45_062_500, -123_958_333)?.locator(GridLength::new(12)?);
    assert_eq!(
        matching(&f.store, &format!("contact.grid:{fine}")).await?,
        vec![support::id(13)]
    );
    f.store.close().await?;
    Ok(())
}

#[tokio::test]
async fn typed_domain_predicates_match_saved_facts() -> anyhow::Result<()> {
    let f = fixture().await?;
    assert_eq!(
        matching(&f.store, "mode:SSB").await?,
        vec![support::id(10), support::id(11)]
    );
    assert_eq!(
        matching(&f.store, "mode:=USB").await?,
        vec![support::id(10)]
    );
    assert_eq!(
        matching(&f.store, "mode:FUTURE-MODE").await?,
        vec![support::id(13)]
    );
    assert_eq!(
        matching(&f.store, "frequency:>14.0000005MHz").await?,
        vec![support::id(10), support::id(12)]
    );
    assert_eq!(
        matching(&f.store, "band:20M receive.band:40M").await?,
        vec![support::id(10)]
    );
    assert_eq!(
        matching(&f.store, "lotw.uploaded:false qrz.confirmed:false").await?,
        vec![
            support::id(10),
            support::id(11),
            support::id(12),
            support::id(13)
        ]
    );
    f.store.close().await?;
    Ok(())
}

#[tokio::test]
async fn filtered_counts_and_rows_share_a_snapshot() -> anyhow::Result<()> {
    let f = fixture().await?;
    let mut snapshot = read::snapshot(&f.store).await?;
    let mut query = ContactPageQuery {
        filter: query::parse("comment:rain", RecognitionCatalog::adif_318()?)?,
        limit: 1,
        selected_ids: [support::id(10), support::id(11), support::id(13)].into(),
        ..Default::default()
    };
    let page = snapshot.contact_page(&query).await?;
    assert_eq!(page.rows.first().unwrap().working.qso.id, support::id(12));
    assert_eq!(
        (
            page.matching_total,
            page.selected_total,
            page.selected_outside_filter
        ),
        (2, 3, 2)
    );
    assert!(page.has_more);
    query.show_selected = true;
    let page = snapshot.contact_page(&query).await?;
    assert_eq!(page.rows.first().unwrap().working.qso.id, support::id(11));
    assert_eq!(
        (
            page.matching_total,
            page.selected_total,
            page.selected_outside_filter
        ),
        (2, 3, 2)
    );
    let record = save::load(&f.store, support::qso_key(support::id(10)))
        .await?
        .unwrap();
    save::save(&f.store, record.key, Some(&record), RecordState::Deleted).await?;
    query.show_selected = false;
    query.offset = 1;
    let page = snapshot.contact_page(&query).await?;
    assert_eq!(page.rows.first().unwrap().working.qso.id, support::id(10));
    assert_eq!(page.matching_total, 2);
    snapshot.close().await?;
    let mut snapshot = read::snapshot(&f.store).await?;
    let page = snapshot.contact_page(&query).await?;
    assert!(page.rows.is_empty());
    assert_eq!(
        (
            page.matching_total,
            page.selected_total,
            page.selected_outside_filter
        ),
        (1, 2, 2)
    );
    let scoped = snapshot
        .contact_page(&ContactPageQuery {
            view: SelectedView::Logbook(support::id(20)),
            ..Default::default()
        })
        .await?;
    assert_eq!(scoped.matching_total, 1);
    snapshot.close().await?;
    f.store.close().await?;
    Ok(())
}

#[tokio::test]
async fn service_queries_distinguish_current_upload_and_confirmation() -> anyhow::Result<()> {
    use clacks_model::service::{destination::Service, evidence::ImportedClaim};
    let f = fixture().await?;
    for (qso_id, assertion, service, claim) in [
        (10, 1000, Service::Qrz, ImportedClaim::Uploaded),
        (11, 1001, Service::Lotw, ImportedClaim::Confirmed),
    ] {
        let record = save::load(&f.store, support::qso_key(support::id(qso_id)))
            .await?
            .unwrap();
        let Some(RecordState::Live(RecordValue::Qso(qso))) = record.working_state() else {
            panic!("saved QSO")
        };
        let value = support::service::imported(qso, assertion, service, claim)?;
        let RecordState::Live(v) = &value else {
            panic!("claim")
        };
        save::save(&f.store, support::key(v), None, value.clone()).await?;
    }
    assert_eq!(
        matching(&f.store, "qrz.uploaded:true").await?,
        vec![support::id(10)]
    );
    assert!(matching(&f.store, "qrz.confirmed:true").await?.is_empty());
    assert_eq!(
        matching(&f.store, "lotw.confirmed:true lotw.uploaded:false").await?,
        vec![support::id(11)]
    );
    assert!(matching(&f.store, "lotw.uploaded:true").await?.is_empty());
    f.store.close().await?;
    Ok(())
}

#[tokio::test]
async fn catalog_and_group_interpretation_changes_do_not_require_qso_edits() -> anyhow::Result<()> {
    use clacks_model::recognition::{ModeRow, SubmodeRow};
    let f = fixture().await?;
    let mut catalog = RecognitionCatalog::adif_318()?.clone();
    catalog.modes.push(ModeRow {
        code: "FutureFamily".into(),
        deprecated: false,
    });
    catalog.submodes.push(SubmodeRow {
        code: "future-mode".into(),
        parent: "FutureFamily".into(),
        deprecated: false,
    });
    let mut snapshot = read::snapshot(&f.store).await?;
    let page = snapshot
        .contact_page(&ContactPageQuery {
            filter: query::parse("mode:FutureFamily", &catalog)?,
            ..Default::default()
        })
        .await?;
    assert_eq!(
        page.rows
            .iter()
            .map(|r| r.working.qso.id)
            .collect::<Vec<_>>(),
        vec![support::id(13)]
    );
    assert_eq!(
        page.rows.first().unwrap().working.qso.mode.identity(),
        ModeIdentity::Specialization {
            label: "future-mode".into(),
            parent: None
        }
    );
    snapshot.close().await?;
    let mut definition = account_definitions()?
        .groups
        .into_iter()
        .find(|g| g.name == "POTA")
        .unwrap();
    definition.id = support::id(99);
    definition.name = "Later group".into();
    let value = RecordValue::Group(Box::new(definition));
    save::save(
        &f.store,
        support::key(&value),
        None,
        RecordState::Live(value),
    )
    .await?;
    assert_eq!(
        matching(&f.store, r#"contact.reference:"Later group"/unknown-1"#).await?,
        vec![support::id(13)]
    );
    f.store.close().await?;
    Ok(())
}
