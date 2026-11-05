//! Embed the Windows application identity in native Windows builds.

#[cfg(windows)]
fn main() -> std::io::Result<()> {
    winresource::WindowsResource::new()
        .set_icon("../../lib/clacks-desktop/assets/clacks-log.ico")
        .set("ProductName", "Clacks Log")
        .set("FileDescription", "Clacks Log native desktop application")
        .set("CompanyName", "Technosorcery, LLC")
        .set("LegalCopyright", "Copyright (c) 2026 Technosorcery, LLC.")
        .set("OriginalFilename", "clacks-log.exe")
        .compile()
}

#[cfg(not(windows))]
fn main() {}
