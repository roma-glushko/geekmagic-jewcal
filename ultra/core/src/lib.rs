//! JewCal's Rust core for the GeekMagic SmallTV Ultra (ESP8266).
//!
//! The Arduino firmware (`ultra/src`, C++) owns the hardware: WiFi, HTTP,
//! the display, EEPROM. It calls the functions below (`jc_*`, declared in
//! `ultra/src/jewcal_core.h`), which run [`jewcal_app::device::Device`]: the
//! same calendar model, screens and settings page as the SmallTV Pro.
//!
//! Everything runs on the Arduino `loop()` task, one call at a time.
//! Strings cross as NUL-terminated UTF-8; pages are written through a sink
//! callback in small chunks, never as one big buffer.

#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ffi::{c_char, c_void, CStr};
use core::fmt::{self, Write};

use jewcal_app::device::{Device, Outcome};
use jewcal_app::web;
use jewcal_app::Settings;
use jewcal_ui::canvas::{H, W};
use jewcal_ui::{palette, Canvas};

// ── Heap and panics ──────────────────────────────────────────────────────────

extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(p: *mut c_void);
    fn realloc(p: *mut c_void, size: usize) -> *mut c_void;
    /// Records the message for the next boot and restarts (C++ side).
    fn jc_panic(msg: *const u8, len: usize) -> !;
}

/// The Arduino core's umm_malloc: 4-byte aligned blocks.
struct Heap;

const MIN_ALIGN: usize = 4;

unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if l.align() <= MIN_ALIGN {
            return malloc(l.size()) as *mut u8;
        }
        // Over-allocate and keep the block's real address just below the
        // aligned pointer (types holding an i64 want 8-byte alignment).
        let raw = malloc(l.size() + l.align()) as *mut u8;
        if raw.is_null() {
            return raw;
        }
        let aligned = (raw as usize + MIN_ALIGN + l.align() - 1) & !(l.align() - 1);
        let aligned = aligned as *mut u8;
        (aligned as *mut *mut u8).sub(1).write(raw);
        aligned
    }

    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        if l.align() <= MIN_ALIGN {
            free(p as *mut c_void)
        } else {
            free((p as *mut *mut u8).sub(1).read() as *mut c_void)
        }
    }

    unsafe fn realloc(&self, p: *mut u8, l: Layout, new_size: usize) -> *mut u8 {
        if l.align() <= MIN_ALIGN {
            return realloc(p as *mut c_void, new_size) as *mut u8;
        }
        // Let umm_malloc grow the block in place when it can (a growing Vec
        // then never needs old + new at once), and re-align afterwards: the
        // block may have moved to an address with another offset.
        let raw = (p as *mut *mut u8).sub(1).read();
        let off = p as usize - raw as usize;
        let new_raw = realloc(raw as *mut c_void, new_size + l.align()) as *mut u8;
        if new_raw.is_null() {
            return new_raw;
        }
        let aligned = (new_raw as usize + MIN_ALIGN + l.align() - 1) & !(l.align() - 1);
        let new_off = aligned - new_raw as usize;
        if new_off != off {
            core::ptr::copy(new_raw.add(off), new_raw.add(new_off), l.size().min(new_size));
        }
        let aligned = aligned as *mut u8;
        (aligned as *mut *mut u8).sub(1).write(new_raw);
        aligned
    }
}

#[global_allocator]
static HEAP: Heap = Heap;

/// Writes into a fixed buffer, dropping what does not fit.
struct Fixed<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> Write for Fixed<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let n = s.len().min(N - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let mut out = Fixed::<120> { buf: [0; 120], len: 0 };
    if let Some(loc) = info.location() {
        // Keep the tail of the path: `src/canvas.rs:88`.
        let file = loc.file();
        let cut = file.rfind('/').and_then(|i| file[..i].rfind('/')).map_or(0, |i| i + 1);
        let _ = write!(out, "{}:{}: ", &file[cut..], loc.line());
    }
    let _ = write!(out, "{}", info.message());
    unsafe { jc_panic(out.buf.as_ptr(), out.len) }
}

// ── State ────────────────────────────────────────────────────────────────────

struct Core {
    dev: Device,
    canvas: Canvas,
    /// Fields of the form being submitted (`jc_form_add`).
    form: Vec<(String, String)>,
    /// `millis()` wraps every 49 days; the scheduler wants a monotonic u64.
    last_ms: u32,
    epoch_ms: u64,
}

struct Global(UnsafeCell<Option<Core>>);
// SAFETY: only the Arduino loop task calls in, one call at a time.
unsafe impl Sync for Global {}
static CORE: Global = Global(UnsafeCell::new(None));

fn core_mut() -> &'static mut Core {
    // SAFETY: single caller; no `jc_*` function re-enters another.
    unsafe { (*CORE.0.get()).as_mut().expect("jc_init not called") }
}

impl Core {
    fn mono(&mut self, now_ms: u32) -> u64 {
        if now_ms < self.last_ms {
            self.epoch_ms += 1 << 32;
        }
        self.last_ms = now_ms;
        self.epoch_ms + now_ms as u64
    }

    fn field(&self, k: &str) -> Option<String> {
        self.form.iter().find(|(name, _)| name == k).map(|(_, v)| v.clone())
    }
}

unsafe fn text<'a>(p: *const c_char) -> &'a str {
    if p.is_null() {
        return "";
    }
    CStr::from_ptr(p).to_str().unwrap_or("")
}

unsafe fn bytes<'a>(p: *const u8, len: usize) -> &'a [u8] {
    if p.is_null() || len == 0 {
        &[]
    } else {
        core::slice::from_raw_parts(p, len)
    }
}

/// Copy `s` NUL-terminated into `out` (truncated to fit). Returns the
/// length of `s`, so the caller can tell when it was cut.
unsafe fn put(s: &str, out: *mut u8, cap: usize) -> usize {
    if !out.is_null() && cap > 0 {
        let n = s.len().min(cap - 1);
        core::ptr::copy_nonoverlapping(s.as_ptr(), out, n);
        *out.add(n) = 0;
    }
    s.len()
}

// ── Lifecycle ────────────────────────────────────────────────────────────────

/// Start the core with the settings text stored in EEPROM (may be empty).
#[no_mangle]
pub unsafe extern "C" fn jc_init(settings: *const u8, len: usize, seed: u32, now_ms: u32) {
    let text = core::str::from_utf8(bytes(settings, len)).unwrap_or("");
    let core = Core {
        dev: Device::new(Settings::from_text(text), now_ms as u64, seed),
        canvas: Canvas::new(),
        form: Vec::new(),
        last_ms: now_ms,
        epoch_ms: 0,
    };
    *CORE.0.get() = Some(core);
}

/// The settings as text, for EEPROM. Returns the full length (the copy is
/// cut at `cap - 1` bytes).
#[no_mangle]
pub unsafe extern "C" fn jc_settings_text(out: *mut u8, cap: usize) -> usize {
    put(&core_mut().dev.settings().to_text(), out, cap)
}

/// Whether the device changed its own settings (the time zone it learned)
/// since the last call: save them.
#[no_mangle]
pub extern "C" fn jc_take_settings_dirty() -> bool {
    core_mut().dev.take_settings_dirty()
}

/// The station IP as text, or NULL while offline.
#[no_mangle]
pub unsafe extern "C" fn jc_set_ip(ip: *const c_char) {
    let ip = (!ip.is_null()).then(|| text(ip).to_string());
    core_mut().dev.set_ip(ip);
}

// ── Drawing ──────────────────────────────────────────────────────────────────

/// Draw the current screen into the canvas. Returns the backlight percent.
#[no_mangle]
pub extern "C" fn jc_frame(now: i64, now_ms: u32) -> u8 {
    let core = core_mut();
    let ms = core.mono(now_ms);
    core.dev.frame(&mut core.canvas, now, ms)
}

/// A boot or status message (title, one line, palette color).
#[no_mangle]
pub unsafe extern "C" fn jc_message(title: *const c_char, line: *const c_char, color: u8, frame: u32) {
    let line = text(line);
    let lines: &[&str] = if line.is_empty() { &[] } else { &[line] };
    jewcal_ui::draw_message(&mut core_mut().canvas, text(title), lines, color, frame);
}

/// The setup screen: join WiFi `ssid`, open `url`.
#[no_mangle]
pub unsafe extern "C" fn jc_setup_screen(ssid: *const c_char, url: *const c_char, frame: u32) {
    jewcal_ui::draw_setup(&mut core_mut().canvas, text(ssid), text(url), frame);
}

/// Palette indices of canvas row `y` (120 bytes).
#[no_mangle]
pub unsafe extern "C" fn jc_row(y: u32, out: *mut u8) {
    if out.is_null() || y >= H as u32 {
        return;
    }
    let out = &mut *(out as *mut [u8; W as usize]);
    core_mut().canvas.row(y as usize, out);
}

/// The 16 palette colors as RGB565.
#[no_mangle]
pub unsafe extern "C" fn jc_lut(out: *mut u16) {
    if !out.is_null() {
        core::ptr::copy_nonoverlapping(palette::lut().as_ptr(), out, 16);
    }
}

/// Palette index constants for `jc_message`.
#[no_mangle]
pub extern "C" fn jc_color(name: u8) -> u8 {
    match name {
        b'r' => palette::RED,
        b's' => palette::SKY,
        b'g' => palette::GREEN,
        _ => palette::YELLOW,
    }
}

/// Header of `/screen.bmp` (8-bit palette, 120×120, rows bottom-up).
/// Writes up to `cap` bytes; returns its length (118).
#[no_mangle]
pub unsafe extern "C" fn jc_bmp_header(out: *mut u8, cap: usize) -> usize {
    let h = web::bmp_header();
    if !out.is_null() {
        core::ptr::copy_nonoverlapping(h.as_ptr(), out, h.len().min(cap));
    }
    h.len()
}

// ── Fetching ─────────────────────────────────────────────────────────────────

/// The next GET to make: writes the URL (NUL-terminated) and returns 1
/// (events), 2 (sunrise/sunset) or 3 (weather); 0 when nothing is due.
/// Then: `jc_feed` the body, `jc_finish` once.
#[no_mangle]
pub unsafe extern "C" fn jc_next_request(now: i64, clock_valid: bool, url: *mut u8, cap: usize) -> u8 {
    match core_mut().dev.next_request(now, clock_valid) {
        Some((kind, u)) if u.len() < cap => {
            put(&u, url, cap);
            kind as u8
        }
        Some(_) => {
            // Cannot happen with a 512-byte buffer; drop the request.
            core_mut().dev.cancel(now);
            0
        }
        None => 0,
    }
}

#[no_mangle]
pub unsafe extern "C" fn jc_feed(data: *const u8, len: usize) {
    core_mut().dev.feed(bytes(data, len));
}

/// `status`: 200 = complete body; another HTTP status; ≤ 0 = network error
/// or truncated body. `retry_after`: seconds from the header, or < 0.
#[no_mangle]
pub extern "C" fn jc_finish(status: i32, retry_after: i32, now: i64) {
    let outcome = match status {
        200 => Outcome::Ok,
        s if s > 0 => Outcome::Status {
            code: s.min(u16::MAX as i32) as u16,
            retry_after: (retry_after > 0).then_some(retry_after as u32),
        },
        _ => Outcome::Network,
    };
    core_mut().dev.finish(outcome, now);
}

// ── Web pages ────────────────────────────────────────────────────────────────

/// Receives a page in chunks: `sink(ctx, bytes, len)`.
pub type Sink = unsafe extern "C" fn(ctx: *mut c_void, data: *const u8, len: usize);

/// A `fmt::Write` that hands the text to the sink 256 bytes at a time.
struct Chunks {
    sink: Sink,
    ctx: *mut c_void,
    buf: [u8; 256],
    len: usize,
}

impl Chunks {
    fn new(sink: Sink, ctx: *mut c_void) -> Self {
        Self { sink, ctx, buf: [0; 256], len: 0 }
    }

    fn flush(&mut self) {
        if self.len > 0 {
            unsafe { (self.sink)(self.ctx, self.buf.as_ptr(), self.len) };
            self.len = 0;
        }
    }
}

impl Write for Chunks {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut s = s.as_bytes();
        while !s.is_empty() {
            let n = s.len().min(self.buf.len() - self.len);
            self.buf[self.len..self.len + n].copy_from_slice(&s[..n]);
            self.len += n;
            s = &s[n..];
            if self.len == self.buf.len() {
                self.flush();
            }
        }
        Ok(())
    }
}

impl Drop for Chunks {
    fn drop(&mut self) {
        self.flush();
    }
}

/// The SmallTV Ultra's own section of the settings page. `info` is a line of
/// plain text from the firmware (version, memory, last restart); `token` is
/// the per-boot secret in the update and forget links (see web.cpp).
fn extras(info: &str, token: &str) -> String {
    let token = web::html_escape(token);
    alloc::format!(
        "<fieldset><legend>Device</legend><p class=dim>{}</p>\
<p><a href='/update-{token}'>Update firmware</a></p>\
<form method=post action=/forget onsubmit=\"return confirm('Forget this WiFi network and restart into setup?')\">\
<input type=hidden name=token value='{token}'>\
<button style='background:var(--red);color:var(--text)'>Forget WiFi</button></form></fieldset>",
        web::html_escape(info)
    )
}

#[no_mangle]
pub unsafe extern "C" fn jc_settings_page(
    sink: Sink,
    ctx: *mut c_void,
    notice: *const c_char,
    info: *const c_char,
    token: *const c_char,
) {
    let mut out = Chunks::new(sink, ctx);
    let _ = core_mut().dev.settings_page(&mut out, text(notice), &extras(text(info), text(token)));
}

/// Start collecting a submitted form.
#[no_mangle]
pub extern "C" fn jc_form_reset() {
    core_mut().form.clear();
}

/// One decoded form field.
#[no_mangle]
pub unsafe extern "C" fn jc_form_add(name: *const c_char, value: *const c_char) {
    let core = core_mut();
    if core.form.len() < 32 {
        core.form.push((text(name).to_string(), text(value).chars().take(120).collect()));
    }
}

/// Apply the collected settings form and write the resulting page. Returns
/// true when the settings changed (save them).
#[no_mangle]
pub unsafe extern "C" fn jc_form_apply(sink: Sink, ctx: *mut c_void, info: *const c_char, token: *const c_char) -> bool {
    let core = core_mut();
    let form = core::mem::take(&mut core.form);
    let field = |k: &str| form.iter().find(|(name, _)| name == k).map(|(_, v)| v.clone());
    let result = core.dev.apply_form(field);
    drop(form);
    let mut out = Chunks::new(sink, ctx);
    let _ = core.dev.settings_page(&mut out, &web::save_notice(&result), &extras(text(info), text(token)));
    result.is_ok()
}

/// First-run page on the access point, with the networks in range.
#[no_mangle]
pub unsafe extern "C" fn jc_setup_page(sink: Sink, ctx: *mut c_void, networks: *const *const c_char, n: usize) {
    let mut list = Vec::new();
    for i in 0..n {
        let p = *networks.add(i);
        let name = text(p);
        if !name.is_empty() && !list.iter().any(|x: &String| x == name) {
            list.push(name.to_string());
        }
    }
    let mut out = Chunks::new(sink, ctx);
    let _ = web::setup_page(&mut out, &list);
}

/// Check the collected WiFi form. On success copies the SSID and password
/// out (NUL-terminated) and returns true; writes the answer page either way.
#[no_mangle]
pub unsafe extern "C" fn jc_wifi_form(sink: Sink, ctx: *mut c_void, ssid: *mut u8, pass: *mut u8) -> bool {
    let core = core_mut();
    let pass_text = core.field("pass").unwrap_or_default();
    let picked = web::wifi_ssid(|k| core.field(k));
    core.form.clear();
    let mut out = Chunks::new(sink, ctx);
    match web::wifi_form(picked, &pass_text) {
        Ok((name, msg)) => {
            put(&name, ssid, 33);
            put(&pass_text, pass, 65);
            let _ = web::message_page(&mut out, &msg);
            true
        }
        Err(msg) => {
            let _ = web::message_page(&mut out, &msg);
            false
        }
    }
}

/// A bare page with one message (`html` is trusted HTML from the firmware).
#[no_mangle]
pub unsafe extern "C" fn jc_message_page(sink: Sink, ctx: *mut c_void, html: *const c_char) {
    let mut out = Chunks::new(sink, ctx);
    let _ = web::message_page(&mut out, text(html));
}

/// The Hebcal city-search URL for `q` (NUL-terminated into `out`); 0 when
/// `q` is too short or too long.
#[no_mangle]
pub unsafe extern "C" fn jc_city_url(q: *const c_char, out: *mut u8, cap: usize) -> usize {
    match Device::city_url(text(q)) {
        Some(u) if u.len() < cap => put(&u, out, cap),
        _ => 0,
    }
}
