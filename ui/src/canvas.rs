//! A 120×120 palette-indexed framebuffer: one art pixel = 2×2 panel pixels.

use crate::font::Font;
use crate::palette::CLEAR;
use crate::prelude::*;
use crate::sprite::Sprite;

pub const W: i32 = 120;
pub const H: i32 = 120;
pub const LEN: usize = (W * H) as usize;

/// 4×4 Bayer matrix, thresholds 0..16.
const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

pub struct Canvas {
    /// One palette index per byte, row-major.
    #[cfg(not(feature = "packed"))]
    pub px: Box<[u8; LEN]>,
    /// Two palette indices per byte (even x in the low nibble): 7.2 KB
    /// instead of 14.4 KB, for the ESP8266's heap.
    #[cfg(feature = "packed")]
    px: Box<[u8; BYTES]>,
}

#[cfg(not(feature = "packed"))]
const BYTES: usize = LEN;
#[cfg(feature = "packed")]
const BYTES: usize = LEN / 2;

/// Allocate straight on the heap: `Box::new([0; BYTES])` may build the
/// array on the stack first, and 14 KB overflows an ESP32 task stack.
fn heap_buf() -> Box<[u8; BYTES]> {
    vec![0u8; BYTES].into_boxed_slice().try_into().expect("BYTES bytes")
}

impl Clone for Canvas {
    fn clone(&self) -> Self {
        let mut px = heap_buf();
        px.copy_from_slice(&self.px[..]);
        Self { px }
    }
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}

impl Canvas {
    pub fn new() -> Self {
        Self { px: heap_buf() }
    }

    #[cfg(not(feature = "packed"))]
    #[inline]
    fn put(&mut self, i: usize, c: u8) {
        self.px[i] = c;
    }

    #[cfg(feature = "packed")]
    #[inline]
    fn put(&mut self, i: usize, c: u8) {
        let b = &mut self.px[i >> 1];
        *b = if i & 1 == 0 { (*b & 0xF0) | (c & 0x0F) } else { (*b & 0x0F) | (c << 4) };
    }

    #[cfg(not(feature = "packed"))]
    #[inline]
    fn at(&self, i: usize) -> u8 {
        self.px[i]
    }

    #[cfg(feature = "packed")]
    #[inline]
    fn at(&self, i: usize) -> u8 {
        let b = self.px[i >> 1];
        if i & 1 == 0 {
            b & 0x0F
        } else {
            b >> 4
        }
    }

    /// Fill pixels `i0..i1` (same row) with `c`.
    #[cfg(not(feature = "packed"))]
    fn span(&mut self, i0: usize, i1: usize, c: u8) {
        self.px[i0..i1].fill(c);
    }

    #[cfg(feature = "packed")]
    fn span(&mut self, mut i0: usize, i1: usize, c: u8) {
        if i0 < i1 && i0 & 1 == 1 {
            self.put(i0, c);
            i0 += 1;
        }
        let pairs = (i1 - i0.min(i1)) / 2;
        let c = c & 0x0F;
        self.px[i0 / 2..i0 / 2 + pairs].fill(c | c << 4);
        if i0 + pairs * 2 < i1 {
            self.put(i1 - 1, c);
        }
    }

    /// Palette indices of row `y`, for the display driver and screenshots.
    pub fn row(&self, y: usize, out: &mut [u8; W as usize]) {
        let base = y * W as usize;
        for (x, o) in out.iter_mut().enumerate() {
            *o = self.at(base + x);
        }
    }

    pub fn copy_from(&mut self, other: &Canvas) {
        self.px.copy_from_slice(&other.px[..]);
    }

    pub fn clear(&mut self, c: u8) {
        self.span(0, LEN, c);
    }

    #[inline]
    pub fn pset(&mut self, x: i32, y: i32, c: u8) {
        if (0..W).contains(&x) && (0..H).contains(&y) && c != CLEAR {
            self.put((y * W + x) as usize, c);
        }
    }

    pub fn get(&self, x: i32, y: i32) -> u8 {
        self.at((y * W + x) as usize)
    }

    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: u8) {
        let (x0, y0) = (x.max(0), y.max(0));
        let (x1, y1) = ((x + w).min(W), (y + h).min(H));
        // Also covers x0 >= W: the bottom row's slice would start past the end.
        if x0 >= x1 {
            return;
        }
        for yy in y0..y1 {
            let row = (yy * W) as usize;
            self.span(row + x0 as usize, row + x1 as usize, c);
        }
    }

    pub fn hline(&mut self, x: i32, y: i32, w: i32, c: u8) {
        self.fill_rect(x, y, w, 1, c);
    }

    pub fn vline(&mut self, x: i32, y: i32, h: i32, c: u8) {
        self.fill_rect(x, y, 1, h, c);
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: u8) {
        self.hline(x, y, w, c);
        self.hline(x, y + h - 1, w, c);
        self.vline(x, y, h, c);
        self.vline(x + w - 1, y, h, c);
    }

    /// A panel with "rounded" (notched) pixel corners.
    pub fn panel(&mut self, x: i32, y: i32, w: i32, h: i32, border: u8, fill: u8) {
        self.fill_rect(x + 1, y + 1, w - 2, h - 2, fill);
        self.hline(x + 1, y, w - 2, border);
        self.hline(x + 1, y + h - 1, w - 2, border);
        self.vline(x, y + 1, h - 2, border);
        self.vline(x + w - 1, y + 1, h - 2, border);
    }

    /// Ordered-dither between `a` and `b`; `level` 0 = all `a`, 16 = all `b`.
    #[allow(clippy::too_many_arguments)]
    pub fn dither_rect(&mut self, x: i32, y: i32, w: i32, h: i32, a: u8, b: u8, level: u8) {
        for yy in y.max(0)..(y + h).min(H) {
            for xx in x.max(0)..(x + w).min(W) {
                let t = BAYER[(yy & 3) as usize][(xx & 3) as usize];
                self.put((yy * W + xx) as usize, if t < level { b } else { a });
            }
        }
    }

    /// Vertical gradient through `stops`, dithered between neighbors.
    pub fn gradient(&mut self, x: i32, y: i32, w: i32, h: i32, stops: &[u8]) {
        if stops.len() == 1 || h <= 1 {
            self.fill_rect(x, y, w, h, stops[0]);
            return;
        }
        let bands = (stops.len() - 1) as i32;
        for row in 0..h {
            // Position in 1/16 steps along the whole gradient.
            let pos = row * bands * 16 / (h - 1).max(1);
            let band = (pos / 16).min(bands - 1);
            let level = (pos - band * 16) as u8;
            self.dither_rect(x, y + row, w, 1, stops[band as usize], stops[band as usize + 1], level);
        }
    }

    pub fn blit(&mut self, s: &Sprite, x: i32, y: i32) {
        for sy in 0..s.h as i32 {
            for sx in 0..s.w as i32 {
                self.pset(x + sx, y + sy, s.px[(sy * s.w as i32 + sx) as usize]);
            }
        }
    }

    /// Blit with colors swapped through `map` (e.g. a white cloud drawn grey).
    pub fn blit_mapped(&mut self, s: &Sprite, x: i32, y: i32, map: &[(u8, u8)]) {
        for sy in 0..s.h as i32 {
            for sx in 0..s.w as i32 {
                let p = s.px[(sy * s.w as i32 + sx) as usize];
                let p = map.iter().find(|(from, _)| *from == p).map_or(p, |(_, to)| *to);
                self.pset(x + sx, y + sy, p);
            }
        }
    }

    /// Blit with every opaque pixel painted `c` (silhouettes, shadows).
    pub fn blit_tinted(&mut self, s: &Sprite, x: i32, y: i32, c: u8) {
        for sy in 0..s.h as i32 {
            for sx in 0..s.w as i32 {
                if s.px[(sy * s.w as i32 + sx) as usize] != CLEAR {
                    self.pset(x + sx, y + sy, c);
                }
            }
        }
    }

    /// Draw `text` with its top-left at (x, y); returns the advance width.
    pub fn text(&mut self, font: &Font, scale: i32, x: i32, y: i32, text: &str, c: u8) -> i32 {
        let mut cx = x;
        for ch in text.chars() {
            let g = font.glyph(ch);
            for (row, bits) in g.rows[..font.height as usize].iter().enumerate() {
                for col in 0..g.w as i32 {
                    if bits & (1 << (g.w as i32 - 1 - col)) != 0 {
                        self.fill_rect(cx + col * scale, y + row as i32 * scale, scale, scale, c);
                    }
                }
            }
            cx += (g.w as i32 + font.spacing as i32) * scale;
        }
        (cx - x - font.spacing as i32 * scale).max(0)
    }

    /// Text with a 1-pixel drop shadow, for legibility over skies.
    #[allow(clippy::too_many_arguments)]
    pub fn text_shadow(&mut self, font: &Font, scale: i32, x: i32, y: i32, text: &str, c: u8, shadow: u8) -> i32 {
        self.text(font, scale, x + 1, y + 1, text, shadow);
        self.text(font, scale, x, y, text, c)
    }

    pub fn text_center(&mut self, font: &Font, scale: i32, cx: i32, y: i32, text: &str, c: u8) {
        let w = font.width(text, scale);
        self.text(font, scale, cx - w / 2, y, text, c);
    }

    pub fn text_right(&mut self, font: &Font, scale: i32, right: i32, y: i32, text: &str, c: u8) {
        let w = font.width(text, scale);
        self.text(font, scale, right - w, y, text, c);
    }

    /// Bounding box `(x, y, w, h)` of pixels that differ from `other`.
    pub fn diff(&self, other: &Canvas) -> Option<(usize, usize, usize, usize)> {
        let (w, bytes_per_row) = (W as usize, BYTES / H as usize);
        let (mut x0, mut y0, mut x1, mut y1) = (w, H as usize, 0, 0);
        for y in 0..H as usize {
            let row = y * bytes_per_row..(y + 1) * bytes_per_row;
            if self.px[row.clone()] == other.px[row] {
                continue;
            }
            let differs = |x: &usize| self.at(y * w + x) != other.at(y * w + x);
            let first = (0..w).position(|x| differs(&x)).unwrap_or(0);
            let last = (0..w).rposition(|x| differs(&x)).unwrap_or(0);
            x0 = x0.min(first);
            x1 = x1.max(last);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
        (y0 <= y1 && x0 <= x1).then(|| (x0, y0, x1 - x0 + 1, y1 - y0 + 1))
    }

    /// Ordered-dither cross-fade: pixels whose Bayer threshold is below
    /// `step` (0..=16) come from `to`, the rest from `from`.
    pub fn dissolve(&mut self, from: &Canvas, to: &Canvas, step: u8) {
        for y in 0..H {
            for x in 0..W {
                let i = (y * W + x) as usize;
                let t = BAYER[(y & 3) as usize][(x & 3) as usize];
                self.put(i, if t < step { to.at(i) } else { from.at(i) });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Random primitives on a canvas and on a plain byte array must agree,
    /// whichever storage the canvas uses (`cargo test --features packed`).
    #[test]
    fn storage_matches_a_plain_array() {
        let mut seed = 0x2545_f491_u32;
        let mut rnd = |n: i32| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed % n as u32) as i32
        };
        let mut c = Canvas::new();
        let mut want = vec![0u8; LEN];
        for step in 0..3000 {
            let (x, y) = (rnd(W + 20) - 10, rnd(H + 20) - 10);
            let (w, h, color) = (rnd(40), rnd(40), rnd(16) as u8);
            match step % 4 {
                0 => {
                    c.fill_rect(x, y, w, h, color);
                    for yy in y.max(0)..(y + h).min(H) {
                        for xx in x.max(0)..(x + w).min(W) {
                            want[(yy * W + xx) as usize] = color;
                        }
                    }
                }
                1 => {
                    c.pset(x, y, color);
                    if (0..W).contains(&x) && (0..H).contains(&y) {
                        want[(y * W + x) as usize] = color;
                    }
                }
                2 => {
                    let (b, level) = (rnd(16) as u8, rnd(17) as u8);
                    c.dither_rect(x, y, w, h, color, b, level);
                    for yy in y.max(0)..(y + h).min(H) {
                        for xx in x.max(0)..(x + w).min(W) {
                            let t = BAYER[(yy & 3) as usize][(xx & 3) as usize];
                            want[(yy * W + xx) as usize] = if t < level { b } else { color };
                        }
                    }
                }
                _ => {
                    let before = c.clone();
                    c.hline(x, y, w, color);
                    for xx in x.max(0)..(x + w).min(W) {
                        if (0..H).contains(&y) {
                            want[(y * W + xx) as usize] = color;
                        }
                    }
                    let d = c.diff(&before);
                    if let Some((dx, dy, dw, dh)) = d {
                        assert_eq!((dy, dh), (y as usize, 1));
                        assert!(dx as i32 >= x && (dx + dw) as i32 <= x + w);
                    }
                }
            }
        }
        let mut row = [0u8; W as usize];
        for y in 0..H {
            c.row(y as usize, &mut row);
            assert_eq!(&row[..], &want[(y * W) as usize..((y + 1) * W) as usize], "row {y}");
            for x in 0..W {
                assert_eq!(c.get(x, y), want[(y * W + x) as usize]);
            }
        }
        c.clear(9);
        assert!((0..H).all(|y| (0..W).all(|x| c.get(x, y) == 9)));
        assert_eq!(c.diff(&c.clone()), None);
    }
}
