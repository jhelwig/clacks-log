//! Named-logbook editing with explicit Save and Cancel boundaries.

use super::*;
use clacks_model::tag::AutomaticUploadPolicy;

impl LogbooksUi {
    /// Show saved records or one independent editor inside the native modal.
    pub(super) fn show_management(
        &mut self,
        context: &egui::Context,
        sender: &mpsc::Sender<SettingsRequest>,
        records: &[ClientRecord],
        tags: &[&LogbookTag],
        labels: &BTreeMap<TagId, String>,
    ) {
        let Some(frame) = &mut self.management else {
            return;
        };
        let mut cancel = false;
        let result = dialog::show(
            context,
            egui::Id::unique("logbooks.manage"),
            "Manage logbooks",
            |ui| {
                if let Some(notice) = &frame.notice {
                    feedback::problem(ui, notice);
                }
                let pending = frame.response.is_some();
                ui.add_enabled_ui(!pending, |ui| {
                if frame.deletion.is_some() {
                    super::deletion::show(ui, frame, sender);
                } else if frame.form.is_some() {
                    show_form(ui, frame, records, sender);
                } else {
                    ui.label("Save changes a named logbook. Choose it in the main selector to make it the current view.");
                    if ui.button("New logbook").clicked() { frame.form = Some(TagForm::new(None)); }
                    for tag in tags {
                        ui.push_id(tag.id, |ui| {
                            theme::section_frame(ui).show(ui, |ui| {
                                if let Some(label) = labels.get(&tag.id) { ui.strong(label); }
                                ui.horizontal_wrapped(|ui| {
                                    ui.monospace(tag.id.to_string());
                                    if ui.button("Copy ID").clicked() { ui.ctx().copy_text(tag.id.to_string()); }
                                    if ui.button("Edit logbook").clicked() {
                                        frame.form = Some(TagForm::new(records.iter().find(|row| row.key.id==RecordId::Tag(tag.id)).cloned()));
                                        frame.notice = None;
                                    }
                                    if ui.button("Delete logbook").clicked() {
                                        frame.send(sender, LogbookAction::PreviewDeletion { id: tag.id });
                                    }
                                });
                                ui.small(format!("LoTW: {:?} · QRZ: {:?}", tag.automatic_upload.lotw, tag.automatic_upload.qrz));
                            });
                        });
                    }
                    if ui.button("Close manager").clicked() { cancel = true; }
                }
            });
                if pending {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Working…");
                    });
                }
            },
        );
        if result.cancel_requested || cancel {
            self.cancel();
        }
    }
}

/// Render one tag form and submit only validated current values.
fn show_form(
    ui: &mut egui::Ui,
    frame: &mut Management,
    records: &[ClientRecord],
    sender: &mpsc::Sender<SettingsRequest>,
) {
    let Some(form) = &mut frame.form else {
        return;
    };
    let now = ui.input(|input| input.time);
    if form.validation.due(now) {
        form.errors = form.editor.build().err().unwrap_or_default();
        form.validation.validated();
    }
    form.validation.request_repaint(ui.ctx(), now);
    ui.label("Save changes only this logbook. It does not select it or change station/equipment settings.");
    let baseline = form.expected.as_ref().and_then(tag);
    let edited = baseline.is_none_or(|tag| form.editor.name != tag.name);
    if crate::ui::operating::fields::text(
        ui,
        "logbook.name",
        "Logbook name",
        &mut form.editor.name,
        &form.errors,
        edited,
    ) {
        form.validation.edited(now);
    }
    ui.separator();
    ui.strong("Automatic uploads");
    ui.label("Enable permits automatic uploads; Forbid blocks them even when another tag enables them. Neither contributes no permission or prohibition.");
    let policies = form.editor.automatic_upload.clone();
    let original_policies = baseline
        .map(|tag| tag.automatic_upload.clone())
        .unwrap_or_default();
    policy(
        ui,
        "LoTW",
        &mut form.editor.automatic_upload.lotw,
        original_policies.lotw != policies.lotw,
    );
    policy(
        ui,
        "QRZ",
        &mut form.editor.automatic_upload.qrz,
        original_policies.qrz != policies.qrz,
    );
    if policies != form.editor.automatic_upload {
        form.validation.edited(now);
    }

    if form.stale {
        feedback::problem(
            ui,
            "This saved logbook changed. Your form is preserved. Reload the saved values before editing and saving again.",
        );
        let current = records
            .iter()
            .find(|row| row.key.id == RecordId::Tag(form.editor.id))
            .filter(|row| tag(row).is_some());
        if ui
            .add_enabled(current.is_some(), egui::Button::new("Reload saved logbook"))
            .clicked()
        {
            frame.form = Some(TagForm::new(current.cloned()));
            frame.notice = None;
            return;
        }
    }
    let can_save = form.validation.can_submit(&form.errors) && !form.stale;
    if !form.errors.is_empty() {
        ui.label("Fix the highlighted fields to save.");
    }
    let mut save = false;
    let mut cancel = false;
    ui.horizontal(|ui| {
        save = crate::ui::actions::primary(ui, "Save logbook", can_save).clicked();
        cancel = ui.button("Cancel edits").clicked();
    });
    if save {
        match form.editor.build() {
            Ok(next) => {
                let expected = form.expected.clone().map(Box::new);
                frame.send(sender, LogbookAction::SaveTag { expected, next });
            }
            Err(errors) => {
                form.errors = errors;
                form.validation.validated();
            }
        }
    } else if cancel {
        frame.form = None;
        frame.notice = None;
    }
}

/// Present one independent service policy with its edited-state distinction.
fn policy(ui: &mut egui::Ui, label: &str, value: &mut AutomaticUploadPolicy, edited: bool) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        if edited {
            ui.colored_label(theme::palette(ui.ctx().theme()).edited_edge, "Not applied");
        }
        for policy in [
            AutomaticUploadPolicy::Neither,
            AutomaticUploadPolicy::Enable,
            AutomaticUploadPolicy::Forbid,
        ] {
            ui.radio_value(value, policy, format!("{policy:?}"));
        }
    });
}
