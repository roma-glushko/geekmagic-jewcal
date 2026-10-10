//! Host tools for JewCal.
//!
//! ```text
//! jewcal-sim gen-tz              regenerate model/src/tz_table.rs from /usr/share/zoneinfo
//! jewcal-sim gen-cfont           regenerate ultra/src/font_body.h (C copy of the BODY font)
//! jewcal-sim record              download the Hebcal fixtures used by tests and the gallery
//! jewcal-sim agenda CITY TIME    print what the model sees at a local time
//! jewcal-sim render CITY TIME [SCREEN] [--out DIR]
//! jewcal-sim gallery [--out DIR] render every screen for every case + index.html
//! ```
//! CITY is one of `nyc`, `jerusalem`, `london`, `moscow`; TIME is local
//! `YYYY-MM-DDTHH:MM`.

mod cases;
mod cfont;
mod fixtures;
mod png_out;
mod tzgen;

use std::path::PathBuf;

use jewcal_model::snapshot::Snapshot;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("gen-tz") => tzgen::run(),
        Some("gen-cfont") => cfont::run(),
        Some("record") => fixtures::record_all(),
        Some("agenda") => agenda(&args[1..]),
        Some("render") => render(&args[1..]),
        Some("gallery") => gallery(&args[1..]),
        _ => Err("usage: jewcal-sim gen-tz | gen-cfont | record | agenda CITY TIME | render CITY TIME [SCREEN] [--out DIR] | gallery [--out DIR]".into()),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

type Res = Result<(), Box<dyn std::error::Error>>;

fn out_dir(args: &[String], default: &str) -> PathBuf {
    args.iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1))
        .map_or_else(|| PathBuf::from(default), PathBuf::from)
}

fn case_from_args(args: &[String]) -> Result<cases::Case, Box<dyn std::error::Error>> {
    let city = cases::City::parse(args.first().ok_or("missing CITY")?)?;
    let at = args.get(1).ok_or("missing TIME")?;
    Ok(cases::Case { name: "custom".into(), city, at: at.clone() })
}

fn agenda(args: &[String]) -> Res {
    let case = case_from_args(args)?;
    let (model, tz, now) = fixtures::load(&case)?;
    let s = Snapshot::at(&model, &tz, now);
    print!("{}", fixtures::describe(&s));
    Ok(())
}

fn render(args: &[String]) -> Res {
    let case = case_from_args(args)?;
    let screen = args.get(2).filter(|s| !s.starts_with("--")).cloned();
    let out = out_dir(args, "shots");
    for name in png_out::render_case(&case, screen.as_deref(), &out)? {
        println!("{}", out.join(name).display());
    }
    Ok(())
}

fn gallery(args: &[String]) -> Res {
    let out = out_dir(args, "shots");
    png_out::gallery(&out)
}
