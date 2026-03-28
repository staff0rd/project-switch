//! Procedurally generated tray icon (32x32 RGBA).

#[cfg(any(windows, target_os = "macos"))]
pub fn create_icon_rgba() -> (Vec<u8>, u32, u32) {
    const SIZE: u32 = 32;
    let mut pixels = vec![0u8; (SIZE * SIZE * 4) as usize];

    let bg = [59u8, 130, 246, 255]; // #3B82F6
    let border = [30u8, 64, 175, 255]; // #1E40AF
    let white = [255u8, 255, 255, 255];

    for y in 0..SIZE {
        for x in 0..SIZE {
            let idx = ((y * SIZE + x) * 4) as usize;
            let is_border = x == 0 || x == SIZE - 1 || y == 0 || y == SIZE - 1;
            let color = if is_border { &border } else { &bg };
            pixels[idx..idx + 4].copy_from_slice(color);
        }
    }

    // Draw "P" (columns 6-13, rows 8-23)
    let p_pixels: &[(u32, u32)] = &[
        // Vertical stroke
        (7, 8),
        (7, 9),
        (7, 10),
        (7, 11),
        (7, 12),
        (7, 13),
        (7, 14),
        (7, 15),
        (7, 16),
        (7, 17),
        (7, 18),
        (7, 19),
        (7, 20),
        (7, 21),
        (7, 22),
        (7, 23),
        (8, 8),
        (8, 9),
        (8, 10),
        (8, 11),
        (8, 12),
        (8, 13),
        (8, 14),
        (8, 15),
        (8, 16),
        (8, 17),
        (8, 18),
        (8, 19),
        (8, 20),
        (8, 21),
        (8, 22),
        (8, 23),
        // Top horizontal
        (9, 8),
        (10, 8),
        (11, 8),
        (12, 8),
        (9, 9),
        (10, 9),
        (11, 9),
        (12, 9),
        // Right of bump
        (13, 10),
        (13, 11),
        (13, 12),
        (13, 13),
        (13, 14),
        // Middle horizontal
        (9, 15),
        (10, 15),
        (11, 15),
        (12, 15),
        (9, 16),
        (10, 16),
        (11, 16),
        (12, 16),
    ];

    // Draw "S" (columns 16-24, rows 8-23)
    let s_pixels: &[(u32, u32)] = &[
        // Top horizontal
        (18, 8),
        (19, 8),
        (20, 8),
        (21, 8),
        (22, 8),
        (18, 9),
        (19, 9),
        (20, 9),
        (21, 9),
        (22, 9),
        // Left top vertical
        (17, 10),
        (17, 11),
        (17, 12),
        (17, 13),
        (18, 10),
        (18, 11),
        (18, 12),
        (18, 13),
        // Middle horizontal
        (19, 14),
        (20, 14),
        (21, 14),
        (22, 14),
        (19, 15),
        (20, 15),
        (21, 15),
        (22, 15),
        // Right bottom vertical
        (23, 16),
        (23, 17),
        (23, 18),
        (23, 19),
        (23, 20),
        (24, 16),
        (24, 17),
        (24, 18),
        (24, 19),
        (24, 20),
        // Bottom horizontal
        (18, 21),
        (19, 21),
        (20, 21),
        (21, 21),
        (22, 21),
        (18, 22),
        (19, 22),
        (20, 22),
        (21, 22),
        (22, 22),
    ];

    for &(x, y) in p_pixels.iter().chain(s_pixels.iter()) {
        if x < SIZE && y < SIZE {
            let idx = ((y * SIZE + x) * 4) as usize;
            pixels[idx..idx + 4].copy_from_slice(&white);
        }
    }

    (pixels, SIZE, SIZE)
}
