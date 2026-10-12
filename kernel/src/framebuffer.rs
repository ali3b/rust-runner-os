//! Presents the 640x480 indexed back buffer on the bootloader's linear framebuffer,
//! honouring its real size, stride, bytes-per-pixel and pixel format.
//!
//! The playfield is scaled with nearest-neighbour sampling to the largest 4:3 area
//! that fits, centred with black borders. That is an exact integer scale when the
//! mode allows (e.g. 1280x960 -> 2x); on the 1280x720 mode the BIOS bootloader
//! picks it is 1.5x, where every 2-pixel unit of the art maps to exactly 3 pixels.
//!
//! Optional CRT scanlines darken every output row that repeats a source row (every
//! third row at 1.5x, every other row at 2x), using a second, dimmed palette LUT so
//! the effect costs nothing per pixel.
use bootloader_api::info::{FrameBuffer, FrameBufferInfo, PixelFormat};
use pc::Screen;
use runner_core::engine::graphics::{Color, HEIGHT, Palette, WIDTH};

/// Widest output row we support (pixels).
const MAX_OUT_WIDTH: usize = 4096;
/// Scanline brightness, out of 256.
const SCANLINE_LEVEL: u32 = 170;

pub struct Display {
    buf: &'static mut [u8],
    info: FrameBufferInfo,
    out_w: usize,
    out_h: usize,
    /// Byte offset of the playfield's top-left corner.
    origin: usize,
    /// Source column for each output column.
    x_map: [u16; MAX_OUT_WIDTH],
    /// Each palette entry pre-encoded in the framebuffer's pixel format: normal
    /// and scanline-dimmed.
    lut: [[u8; 4]; 256],
    dim_lut: [[u8; 4]; 256],
}

impl Display {
    pub fn new(fb: FrameBuffer) -> Result<Self, &'static str> {
        let info = fb.info();
        if info.width < WIDTH || info.height < HEIGHT {
            return Err("smaller than the 640x480 playfield");
        }
        if !(1..=4).contains(&info.bytes_per_pixel) {
            return Err("unsupported bytes per pixel");
        }
        if info.stride < info.width || info.byte_len < info.stride * info.height * info.bytes_per_pixel {
            return Err("stride/length inconsistent with dimensions");
        }

        // Largest 4:3 area that fits.
        let (mut out_w, mut out_h) = (info.height * WIDTH / HEIGHT, info.height);
        if out_w > info.width {
            (out_w, out_h) = (info.width, info.width * HEIGHT / WIDTH);
        }
        out_w = out_w.min(MAX_OUT_WIDTH);
        let ox = (info.width - out_w) / 2;
        let oy = (info.height - out_h) / 2;
        let origin = (oy * info.stride + ox) * info.bytes_per_pixel;

        let mut x_map = [0u16; MAX_OUT_WIDTH];
        for (x, src) in x_map.iter_mut().take(out_w).enumerate() {
            *src = (x * WIDTH / out_w) as u16;
        }

        let buf = fb.into_buffer();
        buf.fill(0); // borders stay black
        Ok(Display { buf, info, out_w, out_h, origin, x_map, lut: [[0; 4]; 256], dim_lut: [[0; 4]; 256] })
    }

    pub fn output_size(&self) -> (usize, usize) {
        (self.out_w, self.out_h)
    }

    fn encode(&self, r: u32, g: u32, b: u32) -> [u8; 4] {
        let px = match self.info.pixel_format {
            PixelFormat::Rgb => r | g << 8 | b << 16,
            PixelFormat::Bgr => b | g << 8 | r << 16,
            PixelFormat::U8 => (r * 77 + g * 150 + b * 29) >> 8,
            PixelFormat::Unknown { red_position, green_position, blue_position } => {
                r << red_position | g << green_position | b << blue_position
            }
            _ => b | g << 8 | r << 16,
        };
        px.to_le_bytes()
    }

}

impl Screen for Display {
    /// Re-encodes the palette (cheap: 512 entries).
    fn set_palette(&mut self, palette: &Palette) {
        for (i, c) in palette.iter().enumerate() {
            let (r, g, b) = (c.r as u32, c.g as u32, c.b as u32);
            self.lut[i] = self.encode(r, g, b);
            let dim = |v: u32| (v * SCANLINE_LEVEL) >> 8;
            self.dim_lut[i] = self.encode(dim(r), dim(g), dim(b));
        }
    }

    /// Copies a full `WIDTH * HEIGHT` indexed frame to the screen.
    fn present(&mut self, frame: &[Color], crt: bool) {
        self.present_frame(frame, crt);
    }
}

impl Display {
    fn stage(&self, line: &mut [u8], src: &[Color], dim: bool) {
        let lut = if dim { &self.dim_lut } else { &self.lut };
        let x_map = &self.x_map[..self.out_w];
        // Fixed-size paths for the common formats: a per-pixel `copy_from_slice`
        // with a runtime length becomes a memcpy call.
        match self.info.bytes_per_pixel {
            4 => {
                for (dst, &sx) in line.as_chunks_mut::<4>().0.iter_mut().zip(x_map) {
                    *dst = lut[src[sx as usize] as usize];
                }
            }
            3 => {
                for (dst, &sx) in line.as_chunks_mut::<3>().0.iter_mut().zip(x_map) {
                    let px = lut[src[sx as usize] as usize];
                    *dst = [px[0], px[1], px[2]];
                }
            }
            bpp => {
                for (dst, &sx) in line.chunks_exact_mut(bpp).zip(x_map) {
                    dst.copy_from_slice(&lut[src[sx as usize] as usize][..bpp]);
                }
            }
        }
    }

    fn present_frame(&mut self, frame: &[Color], crt: bool) {
        let bpp = self.info.bytes_per_pixel;
        let pitch = self.info.stride * bpp;
        let row_bytes = self.out_w * bpp;
        let mut normal = [0u8; MAX_OUT_WIDTH * 4];
        let mut dimmed = [0u8; MAX_OUT_WIDTH * 4];
        let (mut normal_src, mut dimmed_src) = (usize::MAX, usize::MAX);
        let mut prev_src = usize::MAX;

        for y in 0..self.out_h {
            let src_y = y * HEIGHT / self.out_h;
            let repeat = src_y == prev_src || (self.out_h == HEIGHT && y % 2 == 1);
            prev_src = src_y;
            let src = &frame[src_y * WIDTH..][..WIDTH];
            let line: &[u8] = if crt && repeat {
                if dimmed_src != src_y {
                    self.stage(&mut dimmed[..row_bytes], src, true);
                    dimmed_src = src_y;
                }
                &dimmed[..row_bytes]
            } else {
                if normal_src != src_y {
                    self.stage(&mut normal[..row_bytes], src, false);
                    normal_src = src_y;
                }
                &normal[..row_bytes]
            };
            let start = self.origin + y * pitch;
            self.buf[start..start + row_bytes].copy_from_slice(line);
        }
    }
}
