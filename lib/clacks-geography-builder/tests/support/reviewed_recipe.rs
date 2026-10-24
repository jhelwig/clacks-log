//! Reproducible geometry fixtures and independent facts from pinned original sources.

use clacks_geography::artifact::SourceIdentity;
use clacks_geography_builder::{recipe::read_recipe, source::decode_sources};
use geo::{BooleanOps, Relate};
use geo_types::{Geometry, Polygon, Rect};
use std::{path::Path, sync::atomic::AtomicBool};

fn identity(source: &str, feature: &str) -> SourceIdentity {
    SourceIdentity {
        source_id: source.into(),
        feature_id: feature.into(),
    }
}

pub fn verify_or_refresh(cache: &Path) {
    let path = Path::new("../../references/geography/representative.json");
    let recipe = read_recipe(path).unwrap();
    let sources = decode_sources(&recipe, cache, &AtomicBool::new(false)).unwrap();
    let original = sources
        .features
        .iter()
        .find(|feature| feature.identity == identity("itu", "2"))
        .unwrap();
    let Geometry::LineString(line) = &original.geometry else {
        panic!("not original ITU linework")
    };
    let correction = recipe
        .corrections
        .iter()
        .find(|rule| rule.id == "itu-2-closure")
        .unwrap();
    assert_eq!(correction.expected_feature_sha256, original.feature_sha256);
    let mut reviewed = line.clone();
    assert_eq!(reviewed.0.first().unwrap().x, -141.00403);
    assert_eq!(reviewed.0.last().unwrap().y, 79.99907);
    reviewed.0.pop();
    let replacement = geojson::Geometry::from(&Polygon::new(reviewed, vec![]));
    let replacement_bytes = serde_json::to_vec_pretty(&replacement).unwrap();
    let athos = geojson::Geometry::from(&Rect::new((23.9, 40.1), (24.5, 40.6)).to_polygon());
    let athos_bytes = serde_json::to_vec_pretty(&athos).unwrap();
    let cq19 = sources
        .features
        .iter()
        .find(|feature| feature.identity == identity("cq", "19"))
        .unwrap();
    let cq19_bytes = serde_json::to_vec_pretty(&geojson::Geometry::from(&cq19.geometry)).unwrap();
    let state = |id: &str| match &sources
        .features
        .iter()
        .find(|feature| feature.identity == identity("states", id))
        .unwrap()
        .geometry
    {
        Geometry::MultiPolygon(geometry) => geometry.clone(),
        _ => panic!("state must be multipart"),
    };
    let raw_states = state("53").union(&state("41"));
    assert!(
        raw_states
            .relate(&Rect::new((-124., 45.), (-122., 46.)).to_polygon())
            .is_covers(),
        "original Census administrative land/water geometry independently supports complete CN85 coverage"
    );
    let catalog: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../clacks-model/data/lotw-11.34.json"
    )))
    .unwrap();
    for code in [
        "291", "6", "110", "223", "279", "236", "9", "103", "166", "202", "285",
    ] {
        assert!(
            catalog["entity_zones"]
                .as_object()
                .unwrap()
                .contains_key(code),
            "authored entity code must exist in the pinned native catalog"
        );
    }
    let license = recipe.notices.get("notices/ha8tks.txt").unwrap();
    let fixtures = [
        (path.parent().unwrap().join("itu-2.json"), replacement_bytes),
        (
            path.parent().unwrap().join("athos-review-area.json"),
            athos_bytes,
        ),
        (
            Path::new("tests/fixtures/cq-19-original.json").to_owned(),
            cq19_bytes,
        ),
        (
            Path::new("tests/fixtures/HA8TKS-LICENSE.txt").to_owned(),
            license.as_bytes().to_vec(),
        ),
    ];
    for (path, bytes) in fixtures {
        if std::env::var_os("CLACKS_REFRESH_GEOGRAPHY_FIXTURES").is_some() {
            std::fs::write(path, bytes).unwrap();
        } else {
            assert_eq!(std::fs::read(path).unwrap(), bytes);
        }
    }
}
