//! Look up the POTA equivalents of Japanese prefecture codes.

use qso_io::adif::primary_subdivisions::japan::PrimarySubdivision;
use serde::Deserialize;
use std::sync::OnceLock;

/// Equivalent subdivision identifiers for one prefecture under DXCC entity 339.
#[derive(Deserialize)]
struct Prefecture {
    /// ADIF primary subdivision code.
    adif: String,
    /// Suffix of its ISO 3166-2 code.
    iso_suffix: String,
    /// Suffix of the location code used by the POTA uploader.
    pota_suffix: String,
}

/// Offline cross-reference for Japanese prefectures under DXCC entity 339.
#[derive(Deserialize)]
pub(super) struct Prefectures {
    prefectures: Vec<Prefecture>,
}

impl Prefectures {
    /// Load the same bundled cross-reference for all POTA export preparations.
    pub(super) fn bundled() -> anyhow::Result<&'static Self> {
        static BUNDLED: OnceLock<Result<Prefectures, serde_json::Error>> = OnceLock::new();
        BUNDLED
            .get_or_init(|| {
                serde_json::from_str(include_str!("../../../data/pota-japan-prefectures.json"))
            })
            .as_ref()
            .map_err(|error| {
                anyhow::anyhow!("Could not read the bundled POTA prefecture data: {error}")
            })
    }

    /// Resolve a bare ADIF or POTA prefecture code recorded for entity 339.
    pub(super) fn primary_subdivision(&self, code: &str) -> Option<&str> {
        let adif = code.parse::<PrimarySubdivision>().ok().map(<&str>::from);
        self.prefectures
            .iter()
            .find(|item| {
                Some(item.adif.as_str()) == adif || item.pota_suffix.eq_ignore_ascii_case(code)
            })
            .map(|item| item.pota_suffix.as_str())
    }

    /// Resolve an ISO or POTA suffix from a qualified location within entity 339.
    pub(super) fn qualified_subdivision(&self, code: &str) -> Option<&str> {
        self.prefectures
            .iter()
            .find(|item| item.iso_suffix == code || item.pota_suffix.eq_ignore_ascii_case(code))
            .map(|item| item.pota_suffix.as_str())
    }
}
