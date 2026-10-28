//! Optional viewport over the same Worked Before data and table preferences.

use super::*;

impl WorkedBeforeUi {
    /// Draw the dedicated window without creating another entry or query model.
    pub fn show_window(&mut self, context: &egui::Context) -> Option<QsoId> {
        if !self.window_open {
            return None;
        }
        let mut opened = None;
        context.show_viewport_immediate(
            egui::ViewportId::from_hash_of("clacks.worked_before"),
            egui::ViewportBuilder::default()
                .with_title("Clacks Log · Worked Before")
                .with_inner_size([1000.0, 450.0]),
            |ui, _class| {
                if ui.input(|i| i.viewport().close_requested()) {
                    self.window_open = false;
                }
                egui::CentralPanel::default()
                    .frame(crate::ui::theme::section_frame(ui))
                    .show(ui, |ui| {
                        if ui.small_button("Close window").clicked() {
                            self.window_open = false;
                        }
                        opened = ui
                            .push_id("worked_before.window", |ui| {
                                show_table(
                                    ui,
                                    &mut self.model,
                                    &mut self.preferences,
                                    &mut self.window,
                                )
                            })
                            .inner;
                    });
            },
        );
        opened
    }
}
