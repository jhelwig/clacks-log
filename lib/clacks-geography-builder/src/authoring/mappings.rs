//! Factual administrative correspondence, independent of compiled query output.

use crate::{
    recipe::{BuildRecipe, ValueMapping},
    source::{DecodedSources, SourceFeature},
};
use anyhow::{Context, Result};
use clacks_geography::artifact::{Field, ParentScope};

/// Obtain an original publisher string attribute without fuzzy matching.
fn text<'a>(feature: &'a SourceFeature, key: &str) -> Result<&'a str> {
    feature
        .properties
        .get(key)
        .and_then(serde_json::Value::as_str)
        .with_context(|| format!("{} lacks {key}", feature.identity.feature_id))
}

/// Census state/territory to published DXCC entity correspondence.
fn entity(state: &str) -> &'static str {
    match state {
        "AK" => "6",
        "HI" => "110",
        "AS" => "9",
        "GU" => "103",
        "MP" => "166",
        "PR" => "202",
        "VI" => "285",
        _ => "291",
    }
}

/// CQ's published whole-state definitions, including call-area terminology.
fn cq(state: &str) -> &'static str {
    match state {
        "AK" => "1",
        "HI" => "31",
        "AS" => "32",
        "GU" | "MP" => "27",
        "PR" | "VI" => "8",
        "CA" | "AZ" | "ID" | "NV" | "OR" | "UT" | "WA" => "3",
        "CT" | "ME" | "MA" | "NH" | "RI" | "VT" | "NJ" | "NY" | "DE" | "DC" | "MD" | "PA"
        | "FL" | "GA" | "SC" | "NC" | "VA" | "WV" => "5",
        _ => "4",
    }
}

/// Replace representative administrative rows with explicit complete selections.
pub fn author(
    recipe: &mut BuildRecipe,
    sources: &DecodedSources,
    references: &std::path::Path,
) -> Result<()> {
    let catalog: serde_json::Value =
        serde_json::from_str(include_str!("../../../clacks-model/data/lotw-11.34.json"))?;
    let release = catalog
        .get("release")
        .and_then(serde_json::Value::as_str)
        .context("native catalog release")?;
    let upstream = catalog
        .get("upstream_commit")
        .and_then(serde_json::Value::as_str)
        .context("native catalog upstream revision")?;
    let catalog_citation = format!(
        "Native lib/clacks-model/data/lotw-11.34.json, configuration {release}, extracted from TrustedQSL 2.8.6 commit {upstream}; US_COUNTY record values use exact case-insensitive type/name label correspondence where present, otherwise current Census NAME. Upstream: https://sourceforge.net/p/trustedqsl/tqsl/ci/{upstream}/tree/ ; bundled notice: notices/trustedqsl-catalog.txt"
    );
    recipe.notices.insert(
        "notices/trustedqsl-catalog.txt".into(),
        std::fs::read_to_string(references.join("trustedqsl/upstream/LICENSE.txt"))?,
    );
    let county_values = catalog
        .get("fields")
        .and_then(|fields| fields.get("US_COUNTY"))
        .and_then(|field| field.get("values"))
        .and_then(serde_json::Value::as_array)
        .context("native county catalog")?;
    let states: Vec<_> = sources
        .features
        .iter()
        .filter(|f| f.identity.source_id == "states")
        .collect();
    recipe.mappings.retain(|m| {
        !matches!(
            m.field,
            Field::PrimarySubdivision | Field::SecondarySubdivision
        ) && !m.id.starts_with("state-cq-")
            && !["dxcc-USB", "dxcc-USH", "dxcc-USK"].contains(&m.id.as_str())
    });
    for state in &states {
        let abbreviation = text(state, "STUSPS")?;
        let name = text(state, "NAME")?;
        let dxcc = entity(abbreviation);
        recipe.mappings.extend([
            ValueMapping {
                id: format!("state-{abbreviation}"),
                field: Field::PrimarySubdivision,
                parent_scope: ParentScope::Entity(dxcc.into()),
                recordable_value: abbreviation.into(),
                display_label: name.into(),
                source_components: vec![],
                source_feature_selectors: vec![state.identity.clone()],
                citations: vec!["https://www.census.gov/geographies/mapping-files/time-series/geo/tiger-line-file.html".into()],
            },
            ValueMapping {
                id: format!("state-dxcc-{abbreviation}"),
                field: Field::Dxcc,
                parent_scope: ParentScope::Global,
                recordable_value: dxcc.into(),
                display_label: name.into(),
                source_components: vec![],
                source_feature_selectors: vec![state.identity.clone()],
                citations: vec!["https://www.arrl.org/country-lists-prefixes".into()],
            },
            ValueMapping {
                id: format!("state-cq-{abbreviation}"),
                field: Field::CqZone,
                parent_scope: ParentScope::Global,
                recordable_value: cq(abbreviation).into(),
                display_label: cq(abbreviation).into(),
                source_components: vec![],
                source_feature_selectors: vec![state.identity.clone()],
                citations: vec!["https://www.cqww.com/cq_waz_list.htm".into()],
            },
        ]);
    }
    for county in sources
        .features
        .iter()
        .filter(|f| ["counties", "ct-counties"].contains(&f.identity.source_id.as_str()))
    {
        let statefp = text(county, "STATEFP")?;
        let state = states
            .iter()
            .find(|s| s.identity.feature_id == statefp)
            .context("county parent state")?;
        let abbreviation = text(state, "STUSPS")?;
        let name = text(county, "NAME")?;
        let label = text(county, "NAMELSAD")?;
        let candidates: Vec<_> = county_values
            .iter()
            .filter(|value| {
                value.get("dependency").and_then(serde_json::Value::as_str) == Some(abbreviation)
            })
            .collect();
        let native = candidates
            .iter()
            .find(|value| {
                value
                    .get("label")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|native| native.eq_ignore_ascii_case(label))
            })
            .or_else(|| {
                candidates.iter().find(|value| {
                    value
                        .get("label")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|native| native.eq_ignore_ascii_case(name))
                })
            });
        let recordable = native
            .and_then(|value| value.get("value"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(name);
        recipe.mappings.push(ValueMapping {
            id: format!("county-{}", county.identity.feature_id),
            field: Field::SecondarySubdivision,
            parent_scope: ParentScope::Subdivision {
                entity: entity(abbreviation).into(),
                primary: abbreviation.into(),
            },
            recordable_value: format!("{abbreviation},{recordable}"),
            display_label: label.into(),
            source_components: vec![],
            source_feature_selectors: vec![county.identity.clone()],
            citations: vec![
                "https://www.census.gov/geographies/mapping-files/time-series/geo/tiger-line-file.html".into(),
                catalog_citation.clone(),
            ],
        });
    }
    for exception in &mut recipe.exceptions {
        if exception.id == "detailed-state-cq" {
            exception.exclusion_mapping_ids = recipe
                .mappings
                .iter()
                .filter(|m| m.id.starts_with("state-cq-"))
                .map(|m| m.id.clone())
                .collect();
        }
    }
    Ok(())
}
