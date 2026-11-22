//! The projected logbook catalog and its selected in-section editor.

use super::*;

impl AccountDraft {
    /// Render named logbooks against the current catalog, keeping raw edits between sections.
    pub(in crate::ui::settings) fn logbooks(
        &mut self,
        ui: &mut egui::Ui,
        snapshot: &SettingsSnapshot,
        sender: &mpsc::Sender<SettingsRequest>,
    ) {
        for message in &self.deleted_logbooks {
            ui.label(message);
        }
        if ui.button("New logbook").clicked() {
            let id = TagId::new();
            self.tags.insert(
                id,
                TagForm {
                    expected: None,
                    editor: TagEditor::new(id),
                },
            );
            self.selected_tag = Some(id);
        }

        let mut catalog: BTreeMap<_, _> = snapshot
            .logbooks
            .tags
            .iter()
            .filter_map(tag)
            .map(|value| (value.id, value.clone()))
            .collect();
        for (&id, form) in &self.tags {
            catalog.insert(
                id,
                LogbookTag {
                    id,
                    name: form.editor.name.clone(),
                    automatic_upload: form.editor.automatic_upload.clone(),
                },
            );
        }
        let mut records: Vec<_> = catalog.values().collect();
        records.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        let labels = labels::labels(&records);
        for record in records {
            let id = record.id;
            let label = labels
                .get(&id)
                .filter(|name| !name.is_empty())
                .map(String::as_str)
                .unwrap_or("Unnamed logbook");
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.selected_tag, Some(id), label);
                if self
                    .deletions
                    .tag_deletions
                    .iter()
                    .any(|review| review.id == id)
                {
                    ui.small("Deletion staged");
                }
            });
            self.diagnostics(ui, RecordId::Tag(id));
        }
        for id in self.problems.keys() {
            if let RecordId::Tag(tag) = id
                && !catalog.contains_key(tag)
            {
                self.diagnostics(ui, *id);
            }
        }

        if let Some(id) = self.selected_tag
            && catalog.contains_key(&id)
        {
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.monospace(id.to_string());
                if ui.button("Copy ID").clicked() {
                    ui.ctx().copy_text(id.to_string());
                }
            });
            if self
                .deletions
                .tag_deletions
                .iter()
                .any(|review| review.id == id)
            {
                if ui.button("Undo staged deletion").clicked() {
                    self.deletions
                        .tag_deletions
                        .retain(|review| review.id != id);
                }
            } else {
                self.logbook_form(ui, id, snapshot, sender);
            }
        }
        self.show_review(ui.ctx());
    }

    /// Display the selected tag's fields and submit only the read required to review deletion.
    fn logbook_form(
        &mut self,
        ui: &mut egui::Ui,
        id: TagId,
        snapshot: &SettingsSnapshot,
        sender: &mpsc::Sender<SettingsRequest>,
    ) {
        let key = RecordId::Tag(id);
        let form = self.tags.remove(&id);
        let was_dirty = form.is_some();
        let form = form.or_else(|| {
            let expected = snapshot
                .logbooks
                .tags
                .iter()
                .find(|record| record.key().id == key)?;
            Some(TagForm {
                editor: TagEditor::from_tag(tag(expected)?),
                expected: Some(expected.clone()),
            })
        });
        let Some(mut form) = form else { return };
        let baseline = form.expected.as_ref().and_then(tag);
        let validation = self.validation.entry(key).or_default();
        validation.advance(ui, || form.editor.build().err().unwrap_or_default());
        let changed = editor::fields(ui, &mut form.editor, baseline, &validation.errors);
        if changed {
            validation.edited(ui);
        }
        let new = form.expected.is_none();
        if (was_dirty || changed) && (new || form.editor.build().ok().as_ref() != baseline) {
            self.tags.insert(id, form);
        }
        if new {
            if ui.button("Discard new logbook").clicked() {
                self.tags.remove(&id);
                self.selected_tag = None;
            }
        } else {
            let button = if self.previous_tag_choices.contains_key(&id) {
                "Review updated deletion"
            } else {
                "Delete logbook"
            };
            if ui.button(button).clicked() {
                let (reply, response) = oneshot::channel();
                let action = SettingsAction::Logbooks(LogbookAction::PreviewDeletion { id });
                self.review_error = None;
                match sender.try_send(SettingsRequest { action, reply }) {
                    Ok(()) => self.review = Some(reviews::Review::LoadingTag { id, response }),
                    Err(error) => {
                        self.review_error =
                            Some((key, format!("Could not load the deletion review: {error}")))
                    }
                }
            }
        }
    }
}
