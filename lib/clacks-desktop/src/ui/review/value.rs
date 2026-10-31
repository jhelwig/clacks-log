//! Read-only native values shared by comparisons and pending-record inspection.

use crate::review::{fields::FieldId, input::FieldInput};
use clacks_client_store::settings::SettingsSnapshot;
use clacks_model::{group::DefinitionSet, ids::TagId};
use clacks_sync::record::RecordState;
use eframe::egui;
use std::collections::BTreeMap;

/// Display one native field with catalog labels, without introducing editable state.
pub(super) fn show(
    ui: &mut egui::Ui,
    field: FieldId,
    input: &FieldInput,
    tags: &BTreeMap<TagId, String>,
    definitions: &DefinitionSet,
    settings: &SettingsSnapshot,
) {
    match input {
        FieldInput::Membership { tags: values } if values.is_empty() => {
            ui.label("Untagged");
        }
        FieldInput::Membership { tags: values } => {
            ui.label(
                values
                    .iter()
                    .map(|id| {
                        tags.get(id)
                            .cloned()
                            .unwrap_or_else(|| format!("Unavailable logbook · {id}"))
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        FieldInput::References { rows } if rows.is_empty() => {
            ui.label("None");
        }
        FieldInput::References { rows } => {
            ui.label(
                rows.iter()
                    .map(|row| {
                        format!(
                            "{}{}",
                            row.group
                                .map(|id| definitions
                                    .get(id)
                                    .map(|group| group.name().to_owned())
                                    .unwrap_or_else(|| format!("Unrecognized group · {id}")))
                                .unwrap_or_else(|| "No group selected".into()),
                            if row.identifier.is_empty() {
                                String::new()
                            } else {
                                format!(": {}", row.identifier)
                            }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        FieldInput::Equipment { selected } => {
            let caption = selected
                .map(|id| {
                    settings
                        .equipment
                        .iter()
                        .find_map(|row| match row.working_state() {
                            Some(RecordState::Live(
                                clacks_sync::record::RecordValue::Equipment(value),
                            )) if value.id() == id => Some(format!("{} · {id}", value.name())),
                            _ => None,
                        })
                        .unwrap_or_else(|| format!("Unavailable equipment · {id}"))
                })
                .unwrap_or_else(|| "No default equipment".into());
            ui.label(caption);
        }
        input => {
            let text = egui::RichText::new(input.display());
            ui.label(if field.radio_data() {
                text.monospace()
            } else {
                text
            });
        }
    }
}
