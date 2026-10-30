//! Loading and band interpretation of pinned LoTW configuration data.

use super::*;

impl LotwConfiguration {
    /// Load a supplied configuration and reject unsupported mappings/ranges.
    #[tracing::instrument(skip_all, name = "model.lotw_configuration.load", level = "debug")]
    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        let c: Self = serde_json::from_str(json)?;
        if c.release.is_empty()
            || c.bands.is_empty()
            || c.mapping.is_empty()
            || c.mapping
                .keys()
                .map(|k| k.to_uppercase())
                .collect::<BTreeSet<_>>()
                .len()
                != c.mapping.len()
            || c.mapping
                .iter()
                .any(|(k, v)| k.trim().is_empty() || !c.modes.contains_key(v))
            || c.bands
                .iter()
                .any(|b| b.label.is_empty() || b.lower_hz == 0 || b.upper_hz < b.lower_hz)
        {
            return Err(ServiceError::Invalid {
                detail: "invalid LoTW normalization configuration".into(),
            }
            .into());
        }
        Ok(c)
    }

    /// Load the offline pinned compatibility configuration, returning malformed data errors.
    pub fn bundled() -> anyhow::Result<Self> {
        Self::from_json(include_str!("../../../data/lotw-11.34.json"))
    }

    /// Classify a supplied pair using service definitions, keeping band-only pairs frequency-free.
    pub fn band(&self, pair: &BandFrequency) -> anyhow::Result<String> {
        let found = if let Some(hz) = pair.frequency() {
            self.bands
                .iter()
                .find(|b| (b.lower_hz..=b.upper_hz).contains(&hz.get()))
        } else {
            pair.band().and_then(|b| {
                self.bands
                    .iter()
                    .find(|v| v.label.eq_ignore_ascii_case(b.label()))
            })
        };
        found.map(|b| b.label.clone()).ok_or_else(|| {
            tracing::debug!(
                event = "projection.conversion_unavailable",
                reason = "unmapped_band"
            );
            ServiceError::Invalid {
                detail: format!("LoTW band unavailable for {pair:?}"),
            }
            .into()
        })
    }
}
