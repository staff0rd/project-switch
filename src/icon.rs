//! App icons decoded from bundled PNGs. `logo-tray.png` (transparent colour
//! artwork) feeds the system tray / menu-bar on both platforms; the macOS dock
//! gets a squircle-masked variant of `logo.png` so the webview window sits like
//! a native app icon instead of the default exec mark.

/// Transparent-background colour artwork for the system tray / menu-bar.
#[cfg(any(windows, target_os = "macos"))]
const LOGO_TRAY_PNG: &[u8] = include_bytes!("../logo-tray.png");

/// Full square logo, used only for the macOS dock squircle.
#[cfg(target_os = "macos")]
const LOGO_PNG: &[u8] = include_bytes!("../logo.png");

/// Decode a bundled logo PNG to a square `size`×`size` RGBA buffer (Lanczos3).
#[cfg(any(windows, target_os = "macos"))]
fn decode_square(png: &[u8], size: u32) -> (Vec<u8>, u32, u32) {
    let img = image::load_from_memory(png)
        .expect("bundled logo is a valid PNG")
        .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    (img.into_raw(), size, size)
}

/// Colour icon for the system tray / menu-bar (same artwork on both platforms).
#[cfg(any(windows, target_os = "macos"))]
pub fn create_icon_rgba() -> (Vec<u8>, u32, u32) {
    decode_square(LOGO_TRAY_PNG, 256)
}

/// macOS dock icon: the logo inset on a transparent canvas with its corners
/// rounded into the system "squircle", so it scales and reads like a native
/// app icon in the dock.
#[cfg(target_os = "macos")]
pub fn create_dock_icon_rgba() -> (Vec<u8>, u32, u32) {
    const CANVAS: u32 = 512;
    // Apple's icon grid: the rounded body fills ~80.5% of the tile, leaving a
    // transparent margin so it sits at the same scale as sibling dock icons.
    const SHAPE_RATIO: f32 = 0.8047;
    // Corner radius as a fraction of the body — approximates the continuous
    // squircle with a plain anti-aliased rounded rectangle.
    const RADIUS_RATIO: f32 = 0.2237;

    let shape = (CANVAS as f32 * SHAPE_RATIO).round() as u32;
    let (logo, _, _) = decode_square(LOGO_PNG, shape);

    let mut canvas = vec![0u8; (CANVAS * CANVAS * 4) as usize];
    let offset = (CANVAS - shape) / 2;
    let radius = shape as f32 * RADIUS_RATIO;

    for y in 0..shape {
        for x in 0..shape {
            let cov = rounded_coverage(x, y, shape, shape, radius);
            if cov <= 0.0 {
                continue;
            }
            let src = ((y * shape + x) * 4) as usize;
            let dst = (((y + offset) * CANVAS + (x + offset)) * 4) as usize;
            // Premultiply: AppKit's legacy NSBitmapImageRep initializer reads the
            // pixels as premultiplied alpha, so the corner feather scales RGB too.
            canvas[dst] = (logo[src] as f32 * cov).round() as u8;
            canvas[dst + 1] = (logo[src + 1] as f32 * cov).round() as u8;
            canvas[dst + 2] = (logo[src + 2] as f32 * cov).round() as u8;
            canvas[dst + 3] = (logo[src + 3] as f32 * cov).round() as u8;
        }
    }

    (canvas, CANVAS, CANVAS)
}

/// Anti-aliased coverage (0..=1) of pixel `(x, y)` inside a rounded rectangle of
/// size `w`×`h` with corner radius `r`. Uses the signed distance to the nearest
/// corner arc centre, feathered over one pixel so the curve stays smooth.
#[cfg(target_os = "macos")]
fn rounded_coverage(x: u32, y: u32, w: u32, h: u32, r: f32) -> f32 {
    let px = x as f32 + 0.5;
    let py = y as f32 + 0.5;
    let cx = px.clamp(r, w as f32 - r);
    let cy = py.clamp(r, h as f32 - r);
    let dx = px - cx;
    let dy = py - cy;
    let d = (dx * dx + dy * dy).sqrt();
    (r + 0.5 - d).clamp(0.0, 1.0)
}
