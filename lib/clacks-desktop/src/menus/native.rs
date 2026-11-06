//! Native menu resources and a weak, process-lifetime activation callback.

#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod platform;
#[cfg(windows)]
#[path = "windows.rs"]
mod platform;

use super::{GROUPS, MenuCommand};
use eframe::egui;
use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use std::sync::{Arc, mpsc};

/// Callback recipient owned by the running window, never by muda's static hook.
struct Delivery {
    /// Commands are executed by the root application logic.
    sender: mpsc::Sender<MenuCommand>,
    /// Eframe's supported idle-loop wake-up mechanism.
    context: egui::Context,
}

/// Native objects stay on the UI thread and outlive attachment.
pub(crate) struct MenuAdapter {
    /// Shared native menu root.
    menu: Menu,
    /// Only application actions; standard OS actions remain platform-owned.
    items: Vec<CommandItem>,
    /// Help needs registration with AppKit.
    #[cfg(target_os = "macos")]
    help: Submenu,
    /// Keeping the root window alive makes native detach safe even on error.
    window: Option<Arc<winit::window::Window>>,
    /// Dropping this releases the context even though muda cannot remove its hook.
    delivery: Option<Arc<Delivery>>,
}

/// Explicit command identity beside its mutable native item.
struct CommandItem {
    /// Application routing key.
    command: MenuCommand,
    /// Native title, enabled state and accelerator.
    item: MenuItem,
}

impl MenuAdapter {
    /// Build once before event-loop construction so Windows can install its hook.
    #[tracing::instrument(skip_all)]
    pub fn prepare() -> anyhow::Result<Self> {
        let result = Self::build();
        if result.is_err() {
            Self::record_initialization("failure");
        }
        result
    }

    fn build() -> anyhow::Result<Self> {
        let menu = Menu::new();
        let mut items = Vec::new();
        platform::application_menu(&menu, &mut items)?;
        #[cfg(target_os = "macos")]
        let mut help = None;
        for group in GROUPS {
            let submenu = Submenu::new(platform::menu_title(group.title), true);
            for command in group.commands {
                if let Some(command) = command {
                    let accelerator = (*command == MenuCommand::SelectAll).then(|| {
                        let modifier = if cfg!(target_os = "macos") {
                            Modifiers::META
                        } else {
                            Modifiers::CONTROL
                        };
                        Accelerator::new(modifier, Code::KeyA)
                    });
                    let item = MenuItem::with_id(
                        command.id(),
                        item_text(*command, false),
                        false,
                        accelerator,
                    );
                    submenu.append(&item)?;
                    items.push(CommandItem {
                        command: *command,
                        item,
                    });
                } else {
                    submenu.append(&PredefinedMenuItem::separator())?;
                }
            }
            menu.append(&submenu)?;
            #[cfg(target_os = "macos")]
            if group.title == "Help" {
                help = Some(submenu);
            }
        }
        Ok(Self {
            menu,
            items,
            window: None,
            delivery: None,
            #[cfg(target_os = "macos")]
            help: help.ok_or_else(|| anyhow::anyhow!("native Help menu is missing"))?,
        })
    }

    /// Apply the platform's standard winit integration hooks.
    pub fn configure(&self, options: &mut eframe::NativeOptions) {
        platform::configure(&self.menu, options);
    }

    /// Attach after eframe has constructed the root native window.
    #[tracing::instrument(skip_all)]
    pub fn attach(
        &mut self,
        creation: &eframe::CreationContext<'_>,
        sender: mpsc::Sender<MenuCommand>,
    ) -> anyhow::Result<()> {
        let result = self.attach_inner(creation, sender);
        Self::record_initialization(if result.is_ok() { "success" } else { "failure" });
        if result.is_ok() {
            tracing::info!("Native application menus attached");
        }
        result
    }

    fn attach_inner(
        &mut self,
        creation: &eframe::CreationContext<'_>,
        sender: mpsc::Sender<MenuCommand>,
    ) -> anyhow::Result<()> {
        let window = creation
            .winit_window()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("native menus require a root window"))?;
        platform::attach(self, &window)?;
        self.window = Some(window);
        let delivery = Arc::new(Delivery {
            sender,
            context: creation.egui_ctx.clone(),
        });
        let weak = Arc::downgrade(&delivery);
        let commands: Vec<_> = self.items.iter().map(|item| item.command).collect();
        // muda 0.21.2 uses OnceLock: set_event_handler(None) cannot remove this.
        // A weak recipient lets shutdown release the sender and egui context.
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let Some(delivery) = weak.upgrade() else {
                return;
            };
            let Some(command) = commands
                .iter()
                .find(|command| event.id.as_ref() == command.id())
            else {
                return;
            };
            if delivery.sender.send(*command).is_err() {
                tracing::warn!("Native menu recipient disconnected while attached");
                return;
            }
            delivery.context.request_repaint_of(egui::ViewportId::ROOT);
        }));
        self.delivery = Some(delivery);
        Ok(())
    }

    /// Apply application policy only when a native item's value changes.
    pub fn set_enabled(&self, command: MenuCommand, enabled: bool) {
        if let Some(item) = self.items.iter().find(|item| item.command == command)
            && item.item.is_enabled() != enabled
        {
            item.item.set_enabled(enabled);
        }
    }

    /// Enrollment changes do not depend on painting a removed opener button.
    pub fn set_connection_label(&self, enrolled: bool) {
        if let Some(item) = self
            .items
            .iter()
            .find(|item| item.command == MenuCommand::Connection)
        {
            let label = item_text(MenuCommand::Connection, enrolled);
            if item.item.text() != label {
                item.item.set_text(&label);
            }
        }
    }

    fn record_initialization(outcome: &'static str) {
        metrics::counter!("clacks.desktop.menu.initializations", "platform" => std::env::consts::OS, "outcome" => outcome).increment(1);
    }
}

impl Drop for MenuAdapter {
    fn drop(&mut self) {
        self.delivery.take();
        if let Some(window) = self.window.take() {
            platform::detach(self, &window);
        }
    }
}

/// Muda treats single ampersands as mnemonics on native platforms.
pub(crate) fn item_text(command: MenuCommand, enrolled: bool) -> String {
    command.label(enrolled).replace('&', "&&")
}
