//! Menu admission and dispatch reuse the existing presentation owners.

use super::DesktopApp;
use crate::{menus::MenuCommand, session::SessionStatus};
use eframe::egui;

impl DesktopApp {
    /// Whether an active workflow prevents opening another workspace action.
    pub(super) fn modal_open(&self) -> bool {
        self.appearance.editor.is_some()
            || self.ws_suite.is_open()
            || self.enrollment.is_open()
            || self.logbooks.is_open()
            || self.settings.is_open()
            || self.details.is_open()
            || self.review.is_exclusive()
            || self.workbench.has_modal()
    }

    /// Current state, not the state when the operating system queued activation.
    pub(super) fn menu_enabled(&self, context: &egui::Context, command: MenuCommand) -> bool {
        if command == MenuCommand::Quit {
            return true;
        }
        let state = self.status.borrow();
        if self.close_requested
            || matches!(
                *state,
                SessionStatus::Closing { .. } | SessionStatus::Closed
            )
        {
            return false;
        }
        if command == MenuCommand::SelectAll
            && !self.review.is_exclusive()
            && context
                .memory(|memory| memory.focused())
                .is_some_and(|id| egui::TextEdit::load_state(context, id).is_some())
        {
            return true;
        }
        if self.modal_open() {
            return false;
        }
        if matches!(command, MenuCommand::Appearance | MenuCommand::Diagnostics) {
            return true;
        }
        if !matches!(*state, SessionStatus::Ready { .. }) || self.settings_view.borrow().is_none() {
            return false;
        }
        match command {
            MenuCommand::LookupDirectory => self.workbench.can_lookup_directory(),
            MenuCommand::SelectAll => {
                self.workbench.history_has_focus(context) && !self.workbench.selection_pending()
            }
            MenuCommand::ManageLogbooks => !self.logbooks.is_busy(),
            MenuCommand::StationProfiles
            | MenuCommand::EquipmentConfigurations
            | MenuCommand::RefreshSettings => !self.settings.is_busy(),
            _ => true,
        }
    }

    /// Execute once on the UI thread; downstream operations keep their own errors.
    pub(super) fn dispatch_menu(&mut self, context: &egui::Context, command: MenuCommand) {
        self.workbench.poll_selection();
        self.workbench.poll_bulk_lookup();
        if let Some(snapshot) = self.settings_view.borrow().as_ref() {
            self.workbench
                .reconcile_history_view(snapshot.logbooks.selected_view);
        }
        let enabled = self.menu_enabled(context, command);
        let outcome = if enabled {
            "accepted"
        } else {
            "stale_disabled"
        };
        tracing::debug!(?command, outcome, "Menu command dispatched");
        metrics::counter!("clacks.desktop.menu.commands", "command" => command.id(), "outcome" => outcome).increment(1);
        if !enabled {
            return;
        }
        // A menu Quit may arrive after this pass's lifecycle advance, including
        // while hidden. Schedule the next logic pass to drain shutdown normally.
        context.request_repaint_of(egui::ViewportId::ROOT);
        let opener = context.memory(|memory| memory.focused());
        match command {
            MenuCommand::LookupDirectory => self
                .workbench
                .start_directory_lookup(&self.settings_requests),
            MenuCommand::SelectAll => {
                if let Some(id) = context.memory(|memory| memory.focused())
                    && egui::TextEdit::load_state(context, id).is_some()
                {
                    self.pending_text_select_all = Some(id);
                } else {
                    self.workbench.select_all_contacts(&self.settings_requests);
                }
            }
            MenuCommand::ManageLogbooks => self.logbooks.open_management(opener),
            MenuCommand::StationProfiles => self.settings.open_profiles(opener),
            MenuCommand::EquipmentConfigurations => self.settings.open_equipment(opener),
            MenuCommand::RefreshSettings => self.settings.refresh_settings(&self.settings_requests),
            MenuCommand::Connection => self.enrollment.open(opener),
            MenuCommand::Appearance => self.appearance.open(opener),
            MenuCommand::WsSuite => {
                self.ws_suite
                    .open(opener, &self.ws_suite_view.borrow(), &self.commands)
            }
            MenuCommand::ResetLayout => self.workbench.reset_layout(),
            MenuCommand::Diagnostics => self.details.open(opener),
            MenuCommand::Quit => self.begin_close(),
        }
        if !matches!(
            command,
            MenuCommand::Quit | MenuCommand::RefreshSettings | MenuCommand::ResetLayout
        ) {
            for action in [
                egui::ViewportCommand::Visible(true),
                egui::ViewportCommand::Minimized(false),
                egui::ViewportCommand::Focus,
            ] {
                context.send_viewport_cmd_to(egui::ViewportId::ROOT, action);
            }
        }
    }
}

impl DesktopApp {
    /// Native resources remain separate from headless component resources.
    #[cfg(any(target_os = "macos", windows))]
    pub(crate) fn install_menus(
        &mut self,
        context: &egui::Context,
        menus: crate::menus::native::MenuAdapter,
        receiver: std::sync::mpsc::Receiver<MenuCommand>,
    ) {
        self.native_menus = Some(menus);
        self.menu_receiver = Some(receiver);
        self.update_menu_state(context);
    }

    /// Consume activation once, including while the workbench is not painted.
    #[cfg(any(target_os = "macos", windows))]
    pub(super) fn process_menus(&mut self, context: &egui::Context) {
        self.logbooks.poll();
        self.settings.poll();
        self.workbench.poll_selection();
        self.workbench.poll_bulk_lookup();
        loop {
            let command = self
                .menu_receiver
                .as_ref()
                .and_then(|receiver| receiver.try_recv().ok());
            let Some(command) = command else {
                break;
            };
            self.dispatch_menu(context, command);
        }
        self.update_menu_state(context);
    }

    /// Current modal, lifecycle and pending-request rules govern native items.
    #[cfg(any(target_os = "macos", windows))]
    pub(super) fn update_menu_state(&self, context: &egui::Context) {
        if let Some(menus) = &self.native_menus {
            for command in crate::menus::GROUPS
                .iter()
                .flat_map(|group| group.commands.iter().flatten())
            {
                menus.set_enabled(*command, self.menu_enabled(context, *command));
            }
            menus.set_connection_label(self.is_enrolled());
        }
    }

    /// Read authoritative account state even before the next rendered frame.
    pub(super) fn is_enrolled(&self) -> bool {
        matches!(&*self.status.borrow(), SessionStatus::Ready { identity, .. } if identity.enrolled)
    }
}
