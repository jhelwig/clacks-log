//! Native desktop entry point; application behavior belongs to clacks-desktop.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    clacks_desktop::run()
}
