//! Heap use of one refresh as the SmallTV Ultra (ESP8266, ~40 KB free heap)
//! would see it: parse the device's fetch window, build the model, render
//! every screen. Measured on the host (64-bit), where pointers and `String`s
//! are twice their ESP8266 size, so these numbers overstate the device.

use std::alloc::{GlobalAlloc, Layout, System};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use jewcal_model::calendar::CalendarModel;
use jewcal_model::hebcal::Window;
use jewcal_model::snapshot::Snapshot;
use jewcal_model::time::CivilDate;
use jewcal_model::tz::Tz;
use jewcal_model::wire::{self, Events, Policy};
use jewcal_ui::{Canvas, Ctx, Status};

struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static BLOCKS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let now = LIVE.fetch_add(l.size(), Relaxed) + l.size();
        PEAK.fetch_max(now, Relaxed);
        BLOCKS.fetch_add(1, Relaxed);
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size(), Relaxed);
        BLOCKS.fetch_sub(1, Relaxed);
        System.dealloc(p, l)
    }
}

#[global_allocator]
static A: Counting = Counting;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../model/tests/fixtures").join(name)
}

fn reset_peak() -> usize {
    let live = LIVE.load(Relaxed);
    PEAK.store(live, Relaxed);
    live
}

struct Usage {
    parse_peak: usize,
    events: usize,
    items: usize,
    model: usize,
    model_blocks: usize,
    frame_peak: usize,
}

fn measure(city: &str, tzid: &str, today: CivilDate, now: i64) -> Usage {
    let w = Window::around(today);
    let policy = Policy { detail_until: Some(w.detail_until), range: Some((w.start, w.end)) };
    let base = reset_peak();
    let mut events = Events::default();
    for year in w.start.y..=w.end.y {
        let Ok(f) = File::open(fixture(&format!("events-{city}-{year}.json"))) else {
            continue;
        };
        let ev = wire::parse_events(BufReader::new(f), policy).unwrap();
        events.location = ev.location;
        events.items.extend(ev.items);
    }
    events.items.shrink_to_fit();
    let parse_peak = PEAK.load(Relaxed) - base;
    let ev_bytes = LIVE.load(Relaxed) - base;
    let items = events.items.len();

    let blocks0 = BLOCKS.load(Relaxed);
    let before = LIVE.load(Relaxed) - ev_bytes;
    let model = CalendarModel::build(events, Vec::new(), (w.start, w.end), false, now);
    let model_bytes = LIVE.load(Relaxed) - before;
    let model_blocks = BLOCKS.load(Relaxed) + items - blocks0;

    let tz = Tz::for_tzid(tzid).unwrap();
    let mut canvas = Canvas::new();
    let status = Status::default();
    let frame_base = reset_peak();
    let snap = Snapshot::at(&model, &tz, now);
    for screen in jewcal_ui::available(&snap, None) {
        let ctx = Ctx { frame: 0, status: &status, weather: None };
        jewcal_ui::draw(screen, &mut canvas, &snap, &ctx);
    }
    drop(snap);
    let frame_peak = PEAK.load(Relaxed) - frame_base;
    Usage { parse_peak, events: ev_bytes, items, model: model_bytes, model_blocks, frame_peak }
}

#[test]
fn one_refresh_fits_an_esp8266() {
    let cases = [
        ("nyc", "America/New_York", CivilDate::new(2026, 9, 20), 1_789_900_000), // before the fall holidays
        ("nyc", "America/New_York", CivilDate::new(2027, 3, 25), 1_806_000_000), // before Pesach
        ("jerusalem", "Asia/Jerusalem", CivilDate::new(2026, 9, 20), 1_789_900_000),
    ];
    println!(
        "{:<10} {:<10} {:>6} {:>10} {:>9} {:>9} {:>7} {:>10}",
        "city", "today", "items", "parse peak", "events", "model", "blocks", "frame peak"
    );
    for (city, tzid, today, now) in cases {
        let u = measure(city, tzid, today, now);
        println!(
            "{city:<10} {today:<10} {:>6} {:>10} {:>9} {:>9} {:>7} {:>10}",
            u.items, u.parse_peak, u.events, u.model, u.model_blocks, u.frame_peak
        );
        // Host bytes; the ESP8266 needs roughly 60% of these.
        assert!(u.model < 48 * 1024, "{city} {today}: model {} bytes", u.model);
        assert!(u.parse_peak < 64 * 1024, "{city} {today}: parse peak {} bytes", u.parse_peak);
    }
}
