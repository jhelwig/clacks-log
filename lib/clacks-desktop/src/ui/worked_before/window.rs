//! Optional viewport over the same Worked Before data and table preferences.

use super::*;

impl WorkedBeforeUi {
    /// Configure the native window using the measured shared controls and a
    /// table viewport large enough to display one complete row.
    pub(super) fn window_builder(&mut self, context: &egui::Context) -> egui::ViewportBuilder {
        let measure = |width, salt| {
            egui::Ui::new(
                context.clone(),
                egui::Id::unique(salt),
                egui::UiBuilder::new()
                    .max_rect(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 10000.0),
                    ))
                    .sizing_pass()
                    .invisible(),
            )
        };
        let mut width_ui = measure(10000.0, "prior.minimum.width");
        show_counts(&mut width_ui, &self.model);
        width_ui.label("No contacts in this view.");
        let width = width_ui.min_rect().width().ceil();
        let mut height_ui = measure(width, "prior.minimum.height");
        let spacing = height_ui.spacing().item_spacing.y;
        height_ui.spacing_mut().item_spacing.y = 0.0;
        height_ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = spacing;
            show_controls(ui, &mut self.model);
        });
        height_ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = spacing;
            show_counts(ui, &self.model);
        });
        height_ui.spacing_mut().item_spacing.y = spacing;
        let height =
            (height_ui.min_rect().height() + contact_table::minimum_height(&height_ui)).ceil();
        egui::ViewportBuilder::default()
            .with_title("Clacks Log · Worked Before")
            .with_inner_size([1000.0, 450.0])
            .with_min_inner_size([width, height])
    }

    /// Show the Worked Before window. While interaction is suspended, decline
    /// close requests and suppress contact selection.
    pub fn show_window(&mut self, context: &egui::Context, interactive: bool) -> Option<QsoId> {
        if !self.window_open {
            return None;
        }
        let mut opened = None;
        let builder = self.window_builder(context);
        context.show_viewport_immediate(
            egui::ViewportId::from_hash_of("clacks.worked_before"),
            builder,
            |ui, _class| {
                if ui.input(|i| i.viewport().close_requested()) {
                    if interactive {
                        self.window_open = false;
                        return;
                    }
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::CancelClose);
                }
                if !interactive {
                    ui.disable();
                }
                egui::CentralPanel::default()
                    .frame(crate::ui::theme::pane_frame(ui))
                    .show(ui, |ui| {
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
