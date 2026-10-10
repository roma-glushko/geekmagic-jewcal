//! `docs-pages`: the device's web pages as static HTML for the user guide,
//! and, with `--chrome`, their screenshots (`make docs-images`). Same code as
//! the firmware serves, with sample settings.

use std::path::{Path, PathBuf};
use std::process::Command;

use jewcal_app::web::{self, PageStatus};
use jewcal_app::Settings;

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
