//! Render screens to PNG exactly as the device would draw them.

use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

use jewcal_model::snapshot::Snapshot;
use jewcal_ui::canvas::Canvas;
use jewcal_ui::palette::{self, BG};
use jewcal_ui::{available, draw, draw_message, draw_setup, sprites, Ctx, Screen, Status};

use crate::cases::{self, Case};
use crate::fixtures;

/// Output pixels per art pixel: 2 = the real 240×240 panel, 4 = easy viewing.
const SCALE: u32 = 4;

type Res = Result<(), Box<dyn std::error::Error>>;

/// Indexed PNG with the Sweetie-16 palette, like the canvas itself.
pub fn write_png(c: &Canvas, path: &Path, scale: u32) -> Res {
    let (w, h) = (120 * scale, 120 * scale);
    let mut data = vec![0u8; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            data[(y * w + x) as usize] = c.px[((y / scale) * 120 + x / scale) as usize] & 15;
        }
    }
    let plte: Vec<u8> =
        palette::SWEETIE16.iter().flat_map(|rgb| [(rgb >> 16) as u8, (rgb >> 8) as u8, *rgb as u8]).collect();
    let mut enc = png::Encoder::new(BufWriter::new(File::create(path)?), w, h);
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_palette(plte);
    enc.set_compression(png::Compression::Best);
    enc.write_header()?.write_image_data(&data)?;
    Ok(())
}

fn parse_screen(s: &str) -> Result<Screen, String> {
    Screen::ALL.into_iter().find(|x| x.name() == s).ok_or_else(|| format!("unknown screen {s:?}"))
}

/// Render one case; returns the file names written.
pub fn render_case(case: &Case, screen: Option<&str>, out: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    fs::create_dir_all(out)?;
    let (model, tz, now) = fixtures::load(case)?;
    let s = Snapshot::at(&model, &tz, now);
    let weather = fixtures::weather(case, now);
    let status = Status {
        url: Some("http://192.168.1.42/".into()),
        host: Some("jewcal.local".into()),
        location: model.location.title.clone().or_else(|| Some(case.city.slug().to_uppercase())),
        israel: model.israel,
        updated: Some(format!("{} 03:12", jewcal_model::time::month_short(s.today.m))),
        error: None,
    };
    let screens: Vec<Screen> = match screen {
        Some(name) => vec![parse_screen(name)?],
        None => std::iter::once(Screen::Main).chain(available(&s, weather.as_ref())).collect(),
    };
    let mut names = Vec::new();
    for sc in screens {
        let mut c = Canvas::new();
        draw(sc, &mut c, &s, &Ctx { frame: 0, status: &status, weather: weather.as_ref() });
        let name = format!("{}-{}.png", case.name, sc.name());
        write_png(&c, &out.join(&name), SCALE)?;
        names.push(name);
    }
    Ok(names)
}

/// All sprites on one canvas, for reviewing the art.
fn sprite_sheet(out: &Path) -> Res {
    let mut c = Canvas::new();
    c.clear(BG);
    let big = [
        &sprites::CANDLES,
        &sprites::MEMORIAL,
        &sprites::SHOFAR,
        &sprites::SUKKAH,
        &sprites::MATZAH,
        &sprites::TABLETS,
        &sprites::TORAH,
        &sprites::MOON,
        &sprites::PLATE,
        &sprites::FLAG,
        &sprites::MASK,
        &sprites::STAR,
    ];
    for (i, s) in big.iter().enumerate() {
        let (x, y) = (2 + (i % 4) as i32 * 30, 2 + (i / 4) as i32 * 28);
        c.blit(s, x, y);
    }
    for (i, &(fx, fy)) in sprites::CANDLES_FLAMES.iter().enumerate() {
        c.blit(&sprites::FLAME[i], 2 + fx, 2 + fy);
    }
    c.blit(&sprites::FLAME[2], 32 + sprites::MEMORIAL_FLAME.0, 2 + sprites::MEMORIAL_FLAME.1);
    let icons = [
        &sprites::ICON_CANDLE,
        &sprites::ICON_NIGHT,
        &sprites::ICON_HAVDALAH,
        &sprites::ICON_PLATE,
        &sprites::ICON_MOON,
        &sprites::ICON_STAR,
        &sprites::ICON_DOT,
        &sprites::ICON_FLAG,
        &sprites::ICON_MENORAH,
        &sprites::ICON_WIFI,
    ];
    for (i, s) in icons.iter().enumerate() {
        c.blit(s, 2 + i as i32 * 11, 88);
    }
    for (i, f) in sprites::FLAME.iter().enumerate() {
        c.blit(f, 2 + i as i32 * 8, 104);
    }
    write_png(&c, &out.join("sprites.png"), SCALE)
}

/// The weather screen in every condition, day and night.
fn weather_sheet(out: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    use jewcal_model::weather::Condition;
    let case = cases::gallery().into_iter().next().ok_or("no cases")?;
    let (model, tz, now) = fixtures::load(&case)?;
    let s = Snapshot::at(&model, &tz, now);
    let base = fixtures::weather(&case, now).ok_or("no weather fixture; run `record`")?;
    let status = Status { location: Some("New York, NY".into()), ..Default::default() };
    let mut names = Vec::new();
    for (i, cond) in Condition::ALL.iter().enumerate() {
        for day in [true, false] {
            if !day && !matches!(cond, Condition::Clear | Condition::PartlyCloudy | Condition::Snow) {
                continue;
            }
            let mut w = base.clone();
            w.current.condition = *cond;
            w.current.is_day = day;
            for (k, d) in w.days.iter_mut().enumerate() {
                d.condition = Condition::ALL[(i + k * 5) % Condition::ALL.len()];
            }
            let mut c = Canvas::new();
            draw(Screen::Weather, &mut c, &s, &Ctx { frame: 3, status: &status, weather: Some(&w) });
            let name = format!(
                "weather-{}{}.png",
                cond.label().to_ascii_lowercase().replace(' ', "-"),
                if day { "" } else { "-night" }
            );
            write_png(&c, &out.join(&name), SCALE)?;
            names.push(name);
        }
    }
    Ok(names)
}

pub fn gallery(out: &Path) -> Res {
    fs::create_dir_all(out)?;
    sprite_sheet(out)?;
    let weather_names = weather_sheet(out).unwrap_or_else(|e| {
        eprintln!("skip weather sheet: {e}");
        Vec::new()
    });
    let mut c = Canvas::new();
    draw_setup(&mut c, "JEWCAL-3F2A", "192.168.71.1", 0);
    write_png(&c, &out.join("device-setup.png"), SCALE)?;
    draw_message(&mut c, "SYNCING TIME", &["connecting to pool.ntp.org"], palette::YELLOW, 0);
    write_png(&c, &out.join("device-message.png"), SCALE)?;

    let mut html = String::from(
        "<!doctype html><meta charset=utf-8><title>JewCal gallery</title><style>\
         body{background:#1a1c2c;color:#f4f4f4;font:14px system-ui;margin:16px}\
         h2{color:#ffcd75;font-size:15px;margin:24px 0 6px}\
         img{width:240px;height:240px;image-rendering:pixelated;margin:0 6px 6px 0;border:1px solid #333c57}\
         figure{display:inline-block;margin:0}figcaption{color:#94b0c2;font-size:12px}</style>\
         <h1>JewCal screens</h1>",
    );
    html.push_str("<h2>Sprites and device screens</h2>");
    for n in ["sprites.png", "device-setup.png", "device-message.png"] {
        let _ = write!(html, "<figure><img src='{n}'><figcaption>{n}</figcaption></figure>");
    }
    html.push_str("<h2>Weather conditions</h2>");
    for n in &weather_names {
        let _ = write!(html, "<figure><img src='{n}'><figcaption>{n}</figcaption></figure>");
    }
    for case in cases::gallery() {
        let names = match render_case(&case, None, out) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("skip {}: {e}", case.name);
                continue;
            }
        };
        let _ = write!(html, "<h2>{} &middot; {} {}</h2>", case.name, case.city.slug(), case.at);
        for n in names {
            let _ = write!(html, "<figure><img src='{n}'><figcaption>{n}</figcaption></figure>");
        }
    }
    fs::write(out.join("index.html"), html)?;
    println!("wrote {}", out.join("index.html").display());
    Ok(())
}
