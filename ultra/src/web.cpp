#include "web.h"

#include <DNSServer.h>
#include <ESP8266HTTPUpdateServer.h>
#include <ESP8266WebServer.h>
#include <ESP8266WiFi.h>

#include "crash.h"
#include "jewcal_core.h"
#include "net.h"

namespace web {

namespace {

ESP8266WebServer server(80);
ESP8266HTTPUpdateServer updater;
DNSServer dns;
bool captive = false;
// Per boot. The firmware update page lives at /update-<token>, and /forget
// wants the token: other web pages cannot read the settings page to learn
// it, so they cannot post a firmware or a WiFi reset blindly.
char token[9];
String update_path;

void make_token() {
  snprintf(token, sizeof token, "%08x", (unsigned)RANDOM_REG32);
  update_path = String("/update-") + token;
}

void sink(void *, const uint8_t *data, size_t len) { server.sendContent(reinterpret_cast<const char *>(data), len); }

void begin_chunked(const char *type) {
  server.sendHeader(F("Cache-Control"), F("no-store"));
  server.setContentLength(CONTENT_LENGTH_UNKNOWN);
  server.send(200, type, "");
}

// Hands the form's decoded fields to the Rust core.
void collect_form() {
  jc_form_reset();
  for (int i = 0; i < server.args(); i++) {
    const String name = server.argName(i);
    if (name != "plain") jc_form_add(name.c_str(), server.arg(i).c_str());
  }
}

void page_settings() {
  begin_chunked("text/html; charset=utf-8");
  jc_settings_page(sink, nullptr, "", device_info().c_str(), token);
  server.sendContent("");
}

void page_save() {
  collect_form();
  begin_chunked("text/html; charset=utf-8");
  const bool saved = jc_form_apply(sink, nullptr, device_info().c_str(), token);
  server.sendContent("");
  if (saved) save_settings();
}

struct Proxy {
  bool sent;
};

// City search: Hebcal's JSON passed through as it arrives (~2 KB).
void page_cities() {
  uint8_t url[256];
  if (!jc_city_url(server.arg("q").c_str(), url, sizeof url)) {
    server.send(200, F("application/json"), F("[]"));
    return;
  }
  begin_chunked("application/json");
  Proxy p{false};
  const int code = net::get(
      reinterpret_cast<const char *>(url),
      [](const uint8_t *data, size_t len, void *ctx) {
        static_cast<Proxy *>(ctx)->sent = true;
        sink(nullptr, data, len);
      },
      &p, nullptr);
  if (code != 200 && !p.sent) server.sendContent("[]");
  server.sendContent("");
}

// The canvas as an 8-bit BMP (rows bottom-up), like the SmallTV Pro's.
void page_screen() {
  uint8_t head[128];
  const size_t hl = jc_bmp_header(head, sizeof head);
  server.sendHeader(F("Cache-Control"), F("no-store"));
  server.setContentLength(hl + 120 * 120);
  server.send(200, F("image/bmp"), "");
  WiFiClient client = server.client();
  client.write(head, hl);
  uint8_t rows[120 * 4];
  for (int y = 119; y >= 0;) {
    size_t n = 0;
    for (int k = 0; k < 4 && y >= 0; k++, y--, n += 120) jc_row(y, rows + n);
    client.write(rows, n);
  }
}

void page_forget() {
  if (server.arg("token") != token) {
    server.send(403, F("text/plain"), F("Reload the settings page and try again."));
    return;
  }
  begin_chunked("text/html; charset=utf-8");
  String msg = F("<p class=ok>WiFi forgotten. JewCal restarts and opens its own network <b>");
  msg += net::ap_name();
  msg += F("</b>: join it to choose another one.</p>");
  jc_message_page(sink, nullptr, msg.c_str());
  server.sendContent("");
  net::forget();
  restart_soon();
}

// Old bookmarks and instructions: the update page has a per-boot address.
void page_update_moved() {
  begin_chunked("text/html; charset=utf-8");
  jc_message_page(sink, nullptr,
                  "<p>Firmware updates: open the <a href='/'>settings page</a> and use "
                  "<b>Update firmware</b> at the bottom.</p>");
  server.sendContent("");
}

void redirect_home() {
  server.sendHeader(F("Location"), String("http://") + WiFi.softAPIP().toString() + "/");
  server.send(302, F("text/plain"), "");
}

}  // namespace

void start_normal() {
  server.on("/", HTTP_GET, page_settings);
  server.on("/save", HTTP_POST, page_save);
  server.on("/cities", HTTP_GET, page_cities);
  server.on("/screen.bmp", HTTP_GET, page_screen);
  server.on("/forget", HTTP_POST, page_forget);
  make_token();
  updater.setup(&server, update_path);
  server.on("/update", HTTP_GET, page_update_moved);
  server.onNotFound([] { server.send(404, F("text/plain"), F("not found")); });
  server.begin();
}

// Kept for the WiFi form handler: names in range, as C strings.
static const char *network_names[16];
static size_t network_count = 0;

void start_setup(const String *networks, size_t count) {
  network_count = count < 16 ? count : 16;
  for (size_t i = 0; i < network_count; i++) network_names[i] = networks[i].c_str();
  server.on("/", HTTP_GET, [] {
    begin_chunked("text/html; charset=utf-8");
    jc_setup_page(sink, nullptr, network_names, network_count);
    server.sendContent("");
  });
  server.on("/wifi", HTTP_POST, [] {
    collect_form();
    uint8_t ssid[33], pass[65];
    begin_chunked("text/html; charset=utf-8");
    const bool ok = jc_wifi_form(sink, nullptr, ssid, pass);
    server.sendContent("");
    if (ok) {
      net::remember(reinterpret_cast<const char *>(ssid), reinterpret_cast<const char *>(pass));
      restart_soon();
    }
  });
  updater.setup(&server);
  // Captive portal: phones open the setup page by themselves.
  server.onNotFound(redirect_home);
  dns.start(53, "*", WiFi.softAPIP());
  captive = true;
  server.begin();
}

static String html_escape(const char *s) {
  String out;
  for (; *s; s++) {
    switch (*s) {
      case '&': out += F("&amp;"); break;
      case '<': out += F("&lt;"); break;
      case '>': out += F("&gt;"); break;
      case '"': out += F("&quot;"); break;
      default: out += *s;
    }
  }
  return out;
}

void start_safe() {
  server.on("/", HTTP_GET, [] {
    String page = F(
        "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'>"
        "<title>JewCal safe mode</title><style>body{background:#1a1c2c;color:#f4f4f4;font:15px/1.5 "
        "ui-monospace,Menlo,monospace;max-width:34rem;margin:0 auto;padding:1rem}h1{color:#b13e53}a{color:#41a6f6}"
        "button{background:#ffcd75;border:0;font:inherit;font-weight:bold;padding:.6rem 1rem}</style>"
        "<h1>JEWCAL SAFE MODE</h1><p>JewCal restarted ");
    page += crash::count();
    page += F(" times in a row, so it started without the calendar.</p><p>Last error:<br><code>");
    page += html_escape(crash::last());
    page += F("</code></p><p><a href='");
    page += update_path;
    page += F("'>Upload a firmware</a> (a fixed JewCal, or GeekMagic's stock firmware), or try again:</p>"
              "<form method=post action=/resume><button>Restart JewCal</button></form>");
    page += F("<p>");
    page += html_escape(device_info().c_str());
    page += F("</p>");
    server.send(200, F("text/html; charset=utf-8"), page);
  });
  server.on("/resume", HTTP_POST, [] {
    crash::clear();
    server.send(200, F("text/html"), F("<meta http-equiv=refresh content='8;url=/'>Restarting&hellip;"));
    restart_soon();
  });
  make_token();
  updater.setup(&server, update_path);
  server.on("/update", HTTP_GET, page_update_moved);
  server.onNotFound([] { server.send(404, F("text/plain"), F("not found")); });
  if (WiFi.getMode() & WIFI_AP) {
    server.onNotFound(redirect_home);
    dns.start(53, "*", WiFi.softAPIP());
    captive = true;
  }
  server.begin();
}

void loop() {
  if (captive) dns.processNextRequest();
  server.handleClient();
}

}  // namespace web
