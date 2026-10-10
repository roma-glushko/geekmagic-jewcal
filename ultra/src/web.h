// The device's web server. Pages come from the Rust core (the same settings
// page as the SmallTV Pro), streamed in chunks; /update takes a new firmware.
#pragma once

#include <WString.h>
#include <stddef.h>

namespace web {

// Settings page, city search, live screenshot, firmware update.
void start_normal();
// First run / lost network: WiFi picker on the access point (captive portal).
void start_setup(const String *networks, size_t count);
// After repeated crashes: no Rust core, only the update page.
void start_safe();
void loop();

}  // namespace web

// Provided by main.cpp.
String device_info();
void save_settings();
void restart_soon();
