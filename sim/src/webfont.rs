//! The BODY pixel font as a TrueType font, so the user guide's headings use
//! the device's own letters. Each ink pixel is a square of `PX` font units.
//! Outlines are traced around the ink, so contours never overlap and holes
//! wind the other way, as TrueType wants. Lowercase letters map to the
//! capitals, as on the device.

use std::collections::BTreeMap;
use std::path::Path;

use jewcal_ui::fonts::BODY;

/// Font units per pixel. There are 8 pixels to the em, so text at 16 px
/// draws 2×2 px squares, at 24 px 3×3, and at 32 px 4×4.
const PX: i32 = 128;
const UPM: i32 = 8 * PX;
/// One blank pixel above the capitals and one below the baseline.
const ASCENT: i32 = 8 * PX;
const DESCENT: i32 = PX;
const FAMILY: &str = "JewCal Pixel";
/// 2026-01-01 in seconds since 1904, for reproducible files.
const DATE: u64 = 3_850_070_400;

struct Glyph {
    advance: i32,
    contours: Vec<Vec<(i32, i32)>>,
}

pub fn write(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let h = BODY.height as usize;
    let mut glyphs = vec![Glyph { advance: 4 * PX, contours: Vec::new() }]; // .notdef
    let mut cmap = BTreeMap::new();
    for g in BODY.glyphs {
        let gid = glyphs.len() as u16;
        glyphs.push(Glyph { advance: (g.w + BODY.spacing) as i32 * PX, contours: trace(&g.rows[..h], g.w as i32) });
        cmap.insert(g.ch as u32, gid);
        if g.ch.is_ascii_uppercase() {
            cmap.insert(g.ch.to_ascii_lowercase() as u32, gid);
        }
    }
    for (alias, of) in [('\u{A0}', ' '), ('\u{2013}', '-'), ('\u{2019}', '\'')] {
        if let Some(&gid) = cmap.get(&(of as u32)) {
            cmap.insert(alias as u32, gid);
        }
    }

    // glyf, loca, hmtx, and the totals the other tables report.
    let (mut glyf, mut loca, mut hmtx) = (Vec::new(), Vec::new(), Vec::new());
    let (mut x_min, mut y_min, mut x_max, mut y_max) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    let (mut max_points, mut max_contours) = (0, 0);
    let (mut min_lsb, mut min_rsb, mut max_extent) = (i32::MAX, i32::MAX, i32::MIN);
    for g in &glyphs {
        u32be(&mut loca, glyf.len() as u32);
        let pts: Vec<(i32, i32)> = g.contours.iter().flatten().copied().collect();
        let mut lsb = 0;
        if !pts.is_empty() {
            let x0 = pts.iter().map(|p| p.0).min().unwrap_or(0);
            let x1 = pts.iter().map(|p| p.0).max().unwrap_or(0);
            let y0 = pts.iter().map(|p| p.1).min().unwrap_or(0);
            let y1 = pts.iter().map(|p| p.1).max().unwrap_or(0);
            (x_min, y_min, x_max, y_max) = (x_min.min(x0), y_min.min(y0), x_max.max(x1), y_max.max(y1));
            (max_points, max_contours) = (max_points.max(pts.len()), max_contours.max(g.contours.len()));
            (min_lsb, min_rsb, max_extent) = (min_lsb.min(x0), min_rsb.min(g.advance - x1), max_extent.max(x1));
            lsb = x0;
            i16be(&mut glyf, g.contours.len() as i32);
            for v in [x0, y0, x1, y1] {
                i16be(&mut glyf, v);
            }
            let mut end = 0;
            for c in &g.contours {
                end += c.len();
                i16be(&mut glyf, end as i32 - 1);
            }
            // No instructions. Every point is on the curve, coordinates are 16-bit deltas.
            i16be(&mut glyf, 0);
            glyf.extend(std::iter::repeat_n(1u8, pts.len()));
            for axis in [0, 1] {
                let mut last = 0;
                for p in &pts {
                    let v = if axis == 0 { p.0 } else { p.1 };
                    i16be(&mut glyf, v - last);
                    last = v;
                }
            }
            glyf.resize(glyf.len().next_multiple_of(4), 0);
        }
        i16be(&mut hmtx, g.advance);
        i16be(&mut hmtx, lsb);
    }
    u32be(&mut loca, glyf.len() as u32);
    let n = glyphs.len() as i32;
    let max_advance = glyphs.iter().map(|g| g.advance).max().unwrap_or(0);
    let widths: Vec<i32> = glyphs.iter().map(|g| g.advance).filter(|&a| a > 0).collect();
    let avg_width = widths.iter().sum::<i32>() / widths.len().max(1) as i32;

    let mut head = Vec::new();
    for v in [1, 0] {
        i16be(&mut head, v);
    }
    u32be(&mut head, 0x0001_0000); // fontRevision 1.0
    u32be(&mut head, 0); // checksumAdjustment, set below
    u32be(&mut head, 0x5F0F_3CF5);
    i16be(&mut head, 0b11); // baseline at y=0, left sidebearing at x=0
    i16be(&mut head, UPM);
    head.extend(DATE.to_be_bytes());
    head.extend(DATE.to_be_bytes());
    for v in [x_min, y_min, x_max, y_max, 0, 8, 2, 1, 0] {
        // ..., macStyle, lowestRecPPEM, fontDirectionHint, long loca, glyphDataFormat
        i16be(&mut head, v);
    }

    let mut hhea = Vec::new();
    u32be(&mut hhea, 0x0001_0000);
    for v in [ASCENT, -DESCENT, 0, max_advance, min_lsb, min_rsb, max_extent, 1, 0, 0, 0, 0, 0, 0, 0, n] {
        i16be(&mut hhea, v);
    }

    let mut maxp = Vec::new();
    u32be(&mut maxp, 0x0001_0000);
    for v in [n, max_points as i32, max_contours as i32, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0] {
        i16be(&mut maxp, v);
    }

    let last_char = cmap.keys().max().copied().unwrap_or(0x20).min(0xFFFF) as i32;
    let mut os2 = Vec::new();
    // version, xAvgCharWidth, usWeightClass, usWidthClass, fsType
    for v in [4, avg_width, 400, 5, 0] {
        i16be(&mut os2, v);
    }
    // Sub- and superscript size and offset, strikeout size and position, sFamilyClass.
    for v in [5 * PX, 5 * PX, 0, PX, 5 * PX, 5 * PX, 0, 4 * PX, PX, 4 * PX, 0] {
        i16be(&mut os2, v);
    }
    // PANOSE: any. Unicode ranges: Basic Latin, Latin-1 Supplement, General Punctuation.
    os2.extend([0u8; 10]);
    for v in [0x8000_0003u32, 0, 0, 0] {
        u32be(&mut os2, v);
    }
    os2.extend(b"NONE");
    // fsSelection REGULAR | USE_TYPO_METRICS, first and last character, typo and win metrics.
    for v in [0xC0, 0x20, last_char, ASCENT, -DESCENT, 0, ASCENT, DESCENT] {
        i16be(&mut os2, v);
    }
    for v in [1u32, 0] {
        u32be(&mut os2, v); // code pages: Latin 1
    }
    // x-height (lowercase is the capitals), cap height, default and break characters, max context.
    for v in [7 * PX, 7 * PX, 0, 0x20, 0] {
        i16be(&mut os2, v);
    }

    let mut post = Vec::new();
    u32be(&mut post, 0x0003_0000); // no glyph names
    u32be(&mut post, 0);
    i16be(&mut post, -PX);
    i16be(&mut post, PX);
    post.extend([0u8; 20]);

    let tables: [(&[u8; 4], Vec<u8>); 10] = [
        (b"OS/2", os2),
        (b"cmap", cmap_table(&cmap)),
        (b"glyf", glyf),
        (b"head", head),
        (b"hhea", hhea),
        (b"hmtx", hmtx),
        (b"loca", loca),
        (b"maxp", maxp),
        (b"name", name_table()),
        (b"post", post),
    ];
    let mut font = Vec::new();
    u32be(&mut font, 0x0001_0000);
    let pow = 1 << (tables.len().ilog2());
    for v in [tables.len(), pow * 16, pow.ilog2() as usize, tables.len() * 16 - pow * 16] {
        i16be(&mut font, v as i32);
    }
    let mut offset = 12 + 16 * tables.len();
    let mut head_at = 0;
    for (tag, data) in &tables {
        font.extend(*tag);
        u32be(&mut font, checksum(data));
        u32be(&mut font, offset as u32);
        u32be(&mut font, data.len() as u32);
        if *tag == b"head" {
            head_at = offset;
        }
        offset += data.len().next_multiple_of(4);
    }
    for (_, data) in &tables {
        font.extend(data);
        font.resize(font.len().next_multiple_of(4), 0);
    }
    let adjust = 0xB1B0_AFBAu32.wrapping_sub(checksum(&font));
    font[head_at + 8..head_at + 12].copy_from_slice(&adjust.to_be_bytes());

    std::fs::create_dir_all(path.parent().ok_or("no parent directory")?)?;
    std::fs::write(path, &font)?;
    println!("wrote {} ({} glyphs, {} bytes)", path.display(), glyphs.len(), font.len());
    Ok(())
}

/// Closed outlines around the ink, corners only, in font units with y up from
/// the baseline: clockwise around ink and counter-clockwise around holes.
fn trace(rows: &[u16], w: i32) -> Vec<Vec<(i32, i32)>> {
    let h = rows.len() as i32;
    let ink = |x: i32, y: i32| {
        (0..w).contains(&x) && (0..h).contains(&y) && rows[(h - 1 - y) as usize] >> (w - 1 - x) & 1 == 1
    };
    // Unit edges with the ink on their right, by start point.
    let mut edges: BTreeMap<(i32, i32), Vec<(i32, i32)>> = BTreeMap::new();
    for y in 0..h {
        for x in (0..w).filter(|&x| ink(x, y)) {
            let mut add = |a: (i32, i32), b: (i32, i32)| edges.entry(a).or_default().push(b);
            if !ink(x - 1, y) {
                add((x, y), (x, y + 1));
            }
            if !ink(x, y + 1) {
                add((x, y + 1), (x + 1, y + 1));
            }
            if !ink(x + 1, y) {
                add((x + 1, y + 1), (x + 1, y));
            }
            if !ink(x, y - 1) {
                add((x + 1, y), (x, y));
            }
        }
    }
    let mut out = Vec::new();
    // Start each contour where only one edge leaves, so it closes only on itself.
    while let Some(start) = edges.iter().find(|(_, v)| v.len() == 1).or_else(|| edges.iter().next()).map(|(&k, _)| k) {
        let (mut at, mut dir, mut pts) = (start, (0, 0), Vec::new());
        loop {
            pts.push(at);
            let outs = edges.get_mut(&at).expect("every edge continues");
            // Where two pixels touch only at a corner, turn right, so each
            // keeps its own outline.
            let right = (dir.1, -dir.0);
            let i = outs.iter().position(|&b| (b.0 - at.0, b.1 - at.1) == right).unwrap_or(0);
            let next = outs.swap_remove(i);
            if outs.is_empty() {
                edges.remove(&at);
            }
            dir = (next.0 - at.0, next.1 - at.1);
            at = next;
            if at == start {
                break;
            }
        }
        let n = pts.len();
        let corners = (0..n)
            .filter(|&i| {
                let (p, c, q) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
                (c.0 - p.0, c.1 - p.1) != (q.0 - c.0, q.1 - c.1)
            })
            .map(|i| (pts[i].0 * PX, pts[i].1 * PX))
            .collect();
        out.push(corners);
    }
    out
}

/// A format 4 subtable for Unicode (0, 3) and Windows (3, 1): one segment
/// per run of consecutive characters with consecutive glyphs.
fn cmap_table(map: &BTreeMap<u32, u16>) -> Vec<u8> {
    let mut segs: Vec<(u32, u32, u16)> = Vec::new(); // start, end, glyph of start
    for (&c, &g) in map {
        match segs.last_mut() {
            Some((s, e, g0)) if c == *e + 1 && g as u32 == *g0 as u32 + (c - *s) => *e = c,
            _ => segs.push((c, c, g)),
        }
    }
    segs.push((0xFFFF, 0xFFFF, 0));
    let n = segs.len();
    let pow = 1 << n.ilog2();
    let mut sub = Vec::new();
    for v in [4, 16 + 8 * n, 0, 2 * n, 2 * pow, n.ilog2() as usize, 2 * n - 2 * pow] {
        i16be(&mut sub, v as i32);
    }
    for s in &segs {
        i16be(&mut sub, s.1 as i32);
    }
    i16be(&mut sub, 0);
    for s in &segs {
        i16be(&mut sub, s.0 as i32);
    }
    for s in &segs {
        // idDelta, modulo 65536; the last segment maps 0xFFFF to glyph 0.
        let delta = if s.0 == 0xFFFF { 1 } else { s.2 as i32 - s.0 as i32 };
        i16be(&mut sub, delta);
    }
    sub.extend(std::iter::repeat_n(0u8, 2 * n)); // idRangeOffset
    let mut t = Vec::new();
    for v in [0, 2, 0, 3] {
        i16be(&mut t, v);
    }
    u32be(&mut t, 20);
    for v in [3, 1] {
        i16be(&mut t, v);
    }
    u32be(&mut t, 20);
    t.extend(sub);
    t
}

fn name_table() -> Vec<u8> {
    let names = [
        (1, FAMILY),
        (2, "Regular"),
        (3, "JewCal Pixel Regular 1.000"),
        (4, FAMILY),
        (5, "Version 1.000"),
        (6, "JewCalPixel-Regular"),
    ];
    let (mut t, mut strings) = (Vec::new(), Vec::new());
    for v in [0, names.len(), 6 + 12 * names.len()] {
        i16be(&mut t, v as i32);
    }
    for (id, s) in names {
        let utf16: Vec<u8> = s.encode_utf16().flat_map(u16::to_be_bytes).collect();
        // Windows, Unicode BMP, US English.
        for v in [3, 1, 0x409, id, utf16.len(), strings.len()] {
            i16be(&mut t, v as i32);
        }
        strings.extend(utf16);
    }
    t.extend(strings);
    t
}

fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, c| {
        let mut w = [0u8; 4];
        w[..c.len()].copy_from_slice(c);
        sum.wrapping_add(u32::from_be_bytes(w))
    })
}

fn i16be(b: &mut Vec<u8>, v: i32) {
    b.extend((v as u16).to_be_bytes());
}

fn u32be(b: &mut Vec<u8>, v: u32) {
    b.extend(v.to_be_bytes());
}
