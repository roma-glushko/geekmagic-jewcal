//! JewCal: a pixel-art Jewish calendar for the GeekMagic SmallTV Pro.
//!
//! Boot: display → settings → WiFi (or the setup access point) → SNTP →
//! web server + fetch thread → render loop. Hebcal.com does the halachic
//! computation; `jewcal-model` interprets it and `jewcal-ui` draws it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use esp_idf_hal::{delay::FreeRtos, peripherals::Peripherals};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    nvs::EspDefaultNvsPartition,
    wifi::{BlockingWifi, EspWifi},
};
use jewcal_model::snapshot::Snapshot;
use jewcal_model::time::month_short;
use jewcal_model::tz::Tz;
use jewcal_ui::{palette, Canvas, Ctx, Scheduler, Screen, Status};
use log::{info, warn};

mod backlight;
mod clock;
mod display;
mod fetch;
mod httpc;
mod settings;
mod state;
#[cfg(feature = "touch")]
mod touch;
mod web;
mod wifi;

use state::{Loaded, NetStatus, Shared};

// ── Hardware (GeekMagic SmallTV Pro, ESP32-WROOM-32) ─────────────────────────
//  GPIO 18  SPI CLK          GPIO  2  display DC
//  GPIO 23  SPI MOSI         GPIO  4  display RST
//  GPIO  3  SPI CS (also UART0 RX: no serial input once the display is up)
//  GPIO 25  backlight, inverted PWM (feature `backlight-gpio21`: GPIO 21, high = on)
//  GPIO 32  top touch pad (T9), feature `touch`

const FRAME_MS: u64 = 125;
/// TLS handshake + streaming JSON on the fetch thread.
const FETCH_STACK: usize = 20 * 1024;
const RETRY_MIN_S: i64 = 60;
/// Open-Meteo updates every 15 minutes; half an hour is plenty for a desk.
const WEATHER_EVERY_S: i64 = 30 * 60;
const WEATHER_RETRY_S: i64 = 10 * 60;
const RETRY_MAX_S: i64 = 3600;
const WIFI_ATTEMPTS: u32 = 3;
/// The settings page is also reachable as `http://jewcal.local/` (mDNS).
const HOSTNAME: &str = "jewcal";

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let p = Peripherals::take()?;
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    #[cfg(not(feature = "backlight-gpio21"))]
    let mut bl = backlight::Backlight::new(p.ledc.timer0, p.ledc.channel0, p.pins.gpio25, true)?;
    #[cfg(feature = "backlight-gpio21")]
    let mut bl = backlight::Backlight::new(p.ledc.timer0, p.ledc.channel0, p.pins.gpio21, false)?;

    let disp = display::Display::new(
        p.spi2,
        p.pins.gpio18, // CLK
        p.pins.gpio23, // MOSI
        p.pins.gpio3,  // CS
        p.pins.gpio2,  // DC
        p.pins.gpio4,  // RST
    )?;
    let mut panel = Panel::new(disp);
    let mut canvas = Canvas::new();
    message(&mut panel, &mut canvas, "JEWCAL", &["starting"], palette::YELLOW);
    log_heap("boot");

    #[cfg(feature = "touch")]
    let touch_baseline = touch::init()
        .inspect_err(|e| warn!("touch unavailable: {e:#}"))
        .ok();

    let settings = settings::load(&nvs)?;
    let mut wifi: wifi::Wifi = BlockingWifi::wrap(EspWifi::new(p.modem, sysloop.clone(), Some(nvs.clone()))?, sysloop)?;

    if settings.ssid.is_empty() {
        setup_mode(&mut panel, &mut canvas, &mut wifi, nvs.clone(), None);
    }
    let mut attempt = 1;
    loop {
        message(&mut panel, &mut canvas, "CONNECTING", &[&settings.ssid], palette::SKY);
        match wifi::connect(&mut wifi, &settings.ssid, &settings.pass) {
            Ok(()) => break,
            Err(e) if attempt >= WIFI_ATTEMPTS => {
                warn!("wifi: giving up on {:?}: {e:#}", settings.ssid);
                let reason = format!("could not join {}", settings.ssid);
                setup_mode(&mut panel, &mut canvas, &mut wifi, nvs.clone(), Some(reason));
            }
            Err(e) => {
                warn!("wifi attempt {attempt}: {e:#}");
                attempt += 1;
                FreeRtos::delay_ms(2_000);
            }
        }
    }
    let ip = wifi::sta_ip(&wifi).unwrap_or_default();
    info!("wifi up: http://{ip}/");
    let _mdns = start_mdns()
        .inspect(|_| info!("settings page: http://{HOSTNAME}.local/"))
        .inspect_err(|e| warn!("mDNS unavailable: {e:#}"))
        .ok();

    let _sntp = clock::start()?;
    let mut waited = 0;
    while !clock::valid() {
        let note = if waited > 40 { "check the network" } else { "pool.ntp.org" };
        message(&mut panel, &mut canvas, "SYNCING TIME", &[note], palette::YELLOW);
        FreeRtos::delay_ms(250);
        waited += 1;
    }
    info!("time synced: {}", clock::now());

    let shared = Arc::new(Shared {
        nvs: nvs.clone(),
        settings: Mutex::new(settings),
        calendar: Mutex::new(None),
        weather: Mutex::new(None),
        status: Mutex::new(NetStatus { ip: Some(ip), ..Default::default() }),
        frame: Mutex::new(Canvas::new()),
        wake_fetch: AtomicBool::new(true),
    });
    let _server = web::start(shared.clone())?;

    {
        let sh = shared.clone();
        std::thread::Builder::new()
            .stack_size(FETCH_STACK)
            .spawn(move || fetch_loop(sh, wifi))?;
    }

    #[cfg(feature = "touch")]
    let touch_rx = touch_baseline.and_then(|baseline| {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .stack_size(3072)
            .spawn(move || touch::run(baseline, tx))
            .ok()
            .map(|_| rx)
    });

    log_heap("running");
    let mut ui = Ui::new(&shared);
    loop {
        let t0 = clock::mono_ms();
        #[cfg(feature = "touch")]
        if let Some(rx) = &touch_rx {
            while let Ok(ev) = rx.try_recv() {
                ui.touch(ev, t0, &shared);
            }
        }
        ui.frame(&mut panel, &mut canvas, &mut bl, &shared, t0);
        let spent = clock::mono_ms() - t0;
        if spent < FRAME_MS {
            FreeRtos::delay_ms((FRAME_MS - spent) as u32);
        }
    }
}

// ── Render loop ──────────────────────────────────────────────────────────────

struct Ui {
    sched: Scheduler,
    last_screen: Screen,
    frame: u32,
    tz: (String, Tz),
    #[cfg(feature = "touch")]
    pending: Vec<touch::Touch>,
}

impl Ui {
    fn new(sh: &Shared) -> Self {
        let s = sh.settings();
        Self {
            sched: Scheduler::new(s.cycle_min as u32, s.dwell_s as u32, clock::mono_ms()),
            last_screen: Screen::Main,
            frame: 0,
            tz: (String::new(), Tz::UTC),
            #[cfg(feature = "touch")]
            pending: Vec::new(),
        }
    }

    #[cfg(feature = "touch")]
    fn touch(&mut self, ev: touch::Touch, _now_ms: u64, sh: &Shared) {
        if ev == touch::Touch::VeryLong {
            warn!("touch held: forgetting WiFi and restarting into setup");
            settings::clear_wifi(&sh.nvs).ok();
            unsafe { esp_idf_svc::sys::esp_restart() };
        }
        self.pending.push(ev);
    }

    fn frame(&mut self, panel: &mut Panel, canvas: &mut Canvas, bl: &mut backlight::Backlight, sh: &Shared, now_ms: u64) {
        let settings = sh.settings();
        self.sched.set_timing(settings.cycle_min as u32, settings.dwell_s as u32, now_ms);
        let cal = sh.calendar.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let tzid = if settings.tzid.is_empty() {
            cal.as_ref().and_then(|c| c.model.location.tzid.clone()).unwrap_or_default()
        } else {
            settings.tzid.clone()
        };
        if tzid != self.tz.0 {
            let tz = Tz::for_tzid(&tzid).unwrap_or_else(|| {
                warn!("unknown time zone {tzid:?}, using UTC");
                Tz::UTC
            });
            self.tz = (tzid, tz);
        }
        let tz = &self.tz.1;
        let now = clock::now();
        let net = sh.status.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let url = net.ip.as_ref().map(|ip| format!("http://{ip}/"));
        let status = Status {
            url: url.clone(),
            host: Some(format!("{HOSTNAME}.local")),
            location: (!settings.place_name.is_empty()).then(|| settings.place_name.clone()),
            israel: settings.force_israel
                || settings.cc == "IL"
                || cal.as_ref().is_some_and(|c| c.model.israel),
            updated: net.last_fetch.map(|t| {
                let l = tz.local(t);
                format!("{} {} {:02}:{:02}", month_short(l.date.m), l.date.d, l.h, l.m)
            }),
            error: net.error.clone(),
        };

        let weather = sh.weather.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let screen = match cal.as_deref().filter(|c| c.key == settings.fetch_key()) {
            Some(loaded) => {
                let snap = Snapshot::at(&loaded.model, tz, now);
                let avail = jewcal_ui::available(&snap, weather.as_deref());
                #[cfg(feature = "touch")]
                for ev in self.pending.drain(..) {
                    match ev {
                        touch::Touch::Tap => {
                            self.sched.tap(now_ms, &avail);
                        }
                        touch::Touch::LongPress => {
                            self.sched.long_press(now_ms);
                        }
                        touch::Touch::VeryLong => {}
                    }
                }
                let screen = self.sched.tick(now_ms, &avail);
                let ctx = Ctx { frame: self.frame, status: &status, weather: weather.as_deref() };
                jewcal_ui::draw(screen, canvas, &snap, &ctx);
                screen
            }
            None => {
                let (title, line) = if settings.place().is_none() {
                    ("SET LOCATION", format!("open {}", url.as_deref().unwrap_or("the settings page")))
                } else if let (Some(e), false) = (&net.error, net.fetching) {
                    ("NO CALENDAR", e.clone())
                } else {
                    ("LOADING", "hebcal.com".to_string())
                };
                jewcal_ui::draw_message(canvas, title, &[&line], palette::YELLOW, self.frame);
                Screen::Main
            }
        };
        let transition = screen != self.last_screen;
        self.last_screen = screen;
        match panel.show(canvas, transition) {
            Ok(true) => sh.frame.lock().unwrap_or_else(|e| e.into_inner()).copy_from(canvas),
            Ok(false) => {}
            Err(e) => warn!("display: {e:#}"),
        }
        if let Err(e) = bl.set_percent(settings.brightness(tz.local(now).h)) {
            warn!("backlight: {e:#}");
        }
        self.frame = self.frame.wrapping_add(1);
    }
}

/// The panel plus what is currently on it, so only changes are sent.
struct Panel<'d> {
    disp: display::Display<'d>,
    front: Canvas,
    fade: Canvas,
    lut: [u16; 16],
    primed: bool,
}

impl<'d> Panel<'d> {
    fn new(disp: display::Display<'d>) -> Self {
        Self { disp, front: Canvas::new(), fade: Canvas::new(), lut: palette::lut(), primed: false }
    }

    fn flush_all(&mut self, c: &Canvas) -> anyhow::Result<()> {
        self.disp.flush(&c.px[..], 120, &self.lut, 0, 0, 120, 120)
    }

    /// Push `next`: an ordered-dither dissolve when the screen changes,
    /// otherwise only the rectangle that differs. Returns whether anything
    /// changed on the panel.
    fn show(&mut self, next: &Canvas, transition: bool) -> anyhow::Result<bool> {
        if !self.primed || transition {
            if self.primed {
                for step in [4u8, 8, 12] {
                    self.fade.dissolve(&self.front, next, step);
                    self.disp.flush(&self.fade.px[..], 120, &self.lut, 0, 0, 120, 120)?;
                    FreeRtos::delay_ms(30);
                }
            }
            self.flush_all(next)?;
            self.front.copy_from(next);
            self.primed = true;
            return Ok(true);
        }
        match next.diff(&self.front) {
            Some((x, y, w, h)) => {
                self.disp.flush(&next.px[..], 120, &self.lut, x, y, w, h)?;
                self.front.copy_from(next);
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

fn message(panel: &mut Panel, canvas: &mut Canvas, title: &str, lines: &[&str], color: u8) {
    jewcal_ui::draw_message(canvas, title, lines, color, (clock::mono_ms() / FRAME_MS) as u32);
    if let Err(e) = panel.show(canvas, false) {
        warn!("display: {e:#}");
    }
}

/// Announce the settings page on the home network as `jewcal.local`.
fn start_mdns() -> anyhow::Result<esp_idf_svc::mdns::EspMdns> {
    let mut mdns = esp_idf_svc::mdns::EspMdns::take()?;
    mdns.set_hostname(HOSTNAME)?;
    mdns.set_instance_name("JewCal")?;
    mdns.add_service(Some("JewCal"), "_http", "_tcp", 80, &[])?;
    Ok(mdns)
}

/// First run (or the saved network is gone): open access point + setup page.
/// Restarts after credentials are saved, so it never returns.
fn setup_mode(panel: &mut Panel, canvas: &mut Canvas, wifi: &mut wifi::Wifi, nvs: EspDefaultNvsPartition, reason: Option<String>) -> ! {
    let name = wifi::ap_name(wifi);
    let started = wifi::start_ap(wifi, &name).and_then(|(ip, networks)| {
        info!("setup AP {name} at http://{ip}/ ({} networks in range)", networks.len());
        Ok((ip, web::start_setup(nvs, networks)?))
    });
    let (ip, _server) = match started {
        Ok(v) => v,
        Err(e) => loop {
            warn!("setup mode failed: {e:#}");
            message(panel, canvas, "SETUP FAILED", &[&format!("{e:#}")], palette::RED);
            FreeRtos::delay_ms(5_000);
        },
    };
    if let Some(r) = &reason {
        warn!("setup mode: {r}");
    }
    let mut frame = 0u32;
    loop {
        jewcal_ui::draw_setup(canvas, &name, &ip, frame);
        if let Err(e) = panel.show(canvas, false) {
            warn!("display: {e:#}");
        }
        frame = frame.wrapping_add(1);
        FreeRtos::delay_ms(250);
    }
}

// ── Fetch thread ─────────────────────────────────────────────────────────────

fn fetch_loop(sh: Arc<Shared>, mut wifi: wifi::Wifi) {
    let mut next_at: i64 = 0;
    let mut backoff: i64 = 0;
    let mut weather_at: i64 = 0;
    let mut weather_key = String::new();
    loop {
        let woke = sh.wake_fetch.swap(false, Ordering::Relaxed);
        let s = sh.settings();
        let now = clock::now();
        let key = s.fetch_key();
        let have = sh
            .calendar
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_some_and(|c| c.key == key);

        if s.place().is_some() && clock::valid() && (woke || !have || now >= next_at) {
            if let Err(e) = wifi::ensure_connected(&mut wifi) {
                warn!("wifi: {e:#}");
            }
            let ip = wifi::sta_ip(&wifi);
            {
                let mut st = sh.status.lock().unwrap_or_else(|e| e.into_inner());
                st.fetching = true;
                if ip.is_some() {
                    st.ip = ip;
                }
            }
            let tz = Tz::for_tzid(&s.tzid).unwrap_or(Tz::UTC);
            let result = fetch::fetch(&s, tz.local(now).date, now);
            let mut st = sh.status.lock().unwrap_or_else(|e| e.into_inner());
            st.fetching = false;
            match result {
                Ok(model) => {
                    if let Some(tzid) = model.location.tzid.clone() {
                        remember_tz(&sh, &key, tzid);
                    }
                    let tz = model
                        .location
                        .tzid
                        .as_deref()
                        .and_then(Tz::for_tzid)
                        .unwrap_or(tz);
                    st.last_fetch = Some(now);
                    st.error = None;
                    backoff = 0;
                    next_at = next_daily_refresh(&tz, now);
                    *sh.calendar.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(Loaded { key, model }));
                }
                Err(e) => {
                    warn!("fetch failed: {e:#}");
                    st.error = Some(short_error(&e));
                    backoff = match httpc::retry_after(&e) {
                        Some(Some(secs)) => secs as i64,
                        _ => (backoff * 2).max(RETRY_MIN_S),
                    }
                    .min(RETRY_MAX_S);
                    next_at = now + backoff;
                }
            }
            drop(st);
            log_heap("after fetch");
        }

        // Weather: every half hour, and right away when the place or units change.
        if let Some((lat, lon)) = coordinates(&sh, &s) {
            let key = format!("{lat},{lon},{}", s.units);
            let now = clock::now();
            if clock::valid() && (now >= weather_at || key != weather_key) {
                weather_key = key;
                match fetch::weather(&lat, &lon, s.units(), now) {
                    Ok(Some(w)) => {
                        info!("weather: {} tenths, {:?}", w.current.temp, w.current.condition);
                        *sh.weather.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(w));
                        weather_at = now + WEATHER_EVERY_S;
                    }
                    Ok(None) => weather_at = now + WEATHER_RETRY_S,
                    Err(e) => {
                        warn!("weather fetch failed: {e:#}");
                        weather_at = now + WEATHER_RETRY_S;
                    }
                }
            }
        }
        let wait_s = (next_at.min(weather_at.max(clock::now() + 1)) - clock::now()).clamp(1, 60);
        wait_or_wake(&sh.wake_fetch, wait_s as u32 * 1000);
    }
}

/// Where to ask for weather: the coordinates typed in settings, else the
/// ones Hebcal reported for the chosen city (kept as text, never floats).
fn coordinates(sh: &Shared, s: &settings::Settings) -> Option<(String, String)> {
    if !s.lat.is_empty() && !s.lon.is_empty() {
        return Some((s.lat.clone(), s.lon.clone()));
    }
    let cal = sh.calendar.lock().unwrap_or_else(|e| e.into_inner()).clone()?;
    if cal.key != s.fetch_key() {
        return None;
    }
    Some((cal.model.location.latitude.clone()?, cal.model.location.longitude.clone()?))
}

/// Store the zone Hebcal reported, so the clock is right from the next boot.
fn remember_tz(sh: &Shared, key: &str, tzid: String) {
    let mut s = sh.settings.lock().unwrap_or_else(|e| e.into_inner());
    if s.tzid != tzid && s.fetch_key() == key {
        info!("time zone: {tzid}");
        s.tzid = tzid;
        if let Err(e) = settings::save(&sh.nvs, &s) {
            warn!("cannot save time zone: {e:#}");
        }
    }
}

/// Next 03:00 local, plus up to half an hour of jitter so devices do not
/// all hit Hebcal at once.
fn next_daily_refresh(tz: &Tz, now: i64) -> i64 {
    let local = tz.local(now);
    let day = if local.h >= 3 { local.date.add_days(1) } else { local.date };
    let target = day.days() * 86_400 + 3 * 3600 - tz.offset_at(now) as i64;
    let jitter = (unsafe { esp_idf_svc::sys::esp_random() } % 1800) as i64;
    target + jitter
}

fn short_error(e: &anyhow::Error) -> String {
    let s = format!("{e:#}");
    let s = s.split(':').next_back().unwrap_or(&s).trim().to_string();
    s.chars().take(60).collect()
}

fn wait_or_wake(flag: &AtomicBool, max_ms: u32) {
    let mut elapsed = 0u32;
    while elapsed < max_ms {
        FreeRtos::delay_ms(250);
        elapsed += 250;
        if flag.load(Ordering::Relaxed) {
            break;
        }
    }
}

fn log_heap(what: &str) {
    use esp_idf_svc::sys;
    let (free, min, largest) = unsafe {
        (
            sys::esp_get_free_heap_size(),
            sys::esp_get_minimum_free_heap_size(),
            sys::heap_caps_get_largest_free_block(sys::MALLOC_CAP_8BIT),
        )
    };
    info!("heap {what}: free {free}, min {min}, largest block {largest}");
}
