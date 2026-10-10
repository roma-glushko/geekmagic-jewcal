#include "store.h"

#include <EEPROM.h>

namespace store {

namespace {

constexpr uint32_t MAGIC = 0x5443414A;  // "JACT"
constexpr size_t SIZE = 1024;

struct Header {
  uint32_t magic;
  uint32_t len;
  uint32_t crc;
};

uint32_t crc32(const uint8_t *p, size_t n) {
  uint32_t c = 0xFFFFFFFF;
  while (n--) {
    c ^= *p++;
    for (int k = 0; k < 8; k++) c = (c >> 1) ^ (0xEDB88320 & -(c & 1));
  }
  return ~c;
}

}  // namespace

String load() {
  String out;
  EEPROM.begin(SIZE);
  Header h;
  EEPROM.get(0, h);
  if (h.magic == MAGIC && h.len <= SIZE - sizeof h) {
    const uint8_t *p = EEPROM.getConstDataPtr() + sizeof h;
    if (crc32(p, h.len) == h.crc) out.concat(reinterpret_cast<const char *>(p), h.len);
  }
  EEPROM.end();
  return out;
}

bool save(const uint8_t *text, size_t len) {
  if (len > SIZE - sizeof(Header)) return false;
  EEPROM.begin(SIZE);
  Header h{MAGIC, (uint32_t)len, crc32(text, len)};
  EEPROM.put(0, h);
  memcpy(EEPROM.getDataPtr() + sizeof h, text, len);
  bool ok = EEPROM.commit();
  EEPROM.end();
  return ok;
}

}  // namespace store
