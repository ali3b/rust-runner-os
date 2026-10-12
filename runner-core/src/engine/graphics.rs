/// Logical playfield size. The platform layer scales this to the real framebuffer.
pub const WIDTH: usize = 640;
pub const HEIGHT: usize = 480;

/// Palette index. Index 0 is treated as "transparent" by sprite blits.
pub type Color = u8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn hex(v: u32) -> Self {
        Rgb { r: (v >> 16) as u8, g: (v >> 8) as u8, b: v as u8 }
    }
}

pub type Palette = [Rgb; 256];

/// A sprite stored as ASCII-art rows; [`Canvas::blit_sprite`] maps each byte to a color.
pub struct Sprite {
    pub rows: &'static [&'static [u8]],
}

impl Sprite {
    pub fn width(&self) -> usize {
        self.rows.first().map_or(0, |r| r.len())
    }

    pub fn height(&self) -> usize {
        self.rows.len()
    }
}

/// An 8-bit indexed drawing surface of `WIDTH` x `HEIGHT`. All drawing is clipped.
pub struct Canvas<'a> {
    px: &'a mut [Color],
    /// Camera offset added to every draw (screen shake).
    dx: i32,
    dy: i32,
}

impl<'a> Canvas<'a> {
    /// `px` must hold exactly `WIDTH * HEIGHT` entries.
    pub fn new(px: &'a mut [Color]) -> Self {
        assert_eq!(px.len(), WIDTH * HEIGHT, "canvas buffer has wrong size");
        Self { px, dx: 0, dy: 0 }
    }

    pub fn pixels(&self) -> &[Color] {
        self.px
    }

    pub fn get(&self, x: i32, y: i32) -> Option<Color> {
        if x < 0 || y < 0 || x as usize >= WIDTH || y as usize >= HEIGHT {
            return None;
        }
        Some(self.px[y as usize * WIDTH + x as usize])
    }

    pub fn clear(&mut self, c: Color) {
        self.px.fill(c);
    }

    /// Shifts all following drawing by `(dx, dy)`; `(0, 0)` restores it.
    pub fn set_offset(&mut self, dx: i32, dy: i32) {
        self.dx = dx;
        self.dy = dy;
    }

    pub fn set(&mut self, x: i32, y: i32, c: Color) {
        let (x, y) = (x + self.dx, y + self.dy);
        if x >= 0 && y >= 0 && (x as usize) < WIDTH && (y as usize) < HEIGHT {
            self.px[y as usize * WIDTH + x as usize] = c;
        }
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        let (x, y) = (x.saturating_add(self.dx), y.saturating_add(self.dy));
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = x.saturating_add(w).min(WIDTH as i32);
        let y1 = y.saturating_add(h).min(HEIGHT as i32);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for row in y0..y1 {
            let base = row as usize * WIDTH;
            self.px[base + x0 as usize..base + x1 as usize].fill(c);
        }
    }

    /// Filled circle of radius `r` centred on `(cx, cy)`.
    pub fn fill_circle(&mut self, cx: i32, cy: i32, r: i32, c: Color) {
        for dy in -r..=r {
            let half = isqrt((r * r - dy * dy) as u32) as i32;
            self.fill_rect(cx - half, cy + dy, 2 * half + 1, 1, c);
        }
    }

    /// 1-pixel-thick rectangle outline.
    pub fn rect_outline(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        if w <= 0 || h <= 0 {
            return;
        }
        self.fill_rect(x, y, w, 1, c);
        self.fill_rect(x, y + h - 1, w, 1, c);
        self.fill_rect(x, y, 1, h, c);
        self.fill_rect(x + w - 1, y, 1, h, c);
    }

    /// Draws `sprite` with its top-left at `(x, y)`, each source pixel as a `scale`² block.
    /// `map` turns each byte into a palette index; 0 leaves the pixel untouched.
    pub fn blit_sprite(&mut self, sprite: &Sprite, x: i32, y: i32, scale: i32, map: impl Fn(u8) -> Color) {
        for (sy, row) in sprite.rows.iter().enumerate() {
            for (sx, &b) in row.iter().enumerate() {
                let c = map(b);
                if c != 0 {
                    self.fill_rect(x + sx as i32 * scale, y + sy as i32 * scale, scale, scale, c);
                }
            }
        }
    }
}

/// Integer square root (floor).
pub fn isqrt(n: u32) -> u32 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Floor for `f32` without `std` (positions can be negative while scrolling off-screen).
pub fn floor_i32(v: f32) -> i32 {
    let i = v as i32;
    if (i as f32) > v { i - 1 } else { i }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf() -> Vec<Color> {
        vec![0; WIDTH * HEIGHT]
    }

    #[test]
    fn fill_rect_clips_to_bounds() {
        let mut b = buf();
        let mut c = Canvas::new(&mut b);
        c.fill_rect(-10, -10, 20, 20, 7);
        c.fill_rect(WIDTH as i32 - 5, HEIGHT as i32 - 5, 100, 100, 9);
        c.fill_rect(i32::MAX - 1, 0, i32::MAX, 10, 3);
        assert_eq!(c.get(0, 0), Some(7));
        assert_eq!(c.get(9, 9), Some(7));
        assert_eq!(c.get(10, 10), Some(0));
        assert_eq!(c.get(WIDTH as i32 - 1, HEIGHT as i32 - 1), Some(9));
        assert_eq!(b.iter().filter(|&&p| p == 7).count(), 100);
        assert_eq!(b.iter().filter(|&&p| p == 9).count(), 25);
    }

    #[test]
    fn empty_and_offscreen_rects_draw_nothing() {
        let mut b = buf();
        let mut c = Canvas::new(&mut b);
        c.fill_rect(10, 10, 0, 5, 1);
        c.fill_rect(10, 10, -5, 5, 1);
        c.fill_rect(WIDTH as i32, 0, 10, 10, 1);
        c.fill_rect(0, -50, 10, 10, 1);
        c.set(-1, 0, 1);
        c.set(0, HEIGHT as i32, 1);
        assert!(b.iter().all(|&p| p == 0));
    }

    #[test]
    fn sprite_skips_transparent_and_scales() {
        static S: Sprite = Sprite { rows: &[b"#.", b".#"] };
        let mut b = buf();
        let mut c = Canvas::new(&mut b);
        c.blit_sprite(&S, 0, 0, 2, |ch| if ch == b'#' { 5 } else { 0 });
        assert_eq!(c.get(1, 1), Some(5));
        assert_eq!(c.get(2, 0), Some(0));
        assert_eq!(c.get(3, 3), Some(5));
        assert_eq!(S.width(), 2);
        assert_eq!(S.height(), 2);
    }

    #[test]
    fn offset_shifts_drawing() {
        let mut b = buf();
        let mut c = Canvas::new(&mut b);
        c.set_offset(5, 3);
        c.fill_rect(0, 0, 1, 1, 9);
        c.set_offset(0, 0);
        assert_eq!(c.get(5, 3), Some(9));
        assert_eq!(c.get(0, 0), Some(0));
    }

    #[test]
    fn isqrt_and_circle() {
        for n in [0u32, 1, 2, 3, 4, 15, 16, 17, 99, 100, 1_000_000] {
            let r = isqrt(n);
            assert!(r * r <= n && (r + 1) * (r + 1) > n, "{n}");
        }
        let mut b = buf();
        let mut c = Canvas::new(&mut b);
        c.fill_circle(50, 50, 10, 4);
        assert_eq!(c.get(50, 50), Some(4));
        assert_eq!(c.get(50, 40), Some(4));
        assert_eq!(c.get(58, 58), Some(0), "corner of the bounding box is outside");
    }

    #[test]
    fn floor_handles_negatives() {
        assert_eq!(floor_i32(1.7), 1);
        assert_eq!(floor_i32(-0.5), -1);
        assert_eq!(floor_i32(-2.0), -2);
    }
}
