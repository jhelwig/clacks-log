mod support;
use clacks_client_store::{
    logbooks::selection::SelectedView,
    read::{
        self,
        contacts::{ContactColumn, ContactOrder, ContactPageQuery, SortDirection},
    },
    save,
    store::{ClientStore, StoreSettings},
};

#[tokio::test]
async fn selected_views_show_each_working_contact_once() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("contacts.sqlite"),
        support::id(1),
        StoreSettings::default(),
    )
    .await?;
    let tag = support::id(20);
    for n in [10, 11] {
        let mut qso = support::qso(support::id(n));
        if n == 10 {
            qso.tags.insert(tag);
            qso.tags.insert(support::id(21));
        }
        save::save(
            &store,
            support::qso_key(qso.id),
            None,
            support::contact(qso),
        )
        .await?;
    }
    let mut snapshot = read::snapshot(&store).await?;
    let query = ContactPageQuery {
        view: SelectedView::Untagged,
        ..Default::default()
    };
    let page = snapshot.contact_page(&query).await?;
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows.first().unwrap().qso.id, support::id(11));
    let all = snapshot.contact_page(&ContactPageQuery::default()).await?;
    assert_eq!(all.rows.len(), 2);
    let selected = snapshot
        .contact_page(&ContactPageQuery {
            view: SelectedView::Logbook(tag),
            selected: Some([support::id(10), support::id(11)].into()),
            ..Default::default()
        })
        .await?;
    assert_eq!(selected.rows.len(), 1);
    assert_eq!(selected.rows.first().unwrap().qso.id, support::id(10));
    snapshot.close().await?;
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn contact_pages_sort_complete_values_and_break_ties_by_id() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("contacts.sqlite"),
        support::id(1),
        StoreSettings::default(),
    )
    .await?;
    for (n, seconds) in [(10, 86_401), (11, 86_399), (12, 86_401)] {
        let mut qso = support::qso(support::id(n));
        qso.start = clacks_model::scalar::UtcSecond::from_unix_seconds(seconds);
        qso.end = None;
        save::save(
            &store,
            support::qso_key(qso.id),
            None,
            support::contact(qso),
        )
        .await?;
    }
    let mut snapshot = read::snapshot(&store).await?;
    let query = ContactPageQuery {
        limit: 1,
        order: ContactOrder {
            column: ContactColumn::Start,
            direction: SortDirection::Descending,
        },
        ..Default::default()
    };
    let page = snapshot.contact_page(&query).await?;
    assert!(page.has_more);
    assert_eq!(page.rows.first().unwrap().qso.id, support::id(10));
    let page = snapshot
        .contact_page(&ContactPageQuery { offset: 1, ..query })
        .await?;
    assert_eq!(page.rows.first().unwrap().qso.id, support::id(12));
    snapshot.close().await?;
    store.close().await?;
    Ok(())
}

#[tokio::test]
async fn grid_sort_orders_recorded_areas_and_point_display_locators_together() -> anyhow::Result<()>
{
    use clacks_model::{
        location::RecordedLocation,
        position::{Coordinates, GridLength, Locator, Position},
    };
    let dir = tempfile::tempdir()?;
    let store = ClientStore::create(
        &dir.path().join("grid.sqlite"),
        support::id(1),
        StoreSettings::default(),
    )
    .await?;
    for n in [10, 11, 12, 13] {
        let mut qso = support::qso(support::id(n));
        let position = match n {
            10 => Position::GridArea(Locator::parse("AA00")?),
            11 => Position::ExplicitPoint {
                coordinates: Coordinates::from_microdegrees(0, 0)?,
                grid_length: GridLength::new(6)?,
            },
            12 => Position::GridArea(Locator::parse("KK00")?),
            _ => Position::ExplicitPoint {
                coordinates: Coordinates::from_microdegrees(0, 180_000_000)?,
                grid_length: GridLength::new(6)?,
            },
        };
        qso.contacted_station.location = Some(RecordedLocation {
            position: Some(position),
            ..Default::default()
        });
        save::save(
            &store,
            support::qso_key(qso.id),
            None,
            support::contact(qso),
        )
        .await?;
    }
    let mut snapshot = read::snapshot(&store).await?;
    let query = ContactPageQuery {
        order: ContactOrder {
            column: ContactColumn::Grid,
            direction: SortDirection::Ascending,
        },
        ..Default::default()
    };
    let page = snapshot.contact_page(&query).await?;
    assert_eq!(
        page.rows.iter().map(|r| r.qso.id).collect::<Vec<_>>(),
        vec![
            support::id(10),
            support::id(13),
            support::id(11),
            support::id(12)
        ]
    );
    snapshot.close().await?;
    store.close().await?;
    Ok(())
}
