//! Explicit reviewed worldwide entity correspondence and source omissions.

use crate::{
    recipe::{BuildRecipe, ValueMapping},
    source::DecodedSources,
};
use anyhow::{Context, Result, ensure};
use clacks_geography::artifact::{Field, ParentScope, ReviewedOmission};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};

/// Read factual correspondence rather than derive identity from proximity or names.
pub fn author(
    recipe: &mut BuildRecipe,
    sources: &DecodedSources,
    references: &std::path::Path,
) -> Result<()> {
    let catalog: serde_json::Value =
        serde_json::from_str(include_str!("../../../clacks-model/data/lotw-11.34.json"))?;
    let entities = catalog
        .get("entity_zones")
        .and_then(serde_json::Value::as_object)
        .context("native entity catalog")?;
    let table = fs::read_to_string(references.join("geography/shipping/dxcc-correspondence.tsv"))?;
    let mut assignments = BTreeMap::new();
    for line in table
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let (key, value) = line.split_once('\t').context("DXCC correspondence row")?;
        ensure!(
            entities.contains_key(value),
            "unknown native DXCC ID {value}"
        );
        ensure!(
            assignments.insert(key, value).is_none(),
            "duplicate subunit correspondence"
        );
    }
    recipe
        .mappings
        .retain(|mapping| !mapping.id.starts_with("dxcc-"));
    recipe
        .exceptions
        .retain(|exception| exception.id != "athos-unknown-review-area");
    recipe.omissions.clear();
    let specific: Vec<ReviewedOmission> = serde_json::from_slice(&fs::read(
        references.join("geography/shipping/omissions.json"),
    )?)?;
    let specific_ids: BTreeSet<_> = specific
        .iter()
        .flat_map(|omission| omission.source_identities.iter())
        .cloned()
        .collect();
    let mut unclassified = Vec::new();
    let mut replaced = Vec::new();
    for source in sources
        .features
        .iter()
        .filter(|feature| feature.identity.source_id == "subunits")
    {
        let key = source.identity.feature_id.as_str();
        if specific_ids.contains(&source.identity) {
            ensure!(
                !assignments.contains_key(key),
                "subunit is both mapped and explicitly omitted"
            );
            continue;
        }
        if ["USB", "USK", "USH", "ASM", "GUM", "MNP", "PRI", "VIR"].contains(&key) {
            replaced.push(source.identity.clone());
            continue;
        }
        if let Some(value) = assignments.remove(key) {
            let label = source
                .properties
                .get("NAME")
                .and_then(serde_json::Value::as_str)
                .context("subunit label")?;
            recipe.mappings.push(ValueMapping{id:format!("dxcc-{key}"),source_components:vec![],source_feature_selectors:vec![source.identity.clone()],field:Field::Dxcc,parent_scope:ParentScope::Global,recordable_value:value.into(),display_label:label.into(),citations:vec!["https://www.arrl.org/country-lists-prefixes".into(),"references/geography/shipping/dxcc-correspondence.tsv: explicitly authored correspondence; IDs checked against native lotw-11.34 catalog".into()]});
        } else {
            unclassified.push(source.identity.clone());
        }
    }
    ensure!(
        assignments.is_empty(),
        "DXCC correspondence references absent subunit(s): {:?}",
        assignments.keys()
    );
    for (id, identities, reason) in [
        (
            "natural-earth-us-replaced",
            replaced,
            "Natural Earth U.S. administrative subunits are not used for radio classification; detailed Census geometry supplies those fields, including separate Kure, Swains and Desecheo treatment.",
        ),
        (
            "natural-earth-without-reviewed-correspondence",
            unclassified,
            "No whole-feature DXCC correspondence is asserted. Unclassified records include multi-entity geography requiring additional splits (Canada, Greece, Brazil, Australia, Chile, Colombia, Fiji, Kiribati, Malaysia, Mauritius, New Caledonia, Cook Islands, French Polynesia, Russia, Taiwan), disputed/nonentity areas and remaining outlying islands. Geographic omission is preferable to asserting a whole ordinary administrative unit equals one radio entity; the maintained correspondence table identifies every supported selected subunit.",
        ),
    ] {
        recipe.omissions.push(ReviewedOmission{id:id.into(),field:Field::Dxcc,source_identities:identities,reason:reason.into(),citations:vec!["https://www.arrl.org/country-lists-prefixes".into(),"https://www.naturalearthdata.com/downloads/10m-cultural-vectors/10m-admin-0-details/".into()]});
    }
    recipe.omissions.push(ReviewedOmission{id:"unnamed-minor-islands".into(),field:Field::Dxcc,source_identities:vec![],reason:"The entire pinned minor-islands source is omitted. All 2,795 selected minor-island records lack names, administrative identity or a unique publisher key: their attributes are only featurecla, min_zoom and scalerank. None receives an inferred entity from proximity; separate supported island subunits retain their explicit correspondence.".into(),citations:vec!["https://www.naturalearthdata.com/downloads/10m-physical-vectors/10m-minor-islands/".into()]});
    recipe.omissions.extend(specific);
    Ok(())
}
