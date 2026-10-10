//! Proportional pixel fonts drawn as ASCII art (`#` = ink), decoded at
//! compile time. All caps: lowercase renders as uppercase. Unknown
//! characters render as `?`.

pub struct Glyph {
    pub ch: char,
    pub w: u8,
    /// Row bitmasks, bit `w-1-x` set for ink at column `x`.
    pub rows: [u16; 8],
}

pub struct Font {
    pub height: u8,
    /// Blank columns between glyphs.
    pub spacing: u8,
    pub glyphs: &'static [Glyph],
}

pub const fn glyph(ch: char, rows: &[&str]) -> Glyph {
    assert!(rows.len() <= 8);
    let w = rows[0].len();
    assert!(w <= 16);
    let mut out = [0u16; 8];
    let mut y = 0;
    while y < rows.len() {
        let r = rows[y].as_bytes();
        assert!(r.len() == w, "glyph rows must have equal width");
        let mut x = 0;
        while x < w {
            if r[x] == b'#' {
                out[y] |= 1 << (w - 1 - x);
            } else {
                assert!(r[x] == b'.', "glyph pixels are '#' or '.'");
            }
            x += 1;
        }
        y += 1;
    }
    Glyph { ch, w: w as u8, rows: out }
}

impl Font {
    pub fn glyph(&self, ch: char) -> &Glyph {
        let ch = ch.to_ascii_uppercase();
        self.glyphs
            .iter()
            .find(|g| g.ch == ch)
            .or_else(|| self.glyphs.iter().find(|g| g.ch == '?'))
            .expect("every font defines '?'")
    }

    pub fn has(&self, ch: char) -> bool {
        let ch = ch.to_ascii_uppercase();
        self.glyphs.iter().any(|g| g.ch == ch)
    }

    pub fn width(&self, text: &str, scale: i32) -> i32 {
        let mut w = 0;
        let mut n = 0;
        for ch in text.chars() {
            w += self.glyph(ch).w as i32;
            n += 1;
        }
        if n == 0 {
            return 0;
        }
        (w + (n - 1) * self.spacing as i32) * scale
    }

    pub fn line_height(&self, scale: i32) -> i32 {
        (self.height as i32 + 2) * scale
    }
}
