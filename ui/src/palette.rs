//! Sweetie-16 by GrafxKid: a 16-color palette with good contrast on a dark
//! background. Every pixel on the canvas is an index into it.

pub const SWEETIE16: [u32; 16] = [
    0x1a1c2c, 0x5d275d, 0xb13e53, 0xef7d57, 0xffcd75, 0xa7f070, 0x38b764, 0x257179, 0x29366f, 0x3b5dc9, 0x41a6f6,
    0x73eff7, 0xf4f4f4, 0x94b0c2, 0x566c86, 0x333c57,
];

pub const BLACK: u8 = 0;
pub const PURPLE: u8 = 1;
pub const RED: u8 = 2;
pub const ORANGE: u8 = 3;
pub const YELLOW: u8 = 4;
pub const LIME: u8 = 5;
pub const GREEN: u8 = 6;
pub const TEAL: u8 = 7;
pub const NAVY: u8 = 8;
pub const BLUE: u8 = 9;
pub const SKY: u8 = 10;
pub const CYAN: u8 = 11;
pub const WHITE: u8 = 12;
pub const SILVER: u8 = 13;
pub const SLATE: u8 = 14;
pub const DARK: u8 = 15;

/// Marks a transparent sprite pixel.
pub const CLEAR: u8 = 0xFF;

// Roles, so screens speak in meaning rather than colors.
pub const BG: u8 = BLACK;
pub const PANEL: u8 = DARK;
pub const TEXT: u8 = WHITE;
pub const DIM: u8 = SILVER;
pub const MUTED: u8 = SLATE;
pub const YOM_TOV: u8 = YELLOW;
pub const SHABBAT: u8 = SKY;
pub const CHOL_HAMOED: u8 = GREEN;
pub const FAST: u8 = RED;
pub const ROSH_CHODESH: u8 = SILVER;
pub const MODERN: u8 = SKY;
pub const HOLIDAY: u8 = LIME;
pub const FLAME: u8 = ORANGE;

pub const fn rgb565(rgb: u32) -> u16 {
    let r = (rgb >> 16) & 0xff;
    let g = (rgb >> 8) & 0xff;
    let b = rgb & 0xff;
    (((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)) as u16
}

/// Palette index → RGB565 for the display driver.
pub const fn lut() -> [u16; 16] {
    let mut out = [0u16; 16];
    let mut i = 0;
    while i < 16 {
        out[i] = rgb565(SWEETIE16[i]);
        i += 1;
    }
    out
}

/// One-letter names used in ASCII-art sprites.
pub const fn from_letter(c: u8) -> u8 {
    match c {
        b'.' | b' ' => CLEAR,
        b'k' => BLACK,
        b'p' => PURPLE,
        b'r' => RED,
        b'o' => ORANGE,
        b'y' => YELLOW,
        b'l' => LIME,
        b'g' => GREEN,
        b't' => TEAL,
        b'n' => NAVY,
        b'b' => BLUE,
        b's' => SKY,
        b'c' => CYAN,
        b'w' => WHITE,
        b'h' => SILVER,
        b'm' => SLATE,
        b'd' => DARK,
        _ => panic!("unknown palette letter in sprite"),
    }
}
