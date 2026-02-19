"""Generate icon.ico for project-switch — a blue square with white "PS" letters."""
import struct

SIZE = 32

def generate_rgba():
    """Generate 32x32 RGBA pixel data (top-to-bottom for reference)."""
    pixels = [[None] * SIZE for _ in range(SIZE)]

    for y in range(SIZE):
        for x in range(SIZE):
            # Blue background
            pixels[y][x] = (0x3B, 0x82, 0xF6, 0xFF)
            # Border
            if x == 0 or x == SIZE - 1 or y == 0 or y == SIZE - 1:
                pixels[y][x] = (0x1E, 0x40, 0xAF, 0xFF)

    white = (0xFF, 0xFF, 0xFF, 0xFF)

    def fill_rect(x0, y0, x1, y1):
        for y in range(y0, y1):
            for x in range(x0, x1):
                if 0 <= x < SIZE and 0 <= y < SIZE:
                    pixels[y][x] = white

    # Letter "P"
    fill_rect(4, 6, 7, 25)     # vertical stem
    fill_rect(7, 6, 13, 9)     # top bar
    fill_rect(11, 9, 14, 14)   # right bump
    fill_rect(7, 14, 13, 17)   # middle bar

    # Letter "S"
    fill_rect(18, 6, 27, 9)    # top bar
    fill_rect(18, 9, 21, 14)   # left upper
    fill_rect(18, 14, 27, 17)  # middle bar
    fill_rect(24, 17, 27, 22)  # right lower
    fill_rect(18, 22, 27, 25)  # bottom bar

    return pixels


def write_ico(path):
    pixels = generate_rgba()

    # Convert to BGRA, bottom-to-top for BMP
    pixel_data = bytearray()
    for y in range(SIZE - 1, -1, -1):
        for x in range(SIZE):
            r, g, b, a = pixels[y][x]
            pixel_data.extend([b, g, r, a])

    # AND mask (all zeros = fully opaque)
    and_mask = bytes(SIZE * SIZE // 8)

    image_size = len(pixel_data) + len(and_mask)
    bmp_header_size = 40

    # ICO header
    ico_header = struct.pack('<HHH', 0, 1, 1)

    # Directory entry
    dir_entry = struct.pack('<BBBBHHII',
        SIZE, SIZE, 0, 0,
        1, 32,
        bmp_header_size + image_size,
        6 + 16)  # offset = header + 1 dir entry

    # BMP info header
    bmp_header = struct.pack('<IiiHHIIiiII',
        bmp_header_size,
        SIZE,
        SIZE * 2,  # doubled for AND mask
        1, 32,
        0, image_size,
        0, 0,
        0, 0)

    with open(path, 'wb') as f:
        f.write(ico_header)
        f.write(dir_entry)
        f.write(bmp_header)
        f.write(pixel_data)
        f.write(and_mask)

    print(f"Wrote {path} ({6 + 16 + bmp_header_size + image_size} bytes)")


if __name__ == '__main__':
    write_ico('assets/icon.ico')
