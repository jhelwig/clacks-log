//! Searchable assignment choices shared by contact editors.

#[cfg(test)]
mod tests;

use super::{logbooks::labels, theme};
use clacks_client_store::logbooks::LogbookSnapshot;
use clacks_model::ids::TagId;
use clacks_sync::record::{RecordState, RecordValue};
use eframe::egui;
use std::collections::BTreeSet;

/// Membership across the captured targets.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MembershipMark {
    /// Every target has this tag.
    All,
    /// Only some targets have this tag; preserve their individual values.
    Some,
    /// No targets have this tag.
    None,
}

/// One stable catalog identity and pending assignment intent.
struct TagChoice {
    /// Native identity, never derived from its name.
    id: TagId,
    /// Disambiguated catalog caption or unavailable-identity caption.
    label: String,
    /// Membership when the picker opened.
    initial: MembershipMark,
    /// Pending membership intent.
    current: MembershipMark,
    /// Only live tags may be newly assigned.
    available: bool,
}

/// Explicit changes, leaving all other per-contact memberships untouched.
pub(super) struct MembershipIntent {
    /// Include these tags on every target.
    pub add: BTreeSet<TagId>,
    /// Remove these tags from every target.
    pub remove: BTreeSet<TagId>,
}

/// Pending searchable choices; the containing workflow owns persistence.
pub(super) struct MembershipPicker {
    /// Complete choices, independent of the current search.
    choices: Vec<TagChoice>,
    /// Literal local catalog search.
    search: String,
}

impl MembershipPicker {
    /// Copy membership counts and captions without retaining a database reader.
    pub fn new(catalog: &LogbookSnapshot, memberships: &[BTreeSet<TagId>]) -> Self {
        let tags: Vec<_> = catalog
            .tags
            .iter()
            .filter_map(|row| match row.working_state() {
                Some(RecordState::Live(RecordValue::Tag(tag))) => Some(tag.as_ref()),
                _ => None,
            })
            .collect();
        let captions = labels::labels(&tags);
        let mut ids: BTreeSet<_> = captions.keys().copied().collect();
        ids.extend(memberships.iter().flat_map(|set| set.iter().copied()));
        let mut choices: Vec<_> = ids
            .into_iter()
            .map(|id| {
                let count = memberships.iter().filter(|set| set.contains(&id)).count();
                let initial = if count == 0 {
                    MembershipMark::None
                } else if count == memberships.len() {
                    MembershipMark::All
                } else {
                    MembershipMark::Some
                };
                TagChoice {
                    id,
                    available: captions.contains_key(&id),
                    label: captions
                        .get(&id)
                        .cloned()
                        .unwrap_or_else(|| format!("Unavailable logbook · {id}")),
                    initial,
                    current: initial,
                }
            })
            .collect();
        let names: std::collections::BTreeMap<_, _> =
            tags.iter().map(|tag| (tag.id, tag.name.as_str())).collect();
        choices.sort_by(|a, b| {
            let a_name = names.get(&a.id).copied().unwrap_or(&a.label);
            let b_name = names.get(&b.id).copied().unwrap_or(&b.label);
            a_name.cmp(b_name).then(a.id.cmp(&b.id))
        });
        Self {
            choices,
            search: String::new(),
        }
    }

    /// Return only explicit changes from the opening membership basis.
    pub fn intent(&self) -> MembershipIntent {
        let mut intent = MembershipIntent {
            add: BTreeSet::new(),
            remove: BTreeSet::new(),
        };
        for choice in &self.choices {
            if choice.current == choice.initial {
                continue;
            }
            match choice.current {
                MembershipMark::All if choice.available => {
                    intent.add.insert(choice.id);
                }
                MembershipMark::None => {
                    intent.remove.insert(choice.id);
                }
                _ => {}
            }
        }
        intent
    }

    /// Whether Apply would change any captured contact membership.
    pub fn is_changed(&self) -> bool {
        self.choices
            .iter()
            .any(|choice| choice.initial != choice.current)
    }

    /// Summarize a bounded number of assigned names without growing the form.
    pub fn summary(&self) -> String {
        let assigned: Vec<_> = self
            .choices
            .iter()
            .filter(|choice| choice.current != MembershipMark::None)
            .collect();
        if assigned.is_empty() {
            return "None".into();
        }
        let mut text = assigned
            .iter()
            .take(3)
            .map(|choice| choice.label.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if assigned.len() > 3 {
            text.push_str(&format!(" +{}", assigned.len() - 3));
        }
        text
    }
}

/// Render bounded choices without choosing a Save/Apply or popup lifecycle.
pub(super) fn show_contents(ui: &mut egui::Ui, picker: &mut MembershipPicker) -> bool {
    ui.set_width(
        360.0_f32
            .min(ui.ctx().content_rect().width() - 32.0)
            .max(160.0),
    );
    let label = ui.small("Search logbooks").id;
    let search = ui.add(
        egui::TextEdit::singleline(&mut picker.search)
            .hint_text("Type a logbook name")
            .desired_width(f32::INFINITY),
    );
    search.labelled_by(label);
    let query = picker.search.to_lowercase();
    let colors = theme::palette(ui.ctx().theme());
    let mut changed = false;
    egui::ScrollArea::vertical()
        .max_height(240.0)
        .show(ui, |ui| {
            for choice in &mut picker.choices {
                if !choice.label.to_lowercase().contains(&query) {
                    continue;
                }
                let mut checked = choice.current == MembershipMark::All;
                let response = ui
                    .push_id(choice.id.to_string(), |ui| {
                        ui.add(
                            egui::Checkbox::new(&mut checked, &choice.label)
                                .indeterminate(choice.current == MembershipMark::Some),
                        )
                    })
                    .inner;
                if response.clicked() {
                    choice.current = if choice.initial == MembershipMark::Some && choice.available {
                        match choice.current {
                            MembershipMark::Some => MembershipMark::All,
                            MembershipMark::All => MembershipMark::None,
                            MembershipMark::None => MembershipMark::Some,
                        }
                    } else if !choice.available {
                        if choice.current == MembershipMark::None {
                            choice.initial
                        } else {
                            MembershipMark::None
                        }
                    } else if choice.current == MembershipMark::All {
                        MembershipMark::None
                    } else {
                        MembershipMark::All
                    };
                    changed = true;
                }
                if choice.current != choice.initial {
                    ui.small(egui::RichText::new("Not applied").color(colors.edited_edge));
                }
            }
        });
    changed
}
