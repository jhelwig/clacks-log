//! Shared native window artwork selected for the application.

/// Decode the selected application artwork for native viewport construction.
///
/// Returns a startup error if the embedded production PNG cannot be decoded.
pub(crate) fn application_icon() -> anyhow::Result<eframe::egui::IconData> {
    let image = image::load_from_memory_with_format(
        include_bytes!("../assets/clacks-log.png"),
        image::ImageFormat::Png,
    )?
    .into_rgba8();
    let (width, height) = image.dimensions();
    Ok(eframe::egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}
