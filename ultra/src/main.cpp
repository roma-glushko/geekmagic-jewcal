// JewCal for the GeekMagic SmallTV Ultra (ESP8266): a pixel-art Jewish
// calendar. The Rust core (ultra/core, the same model, screens and settings
// page as the SmallTV Pro firmware) decides what to show and what to fetch;
// this file does the hardware and the network.
//
// Boot: crash check (safe mode) -> display -> settings from EEPROM -> WiFi
// (or the setup access point) -> clock -> web server -> loop: draw a frame
// every 200 ms, make at most one HTTP request at a time.

#include <Arduino.h>
#include <ESP8266WiFi.h>
#include <ESP8266mDNS.h>
#include <time.h>

#include <memory>
#include <new>

#include "board.h"
#include "crash.h"
#include "display.h"
#include "jewcal_core.h"
#include "net.h"
#include "store.h"
#include "web.h"

namespace {

enum class Mode { Normal, Setup, Safe };

constexpr uint32_t FRAME_MS = 200;
constexpr uint32_t POLL_MS = 500;
constexpr uint32_t WIFI_TIMEOUT_MS = 20000;
constexpr int WIFI_ATTEMPTS = 3;
// After this long without a crash, earlier crashes no longer count.
constexpr uint32_t STABLE_MS = 2 * 60 * 1000;

Mode mode = Mode::Normal;
uint32_t restart_at = 0;
uint32_t frame_no = 0;
uint32_t last_frame = 0;
uint32_t last_poll = 0;
uint32_t last_log = 0;
bool stable = false;
bool was_online = false;
String setup_ap;
String networks[16];

bool clock_valid() { return time(nullptr) > 1700000000; }

void message(const char *title, const char *line, char color) {
  jc_message(title, line, jc_color(color), frame_no++);
  display::flush();
}

void log_heap(const char *what) {
  Serial.printf("heap %s: free %u, largest block %u, stack free %u\n", what, ESP.getFreeHeap(),
                ESP.getMaxFreeBlockSize(), ESP.getFreeContStack());
}

// ── Normal mode ─────────────────────────────────────────────────────────────

void fetch(uint8_t kind, const char *url) {
  Serial.printf("GET %s\n", url);
  int retry_after = 0;
  const int code = net::get(url, [](const uint8_t *data, size_t len, void *) { jc_feed(data, len); }, nullptr,
                            &retry_after);
  jc_finish(code, retry_after, time(nullptr));
  Serial.printf("  -> %d\n", code);
  log_heap(kind == JC_WEATHER ? "after weather" : "after hebcal");
}

void normal_begin() {
  configTime(0, 0, "pool.ntp.org", "time.google.com", "time.cloudflare.com");
  const String ip = net::ip();
  jc_set_ip(ip.c_str());
  was_online = true;
  Serial.printf("settings page: http://%s/ and http://%s.local/\n", ip.c_str(), HOSTNAME);
  if (MDNS.begin(HOSTNAME)) MDNS.addService("http", "tcp", 80);
  web::start_normal();
  for (int i = 0; i < 300 && !clock_valid(); i++) {
    message("SYNCING TIME", i > 100 ? "check the network" : "pool.ntp.org", 'y');
    for (int k = 0; k < 10; k++) {
      web::loop();
      delay(10);
    }
  }
  log_heap("running");
}

void normal_loop() {
  web::loop();
  MDNS.update();
  const uint32_t ms = millis();

  const bool online = net::online();
  if (online != was_online) {
    was_online = online;
    jc_set_ip(online ? net::ip().c_str() : nullptr);
    Serial.printf("wifi %s\n", online ? "back" : "lost");
  }

  if (ms - last_frame >= FRAME_MS) {
    last_frame = ms;
    const uint8_t level = jc_frame(time(nullptr), ms);
    display::flush();
    display::backlight(level);
  }

  if (online && ms - last_poll >= POLL_MS) {
    last_poll = ms;
    // Static: the loop stack is only 4 KB, and the Rust calls below need it.
    static uint8_t url[512];
    const uint8_t kind = jc_next_request(time(nullptr), clock_valid(), url, sizeof url);
    if (kind != JC_NONE) fetch(kind, reinterpret_cast<const char *>(url));
  }

  if (jc_take_settings_dirty()) save_settings();
  if (!stable && ms > STABLE_MS) {
    stable = true;
    crash::stable();
  }
  if (ms - last_log >= 10 * 60 * 1000) {
    last_log = ms;
    log_heap("periodic");
  }
}

// ── Setup mode (no network) ─────────────────────────────────────────────────

void setup_begin() {
  WiFi.persistent(false);
  WiFi.mode(WIFI_AP_STA);
  const int found = WiFi.scanNetworks();
  size_t n = 0;
  for (int i = 0; i < found && n < 16; i++) {
    const String ssid = WiFi.SSID(i);
    if (ssid.length()) networks[n++] = ssid;
  }
  WiFi.scanDelete();
  setup_ap = net::ap_name();
  WiFi.softAP(setup_ap.c_str());
  Serial.printf("setup: join %s, open http://%s/ (%u networks in range)\n", setup_ap.c_str(),
                WiFi.softAPIP().toString().c_str(), (unsigned)n);
  web::start_setup(networks, n);
}

void setup_loop() {
  web::loop();
  const uint32_t ms = millis();
  if (ms - last_frame >= 250) {
    last_frame = ms;
    const String url = "http://" + WiFi.softAPIP().toString() + "/";
    jc_setup_screen(setup_ap.c_str(), url.c_str(), frame_no++);
    display::flush();
  }
}

// ── Safe mode (repeated crashes) ────────────────────────────────────────────

void safe_screen(const String &where) {
  constexpr uint16_t BG = display::rgb(0x1a, 0x1c, 0x2c), RED = display::rgb(0xb1, 0x3e, 0x53);
  constexpr uint16_t WHITE = display::rgb(0xf4, 0xf4, 0xf4), SKY = display::rgb(0x41, 0xa6, 0xf6);
  display::fill(BG);
  display::text_center(30, "SAFE MODE", RED, BG, 3);
  display::text_center(70, "JEWCAL KEPT CRASHING", WHITE, BG, 1);
  display::text_center(100, "OPEN IN A BROWSER", WHITE, BG, 1);
  display::text_center(116, where.c_str(), SKY, BG, 1);
  display::text_center(140, "TO UPDATE THE FIRMWARE", WHITE, BG, 1);
}

void safe_begin() {
  Serial.printf("safe mode after %u crashes; last: %s\n", crash::count(), crash::last());
  display::backlight(70);
  safe_screen("CONNECTING...");
  String where;
  if (net::join(WIFI_TIMEOUT_MS)) {
    where = "HTTP://" + net::ip();
    if (MDNS.begin(HOSTNAME)) MDNS.addService("http", "tcp", 80);
  } else {
    net::stop_joining();
    WiFi.mode(WIFI_AP_STA);
    setup_ap = net::ap_name();
    WiFi.softAP(setup_ap.c_str());
    where = "WIFI " + setup_ap + ": HTTP://" + WiFi.softAPIP().toString();
  }
  safe_screen(where);
  web::start_safe();
}

void safe_loop() {
  web::loop();
  MDNS.update();
}

}  // namespace

// ── Shared with web.cpp ─────────────────────────────────────────────────────

String device_info() {
  const uint32_t up = millis() / 60000;
  char buf[200];
  snprintf(buf, sizeof buf,
           "JewCal %s for the SmallTV Ultra. Up %ud %uh %um, free memory %u B (largest block %u B), "
           "stack never below %u B free.",
           JEWCAL_VERSION, up / 1440, up / 60 % 24, up % 60, ESP.getFreeHeap(), ESP.getMaxFreeBlockSize(),
           ESP.getFreeContStack());
  String s = buf;
  if (crash::last()[0]) {
    s += F(" Last crash: ");
    s += crash::last();
  }
  return s;
}

void save_settings() {
  // Rare: allocate just what the text needs, for as long as it takes.
  const size_t len = jc_settings_text(nullptr, 0);
  std::unique_ptr<uint8_t[]> text(new (std::nothrow) uint8_t[len + 1]);
  if (!text || jc_settings_text(text.get(), len + 1) != len || !store::save(text.get(), len)) {
    Serial.println(F("settings: not saved"));
  }
}

void restart_soon() { restart_at = millis() + 1500; }

extern "C" void jc_panic(const uint8_t *msg, size_t len) {
  crash::record_panic(reinterpret_cast<const char *>(msg), len);
  Serial.print(F("\npanic: "));
  Serial.write(msg, len);
  Serial.println();
  Serial.flush();
  ESP.restart();
  while (true) delay(1000);
}

void setup() {
  // Backlight off (active low) until something is drawn.
  pinMode(PIN_BL, OUTPUT);
  digitalWrite(PIN_BL, HIGH);
  Serial.begin(115200);
  Serial.printf("\n\nJewCal %s (SmallTV Ultra), reset: %s\n", JEWCAL_VERSION, ESP.getResetReason().c_str());

  const bool safe = crash::begin();
  display::begin();
  if (safe) {
    mode = Mode::Safe;
    safe_begin();
    return;
  }

  {
    const String text = store::load();
    jc_init(reinterpret_cast<const uint8_t *>(text.c_str()), text.length(), RANDOM_REG32, millis());
  }
  message("JEWCAL", "starting", 'y');
  display::backlight(80);
  log_heap("boot");

  if (!net::has_saved_network()) {
    mode = Mode::Setup;
    setup_begin();
    return;
  }
  const String ssid = net::saved_ssid();
  for (int attempt = 1;; attempt++) {
    message("CONNECTING", ssid.c_str(), 's');
    if (net::join(WIFI_TIMEOUT_MS)) break;
    Serial.printf("wifi: attempt %d to join %s failed\n", attempt, ssid.c_str());
    if (attempt >= WIFI_ATTEMPTS) {
      mode = Mode::Setup;
      setup_begin();
      return;
    }
  }
  mode = Mode::Normal;
  normal_begin();
}

void loop() {
  switch (mode) {
    case Mode::Normal:
      normal_loop();
      break;
    case Mode::Setup:
      setup_loop();
      break;
    case Mode::Safe:
      safe_loop();
      break;
  }
  if (restart_at && (int32_t)(millis() - restart_at) >= 0) ESP.restart();
}
