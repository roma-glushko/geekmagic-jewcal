// JewCal installer for the SmallTV Ultra: step 1 of 2.
//
// The stock "Ultra" firmware keeps most of the flash for photos, so its
// updater only has room for an image of about 500 KB; JewCal is larger.
// This small image passes through that updater. It uses JewCal's own flash
// layout (4 MB, 1 MB filesystem), whose updater has room for the full image,
// and it does nothing but receive that image on /update.
//
// No Rust core here: the screen is drawn with the C copy of the BODY font.

#include <Arduino.h>
#include <DNSServer.h>
#include <ESP8266HTTPUpdateServer.h>
#include <ESP8266WebServer.h>
#include <ESP8266WiFi.h>

#include "../board.h"
#include "../display.h"

namespace {

constexpr uint16_t BG = display::rgb(0x1a, 0x1c, 0x2c);
constexpr uint16_t GOLD = display::rgb(0xff, 0xcd, 0x75);
constexpr uint16_t WHITE = display::rgb(0xf4, 0xf4, 0xf4);
constexpr uint16_t DIM = display::rgb(0x94, 0xb0, 0xc2);
constexpr uint16_t SKY = display::rgb(0x41, 0xa6, 0xf6);

ESP8266WebServer server(80);
ESP8266HTTPUpdateServer updater;
DNSServer dns;
String ap;
bool joined = false;

const char PAGE[] PROGMEM =
    "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'>"
    "<title>JewCal installer</title><style>body{background:#1a1c2c;color:#f4f4f4;font:16px/1.5 "
    "ui-monospace,Menlo,monospace;max-width:32rem;margin:0 auto;padding:1rem}h1{color:#ffcd75;letter-spacing:.1em}"
    "input,button{font:inherit}button{background:#ffcd75;border:0;font-weight:bold;padding:.5rem 1rem}"
    ".dim{color:#94b0c2}</style><h1>JEWCAL INSTALLER</h1>"
    "<p>Step 2 of 2: choose <b>jewcal-ultra.bin</b> from the JewCal download and install it.</p>"
    "<form method=post action=/update enctype=multipart/form-data><p><input type=file name=firmware accept=.bin></p>"
    "<p><button>Install</button></p></form>"
    "<p class=dim>Writing takes about half a minute. Then JewCal starts and shows where to open its settings.</p>";

void screen() {
  display::fill(BG);
  display::text_center(24, "JEWCAL", GOLD, BG, 3);
  display::text_center(52, "INSTALLER", WHITE, BG, 2);
  display::text_center(84, "STEP 2: OPEN IN A BROWSER", DIM, BG, 1);
  int y = 100;
  if (joined) {
    const String url = "HTTP://" + WiFi.localIP().toString();
    display::text_center(y, url.c_str(), SKY, BG, 1);
    y += 16;
    display::text_center(y, "OR JOIN WIFI", DIM, BG, 1);
    y += 12;
  } else {
    display::text_center(y, "JOIN WIFI", DIM, BG, 1);
    y += 12;
  }
  display::text_center(y, ap.c_str(), GOLD, BG, 1);
  y += 12;
  const String local = "AND OPEN HTTP://" + WiFi.softAPIP().toString();
  display::text_center(y, local.c_str(), SKY, BG, 1);
  display::text_center(188, "UPLOAD JEWCAL-ULTRA.BIN", WHITE, BG, 1);
}

}  // namespace

void setup() {
  pinMode(PIN_BL, OUTPUT);
  digitalWrite(PIN_BL, HIGH);
  Serial.begin(115200);
  Serial.printf("\n\nJewCal installer %s\n", JEWCAL_VERSION);
  display::begin();
  display::fill(BG);
  display::text_center(110, "STARTING...", WHITE, BG, 2);
  display::backlight(80);

  // An open network of our own, always: it works even when the saved
  // network does not.
  WiFi.persistent(false);
  WiFi.mode(WIFI_AP_STA);
  WiFi.hostname(HOSTNAME);
  char name[24];
  snprintf(name, sizeof name, "JEWCAL-INSTALL-%04X", (unsigned)(ESP.getChipId() & 0xFFFF));
  ap = name;
  WiFi.softAP(name);
  // The network the stock firmware was using, if the SDK remembers it.
  if (WiFi.SSID().length()) {
    WiFi.begin();
    for (uint32_t t0 = millis(); millis() - t0 < 15000 && WiFi.status() != WL_CONNECTED;) delay(100);
    joined = WiFi.status() == WL_CONNECTED;
    if (!joined) {
      // Stop hunting for it (credentials stay saved): a searching station
      // drags our access point's channel along.
      WiFi.setAutoReconnect(false);
      WiFi.disconnect(false, false);
    }
  }
  Serial.printf("installer: http://%s/ or join %s -> http://%s/\n", WiFi.localIP().toString().c_str(), name,
                WiFi.softAPIP().toString().c_str());

  updater.setup(&server);
  server.on("/", HTTP_GET, [] { server.send_P(200, PSTR("text/html; charset=utf-8"), PAGE); });
  // Captive portal for phones joining our network.
  server.onNotFound([] {
    server.sendHeader(F("Location"), F("/"));
    server.send(302, F("text/plain"), "");
  });
  dns.start(53, "*", WiFi.softAPIP());
  server.begin();
  screen();
}

void loop() {
  dns.processNextRequest();
  server.handleClient();
  const bool now = WiFi.status() == WL_CONNECTED;
  if (now != joined) {
    joined = now;
    screen();
  }
}
