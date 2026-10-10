// GeekMagic SmallTV Ultra: ESP-12F (ESP8266, 4 MB flash), 1.54" 240x240
// ST7789 IPS over hardware SPI. Pin map from ESPHome's device page and
// smalltv-mod's board_esp8266.h (both from real units).
//
//   GPIO14  SPI clock (HSPI, fixed)      GPIO13  SPI MOSI (HSPI, fixed)
//   GPIO15  panel CS: tied to GND on the board; driven low anyway
//   GPIO0   display DC  (boot strap: must be high at reset)
//   GPIO2   display RST (boot strap: must be high at reset)
//   GPIO5   backlight, PWM, active low
//
// The panel only latches SPI in mode 3 (its CS never toggles). No touch pad,
// no button: everything happens on the screen rotation and the web page.
#pragma once

#include <stdint.h>

constexpr uint8_t PIN_DC = 0;
constexpr uint8_t PIN_RST = 2;
constexpr uint8_t PIN_CS = 15;
constexpr uint8_t PIN_BL = 5;
constexpr uint32_t SPI_HZ = 40000000;

// The settings page is also reachable as http://jewcal.local/ (mDNS).
constexpr const char *HOSTNAME = "jewcal";

#ifndef JEWCAL_VERSION
#define JEWCAL_VERSION "dev"
#endif
