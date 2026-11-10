//! Maintained shipping recipe authoring from verified publisher originals.

mod egypt;
mod labels;
mod mappings;
mod operations;
mod specials;
mod world;
mod zones;

use crate::{
    recipe::{SourcePin, SourceSelection, read_recipe, validate_recipe},
    source::decode_sources,
};
use anyhow::{Context, Result, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

/// Reviewed onshore source pin and exact land-feature selection.
#[derive(serde::Deserialize)]
struct OnshoreSource {
    /// Pinned original acquisition and interpretation.
    pin: SourcePin,
    /// Explicit source-defined land identities.
    selection: SourceSelection,
}

/// Author the shipping recipe and guarded correction files in the selected references tree.
/// Source acquisition stays in fetch; this operation reads only verified cached bytes.
pub(crate) fn author(references: &Path, cache: &Path, cancelled: &AtomicBool) -> Result<PathBuf> {
    let geography = references.join("geography");
    let inputs = geography.join("shipping");
    let path = geography.join("shipping.json");
    let mut recipe = read_recipe(&geography.join("representative.json"))?;
    recipe.scope = "Selected Census U.S. states/territories and county equivalents, retaining eight actual Connecticut counties; explicit radio correspondence and worldwide omissions.".into();
    let all_states = "01 02 04 05 06 08 09 10 11 12 13 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41 42 44 45 46 47 48 49 50 51 53 54 55 56 60 66 69 72 78";
    for selection in &mut recipe.selections {
        match selection.source_id.as_str() {
            "states" => {
                selection.properties.clear();
                selection.expected_count = 56;
            }
            "counties" => {
                selection.properties = BTreeMap::from([(
                    "STATEFP".into(),
                    all_states
                        .split_whitespace()
                        .filter(|id| *id != "09")
                        .map(String::from)
                        .collect(),
                )]);
                selection.expected_count = 3226;
            }
            "itu" => {
                selection.properties.clear();
                selection.expected_count = 90;
            }
            "subunits" => {
                selection.properties.clear();
                selection.expected_count = 360;
            }
            "minor-islands" => {
                selection.feature_sha256 = None;
                selection.expected_count = 2795;
            }
            _ => (),
        }
    }
    validate_recipe(&recipe)?;
    let sources = decode_sources(&recipe, cache, cancelled)?;
    let county_ids: Vec<_> = sources
        .features
        .iter()
        .filter(|f| {
            f.identity.source_id == "counties"
                && (f.identity.feature_id.starts_with("72")
                    || f.identity.feature_id.starts_with("78"))
        })
        .map(|f| f.identity.feature_id.clone())
        .collect();
    for pins_file in ["caribbean-pins.json", "aleutian-pins.json"] {
        let pins_path = inputs.join(pins_file);
        let acquired: Vec<OnshoreSource> =
            serde_json::from_slice(&std::fs::read(&pins_path).with_context(|| {
                format!("required onshore source inventory {}", pins_path.display())
            })?)?;
        let expected: BTreeSet<_> = if pins_file == "caribbean-pins.json" {
            county_ids
                .iter()
                .map(|county| format!("caribbean-faces-{county}"))
                .collect()
        } else {
            BTreeSet::from(["aleutian-faces-02013".into()])
        };
        let actual: BTreeSet<_> = acquired
            .iter()
            .map(|source| source.pin.id.clone())
            .collect();
        ensure!(
            actual == expected && actual.len() == acquired.len(),
            "onshore inventory does not match required original county identities"
        );
        recipe
            .sources
            .retain(|source| !["san-juan-faces", "st-thomas-faces"].contains(&source.id.as_str()));
        recipe.selections.retain(|selection| {
            !["san-juan-faces", "st-thomas-faces"].contains(&selection.source_id.as_str())
        });
        for OnshoreSource { pin, selection } in acquired {
            recipe.sources.push(pin);
            recipe.selections.push(selection);
        }
    }
    mappings::author(&mut recipe, &sources, references)?;
    zones::author(&mut recipe, &sources, references)?;
    world::author(&mut recipe, &sources, references)?;
    egypt::author(&mut recipe, &sources, references)?;
    operations::author(&mut recipe, &sources)?;
    specials::author(&mut recipe, &sources)?;
    labels::author(&mut recipe, references)?;
    for case_file in ["cases.json", "polar-cases.json"] {
        let additional_cases = std::fs::read(inputs.join(case_file))?;
        let additional_cases: Vec<crate::recipe::AcceptanceCase> =
            serde_json::from_slice(&additional_cases)?;
        recipe.acceptance_cases.extend(additional_cases);
    }
    for case in &mut recipe.acceptance_cases {
        if case.id == "athens" || case.id == "athos-unknown" {
            case.expected = std::collections::BTreeMap::from([(
                clacks_geography::artifact::Field::Dxcc,
                crate::recipe::ExpectedField {
                    coverage: crate::recipe::ExpectedCoverage::None,
                    candidates: vec![],
                },
            )]);
            case.citations = vec!["Greece Natural Earth subunit remains unclassified because ordinary Greek territory includes multiple DXCC entities (mainland, Crete, Dodecanese, Mount Athos), without reviewed full component correspondence in this shipping selection.".into()];
        }
    }
    validate_recipe(&recipe)?;
    std::fs::write(&path, serde_json::to_vec_pretty(&recipe)?)?;
    Ok(path)
}
