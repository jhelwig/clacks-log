//! Expand the accepted operation partition using complete source applicability.

use crate::{
    recipe::{Assignment, BuildRecipe},
    source::DecodedSources,
};
use anyhow::Result;

/// Use complete Census mainland footprints and all Caribbean onshore FACES.
pub fn author(recipe: &mut BuildRecipe, sources: &DecodedSources) -> Result<()> {
    let caribbean: Vec<_> = recipe
        .sources
        .iter()
        .filter(|source| source.id.starts_with("caribbean-faces-"))
        .map(|source| source.id.clone())
        .collect();
    for operation in &mut recipe.operations {
        match (operation.id.as_str(), &mut operation.assignment) {
            ("aleutians-1251", Assignment::SourceMask { source_ids, .. }) => {
                *source_ids = recipe
                    .sources
                    .iter()
                    .filter(|source| source.id.starts_with("aleutian-faces"))
                    .map(|source| source.id.clone())
                    .collect();
                operation.description = "EPSG:1251; complete selected Aleutians West and Aleutians East county land FACES within published onshore applicability; stated accuracy 8 m.".into();
            }
            ("mainland-1188", Assignment::SourceMask { identities, .. }) => {
                *identities = sources
                    .features
                    .iter()
                    .filter(|feature| {
                        feature.identity.source_id == "states"
                            && !["02", "15", "60", "66", "69", "72", "78"]
                                .contains(&feature.identity.feature_id.as_str())
                    })
                    .map(|feature| feature.identity.clone())
                    .collect();
                operation.description = "EPSG:1188; zero-height geocentric translation; complete selected contiguous mainland state/DC footprints and Alaska mainland component within published applicability; stated accuracy 4 m.".into();
            }
            ("caribbean-1731", Assignment::SourceMask { source_ids, .. })
                if !caribbean.is_empty() =>
            {
                *source_ids = caribbean.clone();
                operation.description = "EPSG:1731; all 78 Puerto Rico municipalities and three U.S. Virgin Islands county-equivalent land FACES, intersected with published extent. HARN target approximated as WGS84; stated accuracy 2 m. Offshore/inland-water residuals retain disclosed D259 no-shift assignment; exact operation seams retained.".into();
            }
            ("remaining-d259", _) => {
                operation.description = "D259 no-shift approximation for Pacific/outlying/offshore/inland-water source components outside prescribed regional masks. Unestablished accuracy; not an automatic fallback after a failed operation.".into();
            }
            _ => (),
        }
    }
    recipe
        .limitations
        .retain(|limit| !limit.starts_with("Representative Caribbean"));
    recipe.limitations.push("Census administrative geometry includes waters, not only coastlines. Caribbean regional grid assignments use all selected land FACES within the published operation extent; residual waters use disclosed D259 no-shift. Exact mixed-operation seams remain measurable and are not silently snapped.".into());
    Ok(())
}
