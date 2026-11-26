//! AppKit menus installed on eframe's main thread.
use super::{CommandItem, MenuAdapter};
use crate::{build_identity::BUILD_IDENTITY, menus::MenuCommand};
use muda::{
    Menu, MenuItem, PredefinedMenuItem, Submenu,
    accelerator::{Accelerator, Code, Modifiers},
};
use winit::platform::macos::EventLoopBuilderExtMacOS;

/// Preserve standard application actions while Quit uses Clacks's lifecycle.
pub(super) fn application_menu(menu: &Menu, items: &mut Vec<CommandItem>) -> anyhow::Result<()> {
    let app = Submenu::new("Clacks Log", true);
    let settings = MenuItem::with_id(
        MenuCommand::OpenSettings.id(),
        MenuCommand::OpenSettings.label(),
        false,
        Some(Accelerator::new(Modifiers::META, Code::Comma)),
    );
    let quit = MenuItem::with_id(
        MenuCommand::Quit.id(),
        MenuCommand::Quit.label(),
        true,
        Some(Accelerator::new(Modifiers::META, Code::KeyQ)),
    );
    app.append_items(&[
        &PredefinedMenuItem::about(
            Some("About Clacks Log"),
            Some(muda::AboutMetadata {
                name: Some("Clacks Log".into()),
                version: Some(BUILD_IDENTITY.version.into()),
                credits: BUILD_IDENTITY.provenance(),
                copyright: Some("Copyright (c) 2026 Technosorcery, LLC.".into()),
                ..Default::default()
            }),
        ),
        &PredefinedMenuItem::separator(),
        &settings,
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::services(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::hide(None),
        &PredefinedMenuItem::hide_others(None),
        &PredefinedMenuItem::show_all(None),
        &PredefinedMenuItem::separator(),
        &quit,
    ])?;
    menu.append(&app)?;
    items.push(CommandItem {
        command: MenuCommand::OpenSettings,
        item: settings,
    });
    items.push(CommandItem {
        command: MenuCommand::Quit,
        item: quit,
    });
    Ok(())
}
/// macOS has no mnemonic ampersands in top-level titles.
pub(super) fn menu_title(title: &str) -> String {
    title.into()
}
/// Install a complete replacement rather than retaining duplicate default menus.
pub(super) fn configure(_menu: &Menu, options: &mut eframe::NativeOptions) {
    options.event_loop_builder = Some(Box::new(|builder| {
        builder.with_default_menu(false);
    }));
}
/// Called only after eframe created NSApplication and its root window.
pub(super) fn attach(adapter: &MenuAdapter, _window: &winit::window::Window) -> anyhow::Result<()> {
    adapter.menu.init_for_nsapp();
    adapter.help.set_as_help_menu_for_nsapp();
    Ok(())
}
/// Remove while the application and retained window are still alive.
pub(super) fn detach(adapter: &MenuAdapter, _window: &winit::window::Window) {
    adapter.menu.remove_for_nsapp();
}
