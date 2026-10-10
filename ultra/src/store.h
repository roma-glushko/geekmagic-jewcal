// Settings persistence: the Rust core's `key=value` text in the EEPROM
// sector, behind a magic and a CRC (the stock firmware's data there is
// ignored). WiFi credentials are not here: the ESP8266 SDK keeps its own.
#pragma once

#include <WString.h>
#include <stddef.h>
#include <stdint.h>

namespace store {

// The saved text, or "" (first boot, or unreadable).
String load();
bool save(const uint8_t *text, size_t len);

}  // namespace store
