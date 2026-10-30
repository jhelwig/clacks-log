//! Caret-local completion using the same field and recognition catalogs as parsing.

use super::*;
use clacks_model::query::{
    completion::{self, CompletionContext},
    field::{self, Field, FieldType},
};
use clacks_sync::record::{RecordState, RecordValue};

/// One insertion and the explanation shown next to it.
struct Suggestion {
    /// Complete replacement for the current token component.
    value: String,
    /// Concise description without fetched contact data.
    description: String,
}

impl HistoryQueryUi {
    /// Complete only the active token, preserving surrounding expression text.
    pub(super) fn suggestions(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        snapshot: &SettingsSnapshot,
        definitions: &DefinitionSet,
        now: f64,
    ) {
        if self.dismissed {
            return;
        }
        let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), response.id) else {
            return;
        };
        let caret = state
            .cursor
            .char_range()
            .map_or(self.state.edited.chars().count(), |r| {
                r.primary.index.into()
            });
        let byte = self
            .state
            .edited
            .char_indices()
            .nth(caret)
            .map_or(self.state.edited.len(), |(i, _)| i);
        let site = completion::site(&self.state.edited, byte);
        let choices = choices(&site, snapshot, definitions);
        if choices.is_empty() {
            return;
        }
        self.choice = self.choice.min(choices.len().saturating_sub(1));
        let mut accept = None;
        ui.input_mut(|input| {
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                self.dismissed = true;
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                self.choice = (self.choice + 1) % choices.len();
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                self.choice = self
                    .choice
                    .checked_sub(1)
                    .unwrap_or(choices.len().saturating_sub(1));
            }
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::Tab)
            {
                accept = Some(self.choice);
            }
        });
        egui::Popup::from_response(response)
            .open(!self.dismissed)
            .width(response.rect.width())
            .show(|ui| {
                for (index, choice) in choices.iter().enumerate() {
                    if ui
                        .selectable_label(
                            index == self.choice,
                            format!("{}  — {}", choice.value, choice.description),
                        )
                        .clicked()
                    {
                        accept = Some(index);
                    }
                }
            });
        if let Some(choice) = accept.and_then(|i| choices.get(i)) {
            self.state
                .edited
                .replace_range(site.replacement.clone(), &choice.value);
            let caret = self
                .state
                .edited
                .get(..site.replacement.start + choice.value.len())
                .map_or(0, |s| s.chars().count());
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(
                    egui::text::CCursor::new(caret),
                )));
            state.store(ui.ctx(), response.id);
            response.request_focus();
            self.edited(now);
            self.dismissed = true;
        }
    }
}

/// Build bounded type-valid choices; no provider request or contact-value harvesting.
fn choices(
    site: &completion::CompletionSite,
    snapshot: &SettingsSnapshot,
    definitions: &DefinitionSet,
) -> Vec<Suggestion> {
    let mut choices = Vec::new();
    match site.context {
        CompletionContext::Field | CompletionContext::Presence => {
            for field in field::definitions() {
                choices.push(Suggestion {
                    value: format!(
                        "{}{}",
                        field.name,
                        if site.context == CompletionContext::Field {
                            ":"
                        } else {
                            ""
                        }
                    ),
                    description: field.description.into(),
                });
            }
            if site.context == CompletionContext::Field {
                choices.push(Suggestion {
                    value: "has:".into(),
                    description: "Find a present value; negate to find missing values".into(),
                });
            }
        }
        CompletionContext::Value(field) => {
            let Ok(catalog) = RecognitionCatalog::adif_318() else {
                return choices;
            };
            let mut add = |value: String, description: &str| {
                choices.push(Suggestion {
                    value,
                    description: description.into(),
                })
            };
            match field {
                Field::Mode => {
                    for row in &catalog.modes {
                        add(
                            row.code.clone(),
                            "Mode family, including recognized submodes",
                        );
                    }
                    for row in &catalog.submodes {
                        add(row.code.clone(), &format!("{} specialization", row.parent));
                    }
                }
                Field::Band | Field::ReceiveBand | Field::RecordedReceiveBand => {
                    for row in &catalog.bands {
                        add(row.code.clone(), "Whole band label");
                    }
                }
                Field::Tag => {
                    for row in &snapshot.logbooks.tags {
                        if let Some(RecordState::Live(RecordValue::Tag(tag))) = row.working_state()
                        {
                            add(quote(&tag.name), "All logbooks with this name");
                        }
                    }
                }
                Field::OwnActivity
                | Field::ContactActivity
                | Field::OwnReference
                | Field::ContactReference => {
                    for group in definitions.iter() {
                        add(
                            format!(
                                "{}{}",
                                quote(group.name()),
                                if field.kind() == FieldType::Reference {
                                    "/"
                                } else {
                                    ""
                                }
                            ),
                            if field.kind() == FieldType::Reference {
                                "Continue with a reference identifier"
                            } else {
                                "Recorded participation in this group"
                            },
                        );
                    }
                }
                _ => match field.kind() {
                    FieldType::Boolean => {
                        add("true".into(), "Current qualifying positive evidence");
                        add("false".into(), "No current qualifying positive evidence");
                    }
                    FieldType::Frequency | FieldType::Power | FieldType::Time => {
                        let example = match field.kind() {
                            FieldType::Frequency => "14MHz",
                            FieldType::Power => "100W",
                            _ => &time::OffsetDateTime::now_utc().date().to_string(),
                        };
                        for operator in ["", "=", ">", ">=", "<", "<="] {
                            add(
                                format!("{operator}{example}"),
                                "Comparison prefix; replace the example value",
                            );
                        }
                    }
                    _ => {}
                },
            }
        }
    }
    let prefix = site.prefix.to_lowercase();
    choices.retain(|c| {
        c.value
            .trim_start_matches('"')
            .to_lowercase()
            .starts_with(prefix.trim_start_matches('"'))
    });
    choices.sort_by(|a, b| a.value.cmp(&b.value));
    choices.dedup_by(|a, b| a.value == b.value);
    choices.truncate(8);
    choices
}

/// Quote catalog names as literal token values, preserving slashes in group names.
fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
