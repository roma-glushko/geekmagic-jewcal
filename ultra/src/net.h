// WiFi and plain-HTTP GETs for the SmallTV Ultra.
#pragma once

#include <WString.h>
#include <stddef.h>
#include <stdint.h>

namespace net {

// Join the network the ESP8266 SDK remembers (saved by our setup page, or
// by the stock firmware). False when none is saved or it does not answer
// within `timeout_ms`.
bool join(uint32_t timeout_ms);
bool has_saved_network();
String saved_ssid();
bool online();
String ip();
// Give up on the saved network for this boot (credentials stay saved).
void stop_joining();

// Save credentials in the SDK's flash area (next boot joins them).
void remember(const char *ssid, const char *pass);
// Erase them: the next boot starts the setup access point.
void forget();

// The setup access point's name: JEWCAL-XXXX (from the chip id).
String ap_name();

// Called with each piece of the body as it arrives.
typedef void (*on_data_fn)(const uint8_t *data, size_t len, void *ctx);

// GET over plain HTTP (HTTP/1.0: the body is the raw stream). Returns the
// HTTP status; 200 means the whole body went to `on_data`. Negative: no
// connection, a stall, or a body cut short. `retry_after` gets the header's
// seconds (0 if absent).
int get(const char *url, on_data_fn on_data, void *ctx, int *retry_after);

}  // namespace net
