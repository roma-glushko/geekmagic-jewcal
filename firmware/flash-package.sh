#!/usr/bin/env bash
# Flash this JewCal package to a GeekMagic SmallTV Pro over USB.
#
#   ./flash.sh                  fresh install (full image at 0x0; clears settings)
#   ./flash.sh --update         update the app only (0x10000; keeps settings)
#   ./flash.sh --backup         save the device's current firmware first
#   PORT=/dev/cu.usbserial-110 ./flash.sh   pick the serial port
#
# Uses espflash (cargo install espflash) or esptool (pip install esptool).
# For the SmallTV *Pro* (ESP32) only: both tools refuse other chips.
set -euo pipefail
cd "$(dirname "$0")"

mode=full
case "${1:-}" in
  --update) mode=update ;;
  --backup) mode=backup ;;
  "") ;;
  *) sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'; exit 1 ;;
esac

if command -v sha256sum >/dev/null; then
  sha256sum -c --quiet SHA256SUMS
else
  shasum -a 256 -c --quiet SHA256SUMS
fi || { echo "checksum mismatch: re-download the package"; exit 1; }

backup_file="smalltv-backup-$(date +%Y%m%d-%H%M%S).bin"
# ${PORT:+...} expands to nothing when PORT is unset (safe under `set -u`,
# unlike an empty array on macOS's bash 3.2).
if command -v espflash >/dev/null; then
  case $mode in
    full)   espflash write-bin --chip esp32 ${PORT:+--port "$PORT"} 0x0 jewcal-pro-full.bin ;;
    update) espflash write-bin --chip esp32 ${PORT:+--port "$PORT"} 0x10000 jewcal-pro-app.bin ;;
    backup) espflash read-flash ${PORT:+--port "$PORT"} 0 0x400000 "$backup_file" ;;
  esac
elif command -v esptool.py >/dev/null || command -v esptool >/dev/null; then
  tool=$(command -v esptool.py || command -v esptool)
  case $mode in
    full)   "$tool" --chip esp32 ${PORT:+--port "$PORT"} write_flash 0x0 jewcal-pro-full.bin ;;
    update) "$tool" --chip esp32 ${PORT:+--port "$PORT"} write_flash 0x10000 jewcal-pro-app.bin ;;
    backup) "$tool" --chip esp32 ${PORT:+--port "$PORT"} read_flash 0 0x400000 "$backup_file" ;;
  esac
else
  echo "Install espflash (cargo install espflash) or esptool (pip install esptool),"
  echo "or use https://espressif.github.io/esptool-js/ in a browser (see FLASHING.txt)."
  exit 1
fi

if [ "$mode" != backup ]; then
  echo
  echo "Done. If this was a fresh install, the screen shows SETUP: join the JEWCAL-XXXX"
  echo "WiFi from a phone, pick your network, then open http://jewcal.local/ to choose your city."
fi
