//! Transient editor input; native source selection does not pass through it.

use crate::operating::{position::PositionForm, station::ReferenceForm};
use clacks_model::ids::TagId;
use std::collections::BTreeSet;

/// Concrete controls reused by operator-owned record editors.
#[derive(Clone, Debug)]
pub(crate) enum FieldInput {
    /// Optional or required text, interpreted by its owning native field.
    Text {
        /// Exact unfinished user input.
        value: String,
    },
    /// Coupled grid/coordinate editor preserving the chosen evidence form.
    Position {
        /// Existing station-position editor state.
        form: PositionForm,
    },
    /// Frequency and band with the existing edit-authority rules.
    Radio {
        /// Existing whole-hertz radio editor state.
        form: crate::logging::radio::RadioForm,
    },
    /// Recorded mode label and optional explicit parent.
    Mode {
        /// Mode or submode label.
        label: String,
        /// Parent for an unfamiliar specialization, if supplied.
        parent: String,
    },
    /// One station's data-driven group associations.
    References {
        /// Existing editable reference rows, including their preservation metadata.
        rows: Vec<ReferenceForm>,
    },
    /// Selected native optional equipment reference.
    Equipment {
        /// Default configuration, absent for no default.
        selected: Option<clacks_model::ids::EquipmentConfigurationId>,
    },
    /// Independent service upload policy.
    Policy {
        /// Explicit contribution of the tag.
        value: clacks_model::tag::AutomaticUploadPolicy,
    },
    /// Direct resolved membership; never a mixed selection of contacts.
    Membership {
        /// Complete set that will be saved.
        tags: BTreeSet<TagId>,
    },
}

impl FieldInput {
    /// Describe a compared value, without using that text for native source copying.
    pub fn display(&self) -> String {
        match self {
            Self::Text { value } if value.is_empty() => "Empty".into(),
            Self::Text { value } => value.clone(),
            Self::Position { form } => match form.kind {
                crate::operating::position::PositionKind::Grid => {
                    if form.grid.is_empty() {
                        "Unknown".into()
                    } else {
                        format!("{} · grid area", form.grid)
                    }
                }
                crate::operating::position::PositionKind::Coordinates => format!(
                    "{}, {} · explicit coordinates",
                    form.latitude, form.longitude
                ),
            },
            Self::Radio { form } => match (form.frequency.is_empty(), form.band.is_empty()) {
                (true, true) => "No receive override".into(),
                (true, false) => form.band.clone(),
                (false, true) => format!("{} MHz", form.frequency),
                (false, false) => format!("{} MHz · {}", form.frequency, form.band),
            },
            Self::Mode { label, parent } => {
                if parent.is_empty() {
                    label.clone()
                } else {
                    format!("{label} ({parent})")
                }
            }
            Self::References { rows } => {
                if rows.is_empty() {
                    "None".into()
                } else {
                    rows.iter()
                        .map(|row| {
                            format!(
                                "{}{}",
                                row.group.map(|id| id.to_string()).unwrap_or_default(),
                                if row.identifier.is_empty() {
                                    String::new()
                                } else {
                                    format!(": {}", row.identifier)
                                }
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            }
            Self::Equipment { selected } => selected
                .map(|id| id.to_string())
                .unwrap_or_else(|| "No default".into()),
            Self::Policy { value } => format!("{value:?}"),
            Self::Membership { tags } => {
                if tags.is_empty() {
                    "Untagged".into()
                } else {
                    tags.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            }
        }
    }
}
