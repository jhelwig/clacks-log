#[path = "support/reviewed_recipe.rs"]
mod reviewed_recipe;

use clacks_geography_builder::{compile::compile_bundle, recipe::read_recipe};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

fn cache() -> PathBuf {
    std::env::var_os("CLACKS_GEOGRAPHY_CACHE")
        .map(PathBuf::from)
        .expect("set CLACKS_GEOGRAPHY_CACHE to the explicitly fetched pinned-source cache")
}

#[test]
#[ignore = "explicit maintenance of reviewed inputs; requires pinned geographic source cache"]
fn reviewed_recipe_inputs_match_original_sources() {
    reviewed_recipe::verify_or_refresh(&cache());
}

/// Shipping data must retain actual county identities and classify independent
/// published-source locations through the production reader, including all U.S.
/// regions and separate radio entities. This explicit full-source acceptance is
/// release preparation, not ordinary application CI.
#[test]
#[ignore = "requires pinned full shipping-source cache; optional CLACKS_GEOGRAPHY_BUNDLE reuses an already built immutable bundle"]
fn shipping_inventory_and_required_us_fields() {
    use clacks_geography::artifact::SourceIdentity;
    use clacks_geography_builder::source::decode_sources;
    use geo::{BooleanOps, Relate};
    use geo_types::{Geometry, Rect};

    let recipe = read_recipe(Path::new("../../references/geography/shipping.json")).unwrap();
    let mut administrative = recipe.clone();
    administrative
        .sources
        .retain(|source| ["states", "counties", "ct-counties"].contains(&source.id.as_str()));
    administrative.selections.retain(|selection| {
        ["states", "counties", "ct-counties"].contains(&selection.source_id.as_str())
    });
    let originals = decode_sources(&administrative, &cache(), &AtomicBool::new(false)).unwrap();
    let ids = |source: &str| {
        originals
            .features
            .iter()
            .filter(|feature| feature.identity.source_id == source)
            .map(|feature| feature.identity.clone())
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(ids("states").len(), 56);
    assert_eq!(ids("counties").len(), 3226);
    assert!(
        ids("counties")
            .iter()
            .all(|identity| !identity.feature_id.starts_with("09"))
    );
    assert_eq!(
        ids("ct-counties"),
        [
            "09001", "09003", "09005", "09007", "09009", "09011", "09013", "09015"
        ]
        .into_iter()
        .map(|feature| SourceIdentity {
            source_id: "ct-counties".into(),
            feature_id: feature.into()
        })
        .collect()
    );

    let county = |id: &str| {
        let feature = originals
            .features
            .iter()
            .find(|feature| {
                feature.identity.source_id == "counties" && feature.identity.feature_id == id
            })
            .unwrap();
        let Geometry::MultiPolygon(geometry) = &feature.geometry else {
            panic!("Census county must be multipart");
        };
        geometry.clone()
    };
    let multnomah = county("41051");
    let clark = county("53011");
    let cell = Rect::new(
        (-122.66666666666667, 45.583333333333336),
        (-122.58333333333333, 45.625),
    )
    .to_polygon();
    assert!(
        multnomah.union(&clark).relate(&cell).is_covers(),
        "original county administrative land/water footprints establish complete whole-cell coverage independently of compiled lookup"
    );
    assert!(multnomah.relate(&cell).is_intersects());
    assert!(clark.relate(&cell).is_intersects());
    let original_ids: BTreeSet<_> = originals
        .features
        .iter()
        .map(|feature| feature.identity.clone())
        .collect();
    drop(originals);

    let temporary = tempfile::tempdir().unwrap();
    let bundle_path = if let Some(path) = std::env::var_os("CLACKS_GEOGRAPHY_BUNDLE") {
        PathBuf::from(path)
    } else {
        let compiled =
            compile_bundle(&recipe, "real-sources", &cache(), &AtomicBool::new(false)).unwrap();
        clacks_geography::artifact::write::write_bundle(temporary.path(), &compiled.contents)
            .unwrap();
        temporary.path().to_owned()
    };
    let mut bundle = clacks_geography::artifact::validate::open_bundle(&bundle_path).unwrap();
    let compiled_ids: BTreeSet<_> = bundle
        .manifest()
        .derivations
        .iter()
        .flat_map(|row| &row.source_identities)
        .filter(|identity| {
            ["states", "counties", "ct-counties"].contains(&identity.source_id.as_str())
        })
        .cloned()
        .collect();
    assert_eq!(
        compiled_ids, original_ids,
        "declared output provenance covers every original administrative identity"
    );
    clacks_geography_builder::compile::verify_bundle(&mut bundle, &recipe).unwrap();
}
