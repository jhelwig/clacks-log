//! Canonical value labels shared by multiple source features.

use crate::recipe::BuildRecipe;
use anyhow::{Context, Result, ensure};
use clacks_geography::artifact::Field;
use std::{collections::BTreeMap, fs};

/// Apply factual shared labels and fail before expensive compilation on conflicts.
pub fn author(recipe: &mut BuildRecipe, references: &std::path::Path) -> Result<()> {
    let text = fs::read_to_string(references.join("geography/shipping/dxcc-labels.tsv"))?;
    let mut labels = BTreeMap::new();
    for row in text
        .lines()
        .filter(|row| !row.is_empty() && !row.starts_with('#'))
    {
        let (value, label) = row.split_once('\t').context("DXCC label row")?;
        ensure!(
            labels.insert(value, label).is_none(),
            "duplicate DXCC label key"
        );
    }
    let mut scoped = BTreeMap::new();
    for mapping in &mut recipe.mappings {
        match mapping.field {
            Field::Dxcc => {
                if let Some(label) = labels.get(mapping.recordable_value.as_str()) {
                    mapping.display_label = (*label).into();
                }
            }
            Field::CqZone => {
                mapping.display_label = format!("CQ zone {}", mapping.recordable_value)
            }
            Field::ItuZone => {
                mapping.display_label = format!("ITU zone {}", mapping.recordable_value)
            }
            _ => (),
        }
        let key = (
            mapping.field,
            mapping.parent_scope.clone(),
            mapping.recordable_value.clone(),
        );
        if let Some(previous) = scoped.insert(key, mapping.display_label.clone()) {
            ensure!(
                previous == mapping.display_label,
                "conflicting labels for mapping {}: {previous:?} versus {:?}",
                mapping.id,
                mapping.display_label
            );
        }
    }
    Ok(())
}
