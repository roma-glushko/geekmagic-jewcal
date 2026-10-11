//! `docs-pages`: the user guide's generated assets (`make docs-assets`):
//! - the logo, the pixel font and the SmallTV picture on the overview page;
//! - the device's web pages as static HTML, from the same code the firmware
//!   serves, with sample settings, and with `--chrome` their screenshots.

use std::path::{Path, PathBuf};
use std::process::Command;

use jewcal_app::web::{self, PageStatus};
use jewcal_app::Settings;
use jewcal_ui::palette::{CLEAR, SWEETIE16};
use jewcal_ui::sprite::Sprite;
use jewcal_ui::sprites;

/// Headless Chrome lays pages out at least 500 px wide: a large phone.
const WIDTH: u32 = 500;

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut out = PathBuf::from("target/docs-pages");
    let mut images = PathBuf::from("docs/images");
    let mut chrome: Option<PathBuf> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" => out = it.next().ok_or("--out DIR")?.into(),
            "--images" => images = it.next().ok_or("--images DIR")?.into(),
            "--chrome" => chrome = Some(it.next().ok_or("--chrome PATH")?.into()),
            other => return Err(format!("unknown argument {other}").into()),
        }
    }
    std::fs::create_dir_all(&out)?;
    write_pages(&out)?;
    write_logo(Path::new("docs/assets/logo.png"))?;
    crate::webfont::write(Path::new("docs/assets/fonts/jewcal-pixel.ttf"))?;
    let tv = crate::hero::frames()?;
    crate::hero::write(Path::new("docs/assets/smalltv.png"), &tv, 1)?;
    // Shown 3:1 in the README: whole pixels on screens of 1x and 2x density.
    crate::hero::write(Path::new("docs/assets/smalltv-6x.png"), &tv, 6)?;
    println!("wrote {}/settings.html and setup.html", out.display());
    if let Some(chrome) = chrome {
        std::fs::create_dir_all(&images)?;
        for (page, png) in [("settings.html", "settings-page.png"), ("setup.html", "wifi-setup-page.png")] {
            let html = out.join(page).canonicalize()?;
            let height = measure(&chrome, &html)?;
            capture(&chrome, &html, height, &images.join(png))?;
            println!("wrote {} ({WIDTH}x{height})", images.join(png).display());
        }
    }
    Ok(())
}

fn write_pages(out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let s = Settings {
        geonameid: Some(5128581),
        place_name: "New York, New York, USA".into(),
        cc: "US".into(),
        tzid: "America/New_York".into(),
        ..Settings::default()
    };
    let info = "JewCal 0.1.0-97f002d for the SmallTV Ultra. Up 2d 4h 13m, free memory 21480 B \
                (largest block 12320 B), stack never below 1384 B free.";
    let mut page = String::new();
    web::settings_page(&mut page, &s, &PageStatus::default(), "", &web::device_section(info, "1a2b3c4d"))?;
    // A picture of the main screen instead of the device's live /screen.bmp
    // (and no refresh timer, which would break it).
    let page = page.replace("src='/screen.bmp'", "src='screen.png'");
    let page: Vec<&str> = page.lines().filter(|l| !l.contains("setInterval(")).collect();
    std::fs::write(out.join("settings.html"), page.join("\n"))?;
    std::fs::copy("docs/screens/today-friday-main.png", out.join("screen.png"))?;

    let networks: Vec<String> = ["Home", "Home-Guest", "Neighbours"].iter().map(|n| n.to_string()).collect();
    let mut setup = String::new();
    web::setup_page(&mut setup, &networks)?;
    std::fs::write(out.join("setup.html"), setup)?;
    Ok(())
}

/// The docs' logo: the device's two Shabbat candles, lit, cropped and
/// scaled up (transparent RGBA; the theme shows it pixelated).
fn write_logo(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    const N: usize = 24;
    const SCALE: usize = 8;
    let mut px = [CLEAR; N * N];
    let mut blit = |s: &Sprite, x0: i32, y0: i32| {
        for y in 0..s.h as i32 {
            for x in 0..s.w as i32 {
                let c = s.px[(y * s.w as i32 + x) as usize];
                let (tx, ty) = (x0 + x, y0 + y);
                if c != CLEAR && (0..N as i32).contains(&tx) && (0..N as i32).contains(&ty) {
                    px[ty as usize * N + tx as usize] = c;
                }
            }
        }
    };
    blit(&sprites::CANDLES, 0, 0);
    for (x, y) in sprites::CANDLES_FLAMES {
        blit(&sprites::FLAME[0], x, y);
    }
    let lit = |x: usize, y: usize| px[y * N + x] != CLEAR;
    let xs: Vec<usize> = (0..N).filter(|&x| (0..N).any(|y| lit(x, y))).collect();
    let ys: Vec<usize> = (0..N).filter(|&y| (0..N).any(|x| lit(x, y))).collect();
    let (x0, x1, y0, y1) = (xs[0], xs[xs.len() - 1], ys[0], ys[ys.len() - 1]);
    // Square, so it sits well in the header and as a favicon.
    let side = (x1 - x0).max(y1 - y0) + 1;
    let (ox, oy) = (x0 as i64 - ((side - (x1 - x0 + 1)) / 2) as i64, y0 as i64 - ((side - (y1 - y0 + 1)) / 2) as i64);
    let size = (side * SCALE) as u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for y in 0..side {
        for x in 0..side {
            let (sx, sy) = (ox + x as i64, oy + y as i64);
            if !(0..N as i64).contains(&sx) || !(0..N as i64).contains(&sy) {
                continue;
            }
            let c = px[sy as usize * N + sx as usize];
            if c == CLEAR {
                continue;
            }
            let rgb = SWEETIE16[c as usize & 15];
            for dy in 0..SCALE {
                for dx in 0..SCALE {
                    let i = ((y * SCALE + dy) * side * SCALE + x * SCALE + dx) * 4;
                    rgba[i..i + 4].copy_from_slice(&[(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255]);
                }
            }
        }
    }
    std::fs::create_dir_all(path.parent().unwrap())?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(path)?), size, size);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Best);
    enc.write_header()?.write_image_data(&rgba)?;
    println!("wrote {} ({size}x{size})", path.display());
    Ok(())
}

/// The page's full height at `WIDTH`: a copy that writes it into its title,
/// read back with `--dump-dom`.
fn measure(chrome: &Path, html: &Path) -> Result<u32, Box<dyn std::error::Error>> {
    let probe = html.with_extension("measure.html");
    let src = std::fs::read_to_string(html)?;
    // The root element's own height: scrollHeight never reports less than
    // the window, so a short page would come out padded.
    let script = "<script>document.title=String(Math.ceil(document.documentElement.getBoundingClientRect().height))</script></body>";
    std::fs::write(&probe, src.replace("</body>", script))?;
    let out = Command::new(chrome)
        .args(["--headless=new", "--disable-gpu", "--hide-scrollbars", &format!("--window-size={WIDTH},100")])
        .arg("--dump-dom")
        .arg(format!("file://{}", probe.display()))
        .output()?;
    let dom = String::from_utf8_lossy(&out.stdout);
    let height = dom
        .split("<title>")
        .nth(1)
        .and_then(|t| t.split("</title>").next())
        .and_then(|t| t.trim().parse::<u32>().ok())
        .ok_or("could not measure the page height (is --chrome a Chrome binary?)")?;
    std::fs::remove_file(probe)?;
    Ok(height)
}

fn capture(chrome: &Path, html: &Path, height: u32, png: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new(chrome)
        .args(["--headless=new", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=2"])
        .arg(format!("--window-size={WIDTH},{height}"))
        .arg(format!("--screenshot={}", png.canonicalize().unwrap_or_else(|_| png.to_path_buf()).display()))
        .arg(format!("file://{}", html.display()))
        .stderr(std::process::Stdio::null())
        .status()?;
    if !status.success() || !png.exists() {
        return Err(format!("chrome could not capture {}", png.display()).into());
    }
    Ok(())
}
