use tray_icon::Icon;

/// Transparent-background colour artwork for the tray / menu-bar (repo-root
/// `logo-tray.png`), shared with the main crate.
const LOGO_TRAY_PNG: &[u8] = include_bytes!("../../logo-tray.png");

/// Creates the tray / menu-bar icon from the bundled colour artwork. Square so
/// `resize_exact` keeps its proportions; same image on macOS and Windows.
pub fn create_tray_icon() -> Icon {
    const SIZE: u32 = 256;
    let rgba = image::load_from_memory(LOGO_TRAY_PNG)
        .expect("bundled logo-tray.png is a valid PNG")
        .resize_exact(SIZE, SIZE, image::imageops::FilterType::Lanczos3)
        .to_rgba8()
        .into_raw();
    Icon::from_rgba(rgba, SIZE, SIZE).expect("Failed to create tray icon")
}
