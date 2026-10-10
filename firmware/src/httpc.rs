//! One-shot HTTPS GETs whose body is handed to the caller as a capped
//! `std::io::Read`, so JSON can be parsed while it streams in (adapted from
//! Clawdmeter's `httpc.rs`). The ESP32 has no PSRAM: a 40 KB response is
//! never held in memory.

use std::io;
use std::sync::Mutex;
use std::time::Duration;

use embedded_svc::http::{client::Client, Method};
use esp_idf_svc::http::client::{Configuration as HttpConfig, EspHttpConnection};

const USER_AGENT: &str = concat!("jewcal/", env!("CARGO_PKG_VERSION"), " (GeekMagic SmallTV desk calendar)");

/// One TLS session at a time: each one costs ~40 KB of heap.
static TLS: Mutex<()> = Mutex::new(());

/// HTTP 429, with the server's `Retry-After` in seconds when it sent one.
#[derive(Debug)]
pub struct RateLimited {
    pub retry_after: Option<u32>,
}

impl std::fmt::Display for RateLimited {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rate limited by hebcal.com")
    }
}

impl std::error::Error for RateLimited {}

/// GET `url` and pass at most `max_bytes` of the body to `f`.
pub fn get<T>(
    url: &str,
    max_bytes: usize,
    f: impl FnOnce(&mut dyn io::Read) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let _tls = TLS.lock().unwrap_or_else(|e| e.into_inner());
    let conn = EspHttpConnection::new(&HttpConfig {
        use_global_ca_store: true,
        crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
        timeout: Some(Duration::from_secs(20)),
        ..Default::default()
    })?;
    let mut client = Client::wrap(conn);
    let headers = [("User-Agent", USER_AGENT), ("Accept", "application/json")];
    let mut resp = client.request(Method::Get, url, &headers)?.submit()?;

    let status = resp.status();
    if status == 429 {
        let retry_after = resp.header("retry-after").and_then(|s| s.trim().parse().ok());
        drain(&mut resp);
        return Err(RateLimited { retry_after }.into());
    }
    if status != 200 {
        drain(&mut resp);
        anyhow::bail!("HTTP {status}");
    }
    let out = f(&mut Body { inner: &mut resp, left: max_bytes });
    // Read to EOF even when the parser stopped early, or mbedTLS complains
    // about the socket being closed mid-response.
    drain(&mut resp);
    out
}

struct Body<'a, R> {
    inner: &'a mut R,
    left: usize,
}

impl<R: embedded_svc::io::Read> io::Read for Body<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.left == 0 {
            return Err(io::Error::other("response larger than expected"));
        }
        let n = buf.len().min(self.left);
        let got = self.inner.read(&mut buf[..n]).map_err(|e| io::Error::other(format!("read: {e:?}")))?;
        self.left -= got;
        Ok(got)
    }
}

fn drain<R: embedded_svc::io::Read>(r: &mut R) {
    let mut chunk = [0u8; 256];
    while let Ok(n) = r.read(&mut chunk) {
        if n == 0 {
            break;
        }
    }
}

pub fn retry_after(e: &anyhow::Error) -> Option<Option<u32>> {
    e.downcast_ref::<RateLimited>().map(|r| r.retry_after)
}
