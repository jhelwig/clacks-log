//! Shared modal chrome with native popup, focus and scroll behavior.

use eframe::egui;

/// Body result and an explicit dismissal request, separate from backdrop input.
pub(super) struct DialogResponse<R> {
    /// Value returned by the domain-specific dialog body.
    pub inner: R,
    /// Close button or Escape outside an open popup requested cancellation.
    pub cancel_requested: bool,
}

/// Show a bounded, scrollable editor; clicks outside it never discard text.
pub(super) fn show<R>(
    context: &egui::Context,
    id: egui::Id,
    title: &str,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> DialogResponse<R> {
    show_with_dismissal(context, id, title, true, body)
}

/// With `can_dismiss` false, disable the close button and decline Escape dismissal.
pub(super) fn show_with_dismissal<R>(
    context: &egui::Context,
    id: egui::Id,
    title: &str,
    can_dismiss: bool,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> DialogResponse<R> {
    let mut close = false;
    let bounds = context.content_rect();
    let size = egui::vec2(704.0_f32.min(bounds.width() - 32.0), bounds.height() - 64.0);
    let response = egui::Modal::new(id)
        .area(egui::Modal::default_area(id).default_size(size))
        .show(context, |ui| {
            ui.set_max_height((bounds.height() - 64.0).max(160.0));
            ui.set_width(680.0_f32.min((bounds.width() - 48.0).max(160.0)));
            ui.horizontal(|ui| {
                ui.heading(title);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    close = ui
                        .add_enabled(can_dismiss, egui::Button::new("×"))
                        .on_hover_text("Cancel and close")
                        .clicked();
                });
            });
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height((bounds.height() - 140.0).max(100.0))
                .show(ui, body)
                .inner
        });
    let escape = response.is_top_modal
        && !response.any_popup_open
        && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    DialogResponse {
        inner: response.inner,
        cancel_requested: can_dismiss && (close || escape),
    }
}
