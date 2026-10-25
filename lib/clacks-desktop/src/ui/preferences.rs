//! Committed workspace-local appearance, stored through eframe persistence.

use crate::operating::FieldError;
use serde::{Deserialize, Serialize};

/// Framework storage key for presentation preferences, independent of account data.
pub(super) const STORAGE_KEY: &str = "clacks.appearance.v1";

/// Appearance policy used for all windows of this client.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub(super) enum Appearance {
    /// Follow the operating system, including changes during operation.
    #[default]
    System,
    /// Use light surfaces regardless of OS appearance.
    Light,
    /// Use dark surfaces regardless of OS appearance.
    Dark,
}

/// Requested typeface for one presentation role.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub(super) enum FontChoice {
    /// Use the bundled default for this role.
    #[default]
    Default,
    /// Use an installed family when available, retaining a fallback otherwise.
    Installed {
        /// Family name understood by the native font catalog.
        family: String,
    },
}

/// Presentation-only preferences; never serialize pending forms or radio facts here.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub(super) struct AppearancePreferences {
    /// OS following or an explicit appearance override.
    pub appearance: Appearance,
    /// Unified zoom on top of native display scaling.
    pub zoom: f32,
    /// Ordinary labels and explanatory text.
    pub ui_font: FontChoice,
    /// Radio identifiers and numerical data, monospace by default.
    pub radio_font: FontChoice,
}

impl Default for AppearancePreferences {
    fn default() -> Self {
        Self {
            appearance: Appearance::System,
            zoom: 1.0,
            ui_font: FontChoice::Default,
            radio_font: FontChoice::Default,
        }
    }
}

impl AppearancePreferences {
    /// Validate the supported UI zoom before previewing or committing it.
    pub fn validate(&self) -> Result<(), Vec<FieldError>> {
        if !self.zoom.is_finite() || !(0.75..=2.0).contains(&self.zoom) {
            return Err(vec![FieldError {
                field: "UI scale".into(),
                message: "Choose a UI scale between 75% and 200%.".into(),
            }]);
        }
        Ok(())
    }
}
