//! Win32 menu attachment and standard accelerator translation.
use super::{CommandItem, MenuAdapter};
use muda::Menu;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::platform::windows::EventLoopBuilderExtWindows;

/// Windows application commands use the shared accepted menu groups.
pub(super) fn application_menu(_menu: &Menu, _items: &mut Vec<CommandItem>) -> anyhow::Result<()> {
    Ok(())
}
/// Native Alt access for the top-level menus.
pub(super) fn menu_title(title: &str) -> String {
    format!("&{title}")
}
/// winit explicitly supports translating accelerators before its own dispatch.
pub(super) fn configure(menu: &Menu, options: &mut eframe::NativeOptions) {
    let menu = menu.clone();
    options.event_loop_builder = Some(Box::new(move |builder| {
        builder.with_msg_hook(move |message| {
            use windows_sys::Win32::UI::WindowsAndMessaging::{MSG, TranslateAcceleratorW};
            // SAFETY: winit supplies a live MSG during this callback. The captured
            // menu owns the accelerator table throughout the hook's lifetime.
            unsafe {
                let message = message.cast::<MSG>();
                TranslateAcceleratorW((*message).hwnd, menu.haccel() as _, message) != 0
            }
        });
    }));
}
/// Resolve a handle only from the live, retained root window.
fn hwnd(window: &winit::window::Window) -> anyhow::Result<isize> {
    match window.window_handle()?.as_raw() {
        RawWindowHandle::Win32(handle) => Ok(handle.hwnd.get()),
        _ => anyhow::bail!("native Windows menu requires a Win32 window"),
    }
}
/// The adapter retains this window until native detachment completes.
pub(super) fn attach(adapter: &MenuAdapter, window: &winit::window::Window) -> anyhow::Result<()> {
    // SAFETY: the handle belongs to the live eframe window on its UI thread.
    unsafe {
        adapter.menu.init_for_hwnd(hwnd(window)?)?;
    }
    Ok(())
}
/// Detach before releasing the adapter's root-window reference.
pub(super) fn detach(adapter: &MenuAdapter, window: &winit::window::Window) {
    let result = hwnd(window).and_then(|handle| {
        // SAFETY: the adapter still retains the attached window on its UI thread.
        unsafe { adapter.menu.remove_for_hwnd(handle).map_err(Into::into) }
    });
    if let Err(error) = result {
        tracing::warn!(%error, "Native menu detach failed");
    }
}
