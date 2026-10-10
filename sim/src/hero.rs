//! The overview page's picture: a SmallTV drawn in the device's palette.
//! Its screen cycles through a few real screens, with the flames flickering
//! and the device's own dissolve between screens. It is an animated PNG at
//! one image pixel per art pixel, and the page scales it up without
//! smoothing.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use jewcal_model::snapshot::Snapshot;
use jewcal_ui::canvas::{self, Canvas};
use jewcal_ui::palette::{SILVER, SLATE, SWEETIE16, WHITE};
use jewcal_ui::{draw, Ctx, Screen};

use crate::{cases, fixtures, png_out};

type Res = Result<(), Box<dyn std::error::Error>>;

/// The tour: a gallery case, its screen, and how long it stays (ms).
const TOUR: [(&str, Screen, u32); 5] = [
    ("today-friday", Screen::Main, 4000),
    ("erev-pesach-3day", Screen::Period, 3000),
    ("pesach-2nd-night", Screen::Pesach, 3000),
    ("chanukah-erev-shabbat", Screen::Chanukah, 3000),
    ("today-friday", Screen::Weather, 3000),
];
/// The device animates at about 8 frames a second...
const FRAME_MS: u32 = 125;
/// ...and dissolves in three dithered steps, about 50 ms apart.
const DISSOLVE: [u8; 3] = [4, 8, 12];
const DISSOLVE_MS: u32 = 50;

// Colors beyond the palette's 16, for the case around the screen.
const CLEAR: u8 = 16;
const GLASS: u8 = 17;
const GLARE: u8 = 18;
const EXTRA: [u32; 3] = [0x000000, 0x0b0c14, 0x262b44];

// The case, in art pixels: black glass around the panel, the shell around
// the glass, and the top face, seen from a little above.
const SCREEN: i32 = canvas::W;
const BEZEL: i32 = 6;
const RIM: i32 = 6;
const FRONT: i32 = SCREEN + 2 * (BEZEL + RIM);
const TOP: i32 = 8;
const SHADOW: i32 = 3;
const PAD: i32 = 1;
const W: i32 = FRONT + 2 * PAD;
const H: i32 = PAD + TOP + FRONT + SHADOW + PAD;
/// The front face's top-left corner.
const FX: i32 = PAD;
const FY: i32 = PAD + TOP;
/// The panel's top-left corner.
const SX: i32 = FX + RIM + BEZEL;
const SY: i32 = FY + RIM + BEZEL;

pub fn write(path: &Path, preview: &Path) -> Res {
    let case = smalltv();
    // Every frame as a whole picture, with how long it shows.
    let mut frames: Vec<(Vec<u8>, u32)> = Vec::new();
    let mut push = |px: Vec<u8>, ms: u32| match frames.last_mut() {
        Some((last, d)) if *last == px => *d += ms,
        _ => frames.push((px, ms)),
    };
    let tour = TOUR.iter().map(|&(name, screen, ms)| screen_frames(name, screen, ms)).collect::<Result<Vec<_>, _>>()?;
    for (i, shots) in tour.iter().enumerate() {
        for c in shots {
            push(with_screen(&case, c), FRAME_MS);
        }
        let (from, to) = (&shots[shots.len() - 1], &tour[(i + 1) % tour.len()][0]);
        for step in DISSOLVE {
            let mut c = Canvas::new();
            c.dissolve(from, to, step);
            push(with_screen(&case, &c), DISSOLVE_MS);
        }
    }

    let mut plte = Vec::new();
    for rgb in SWEETIE16.iter().chain(&EXTRA) {
        plte.extend([(rgb >> 16) as u8, (rgb >> 8) as u8, *rgb as u8]);
    }
    let mut trns = vec![255u8; CLEAR as usize + 1];
    trns[CLEAR as usize] = 0;
    let mut enc = png::Encoder::new(BufWriter::new(File::create(path)?), W as u32, H as u32);
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_palette(plte.clone());
    enc.set_trns(trns.clone());
    enc.set_compression(png::Compression::Best);
    enc.set_animated(frames.len() as u32, 0)?;
    let mut wr = enc.write_header()?;
    let mut prev: Option<&[u8]> = None;
    for (px, ms) in &frames {
        // After the first frame, only the rectangle that changed.
        let (x, y, w, h) = prev.map_or((0, 0, W, H), |p| changed(p, px));
        wr.reset_frame_position()?;
        wr.set_frame_dimension(w as u32, h as u32)?;
        wr.set_frame_position(x as u32, y as u32)?;
        wr.set_frame_delay(*ms as u16, 1000)?;
        wr.set_dispose_op(png::DisposeOp::None)?;
        wr.set_blend_op(png::BlendOp::Source)?;
        let rect: Vec<u8> =
            (y..y + h).flat_map(|r| &px[(r * W + x) as usize..(r * W + x + w) as usize]).copied().collect();
        wr.write_image_data(&rect)?;
        prev = Some(px);
    }
    wr.finish()?;
    let total: u32 = frames.iter().map(|f| f.1).sum();
    println!("wrote {} ({W}x{H}, {} frames, {:.1} s)", path.display(), frames.len(), total as f32 / 1000.0);

    // A still at 3x, for reviewing the art.
    const S: i32 = 3;
    let first = &frames[0].0;
    let mut big = Vec::with_capacity((W * H * S * S) as usize);
    for y in 0..H * S {
        big.extend((0..W * S).map(|x| first[((y / S) * W + x / S) as usize]));
    }
    let mut enc = png::Encoder::new(BufWriter::new(File::create(preview)?), (W * S) as u32, (H * S) as u32);
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_palette(plte);
    enc.set_trns(trns);
    enc.write_header()?.write_image_data(&big)?;
    Ok(())
}

/// One screen of one gallery case, frame by frame for `ms`.
fn screen_frames(name: &str, screen: Screen, ms: u32) -> Result<Vec<Canvas>, Box<dyn std::error::Error>> {
    let case = cases::gallery().into_iter().find(|c| c.name == name).ok_or_else(|| format!("no case {name}"))?;
    let (model, tz, now) = fixtures::load(&case)?;
    let s = Snapshot::at(&model, &tz, now);
    let weather = fixtures::weather(&case, now);
    let status = png_out::sample_status(&case, &model, &s);
    Ok((0..ms / FRAME_MS)
        .map(|frame| {
            let mut c = Canvas::new();
            draw(screen, &mut c, &s, &Ctx { frame, status: &status, weather: weather.as_ref() });
            c
        })
        .collect())
}

/// The SmallTV with an empty screen.
fn smalltv() -> Vec<u8> {
    let mut px = vec![CLEAR; (W * H) as usize];
    let mut set = |x: i32, y: i32, c: u8| {
        if (0..W).contains(&x) && (0..H).contains(&y) {
            px[(y * W + x) as usize] = c;
        }
    };
    // The top face narrows toward the back.
    for k in 1..=TOP {
        let inset = (k + 1) / 2;
        let (x0, x1) = (FX + inset, FX + FRONT - 1 - inset);
        for x in x0..=x1 {
            let back = k == TOP && (x == x0 || x == x1);
            if !back {
                set(x, FY - k, if k == TOP { SILVER } else { WHITE });
            }
        }
    }
    // The front: light from above, a shadowed bottom edge, rounded bottom corners.
    for y in FY..FY + FRONT {
        for x in FX..FX + FRONT {
            let (dx, dy) = ((x - FX).min(FX + FRONT - 1 - x), FY + FRONT - 1 - y);
            if dy < 2 && dx < 2 - dy {
                continue;
            }
            let c = if dy == 0 || x == FX + FRONT - 1 {
                SLATE
            } else if y == FY || x == FX {
                WHITE
            } else {
                SILVER
            };
            set(x, y, c);
        }
    }
    // The glass, with its corners rounded by a pixel, and a glint on it.
    let (gx, gy, g) = (FX + RIM, FY + RIM, SCREEN + 2 * BEZEL);
    for y in gy..gy + g {
        for x in gx..gx + g {
            let corner = (x == gx || x == gx + g - 1) && (y == gy || y == gy + g - 1);
            if !corner {
                set(x, y, GLASS);
            }
        }
    }
    for t in 0..4 {
        set(gx + 1 + t, gy + 4 - t, GLARE);
        set(gx + 3 + t, gy + 4 - t, GLARE);
    }
    // A shadow on the desk.
    for (row, (inset, dither)) in [(1, false), (3, true), (10, true)].into_iter().enumerate() {
        let y = FY + FRONT + row as i32;
        for x in FX + inset..FX + FRONT - inset {
            if !dither || (x + y) % 2 == 0 {
                set(x, y, GLASS);
            }
        }
    }
    px
}

fn with_screen(case: &[u8], c: &Canvas) -> Vec<u8> {
    let mut px = case.to_vec();
    for y in 0..SCREEN {
        for x in 0..SCREEN {
            px[((SY + y) * W + SX + x) as usize] = c.get(x, y) & 15;
        }
    }
    px
}

/// The smallest rectangle holding every pixel that differs.
fn changed(a: &[u8], b: &[u8]) -> (i32, i32, i32, i32) {
    let (mut x0, mut y0, mut x1, mut y1) = (W, H, -1, -1);
    for y in 0..H {
        for x in 0..W {
            if a[(y * W + x) as usize] != b[(y * W + x) as usize] {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
    }
    if x1 < 0 {
        return (0, 0, 1, 1);
    }
    (x0, y0, x1 - x0 + 1, y1 - y0 + 1)
}
