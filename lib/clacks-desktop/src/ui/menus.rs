//! Explicit Linux fallback; Windows/macOS use their operating-system menus.
use super::DesktopApp;
use crate::menus::GROUPS;
use eframe::egui;

impl DesktopApp {
    /// Same commands and admission rules, without adding GTK to eframe's window.
    pub(super) fn show_menus(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("application.menus").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                for group in GROUPS {
                    ui.menu_button(group.title, |ui| {
                        for command in group.commands {
                            if let Some(command) = command {
                                if ui
                                    .add_enabled(
                                        self.menu_enabled(*command),
                                        egui::Button::new(command.label(self.is_enrolled())),
                                    )
                                    .clicked()
                                {
                                    self.dispatch_menu(ui.ctx(), *command);
                                    ui.close();
                                }
                            } else {
                                ui.separator();
                            }
                        }
                    });
                }
            });
        });
    }
}
