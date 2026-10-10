# Install on the SmallTV Pro

The [GeekMagic SmallTV Pro](https://geekmagic.cc/products/smalltv-pro/) is installed over a USB cable, from a
browser or with a command-line tool.

!!! warning "Not yet tested on a real SmallTV Pro"
    The Pro firmware builds and passes every check, but nobody has run it on a Pro yet. If you try it, please
    [open an issue](https://github.com/roma-glushko/geekmagic-jewcal/issues) and say how it went.

    GeekMagic has made several Pro revisions. Some have a USB-C port that only carries power. If your computer does
    not see the device when you plug it in, this guide cannot install JewCal on it.

## Before you start

- A USB-C **data** cable. Some cables only carry power.
- A computer with **Chrome** or **Edge**, for the browser method below.
- From the [latest release](https://github.com/roma-glushko/geekmagic-jewcal/releases/latest), download
  **`jewcal-pro-<version>.zip`** and unzip it. Inside are `jewcal-pro-full.bin`, `jewcal-pro-app.bin`, `flash.sh`
  and instructions.

!!! tip "Keep a copy of the original firmware"
    With a terminal, `./flash.sh --backup` saves the device's current firmware before anything is overwritten.
    GeekMagic also publishes its firmware at
    [github.com/GeekMagicClock/smalltv-pro](https://github.com/GeekMagicClock/smalltv-pro).

## 1. Install

=== "From the browser"

    1. Connect the SmallTV Pro to the computer with the USB cable.
    2. Open the [ESP web flasher](https://espressif.github.io/esptool-js/) and press **Connect**. Pick the serial port
       that appeared when you plugged the device in.
    3. Check that it reports an **ESP32** chip, not an ESP32-S3, an ESP32-C2 or an ESP8266.
    4. Set the address to **`0x0`**, add **`jewcal-pro-full.bin`**, and press **Program**.
    5. When it finishes, unplug the device and plug it back in.

=== "From a terminal"

    In the unzipped folder, with [espflash](https://github.com/esp-rs/espflash) or
    [esptool](https://github.com/espressif/esptool) installed:

    ```bash
    ./flash.sh              # fresh install: clears saved settings
    ./flash.sh --update     # update: keeps WiFi and city
    PORT=/dev/cu.usbserial-110 ./flash.sh   # pick the serial port
    ```

    Both tools are told the chip is an ESP32, so they refuse to write to any other board.

## 2. Connect to your WiFi

On first start the screen shows **SETUP**:

1. On your phone, join the WiFi network **`JEWCAL-XXXX`** shown on the screen.
2. Open the address shown on the screen, pick your network, type its password, and press **Connect**.
3. JewCal restarts and joins your network. If it cannot join three times in a row, it goes back to SETUP.

The SmallTV only sees 2.4 GHz WiFi.

## 3. Choose your city

Open **`http://jewcal.local/`**, or the address on the info screen. Under **Location**, search your city, pick it, and
press **Save**. See [Settings](settings.md) for the rest.

## The touch pad

The Pro has a touch-sensitive top. You never need it, since the screens rotate by themselves, but:

| Touch | Does |
|---|---|
| Tap | Next screen |
| Hold 1.5 seconds | The info screen: location, settings address, last update |
| Hold 8 seconds | Forget the WiFi network and restart into SETUP |

## Updating JewCal

Download the newer `jewcal-pro-<version>.zip`. Then either run `./flash.sh --update`, or in the web flasher write
**`jewcal-pro-app.bin`** at **`0x10000`**. Your WiFi and settings are kept.

## If the screen never dims at night

Some Pro boards wire the backlight differently. If night brightness has no effect, your board needs the
`backlight-gpio21` build; see the [developer notes](https://github.com/roma-glushko/geekmagic-jewcal#development).
