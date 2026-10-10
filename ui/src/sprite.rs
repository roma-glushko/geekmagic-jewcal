//! Sprites written as ASCII art, one palette letter per pixel (see
//! `palette::from_letter`), decoded at compile time. A row of the wrong width
//! or an unknown letter is a compile error.

use crate::palette::from_letter;

pub struct Sprite {
    pub w: u8,
    pub h: u8,
    pub px: &'static [u8],
}

pub const fn decode<const N: usize>(w: usize, rows: &[&str]) -> [u8; N] {
    assert!(w * rows.len() == N, "sprite size mismatch");
    let mut out = [0u8; N];
    let mut y = 0;
    while y < rows.len() {
        let row = rows[y].as_bytes();
        assert!(row.len() == w, "sprite row has the wrong width");
        let mut x = 0;
        while x < w {
            out[y * w + x] = from_letter(row[x]);
            x += 1;
        }
        y += 1;
    }
    out
}

/// `sprite!(W, H, ["row", …])` → `Sprite`.
#[macro_export]
macro_rules! sprite {
    ($w:expr, $h:expr, [$($row:expr),* $(,)?]) => {{
        const PX: [u8; $w * $h] = $crate::sprite::decode::<{ $w * $h }>($w, &[$($row),*]);
        $crate::sprite::Sprite { w: $w, h: $h, px: &PX }
    }};
}
