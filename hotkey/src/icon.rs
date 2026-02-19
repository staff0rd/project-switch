use tray_icon::Icon;

const SIZE: u32 = 32;

/// Creates the tray icon — a blue "PS" square matching the exe icon.
pub fn create_tray_icon() -> Icon {
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];

    fn fill_rect(rgba: &mut [u8], x0: u32, y0: u32, x1: u32, y1: u32, color: [u8; 4]) {
        for y in y0..y1 {
            for x in x0..x1 {
                if x < SIZE && y < SIZE {
                    let i = ((y * SIZE + x) * 4) as usize;
                    rgba[i..i + 4].copy_from_slice(&color);
                }
            }
        }
    }

    let blue = [0x3B, 0x82, 0xF6, 0xFF];
    let border = [0x1E, 0x40, 0xAF, 0xFF];
    let white = [0xFF, 0xFF, 0xFF, 0xFF];

    // Blue background
    fill_rect(&mut rgba, 0, 0, SIZE, SIZE, blue);

    // Border
    fill_rect(&mut rgba, 0, 0, SIZE, 1, border);
    fill_rect(&mut rgba, 0, SIZE - 1, SIZE, SIZE, border);
    fill_rect(&mut rgba, 0, 0, 1, SIZE, border);
    fill_rect(&mut rgba, SIZE - 1, 0, SIZE, SIZE, border);

    // Letter "P"
    fill_rect(&mut rgba, 4, 6, 7, 25, white);
    fill_rect(&mut rgba, 7, 6, 13, 9, white);
    fill_rect(&mut rgba, 11, 9, 14, 14, white);
    fill_rect(&mut rgba, 7, 14, 13, 17, white);

    // Letter "S"
    fill_rect(&mut rgba, 18, 6, 27, 9, white);
    fill_rect(&mut rgba, 18, 9, 21, 14, white);
    fill_rect(&mut rgba, 18, 14, 27, 17, white);
    fill_rect(&mut rgba, 24, 17, 27, 22, white);
    fill_rect(&mut rgba, 18, 22, 27, 25, white);

    Icon::from_rgba(rgba, SIZE, SIZE).expect("Failed to create tray icon")
}
