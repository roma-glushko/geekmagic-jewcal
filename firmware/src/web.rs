//! The device's web pages: settings (location search, times, display) with a
//! live screenshot, and the first-run WiFi setup page served on the AP. The
//! pages and the form logic live in `jewcal_app::web`, shared with the
//! SmallTV Ultra firmware; this module serves them.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use embedded_svc::http::Method;
use embedded_svc::io::Write;
use esp_idf_svc::http::server::{Configuration as HttpServerConfig, EspHttpServer};
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use jewcal_app::web::{self, Applied, PageStatus};
use jewcal_ui::canvas::{H, W};

use crate::state::Shared;
use crate::{fetch, settings};

/// Handlers do TLS (city search) and build pages: 6 KB is not enough.
const STACK: usize = 12_288;
const MAX_BODY: usize = 2048;

pub fn start(sh: Arc<Shared>) -> anyhow::Result<EspHttpServer<'static>> {
    let mut server =
        EspHttpServer::new(&HttpServerConfig { stack_size: STACK, max_uri_handlers: 8, ..Default::default() })?;

    let s = sh.clone();
    server.fn_handler("/", Method::Get, move |req| -> anyhow::Result<()> {
        let page = settings_page(&s, "");
        req.into_response(200, None, &[("Content-Type", "text/html; charset=utf-8")])?.write_all(page.as_bytes())?;
        Ok(())
    })?;

    let s = sh.clone();
    server.fn_handler("/save", Method::Post, move |mut req| -> anyhow::Result<()> {
        let body = read_body(&mut req);
        let notice = web::save_notice(&apply_form(&s, &body));
        let page = settings_page(&s, &notice);
        req.into_response(200, None, &[("Content-Type", "text/html; charset=utf-8")])?.write_all(page.as_bytes())?;
        Ok(())
    })?;

    server.fn_handler("/cities", Method::Get, move |req| -> anyhow::Result<()> {
        let q = web::query_param(req.uri(), "q").unwrap_or_default();
        let q = q.trim();
        let body = if q.len() < 2 || q.len() > 40 {
            "[]".to_string()
        } else {
            fetch::search_cities(q).unwrap_or_else(|e| {
                log::warn!("city search failed: {e:#}");
                "[]".into()
            })
        };
        req.into_response(200, None, &[("Content-Type", "application/json")])?.write_all(body.as_bytes())?;
        Ok(())
    })?;

    let s = sh.clone();
    server.fn_handler("/screen.bmp", Method::Get, move |req| -> anyhow::Result<()> {
        // Copy to the heap: a 14 KB array would not fit the handler stack.
        let px: Vec<u8> = s.frame.lock().unwrap_or_else(|e| e.into_inner()).px.to_vec();
        let mut resp = req.into_response(200, None, &[("Content-Type", "image/bmp"), ("Cache-Control", "no-store")])?;
        resp.write_all(&web::bmp_header())?;
        // BMP rows go bottom-up; 120 bytes per row needs no padding.
        for y in (0..H as usize).rev() {
            resp.write_all(&px[y * W as usize..(y + 1) * W as usize])?;
        }
        Ok(())
    })?;

    Ok(server)
}

/// First-run setup on the access point: pick a network, enter a password.
pub fn start_setup(nvs: EspDefaultNvsPartition, networks: Vec<String>) -> anyhow::Result<EspHttpServer<'static>> {
    let mut server = EspHttpServer::new(&HttpServerConfig { stack_size: STACK, ..Default::default() })?;
    let mut page = String::new();
    web::setup_page(&mut page, &networks)?;
    server.fn_handler("/", Method::Get, move |req| -> anyhow::Result<()> {
        req.into_response(200, None, &[("Content-Type", "text/html; charset=utf-8")])?.write_all(page.as_bytes())?;
        Ok(())
    })?;
    server.fn_handler("/wifi", Method::Post, move |mut req| -> anyhow::Result<()> {
        let body = read_body(&mut req);
        let field = |k: &str| web::form_field(&body, k);
        let pass = field("pass").unwrap_or_default();
        let msg = match web::wifi_form(web::wifi_ssid(field), &pass) {
            Ok((ssid, msg)) => {
                settings::save_wifi(&nvs, &ssid, &pass)?;
                log::info!("wifi credentials saved for {ssid:?}, restarting");
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                    unsafe { esp_idf_svc::sys::esp_restart() };
                });
                msg
            }
            Err(msg) => msg,
        };
        let mut page = String::new();
        web::message_page(&mut page, &msg)?;
        req.into_response(200, None, &[("Content-Type", "text/html; charset=utf-8")])?.write_all(page.as_bytes())?;
        Ok(())
    })?;
    Ok(server)
}

fn settings_page(sh: &Shared, notice: &str) -> String {
    let s = sh.settings();
    let st = sh.status.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let mut page = String::new();
    let status = PageStatus { error: st.error.as_deref(), fetching: st.fetching };
    // Writing into a String cannot fail.
    let _ = web::settings_page(&mut page, &s, &status, notice, "");
    page
}

/// Apply a submitted settings form, persist it, and wake the fetch thread
/// when the calendar or the weather must be fetched again.
fn apply_form(sh: &Shared, body: &str) -> Result<Applied, String> {
    let mut s = sh.settings();
    let applied = web::apply_form(&mut s, |k| web::form_field(body, k))?;
    settings::save(&sh.nvs, &s).map_err(|e| format!("{e:#}"))?;
    *sh.settings.lock().unwrap_or_else(|e| e.into_inner()) = s;
    if applied.refetch || applied.units_changed {
        sh.wake_fetch.store(true, Ordering::Relaxed);
    }
    Ok(applied)
}

fn read_body<C: embedded_svc::http::server::Connection>(req: &mut embedded_svc::http::server::Request<C>) -> String {
    let mut body = Vec::new();
    let mut buf = [0u8; 256];
    loop {
        match req.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if body.len() + n > MAX_BODY {
                    break;
                }
                body.extend_from_slice(&buf[..n]);
            }
            Err(_) => break,
        }
    }
    String::from_utf8_lossy(&body).into_owned()
}
