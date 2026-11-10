//! Real-reader checks for desktop selection and native geometry forwarding.

use super::*;
use clacks_geography::lookup::Coverage;
use clacks_model::position::{Coordinates, GridLength, Locator};

#[tokio::test]
async fn lookup_preserves_native_geometry() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    crate::test_support::geography::write(directory.path(), [-123.5, 47.0, -122.5, 48.0])?;
    let client = Client::open(&crate::test_support::geography::selection(
        directory.path(),
    )?)?;
    let grid = Position::GridArea(Locator::parse("CN87")?);
    let whole = client.lookup(grid.clone()).await?;
    assert_eq!(whole.primary_subdivision.coverage, Coverage::Partial);
    let point = Position::ExplicitPoint {
        coordinates: Coordinates::from_decimal_degrees("47.5", "-123")?,
        grid_length: GridLength::new(6)?,
    };
    let center = client.lookup(point).await?;
    assert_eq!(center.primary_subdivision.coverage, Coverage::Complete);
    assert_eq!(
        center
            .primary_subdivision
            .candidates
            .first()
            .map(|c| c.value.as_str()),
        Some("WA")
    );
    Ok(())
}
