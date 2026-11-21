use crate::support::{self, contact, id, qso, qso_key};
use clacks_client_store::{
    directory::snapshots,
    logbooks::selection::SelectedView,
    read::{
        self,
        export::{ExportCounts, ExportInput, ExportScope},
    },
    save::{self, SaveOutcome},
    store::{ClientStore, StoreSettings},
    transfer,
};
use clacks_model::{
    defaults::account_definitions,
    directory::values::{
        DirectoryFacts, DirectoryProvider, DirectorySnapshot, DirectorySnapshotFields, LookupKey,
    },
    effective::InformationSource,
    ids::QsoId,
    location::RecordedLocationFields,
    position::{Locator, Position},
    query,
    recognition::RecognitionCatalog,
    reference::ReferenceAssociation,
    scalar::{RecordedCallsign, UtcSecond, UtcTimestamp},
    service::{
        destination::Service,
        evidence::{ImportedClaim, ServicePayload},
    },
    tag::LogbookTag,
};
use clacks_sync::{
    account::{AccountMetadata, InstalledCheckpoint},
    lineage::Coverage,
    record::{RecordState, RecordValue},
    transfer::{
        WindowMode,
        discovery::{DiscoveryEnd, DiscoveryPage, DiscoveryStart},
    },
};

fn ids(input: &ExportInput) -> Vec<QsoId> {
    input
        .contacts
        .iter()
        .map(|contact| contact.qso.id())
        .collect()
}

#[tokio::test]
async fn export_scopes_capture_active_working_values_across_batches() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let store = ClientStore::create_with_definitions(
        &directory.path().join("export.sqlite"),
        id(1),
        &account_definitions()?,
        StoreSettings::default(),
    )
    .await?;
    let tag = id(30);
    for (tag, name) in [(tag, "Activation"), (id(31), "Trip")] {
        let value = RecordValue::Tag(Box::new(LogbookTag::new(tag, name.into())));
        save::save(&store, support::key(&value), None, RecordState::Live(value)).await?;
    }

    let mut shared = qso(id(100));
    support::edit(&mut shared, |fields| {
        fields.tags = [tag, id(31)].into();
        fields.comment = Some("original".into());
        Ok(())
    });
    let mut to_delete = qso(id(358));
    support::edit(&mut to_delete, |fields| {
        fields.tags.insert(tag);
        fields.comment = Some("match".into());
        Ok(())
    });
    let deletion_key = qso_key(to_delete.id());
    let deleted = support::accepted(2, contact(to_delete));
    let (_, baseline) = AccountMetadata::initialize(id(1), id(2), id(3));
    let source = InstalledCheckpoint {
        account: id(1),
        baseline: baseline.id,
        coverage: Coverage::from(deleted.stamp.accepted_at()),
    };
    let mut receipt = transfer::begin(
        &store,
        &DiscoveryStart {
            source: source.clone(),
            mode: WindowMode::Full,
        },
    )
    .await?;
    transfer::install_page(
        &store,
        &mut receipt,
        &DiscoveryPage {
            sequence: 0,
            requirements: vec![],
            accepted: vec![support::accepted(1, contact(shared.clone())), deleted],
            support: vec![],
            baselines: vec![baseline],
            boundaries: vec![],
            units: vec![],
        },
    )
    .await?;
    transfer::complete(&store, &receipt, &DiscoveryEnd { source, pages: 1 }).await?;
    let original = save::load(&store, qso_key(shared.id())).await?;
    support::edit(&mut shared, |fields| {
        fields.comment = Some("revised match".into());
        Ok(())
    });
    let SaveOutcome::Saved(working) = save::save(
        &store,
        qso_key(shared.id()),
        original.as_ref(),
        contact(shared.clone()),
    )
    .await?
    else {
        anyhow::bail!("The synced contact must have an unpublished working edit");
    };

    let deletion_basis = save::load(&store, deletion_key).await?;
    let SaveOutcome::Saved(_) = save::save(
        &store,
        deletion_key,
        deletion_basis.as_ref(),
        RecordState::Deleted,
    )
    .await?
    else {
        anyhow::bail!("The synced contact must have an unpublished deletion");
    };

    for identity in 101..=357 {
        let mut local = qso(id(identity));
        support::edit(&mut local, |fields| {
            fields.tags.insert(tag);
            fields.comment = Some(if identity == 101 { "other" } else { "match" }.into());
            Ok(())
        });
        save::save(&store, qso_key(local.id()), None, contact(local)).await?;
    }
    save::save(&store, qso_key(id(359)), None, contact(qso(id(359)))).await?;
    let selected = [id(100), id(101), id(358), id(359), id(360)].into();
    let view = SelectedView::Logbook(tag);
    let filter = query::parse("comment:match", RecognitionCatalog::adif_318()?)?;
    let mut snapshot = read::snapshot(&store).await?;
    assert_eq!(
        snapshot.export_counts(view, &filter, &selected).await?,
        ExportCounts {
            selected: 3,
            hidden: 2,
            matching: 257,
            all: 259
        }
    );
    let selected = snapshot
        .export_input(&ExportScope::Selected(selected))
        .await?;
    support::edit(&mut shared, |fields| {
        fields.comment = Some("later".into());
        fields.tags.clear();
        Ok(())
    });
    let SaveOutcome::Saved(_) = save::save(
        &store,
        qso_key(shared.id()),
        Some(&working),
        contact(shared),
    )
    .await?
    else {
        anyhow::bail!("The later edit must remove the shared contact from the matching scope");
    };
    let matching = snapshot
        .export_input(&ExportScope::Matching { view, filter })
        .await?;
    let all = snapshot.export_input(&ExportScope::All).await?;
    snapshot.close().await?;

    assert_eq!(ids(&selected), vec![id(100), id(101), id(359)]);
    assert_eq!(
        ids(&matching),
        std::iter::once(id(100))
            .chain((102..=357).map(id))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        ids(&all),
        (100..=357).chain([359]).map(id).collect::<Vec<_>>()
    );
    for input in [matching, all] {
        let edited = input
            .contacts
            .iter()
            .find(|contact| contact.qso.id() == id(100))
            .ok_or_else(|| anyhow::anyhow!("The snapshot must still include the edited contact"))?;
        assert_eq!(edited.qso.comment().as_deref(), Some("revised match"));
        assert_eq!(edited.qso.tags(), &[tag, id(31)].into());
    }
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn export_uses_whole_locations_and_only_applicable_confirmation_dates() -> anyhow::Result<()>
{
    let directory = tempfile::tempdir()?;
    let seed = account_definitions()?;
    let park = seed
        .groups
        .iter()
        .find(|group| group.name() == "POTA")
        .ok_or_else(|| anyhow::anyhow!("The account seed must define parks"))?;
    let contest = seed
        .groups
        .iter()
        .find(|group| group.name() == "070-160M-SPRINT")
        .ok_or_else(|| anyhow::anyhow!("The account seed must define the fixture contest"))?;
    let store = ClientStore::create_with_definitions(
        &directory.path().join("facts.sqlite"),
        id(1),
        &seed,
        StoreSettings::default(),
    )
    .await?;
    let mut first = qso(id(10));
    let mut stale = qso(id(11));
    let mut fallback = qso(id(12));
    let participation = ReferenceAssociation::new(contest.id(), None)?;
    let park_reference = ReferenceAssociation::new(park.id(), Some("US-0001".into()))?;
    support::edit(&mut first, |fields| {
        fields.contacted_station.location = Some(
            RecordedLocationFields {
                position: Some(Position::GridArea(Locator::parse("CN85")?)),
                ..Default::default()
            }
            .try_into()?,
        );
        fields
            .contacted_station
            .references
            .insert(participation.clone());
        Ok(())
    });
    support::edit(&mut fallback, |fields| {
        fields
            .contacted_station
            .references
            .insert(participation.clone());
        Ok(())
    });
    let date = UtcSecond::from_unix_seconds(1_793_000_000);
    for (index, qso) in [&first, &stale, &fallback].into_iter().enumerate() {
        save::save(&store, qso_key(qso.id()), None, contact(qso.clone())).await?;
        snapshots::apply(
            &store,
            qso.id(),
            DirectorySnapshot::new(DirectorySnapshotFields {
                provider: DirectoryProvider::Qrz,
                key: LookupKey::from_recorded(&qso.contacted_station().callsign),
                lookup_time: UtcTimestamp::new(1_793_000_100, 0)?,
                lookup_id: id(50),
                facts: DirectoryFacts {
                    name: Some("Fallback name".into()),
                    location: Some(
                        RecordedLocationFields {
                            position: Some(Position::GridArea(Locator::parse("IO91AA")?)),
                            ..Default::default()
                        }
                        .try_into()?,
                    ),
                    references: [park_reference.clone()].into(),
                    ..Default::default()
                },
            })?,
        )
        .await?;
        let mut assertion = support::service::imported(
            qso,
            index as u128 + 100,
            Service::Lotw,
            ImportedClaim::Confirmed,
        )?;
        let RecordState::Live(RecordValue::Service(service)) = &mut assertion else {
            anyhow::bail!("The fixture must contain a live service assertion");
        };
        let ServicePayload::Imported(claim) = &mut service.payload else {
            anyhow::bail!("The fixture must contain an imported confirmation");
        };
        claim.date = Some(date);
        save::save(
            &store,
            support::key(&RecordValue::Service(service.clone())),
            None,
            assertion,
        )
        .await?;
    }
    let original = save::load(&store, qso_key(stale.id())).await?;
    support::edit(&mut stale, |fields| {
        fields.contacted_station.callsign = RecordedCallsign::new("K2ABC")?;
        Ok(())
    });
    let SaveOutcome::Saved(_) = save::save(
        &store,
        qso_key(stale.id()),
        original.as_ref(),
        contact(stale),
    )
    .await?
    else {
        anyhow::bail!("The changed callsign must make the saved evidence stale");
    };

    let filter = query::parse(
        "name:=\"Fallback name\" AND lotw.confirmed:true",
        RecognitionCatalog::adif_318()?,
    )?;
    let mut snapshot = read::snapshot(&store).await?;
    let matching = snapshot
        .export_input(&ExportScope::Matching {
            view: SelectedView::AllContacts,
            filter,
        })
        .await?;
    let all = snapshot.export_input(&ExportScope::All).await?;
    snapshot.close().await?;

    assert_eq!(ids(&matching), vec![id(10), id(12)]);
    let [recorded, stale, directory] = all.contacts.as_slice() else {
        anyhow::bail!("Expected exactly the three saved contact identities");
    };
    assert_eq!(
        recorded
            .facts
            .contact_location
            .as_ref()
            .map(|location| location.source),
        Some(InformationSource::Recorded)
    );
    assert_eq!(
        recorded
            .facts
            .contact_location
            .as_ref()
            .and_then(|location| location.value.location.as_ref()),
        first.contacted_station().location.as_ref()
    );
    assert_eq!(
        recorded.contacted_references,
        [participation.clone()].into()
    );
    assert!(recorded.facts.lotw.confirmed);
    assert_eq!(
        recorded
            .confirmation_dates
            .iter()
            .map(|claim| (claim.service, claim.date))
            .collect::<Vec<_>>(),
        vec![(Service::Lotw, date)]
    );
    assert!(stale.facts.name.is_none());
    assert!(stale.facts.contact_location.is_none());
    assert!(!stale.facts.lotw.confirmed);
    assert!(stale.confirmation_dates.is_empty());
    assert_eq!(
        directory
            .facts
            .contact_location
            .as_ref()
            .map(|location| location.source),
        Some(InformationSource::Directory)
    );
    assert_eq!(
        directory.contacted_references,
        [participation, park_reference].into()
    );
    store.close().await?;
    Ok(())
}
