//! Editable name and independent service policies for a named logbook.

use super::*;
use crate::{logbooks::TagEditor, operating::FieldError};
use clacks_model::tag::AutomaticUploadPolicy;

/// Render the logbook fields with their original values and native validation.
pub(in crate::ui) fn fields(
    ui: &mut egui::Ui,
    editor: &mut TagEditor,
    baseline: Option<&LogbookTag>,
    errors: &[FieldError],
) -> bool {
    let edited = baseline.is_none_or(|tag| editor.name != tag.name);
    let changed = crate::ui::operating::fields::text(
        ui,
        "logbook.name",
        "Logbook name",
        &mut editor.name,
        errors,
        edited,
    );
    ui.separator();
    ui.strong("Automatic uploads");
    ui.label("Enable permits automatic uploads. Forbid blocks them even when another logbook enables them. Neither leaves the decision to other logbooks.");
    let previous = editor.automatic_upload.clone();
    let original = baseline
        .map(|tag| tag.automatic_upload.clone())
        .unwrap_or_default();
    policy(
        ui,
        "LoTW",
        &mut editor.automatic_upload.lotw,
        original.lotw != previous.lotw,
    );
    policy(
        ui,
        "QRZ",
        &mut editor.automatic_upload.qrz,
        original.qrz != previous.qrz,
    );
    changed || previous != editor.automatic_upload
}

/// Present one independent service policy with its edited-state distinction.
pub(in crate::ui) fn policy(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut AutomaticUploadPolicy,
    edited: bool,
) {
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
