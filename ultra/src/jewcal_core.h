// The Rust core (ultra/core, staticlib `jewcal_core`): calendar model,
// screens, settings page. See ultra/core/src/lib.rs for each function.
// Strings are NUL-terminated UTF-8. Everything is called from loop().
#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Receives a web page in chunks.
typedef void (*jc_sink)(void *ctx, const uint8_t *data, size_t len);

// Request kinds from jc_next_request.
enum { JC_NONE = 0, JC_EVENTS = 1, JC_ZMANIM = 2, JC_WEATHER = 3 };

void jc_init(const uint8_t *settings, size_t len, uint32_t seed, uint32_t now_ms);
size_t jc_settings_text(uint8_t *out, size_t cap);
bool jc_take_settings_dirty(void);
void jc_set_ip(const char *ip);

uint8_t jc_frame(int64_t now, uint32_t now_ms);
void jc_message(const char *title, const char *line, uint8_t color, uint32_t frame);
void jc_setup_screen(const char *ssid, const char *url, uint32_t frame);
void jc_row(uint32_t y, uint8_t *out);
void jc_lut(uint16_t *out);
uint8_t jc_color(uint8_t name);
size_t jc_bmp_header(uint8_t *out, size_t cap);

uint8_t jc_next_request(int64_t now, bool clock_valid, uint8_t *url, size_t cap);
void jc_feed(const uint8_t *data, size_t len);
void jc_finish(int32_t status, int32_t retry_after, int64_t now);

// `token` makes the per-boot links: /update-<token> and the /forget form.
void jc_settings_page(jc_sink sink, void *ctx, const char *notice, const char *info, const char *token);
void jc_form_reset(void);
void jc_form_add(const char *name, const char *value);
bool jc_form_apply(jc_sink sink, void *ctx, const char *info, const char *token);
void jc_setup_page(jc_sink sink, void *ctx, const char *const *networks, size_t n);
bool jc_wifi_form(jc_sink sink, void *ctx, uint8_t *ssid, uint8_t *pass);
void jc_message_page(jc_sink sink, void *ctx, const char *html);
size_t jc_city_url(const char *q, uint8_t *out, size_t cap);

// Implemented in C++ (main.cpp): record the panic message, restart.
void jc_panic(const uint8_t *msg, size_t len) __attribute__((noreturn));

#ifdef __cplusplus
}
#endif
