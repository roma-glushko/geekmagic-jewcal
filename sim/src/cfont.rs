//! `gen-cfont`: the BODY font as a C header for the SmallTV Ultra's C++ side
//! (installer and safe-mode screens, which run without the Rust core).

use std::fmt::Write as _;
use std::path::Path;

use jewcal_ui::fonts::BODY;

pub const OUT: &str = "ultra/src/font_body.h";

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut h = String::new();
    h.push_str(
        "// The BODY pixel font (5x7, proportional), generated from ui/src/fonts.rs\n\
         // by `cargo run -p jewcal-sim -- gen-cfont`. Do not edit.\n\
         #pragma once\n\
         #include <pgmspace.h>\n\
         #include <stdint.h>\n\n\
         // rows[y] bit (w-1-x) is ink at column x.\n\
         struct CGlyph {\n  char ch;\n  uint8_t w;\n  uint8_t rows[7];\n};\n\n",
    );
    let _ = writeln!(h, "constexpr uint8_t FONT_BODY_HEIGHT = {};", BODY.height);
    let _ = writeln!(h, "constexpr uint8_t FONT_BODY_SPACING = {};", BODY.spacing);
    h.push_str("static const CGlyph FONT_BODY[] PROGMEM = {\n");
    let mut n = 0;
    for g in BODY.glyphs {
        // ASCII only: the C side draws plain bytes.
        if !g.ch.is_ascii() || g.w > 8 {
            continue;
        }
        let ch = match g.ch {
            '\'' => "'\\''".to_string(),
            '\\' => "'\\\\'".to_string(),
            c => format!("'{c}'"),
        };
        let rows: Vec<String> = g.rows[..BODY.height as usize].iter().map(|r| format!("0x{r:02X}")).collect();
        let _ = writeln!(h, "  {{{ch}, {}, {{{}}}}},", g.w, rows.join(", "));
        n += 1;
    }
    h.push_str("};\n");
    let _ = writeln!(h, "constexpr uint8_t FONT_BODY_COUNT = {n};");
    std::fs::write(Path::new(OUT), h)?;
    println!("wrote {OUT} ({n} glyphs)");
    Ok(())
}
