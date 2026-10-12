//! 8x8 bitmap text using the public-domain `font8x8` glyphs.
use super::graphics::{Canvas, Color};
use font8x8::legacy::BASIC_LEGACY;

pub const GLYPH: i32 = 8;

/// Width in pixels of `text` drawn at `scale`.
pub fn text_width(text: &str, scale: i32) -> i32 {
    text.chars().count() as i32 * GLYPH * scale
}

pub fn draw_text(canvas: &mut Canvas<'_>, text: &str, x: i32, y: i32, scale: i32, color: Color) {
    let mut cx = x;
    for ch in text.chars() {
        let glyph = BASIC_LEGACY.get(ch as usize).unwrap_or(&BASIC_LEGACY[b'?' as usize]);
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                if bits & (1 << col) != 0 {
                    canvas.fill_rect(cx + col * scale, y + row as i32 * scale, scale, scale, color);
                }
            }
        }
        cx += GLYPH * scale;
    }
}

/// Draws `text` horizontally centered on the playfield.
pub fn draw_text_centered(canvas: &mut Canvas<'_>, text: &str, y: i32, scale: i32, color: Color) {
    let x = (super::graphics::WIDTH as i32 - text_width(text, scale)) / 2;
    draw_text(canvas, text, x, y, scale, color);
}

/// Formats `value` as zero-padded decimal into `buf` and returns it as `&str`.
pub fn format_u32(value: u32, digits: usize, buf: &mut [u8; 10]) -> &str {
    let mut v = value;
    let mut len = 0;
    let mut tmp = [0u8; 10];
    loop {
        tmp[len] = b'0' + (v % 10) as u8;
        len += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    while len < digits.min(10) {
        tmp[len] = b'0';
        len += 1;
    }
    for i in 0..len {
        buf[i] = tmp[len - 1 - i];
    }
    core::str::from_utf8(&buf[..len]).unwrap_or("?")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::graphics::{HEIGHT, WIDTH};

    #[test]
    fn formats_numbers() {
        let mut b = [0; 10];
        assert_eq!(format_u32(0, 1, &mut b), "0");
        assert_eq!(format_u32(42, 6, &mut b), "000042");
        assert_eq!(format_u32(u32::MAX, 0, &mut b), "4294967295");
    }

    #[test]
    fn text_draws_pixels_inside_its_box() {
        let mut px = vec![0; WIDTH * HEIGHT];
        let mut c = Canvas::new(&mut px);
        draw_text(&mut c, "A", 10, 10, 2, 3);
        let lit: Vec<usize> = px.iter().enumerate().filter(|(_, p)| **p == 3).map(|(i, _)| i).collect();
        assert!(!lit.is_empty());
        for i in lit {
            let (x, y) = (i % WIDTH, i / WIDTH);
            assert!((10..26).contains(&x) && (10..26).contains(&y));
        }
        assert_eq!(text_width("ABC", 2), 48);
    }
}
