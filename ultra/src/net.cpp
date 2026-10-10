#include "net.h"

#include <ESP8266HTTPClient.h>
#include <ESP8266WiFi.h>

#include "board.h"

namespace net {

namespace {

constexpr uint32_t STALL_MS = 15000;

}  // namespace

bool has_saved_network() { return WiFi.SSID().length() > 0; }

String saved_ssid() { return WiFi.SSID(); }

bool join(uint32_t timeout_ms) {
  // Do not rewrite the SDK's flash config on every boot.
  WiFi.persistent(false);
  WiFi.mode(WIFI_STA);
  WiFi.hostname(HOSTNAME);
  WiFi.setAutoReconnect(true);
  // Modem sleep makes the web page and mDNS sluggish; this is USB powered.
  WiFi.setSleepMode(WIFI_NONE_SLEEP);
  if (!has_saved_network()) return false;
  WiFi.begin();
  const uint32_t t0 = millis();
  while (WiFi.status() != WL_CONNECTED) {
    if (millis() - t0 > timeout_ms) return false;
    delay(100);
  }
  return true;
}

bool online() { return WiFi.status() == WL_CONNECTED; }

void stop_joining() {
  // Keep the saved credentials (no erase, nothing written to flash), but
  // stop retrying: a station hunting for its network drags the access
  // point's channel along.
  WiFi.setAutoReconnect(false);
  WiFi.disconnect(false, false);
}

String ip() { return WiFi.localIP().toString(); }

void remember(const char *ssid, const char *pass) {
  WiFi.persistent(true);
  WiFi.begin(ssid, pass);
  WiFi.persistent(false);
}

void forget() {
  WiFi.persistent(true);
  WiFi.disconnect(true);
  WiFi.persistent(false);
}

String ap_name() {
  char name[16];
  snprintf(name, sizeof name, "JEWCAL-%04X", (unsigned)(ESP.getChipId() & 0xFFFF));
  return name;
}

int get(const char *url, on_data_fn on_data, void *ctx, int *retry_after) {
  if (retry_after) *retry_after = 0;
  WiFiClient client;
  HTTPClient http;
  http.setReuse(false);
  // HTTP/1.0: no chunked encoding, so the stream is exactly the body.
  http.useHTTP10(true);
  http.setTimeout(STALL_MS);
  http.setUserAgent(F("JewCal/" JEWCAL_VERSION " (GeekMagic SmallTV Ultra)"));
  static const char *keep[] = {"Retry-After"};
  http.collectHeaders(keep, 1);
  if (!http.begin(client, url)) return -1;
  const int code = http.GET();
  if (retry_after) *retry_after = http.header("Retry-After").toInt();
  if (code != 200) {
    http.end();
    return code > 0 ? code : -1;
  }
  int remaining = http.getSize();  // -1: until the server closes
  WiFiClient *s = http.getStreamPtr();
  if (!s) {
    // The server already closed an empty body.
    http.end();
    return remaining > 0 ? -3 : 200;
  }
  // Static: the loop stack is only 4 KB (one request at a time, never nested).
  static uint8_t buf[512];
  uint32_t idle = millis();
  while (remaining != 0) {
    const size_t avail = s->available();
    if (avail) {
      const int n = s->read(buf, avail < sizeof buf ? avail : sizeof buf);
      if (n > 0) {
        on_data(buf, n, ctx);
        if (remaining > 0) remaining -= n;
        idle = millis();
      }
    } else if (!s->connected()) {
      break;
    } else if (millis() - idle > STALL_MS) {
      http.end();
      return -2;
    } else {
      delay(2);
    }
  }
  http.end();
  return remaining > 0 ? -3 : 200;
}

}  // namespace net
